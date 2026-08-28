# TASKS — Progresso do ReEmu

Checklist de execução, alinhado 1:1 com `docs/ai-context/`. Atualize o
status ao final de cada sessão de trabalho (manual ou com IA) — isso é o
que permite que uma sessão nova retome sem reler todo o código pra
descobrir onde parou.

**Status**: `todo` · `in-progress` · `blocked` · `done`

Ao pedir pra uma IA continuar o projeto, aponte pra este arquivo primeiro
("veja o TASKS.md e continue da próxima etapa `todo`") — evita que ela
refaça trabalho já feito ou pule pré-requisito.

---

## Fundação (feito neste scaffold)

- [x] `done` — Estrutura de workspace (Cargo + pnpm)
- [x] `done` — Traits do `domain` para todas as portas definidas no design
- [x] `done` — Migration SQL inicial com todos os schemas decididos
- [x] `done` — `.gitignore` e `TASKS.md`

## Setup local (ver STEP_BY_STEP.md)

- [x] `done` — `git init` + primeiro commit (remote: github.com/Kluis6/reemu)
- [x] `done` — Toolchain instalada (Rust 1.97, pnpm 11, cargo-tauri 2.11)
- [x] `done` — `cargo check --workspace` passa sem erro (inclui o crate Tauri)
- [x] `done` — `apps/desktop`: `cargo tauri init` executado (pacote `reemu-desktop`,
      id `com.reemu.desktop`, linka `domain` + `db`, no workspace)
- [x] `done` — `packages/app-desktop`: Vite + React 19 + Fluent + Zustand +
      TanStack Query instalados; `vite.config.ts` afinado p/ Tauri (porta 1420)
- [x] `done` — `cargo tauri dev` abre a janela sem erro (2026-08-27)
- [ ] `todo` — `apps/mobile` / `packages/app-mobile` — só depois do desktop ponta a ponta
- [ ] `todo` — `packages/ui`, `packages/shared` — ainda sem `package.json`

## Etapas de implementação (docs/ai-context/01 a 12)

| # | Etapa | Status | Depende de |
|---|---|---|---|
| 01 | Domain + DB — repositórios sqlx | `done` | Setup local |
| 02 | Core Loader Desktop — caminho GL | `done` (software) | 01 |
| 03 | Tauri Desktop Shell — surface nativa | `in-progress` | 02 |
| 04 | Shader Chain + Decoração | `todo` | 03 |
| 05 | Input, Hotkeys, UI de Binding | `in-progress` | 03 |
| 06 | Áudio — Dynamic Rate Control | `in-progress` | 03 |
| 07 | Frontend React — Fluent/Zustand/Toast | `in-progress` | 03 |
| 08 | Save States e Save RAM | `in-progress` | 02 |
| 09 | Scraping de Metadata | `in-progress` | 01 |
| 10 | Catálogo e Download de Cores | `todo` | 01 |
| 11 | Port Android | `todo` | 03–10 completas no desktop |
| 12 | Vulkan HW Render Fase 2 | `blocked` | Gatilho de maturidade — ver doc 12, não iniciar antes das condições lá |

## Como atualizar

