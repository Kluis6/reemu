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

Siga os pré-requisitos do Tauri v2 (Build Tools do Visual Studio com
"Desenvolvimento para desktop com C++" + WebView2):
<https://v2.tauri.app/start/prerequisites/>.

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
```

Dados do app (banco, cores, BIOS, shaders) ficam em
`~/.local/share/com.reemu.desktop/` no Linux.

## 5. Verificar antes de commitar

É o que o CI roda. Se passar aqui, passa lá:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --filter app-desktop lint
pnpm --filter app-desktop build
```

## 6. Build de release local

```bash
cargo build --release -p core-host-desktop
cargo tauri build --config apps/desktop/src-tauri/tauri.conf.json \
  --config apps/desktop/src-tauri/tauri.bundle.linux.json
```

No Windows, troque por `tauri.bundle.windows.json`. Esses arquivos
declaram o `reemu-core-host` como recurso do pacote. Eles não são
mesclados automaticamente (ver TASKS.md › Infra), por isso o `--config`
explícito. O mesmo fluxo roda no CI em `.github/workflows/release.yml`,
disparado por tag `v*` ou manualmente (`workflow_dispatch`).

## 7. Android (Etapa 11, adiada)

Ainda não existe `apps/mobile`. Quando a etapa começar, veja
`docs/ai-context/11-android-port.md`. Esta máquina precisa, além do SDK,
de NDK, JDK 17, `cmdline-tools` e dos targets Rust `*-linux-android*`.
