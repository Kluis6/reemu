// `beforeDevCommand` do Tauri (apps/desktop/src-tauri/tauri.conf.json).
//
// Sobe o Vite NA HORA e compila o `reemu-core-host` EM PARALELO. O
// `cargo tauri dev` só espera o Vite por 180 s (tauri-cli 2.11,
// `dev.rs`: 90 tentativas × 2 s) e aborta com "Could not connect to
// http://127.0.0.1:1420"; compilar o core-host ANTES do Vite estourava esse
// prazo num clone novo (Windows principalmente — deps em -O3 no perfil dev).
//
// A ordem continua garantida: o `cargo run` do app que o Tauri dispara em
// seguida espera a trava do diretório `target/` até o core-host terminar
// ("Blocking waiting for file lock"), então o app nunca sobe com um core-host
// velho ou ausente.
//
// Node puro (já é pré-requisito do pnpm): funciona igual no `cmd` do Windows
// e no `sh` do Linux/macOS, onde o Tauri roda os hooks.
import { spawn } from 'node:child_process'

const shell = process.platform === 'win32'
const children = []

function run(cmd, args) {
  const child = spawn(cmd, args, { stdio: 'inherit', shell })
  children.push(child)
  return child
}

const vite = run('pnpm', ['--filter', 'app-desktop', 'dev'])
const coreHost = run('cargo', ['build', '-p', 'core-host-desktop'])

coreHost.on('exit', (code) => {
  if (code !== 0) {
    console.error(
      `\n[dev-before] cargo build -p core-host-desktop falhou (código ${code}). ` +
        'O app vai abrir, mas não vai conseguir carregar jogos até isso compilar.\n',
    )
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
