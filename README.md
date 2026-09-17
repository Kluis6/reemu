# ReEmu

[![CI](https://github.com/Kluis6/reemu/actions/workflows/ci.yml/badge.svg)](https://github.com/Kluis6/reemu/actions/workflows/ci.yml)
[![Release](https://github.com/Kluis6/reemu/actions/workflows/release.yml/badge.svg)](https://github.com/Kluis6/reemu/actions/workflows/release.yml)

<img src="apps/desktop/src-tauri/icons/icon.png" alt="logo do ReEmu" width="150"/>

**ReEmu** é um frontend desktop pra emulação de jogos via cores
[libretro](https://www.libretro.com/) (o mesmo formato de core do
RetroArch) — biblioteca de jogos com capas, shaders (CRT, LCD, presets
`.slangp` do RetroArch), molduras/bezels, save states com miniatura,
suporte a controle e uma interface no estilo "modo console" (Xbox/PS).

## Downloads

Baixe a versão mais recente pra Linux ou Windows em
[**GitHub Releases**](https://github.com/Kluis6/reemu/releases/latest).

- **Linux**: `.deb` (Debian/Ubuntu) ou `.AppImage` (qualquer distro)
- **Windows**: `.msi` ou `.exe` (instalador NSIS)

Builds novos saem a cada versão marcada (`vX.Y.Z`) — sem build nightly.

## Instalar

### Linux

Baixe o `.deb` e instale:

```bash
sudo apt install ./ReEmu_*.deb
```

Ou baixe o `.AppImage`, dê permissão de execução e rode direto:

```bash
chmod +x ReEmu_*.AppImage
./ReEmu_*.AppImage
```

### Windows

Baixe o `.msi` (ou o `.exe`) e execute o instalador.

## Cores libretro

O ReEmu não vem com cores/emuladores embutidos. Depois de instalar, abra
**Configurações › Cores** dentro do app pra baixar os cores libretro que
quiser direto do buildbot oficial, ou copie os arquivos `*_libretro.so` /
`*_libretro.dll` manualmente na pasta de cores. Depois é só apontar a
biblioteca pra pasta das suas ROMs.

## Build a partir do código-fonte

Precisa de Rust, Node/pnpm e a [Tauri CLI](https://v2.tauri.app). Passo a
passo completo em [`STEP_BY_STEP.md`](STEP_BY_STEP.md); resumo rápido:

```bash
git clone https://github.com/Kluis6/reemu.git
cd reemu
pnpm install
cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json
```

## Desenvolvimento

Progresso e backlog: [`TASKS.md`](TASKS.md). Decisões de arquitetura:
[`resumo-arquitetura-reemu.md`](resumo-arquitetura-reemu.md).
