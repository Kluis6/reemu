# ReEmu — Monorepo

Frontend emulador para cores libretro (Tauri v2 + React 19 + SQLite +
Fluent Design). Estrutura inicial gerada a partir do design de arquitetura
(ver `resumo-arquitetura-reemu.md`).

## Estrutura

```
apps/
  desktop/    Projeto Tauri v2 (Windows + Linux) — inicializado (crate reemu-desktop)
  mobile/     Projeto Tauri v2 mobile (Android) — ainda não inicializado

crates/
  domain/               Regras de negócio puras — traits/portas, zero I/O de plataforma
  db/                   Schema SQLite (migrations) + repositórios sqlx
  core-loader-desktop/  libloading + FFI libretro (caminho software-only)
  emu-session/          Loop do core em thread dedicada + state machine de foco
  video-surface/        Renderer wgpu: frame do core -> textura -> tela (letterbox)
  audio-desktop/        AudioSink cpal + Dynamic Rate Control
  library-scan/         Hash de ROMs (CRC32/MD5) + varredura de biblioteca
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

Ver `TASKS.md` para o checklist detalhado. Resumo:

- [x] Workspace (Cargo + pnpm), traits do `domain`, migration SQL inicial
- [x] `apps/desktop` inicializado (Tauri v2) + `packages/app-desktop` (React 19)
- [x] `cargo tauri dev` abre a janela
- [x] **Etapa 01** — repositórios sqlx em `crates/db` (7 repos, 18 testes de integração)
- [x] CI (`.github/workflows/ci.yml`), toolchain pin, rustfmt, LICENSE
- [x] **Etapa 02 (parcial)** — `core-loader-desktop`: caminho software-only
      ponta a ponta (libloading + FFI libretro), 5 testes com core-fake em C
- [x] **Etapa 03 (parcial)** — `emu-session` (loop + foco), `video-surface`
      (renderer wgpu testado headless), vídeo no app (Linux X11 child window)
- [x] **Etapa 07 (parcial)** — casca Fluent (MenuOverlay, ToastLayer,
      CoreOptionsPanel), Settings com config de áudio real via SQLite
- [x] **Etapa 06 (parcial)** — `audio-desktop`: DRC puro (testado) +
      `CpalAudioSink`, fiado no `emu-session` (o som do core sai de verdade)
- [x] **Etapa 09 (parcial)** — `library-scan`: hash CRC32/MD5 + varredura;
      a tela Biblioteca escaneia um diretório e lista de verdade
- [x] **Etapa 08 (parcial)** — save state: arquivo em disco + metadata +
      validação de core no load (comandos Tauri, 4 testes)
- [ ] Etapa 02 passo 4 — contexto GL pra cores HW-accelerated
- [ ] `apps/mobile`, `packages/ui`, `packages/shared`

## Rodar

```bash
cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json   # app (modo só-webview no Linux)
cargo run -p video-surface --example play                         # player de vídeo standalone
cargo run -p video-surface --example play -- <core_libretro.so> <rom>
cargo test --workspace                                            # 32 testes
```

## Próximo passo recomendado

Confirmar visualmente que o jogo aparece atrás da webview transparente no Linux
(`cargo tauri dev --features dev-autoload` com `REEMU_DEV_CORE`/`REEMU_DEV_ROM`),
ajustar z-order/transparência se preciso. Depois: passo 4 da etapa 02 (contexto
GL pra cores HW-accelerated).
