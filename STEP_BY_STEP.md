# Passo a Passo — Rodar o ReEmu a partir do código-fonte

Todos os comandos assumem que você está na raiz do repositório (`reemu/`).

**Antes de mexer no código**: confira `TASKS.md` — é o checklist de
progresso do projeto. Ao delegar trabalho pra uma IA (Claude Code ou outra),
aponte pra ele primeiro ("veja o TASKS.md e continue da próxima tarefa
`todo`").

---

## 1. Ferramentas base

```bash
# Rust — a versão exata vem de rust-toolchain.toml (o rustup baixa sozinho)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node 22 + pnpm (versão do pnpm fixada em package.json › packageManager)
corepack enable

# Tauri CLI v2
cargo install tauri-cli --version "^2"
```

## 2. Dependências de sistema

### Linux (Debian/Ubuntu)

Mesma lista que o CI instala (`.github/workflows/ci.yml`):

```bash
sudo apt install \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libsoup-3.0-dev libxkbcommon-dev \
  libudev-dev libasound2-dev \
  gcc g++
```

Pra que serve cada grupo:

- **webkit2gtk / gtk3 / appindicator / rsvg / soup**: janela e webview do Tauri
- **libxkbcommon**: teclado (winit)
- **libudev**: controles (`gilrs`). Sem ele o build para em `libudev-sys`
  com `Package 'libudev' not found`
- **libasound2**: áudio (`cpal`/ALSA). Sem ele o build para em `alsa-sys`
- **g++**: compila o glslang vendorizado (compilador de shaders slang)

Outras distros: nomes equivalentes em
<https://v2.tauri.app/start/prerequisites/>, mais os dev packages de
`libudev` e ALSA.

Pra conferir se o pkg-config enxerga tudo:

```bash
pkg-config --modversion webkit2gtk-4.1 libudev alsa
```

### Windows

Siga os pré-requisitos do Tauri v2: <https://v2.tauri.app/start/prerequisites/>.
Resumo do que o ReEmu precisa (nada além disso — sem CMake, LLVM, NASM ou
vcpkg; todo C/C++ do projeto compila pelo MSVC):

- **Build Tools do Visual Studio** com a carga **"Desenvolvimento para
  desktop com C++"** (traz o `cl.exe`, o linker e o Windows SDK). Compila o
  glslang, o SQLite e o `ring`.
- **WebView2** (já vem no Windows 10/11 atualizado).
- **Rust com toolchain MSVC**. O `rust-toolchain.toml` fixa a versão, mas a
  arquitetura vem do rustup — tem que ser `x86_64-pc-windows-msvc`, não
  `-gnu`:

  ```powershell
  rustup show                                   # "Default host" deve terminar em -msvc
  rustup set default-host x86_64-pc-windows-msvc
  ```

- **Node 22 + pnpm** (`corepack enable`) e a **Tauri CLI**
  (`cargo install tauri-cli --version "^2"`).

Erros comuns no Windows:

| Sintoma | Causa / solução |
| --- | --- |
| `link.exe not found`, `cl.exe` não encontrado, `LNK1181` | Falta a carga "Desenvolvimento para desktop com C++" |
| `x86_64-w64-mingw32-gcc not found` | Toolchain `-gnu`; troque pra `-msvc` (acima) |
| `.\scripts\dev.ps1 não pode ser carregado… execução de scripts foi desabilitada` | Política do PowerShell: `powershell -ExecutionPolicy Bypass -File scripts\dev.ps1`, ou rode `cargo tauri dev` direto |
| `Could not connect to http://127.0.0.1:1420 after 180s` | Corrigido em 2026-09-24 (`scripts/dev-before.mjs`); atualize o repositório |
| Jogo não aparece (tela da biblioteca fica por cima) | Surface nativa ainda não existe no Windows; o padrão lá é o `<canvas>` desde 2026-09-24 — não defina `REEMU_NATIVE_VIDEO=1` |
| `LNK1285: arquivo PDB corrompido` | Corrigido em 2026-09-24 (o core-host compilava no mesmo `target/` em paralelo com o app). Atualize o repositório e apague o PDB estragado uma vez: `cargo clean -p sevenz-rust2` (se outro crate der o mesmo erro, `cargo clean`) |
| Caminho longo demais / `os error 206` | `git config --system core.longpaths true` e clone numa pasta curta (ex.: `C:\dev\reemu`) |

Do Linux dá pra conferir se o código compila pra Windows sem ter a máquina:
`scripts/check-windows.sh` (só checa o Rust, não gera `.exe`).

## 3. Instalar dependências JS

```bash
pnpm install
```

## 4. Rodar em modo dev

```bash
scripts/dev.sh          # Linux
.\scripts\dev.ps1       # Windows (PowerShell)
```

Ou direto, sem o script:

```bash
cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json
```

As duas formas funcionam: o `beforeDevCommand` do `tauri.conf.json`
compila o `reemu-core-host` (o processo filho que carrega o core libretro)
e depois sobe o Vite na porta 1420. A primeira compilação demora (as deps
rodam em `-O3` mesmo no dev, ver `Cargo.toml`); as seguintes são
incrementais.

O script só acrescenta atalhos:

```bash
RUST_LOG=debug scripts/dev.sh
scripts/dev.sh --vk-validation          # validation layer do Vulkan
REEMU_NATIVE_VIDEO=0 scripts/dev.sh     # <canvas> em vez da surface nativa
REEMU_WEBKIT_COMPOSITING=1 scripts/dev.sh
REEMU_PERF=1 scripts/dev.sh             # métricas de desempenho 1×/s no log
```

Com `REEMU_PERF=1`, cada segundo de jogo gera duas linhas no log:

- `perf core`: fps real vs. alvo; intervalo entre frames (médio, p99,
  máximo); tempo do `retro_run`; custo de mandar o frame pro app; quanto o
  pacing dormiu e quanto queimou em spin; frames atrasados.
- `perf vídeo`: frames recebidos do core vs. apresentados (a diferença são
  **frames perdidos**); voltas do loop de vídeo sem frame novo; tempo de
  render. Só no modo surface nativa (o padrão).

Dados do app (banco, cores, BIOS, shaders) ficam em
`~/.local/share/com.reemu.desktop/` no Linux.

## 5. Verificar antes de commitar

É o que o CI roda. Se passar aqui, passa lá:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter app-desktop lint
pnpm --filter app-desktop test
pnpm --filter app-desktop build
```

O hook `.githooks/pre-commit` roda as duas mais rápidas (rustfmt e oxlint)
só quando há arquivo daquela parte no commit. Ative uma vez por clone:

```bash
git config core.hooksPath .githooks
```

## 6. Build de release local

```bash
cargo build --release -p core-host-desktop
cargo tauri build --config apps/desktop/src-tauri/tauri.conf.json \
  --config apps/desktop/src-tauri/tauri.bundle.linux.json
```

No Windows, troque por `tauri.bundle.windows.json`. Esses arquivos
declaram o `reemu-core-host` como recurso do pacote. Eles não são
mesclados automaticamente (ver `docs/historico.md` › Infra), por isso o `--config`
explícito. O mesmo fluxo roda no CI em `.github/workflows/release.yml`,
disparado por tag `v*` ou manualmente (`workflow_dispatch`).

### Publicar uma versão com atualização automática

O app procura versão nova ao abrir e a cada 6 h. Quando acha, mostra um
toast, acende o sino da barra lateral e abre um modal com as notas e o botão
"Atualizar agora". Ele lê
`https://github.com/Kluis6/reemu/releases/latest/download/latest.json`.

**Configuração (uma vez só):**

1. `cargo tauri signer generate -w ~/.tauri/reemu.key`. Anote a senha.
   **Não** faça commit da chave privada e não a perca: sem ela, nenhuma
   versão futura é aceita pelos apps já instalados.
2. Cole o conteúdo de `~/.tauri/reemu.key.pub` em
   `apps/desktop/src-tauri/tauri.conf.json` › `plugins.updater.pubkey` e
   faça o commit (a chave pública não é segredo).
3. No GitHub, em Settings › Secrets and variables › Actions, crie
   `TAURI_SIGNING_PRIVATE_KEY` (conteúdo de `~/.tauri/reemu.key`) e
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

**A cada versão:**

1. Suba o `version` no `tauri.conf.json` (ex.: `0.2.0`). O app compara por
   ele, não pela tag.
2. `git tag v0.2.0 && git push --tags`. O CI gera os instaladores
   assinados e cria a Release em **draft**, com as notas montadas a partir
   dos commits `feat`/`fix`/`perf`.
3. Revise as notas no GitHub e clique em **Publish**. Nessa hora o job
   `updater-manifest` gera o `latest.json` com o texto final. Draft e
   pre-release não chegam aos usuários.

Para testar a interface sem publicar nada:
`REEMU_FAKE_UPDATE=1 cargo tauri dev` finge uma versão 9.9.9 (só em build
de debug; o "Atualizar agora" simula o download e para com um erro).

## 7. Android (Etapa 11, adiada)

Ainda não existe `apps/mobile`. Quando a etapa começar, veja
`docs/ai-context/11-android-port.md`. Esta máquina precisa, além do SDK,
de NDK, JDK 17, `cmdline-tools` e dos targets Rust `*-linux-android*`.
