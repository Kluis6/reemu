// `beforeDevCommand` do Tauri (apps/desktop/src-tauri/tauri.conf.json).
//
// Sobe o Vite NA HORA e compila o `reemu-core-host` EM PARALELO. O
// `cargo tauri dev` só espera o Vite por 180 s (tauri-cli 2.11,
// `dev.rs`: 90 tentativas × 2 s) e aborta com "Could not connect to
// http://127.0.0.1:1420"; compilar o core-host ANTES do Vite estourava esse
// prazo num clone novo (Windows principalmente — deps em -O3 no perfil dev).
//
// O core-host compila num target SEPARADO (`target/core-host/`) e o binário é
// copiado pra `target/debug/`, ao lado do app (onde o `emu-session` procura).
// No mesmo `target/`, o cargo novo trava por unidade de compilação e deixa os
// dois processos (este e o `cargo run` do Tauri) linkarem a mesma dependência
// ao mesmo tempo — no Windows os dois escrevem o mesmo `.pdb` e dá
// `LNK1285: arquivo PDB corrompido`. Custo: as dependências do core-host
// compilam uma vez a mais na 1ª vez.
//
// Node puro (já é pré-requisito do pnpm): funciona igual no `cmd` do Windows
// e no `sh` do Linux/macOS, onde o Tauri roda os hooks.
import { spawn } from 'node:child_process'
import { copyFileSync, mkdirSync } from 'node:fs'
import { join } from 'node:path'

const win = process.platform === 'win32'
const exe = win ? 'reemu-core-host.exe' : 'reemu-core-host'
const children = []

// No Windows o `pnpm` é um `.cmd`, que só roda via shell. Com shell, o comando
// vai como UMA string (argumentos separados + `shell: true` é depreciado no
// Node — DEP0190). Os comandos aqui são fixos, nada vem de fora.
function run(command, env = {}) {
  const child = spawn(command, { stdio: 'inherit', shell: true, env: { ...process.env, ...env } })
  children.push(child)
  return child
}

const vite = run('pnpm --filter app-desktop dev')
const coreHost = run('cargo build -p core-host-desktop', {
  CARGO_TARGET_DIR: join('target', 'core-host'),
})

coreHost.on('exit', (code) => {
  if (code !== 0) {
    console.error(
      `\n[dev-before] cargo build -p core-host-desktop falhou (código ${code}). ` +
        'O app abre, mas não carrega jogos até isso compilar.\n',
    )
    return
  }
  const dest = join('target', 'debug')
  try {
    mkdirSync(dest, { recursive: true })
    copyFileSync(join('target', 'core-host', 'debug', exe), join(dest, exe))
    console.error(`\n[dev-before] ${exe} pronto em ${dest} — já dá pra carregar jogos.\n`)
  } catch (e) {
    // Windows: um jogo aberto segura o .exe antigo em uso.
    console.error(`\n[dev-before] não deu pra copiar o ${exe} (${e.message}). Feche o jogo e rode de novo.\n`)
  }
})

// O Vite é o processo de longa duração: quando ele sai, o hook sai junto.
vite.on('exit', (code) => {
  for (const c of children) if (c.exitCode === null) c.kill()
  process.exit(code ?? 0)
})

for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(sig, () => {
    for (const c of children) if (c.exitCode === null) c.kill(sig)
    process.exit(0)
  })
}