Ao concluir uma etapa:
1. Marque a linha correspondente como `done` nesta tabela
2. Se algo ficou pra trás/foi simplificado, anote em uma linha de nota
   logo abaixo da tabela (ex: "Etapa 04: Mega Bezel funcional, mas sem
   suporte a preset com sub-diretórios aninhados — ver issue X")
3. Se uma etapa não pode prosseguir por dependência externa (ex: aguarda
   decisão sua), marque `blocked` com o motivo

## Notas de progresso

- **2026-08-27 — Setup local**: bootstrap completo. `packages/app-desktop`
  (Vite+React19+Fluent+Zustand+TanStack Query) e `apps/desktop/src-tauri`
  (`reemu-desktop`, linka `domain`+`db`). `cargo tauri dev` abre a janela.
- **2026-08-27 — Etapa 01 (`done`)**:
  - Migration `0001` corrigida: `UNIQUE(scope,system_id,rom_id)` (não previne
    duplicata no SQLite) → CHECK de forma + índices únicos parciais por escopo;
    índices em todas as FKs; `ON DELETE CASCADE`. `foreign_keys` ligado por
    conexão no `pool.rs`.
  - `domain`: novo `error::RepoError`; traits de DB viraram `async` (`#[async_trait]`,
    decisão do usuário); novos ports/models `library::{Rom,RomRepository}`,
    `audio::AudioConfigRepository`, `core_loader::{InstalledCore,InstalledCoreRepository}`,
    `core_options::CoreOptionsStore::replace_schema`.
  - `crates/db`: `pool.rs`, `cascade.rs` (resolução rom→system→default genérica,
    1 função pros 2 casos), `convert.rs` (mapa enum↔CHECK do banco). Repos:
    `ShaderChainRepo`, `DecorationRepo`, `CoreOptionsRepo`, `AudioConfigRepo`,
    `InstalledCoresRepo`, `RomsRepo`, `SaveStateRepo`. **18 testes** de
    integração (SQLite in-memory) — cascata (3 escopos + none), FK/CHECK,
    upsert, cascade delete.
  - `save_state`: `SaveStateMetadata`/`SaveRamMetadata` ganharam `id`; port
    dividido — `SaveStateManager` (alto nível, core-loader, etapa 08) vs
    `SaveStateRepository` (só metadata, implementado agora).
  - **Fica pra depois (não bloqueia)**: métodos de *escrita* de assignment
    (criar/editar preset por rom/sistema) entram junto da UI (etapa 04/07);
    wiring dos repos no shell Tauri (managed state) é etapa 03.
- **2026-08-27 — Infra**: `rust-toolchain.toml` (1.97), `rustfmt.toml`,
  `.editorconfig`, `LICENSE` (MIT), `.github/workflows/ci.yml` (fmt/clippy
  `-D warnings`/test + oxlint/build do frontend). `cargo fmt --all` aplicado no
  workspace. Nada commitado ainda (a pedido).
- **2026-08-27 — Etapa 02 (`done` no caminho software)**: crate
  `crates/core-loader-desktop`. HW render GL (passo 4) fica como próximo item
  fora do MVP-software (Vulkan já era backlog/etapa 12); o `Renderer` tem o
  encaixe `FrameOrigin::HardwareTexture` pronto.
  - FFI libretro em `src/sys.rs` (valores/layout conferidos contra `libretro.h`
    do RetroArch — structs `retro_system_av_info`/`retro_hw_render_callback`,
    enums, `RETRO_ENVIRONMENT_*`).
  - `RawCore` (libloading + símbolos `retro_*`), `ffi_state` (estado global +
    callbacks `extern "C"` — libretro é **um core por processo**, sem userdata
    nos callbacks), `DesktopCore` (= `FrameSource`, cada `next_frame` roda um
    `retro_run`), `DesktopCoreLoader` (`CoreLoader` + `load_core` concreto).
  - **Caminho software-only completo**: dlopen → `retro_run` → `video_refresh`
    (buffer cru, repack sem padding) → `Frame::SoftwareRawBuffer`. `SET_HW_RENDER`
    é detectado, os requisitos persistidos (`InstalledCoreRepository`) e no cache,
    e o load recusado com `CoreLoadError::HwRenderUnsupported`.
  - Ponto de extensão de save state pronto (`request_save_state` /
    `poll_save_state` / `serialize_state`) — implementação real é etapa 08.
  - Teste: **core-fake em C** (`fixtures/testcore.c`, compilado pelo build.rs
    p/ .so) — 5 testes de integração (load/run/frames, save state, rejeição de
    HW core, um-por-processo, not-found). Sem baixar core nem ROM real.
  - **Próximo (fora do MVP-software)**: passo 4 — contexto GL real +
    callbacks (`get_current_framebuffer`, `get_proc_address`, `context_reset`),
    validado com um core GL real. Input (`input_state`) é stub aqui (etapa 05);
    áudio agora sai de verdade via `drain_audio` → `emu-session` → `CpalAudioSink`
    (etapa 06). `domain`: `frame_source` ganhou `SoftwarePixelFormat`;
    `core_loader` ganhou `SystemAvInfo` e o trait `LoadedCore` (substituiu o
    marker `LoadedCoreHandle`).
- **2026-08-27 — Etapa 06 (`in-progress`)**: `crates/audio-desktop`.
  - `rate_control.rs` — DRC como **função pura** (fração de buffer → fator de
    ajuste do resample, limitado a ±delta). 6 testes.
  - `sink.rs` — `CpalAudioSink` impl `domain::audio::AudioSink`: cpal 0.18,
    ring buffer, **resample linear de razão variável** (estado entre chamadas),
    fallback pro dispositivo padrão se o `output_device_id` salvo não existir
    (device por `DeviceId` persistente, não índice). 2 testes de resampler.
  - `domain::audio::AudioSink` perdeu `Send + Sync` (a `cpal::Stream` é `!Send`);
    `emu-session` recebe uma **factory `Send`** e constrói o sink na thread do
    core. `FocusController`/pause → `sink.pause()`/`resume()`.
  - App: lê o `AudioConfig` persistido no startup e passa pra factory.
    **Verificado**: o stream cpal abre neste sistema (sem erro).
  - Falta: verificar sessão longa sem glitch (precisa core real + ouvir);
    resampler linear pode virar `rubato` se a qualidade não bastar; comando
    "aplicar áudio ao vivo" (hoje muda no próximo launch).
- **2026-08-27 — Etapa 05 (`in-progress`)**: input.
  - `core-loader-desktop`: `RetroPadState` global (atômico, por porta) fiado
    no callback `retro_input_state_t` (RetroPad digital). 2 testes.
  - `crates/input-desktop`: `sdl_db` — parser do SDL_GameControllerDB
    (swap Nintendo↔Xbox, `bN`/`hN.M`/`aN`), testado com string real de Xbox;
    `ComboHotkeyResolver` impl `HotkeyResolver` (combinação hold+press, combo
    vence tecla única); `KeyboardMap` + `web_code_to_retropad` (`KeyboardEvent.
    code` → RetroPad). ~11 testes.
  - App: comando `input_key` (Escape/F1 = hotkey de menu; senão teclado →
    RetroPad só em `GameFocused`); hook `useKeyboardInput` encaminha
    keydown/keyup. `FocusController` limpa o pad ao entrar no menu.
  - **Falta**: `gilrs` (enumerar/pollar gamepad, `device_port_assignment`),
    `ControllerMappingResolver` a partir do DB, UI de captura de binding.
- **2026-08-27 — Etapa 08 (`in-progress`)**: save states.
  - `EmuSession.loaded_core()` — rastreia o id do core carregado (states não
    são portáveis entre cores).
  - `apps/desktop/src-tauri/src/save_state.rs` — orquestração (testável):
    `save` grava o `.state` em disco (caminho determinístico por slot, troca o
    anterior no mesmo slot) + `record_state`; `load_bytes` valida `core_id`
    (`CoreMismatch`/`NoCore`/`NotFound`); `list`/`delete` (arquivo + registro).
    **4 testes** de integração (SQLite in-memory + dir temp).
  - Comandos Tauri `save_state`/`list_save_states`/`load_save_state`/
    `delete_save_state`; wrappers no `lib/tauri.ts`.
  - **Falta**: captura de thumbnail (frame no instante do serialize → PNG);
    save RAM (battery) com flush automático; verificação de "sem stutter";
    painel de estados na UI.
- **2026-08-27 — Etapa 09 (`in-progress`)**: `crates/library-scan`.
  - `hash.rs` — `FileRomHasher` impl `domain::metadata::RomHashService`:
    CRC32 (`crc32fast`) + MD5 (`md-5`) do arquivo, com **skip do header iNES**
    (`.nes`, 16 bytes). 3 testes (valores conhecidos de "hello world", skip do
    header, arquivo comum).
  - `systems.rs` — extensão → `system_id` (nes/snes/gba/megadrive/...).
  - `scan.rs` — `scan_into(repo, dir, now)`: varre recursivo, dedup por
    `file_path`, pula extensões desconhecidas, popula `RomRepository`.
    `ScanReport { found, added, skipped_known, skipped_unrecognized, errors }`.
    2 testes de integração (SQLite in-memory).
  - `domain::library::RomRepository` ganhou `list()`.
  - App: comandos `list_roms` / `scan_library(path)`; tela Library agora
    escaneia um diretório e lista de verdade (via TanStack Query).
  - **Falta**: `MetadataProvider` real (IGDB/ScreenScraper), fila de jobs em
    background, `scrape_matches`/`game_metadata`. Match automático só com hash
    exato — a política já está clara, falta o provedor.
- **2026-08-27 — Etapa 07 (`in-progress`)**: casca da UI.
  - `packages/app-desktop/src/`: `lib/tauri.ts` (wrappers de comando/evento,
    toleram fora do Tauri), `hooks/useFocusBridge`, `components/{ToastLayer,
    MenuOverlay,CoreOptionsPanel}`, `screens/{Library (mock),Settings (real)}`,
    `App.tsx` compõe HUD + overlay + toasts.
  - `CoreOptionsPanel` gera os controles do schema (`CoreOptionDefinition[]`).
  - Backend: `AppState.db` (SqlitePool, migrations rodam no startup em
    `<app_data_dir>/reemu.db` — **verificado**, 17 tabelas). Comandos
    `get_audio_config` / `update_audio_config` / `list_installed_cores`.
  - `pnpm build` + `oxlint` limpos; `cargo tauri dev` sobe com SQLite + video.
  - Falta: verificação visual do overlay; telas de shader (04) e binding (05);
    Library real (09).
- **2026-08-27 — Etapa 03 (`in-progress`)**: spine testável do shell.
  - Novo crate `crates/emu-session`: `EmuSession` roda o core numa **thread
    dedicada** (`emu-core-loop`) com API de comandos (`load`/`unload`/
    `set_paused`/`save_state`/`restore_state`, todos round-trip) e saída por
    buffers compartilhados (`take_latest_frame`, `drain_audio`, `frame_seq`).
  - `FocusController` implementa `domain::focus::FocusManager` — `toggle()` e
    `set()` pausam/resumem a `EmuSession` na transição `GameFocused ⇄
    MenuFocused` (o core congela, para de produzir áudio). 5 testes (frames
    avançam, pause congela, resume, save/restore, foco pausa).
  - `apps/desktop/src-tauri`: `commands.rs` — `AppState` (`EmuSession` +
    `Mutex<FocusController>` como managed state), comandos `toggle_focus`
    (emite evento `focus-changed`), `load_game` (spawn_blocking),
    `current_focus`, `session_state`. `cargo tauri dev` continua abrindo.
  - Novo crate `crates/video-surface`: `Renderer` wgpu — sobe o
    `SoftwareRawBuffer` numa textura (conversão RGB565/0RGB1555/XRGB8888 → RGBA8
    em `convert.rs`) e desenha um quad com letterbox (shader.wgsl). 4 testes:
    3 de conversão + 1 **render headless real** (upload → render p/ textura
    offscreen → readback → confere a cor). `examples/play.rs` — player
    standalone winit+wgpu (`cargo run -p video-surface --example play`), roda o
    core-fake e mostra a cor mudando; Espaço pausa. Rodou OK localmente.
  - Integração na janela do Tauri: `video-surface::WindowTarget` (surface a
    partir de raw handles) + `apps/desktop/src-tauri/src/video.rs` +
    render na thread principal a cada `RunEvent::MainEventsCleared` + resize.
    Janela agora é `transparent: true`. Feature `dev-autoload` (env
    `REEMU_DEV_CORE`/`REEMU_DEV_ROM`) pra testar sem UI de biblioteca.
  - **Descoberta Linux**: `wgpu::Surface` Vulkan na `wl_surface` da janela
    GTK (com ou sem webview) = `Gdk-Message: Error 71 (protocolo)`, crash — o
    GTK é dono da submissão de buffer daquela surface.
  - **Solução Linux — child window X11** (`video-surface::window_target` +
    `apps/desktop/src-tauri/src/video.rs` mod `x11`): sob X11/XWayland,
    `XCreateSimpleWindow` filha do XID do GTK + `XLowerWindow` (atrás da
    webview) + wgpu Surface nela. `main.rs` força `GDK_BACKEND=x11` +
    `WEBKIT_DISABLE_DMABUF_RENDERER=1`. **Verificado**: child window criada
    (`0x1400001`), wgpu Vulkan anexado, `cargo tauri dev --features
    dev-autoload` roda ~60s sem crash e sem erro de WebKit. `video-surface`
    usa present mode Mailbox/Immediate (não bloqueia a thread do event loop).
  - Windows/macOS: surface direto no handle da janela principal (webview
    transparente compõe) — código pronto, **não verificado**.
  - **Falta pra fechar 02+03**: (a) confirmar visualmente que o jogo aparece
    atrás da webview transparente no Linux (não dá pra ver daqui — pipeline
    está montado); (b) verificar Win/macOS; (c) passo 4 da etapa 02 —
    contexto GL (`Renderer` tem o encaixe `FrameOrigin::HardwareTexture`).
