# ReEmu — Monorepo

Frontend emulador para cores libretro (Tauri v2 + React 19 + SQLite +
Fluent Design). Estrutura inicial gerada a partir do design de arquitetura
(ver `resumo-arquitetura-reemu.md`).

## Estrutura

```
apps/
  desktop/    Projeto Tauri v2 (crate reemu-desktop) — Linux OK; Win/macOS não verificados
  mobile/     Projeto Tauri v2 mobile (Android) — ainda não inicializado

crates/
  domain/               Regras de negócio puras — traits/portas, zero I/O de plataforma
  db/                   Schema SQLite (migrations) + repositórios sqlx
  core-loader-desktop/  libloading + FFI libretro (software + HW render GL)
  emu-session/          Loop do core em thread dedicada + state machine de foco
  video-surface/        Renderer wgpu: frame do core -> textura -> tela (letterbox)
  audio-desktop/        AudioSink cpal + Dynamic Rate Control
  library-scan/         Hash de ROMs (CRC32/MD5) + varredura + convenção de bezels
  input-desktop/        SDL_GameControllerDB, hotkeys com combinação, keymap, gilrs
  shader-slang/         Parser .slangp + compilador .slang (GLSL -> WGSL via naga)
  # core-loader-mobile/  (a criar) — cores empacotados, JNI

packages/
  ui/             Componentes Fluent, tokens de design — vazio
  shared/         Hooks, cliente IPC, state management (Zustand) — vazio
  app-desktop/    Entrypoint React desktop — Vite + React 19 + Fluent + Zustand + TanStack Query
  app-mobile/     Entrypoint React mobile (layout touch) — vazio
```

## Regra de dependência (hexagonal)

`domain` nunca importa crates de I/O de plataforma. Tudo que toca hardware,
sistema de arquivos ou rede vive nos adapters (`core-loader-*`, `db`), que
implementam as traits definidas em `domain`.

## Status atual

Ver `TASKS.md` para o checklist detalhado e o backlog. Resumo (2026-09-02):

**Desktop (etapas 01–10) fechado.** Roda cores libretro software **e OpenGL**
(N64 etc.) ponta a ponta: biblioteca → detalhe do jogo → jogar (vídeo num
`<canvas>`, áudio com DRC, save states com thumbnail e save RAM), tudo com UI
"modo Xbox" (Griffel) e navegação por controle.

- [x] **01** Domain + `crates/db` (sqlx, ~11 repos, migrations 0001–0003)
- [x] **02** `core-loader-desktop` — FFI libretro; software + HW render GL
      (contexto EGL offscreen + FBO + readback; interop dma_buf opt-in). ROM `.zip`.
- [x] **03** `emu-session` + vídeo via `<canvas>` (surface nativa adiada — ver `docs/ai-context/03`)
- [x] **04** Shader chain (`plain`/`crt`/`lcd` + `.slangp` via `shader-slang`),
      parâmetros ajustáveis, decoração/bezels (Bezel Project/RetroBat)
- [x] **05** Input — teclado, `gilrs`, hotkeys/mapeamento do DB em runtime, UI de binding
- [x] **06** `audio-desktop` — DRC + `CpalAudioSink`, aplicar config ao vivo
- [x] **07** Frontend — Início + Meus jogos + RomDetail + PlayScreen, busca, menu de contexto
- [x] **08** Save states + save RAM (`.srm` atômica, flush no shutdown), thumbnail por slot
- [x] **09** Scraping — ScreenScraper por CRC + fila de revisão manual
- [x] **10** Catálogo — 68 cores do buildbot; software + GL usáveis (badge "OpenGL")
- [ ] **11** Port Android — desbloqueado, adiado
- [ ] **12** HW render Vulkan — backlog

Backlog: surface nativa de vídeo (tira as cópias de CPU do `<canvas>`),
compilador slang via glslang→SPIR-V (destrava CRT-Royale/Mega Bezel/FSR),
integer scaling, interop dma_buf sem gate, `.7z` no scan, `packages/ui`/`shared`,
`apps/mobile`.

## Rodar

```bash
cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json   # app (vídeo via canvas)
cargo tauri build --config apps/desktop/src-tauri/tauri.conf.json # bundle release
cargo run -p video-surface --example play -- <core_libretro.so> <rom>   # player standalone
cargo test --workspace
```

Cores: baixe pela aba **Configurações › Cores** ou copie `*_libretro.so` em
`~/.local/share/com.reemu.desktop/cores/`. Depois aponte a Biblioteca pra pasta
das ROMs.
