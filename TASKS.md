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
| 03 | Tauri Desktop Shell — surface nativa | `done` (vídeo via `<canvas>`; SET_ROTATION 2026-08-31; surface nativa adiada — ver doc 03) | 02 |
| 04 | Shader Chain + Decoração | `done` (shader + decoração + params na UI — 2026-08-30) | 03 |
| 05 | Input, Hotkeys, UI de Binding | `done` (validado c/ DualSense 2026-08-29; foco de menu 2026-08-30) | 03 |
| 06 | Áudio — Dynamic Rate Control | `done` (DRC + sink + "aplicar ao vivo"; falta só validar sessão longa ouvindo — usuário) | 03 |
| 07 | Frontend React — Fluent/Zustand/Toast | `done` (modo Xbox completo: Início/Biblioteca/RomDetail/PlayScreen, Griffel, busca, nav por controle) | 03 |
| 08 | Save States e Save RAM | `done` (thumbnail por slot + painel na UI + "jogar daqui" — 2026-08-30) | 02 |
| 09 | Scraping de Metadata | `done` (ScreenScraper por CRC + revisão manual; multi-provider/IGDB no backlog) | 01 |
| 10 | Catálogo e Download de Cores | `done` (68 cores do buildbot: software + GL marcados "precisa de GPU"; Vulkan-only fora até 12) | 01 |
| 11 | Port Android | `todo` (desbloqueado — 01–10 `done`; usuário adiou 2026-08-30) | 03–10 completas no desktop |
| 12 | Vulkan HW Render Fase 2 | `blocked` (backlog — falta GL HW / etapa 02 passo 4 primeiro) | Gatilho de maturidade — ver doc 12 |

**Desktop (01–10) fechado em 2026-08-30.** Só falta validação em hardware
de sessão longa de áudio (etapa 06). Próximos: 11 (Android) ou os itens de
render do backlog.

## Como atualizar

Ao concluir uma etapa:
1. Marque a linha correspondente como `done` nesta tabela
2. Se algo ficou pra trás/foi simplificado, anote em uma linha de nota
   logo abaixo da tabela (ex: "Etapa 04: Mega Bezel funcional, mas sem
   suporte a preset com sub-diretórios aninhados — ver issue X")
3. Se uma etapa não pode prosseguir por dependência externa (ex: aguarda
   decisão sua), marque `blocked` com o motivo

## Backlog (fora da ordem principal — não iniciar antes de fechar 06/08/09)

Renderização / filtros (independente da etapa 12):
- **Integer scaling** — trava o `<canvas>` num múltiplo inteiro da resolução
  nativa + toggle em Config › Vídeo. Só frontend, ~0,5 dia, risco zero.
- **Seleção de preset por pasta** — comando `list_slangp_dir` + UI pra apontar
  pra uma pasta `shaders_slang` (RetroBat) e escolher `.slangp` de uma lista
  (hoje só "Carregar .slangp…" arquivo a arquivo). ~1 dia, não toca `gpu.rs`.
- **Presets que já compilam no `naga`** — curar lista dos que funcionam
  (sharp-bilinear, pixellate, scale2x/3x, super-xbr, MMPX, crt-geom/lottes/
  zfast, CAS/RCAS). Zero código.
- **Compilador slang via glslang→SPIR-V** (substitui `naga` glsl-in) — ~1-2
  semanas, TOCA `crates/shader-slang`. Destrava FSR 1.0 completo, xBRZ,
  ScaleFX, CRT-Royale, guest-advanced.
- **Feedback / OriginalHistory / LUT no `gpu.rs`** — ping-pong + ring de
  textura + carregar PNGs de LUT do `.slangp`. ~1 semana. Destrava Mega Bezel
  e CRT shaders de qualidade média.
- **HDR / tonemapping** — depois do compilador completo.

Cores com GPU:
- **GL HW render (etapa 02 passo 4)** — contexto GL offscreen + callbacks
  (`get_current_framebuffer`/`get_proc_address`/`context_reset`) + interop
  GL↔wgpu. Destrava N64/PSX-hw/Saturn/DS/Dreamcast/PSP/GC-Wii/PS2 (GL). É
  pré-requisito duro da etapa 12.
- **Etapa 12 (Vulkan HW)** — `blocked`, ver doc 12. Só depois do GL estável +
  lista de cores-alvo definida. NÃO temporal upscaling (DLSS/FSR2/XeSS não
  servem — sem motion vectors na emulação).

Metadata (etapa 09 fechada no MVP):
- Multi-provider (IGDB / TheGamesDB) + cascata; rate-limit por provider;
  match por MD5 além de CRC; badge de "N pendências" no rail.

Áudio (etapa 06):
- Validação de sessão longa sem glitch (precisa core real + ouvir — usuário).
- Resampler linear → `rubato` se a qualidade não bastar.

Correção de vídeo (cores software):
- ~~`RETRO_ENVIRONMENT_SET_ROTATION`~~ — **feito 2026-08-31**: `ffi_state.rs`
  captura o valor, `FrameMetadata.rotation_degrees`, `domain::rotate_rgba`
  aplica na CPU no `poll_frame` (ambos os caminhos), o `<canvas>` acompanha a
  AR (declarada quando a orientação bate, dos pixels quando veio rotacionado).
  Falta: validar com um jogo vertical real (FBNeo). Direção assumida =
  anti-horário (libretro.h); flipar se sair espelhado.
- **`SET_GEOMETRY` / `SET_SYSTEM_AV_INFO` em runtime** — `aspect_ratio` vem do
  `av_info` do load; se o core muda a proporção no meio do jogo não pega
  (resolução em si já pega, é per-frame). Baixa prioridade pros cores atuais.

Infra:
- `packages/ui`, `packages/shared` — ainda sem `package.json`.
- `apps/mobile` / `packages/app-mobile` (etapa 11 Android) — só depois do
  desktop ponta a ponta (decisão do usuário 2026-08-30: deixar pra depois).
- Windows/macOS — o `#[cfg(not(linux))]` do `video.rs`, os paths do buildbot e o
  bundle nunca foram verificados (sem máquina).
- Readback: pool pro `Vec` do `pack_frame`; canvas WebGL em vez de
  `putImageData`.
- Dois controles idênticos colidem na porta (mesmo GUID SDL — usar `GamepadId`
  do gilrs); eixo analógico → RetroPad (hoje só stick→d-pad digital).
- `docs/ai-context/01,02,05,06,07,08,09.md` têm seções "Estado atual
  (in-progress)" desatualizadas — `TASKS.md` é a fonte da verdade.
- `SaveStateMetadata.play_time_at_save` sempre `None` (sem tracking de tempo
  de jogo).

## Notas de progresso

- **2026-08-30 — Etapa 04 fatias 3c/4/5 + modo Xbox + fixes**:
  - **Shader por jogo/sistema no DB** (fatia 3c): `ShaderChainStore`
    (upsert/list/set/clear assignment) em `ShaderChainRepo`; builtins semeados
    no startup; `set_shader(name, scope, rom_id)`; `load_game` resolve em
    cascata rom→sistema→default. `RomDetail` tem `<Select>` de shader do jogo.
  - **Decoração / bezels** (fatias 4+5) — **VALIDADO pelo usuário 2026-08-30**:
    `scan_decoration_pack` (Bezel Project/RetroBat, deep-scan `WalkDir`),
    `DecorationStore` em `DecorationRepo`, `import_pack` casa stem→rom_id
    (contra TODAS as linhas de ROM que batem — a lib pode ter duplicata),
    composição no `gpu.rs` (jogo no viewport do `.cfg` ou centralizado 4:3 +
    bezel alpha-blend), exclusão mútua com shader que já traz moldura.
    Comandos `import_decoration_pack` / `clear_decorations` em Config › Vídeo.
    - **Bug 1**: biblioteca duplicada (2 drives, mesmo rótulo "Novo volume") →
      bezel casava a linha errada. Fix: `match_roms` grava pra todas.
    - **Bug 2 (o que travava)**: bezels do Bezel Project são **PNG paletado +
      tRNS**; `decode_png` só tratava RGB/RGBA → erro silencioso. Fix:
      `Transformations::EXPAND | STRIP_16` + normalização → RGBA8. +1 teste.
  - **Remover ROMs**: comando `remove_rom` (1) + `remove_rom_system` (snes/nes/
    …) + `remove_rom_source` (pasta de origem) + `clear_library`. UI: chip
    "Gerenciar biblioteca" na tela Meus jogos + "Remover sistema" no cabeçalho
    de cada seção.
  - **Modo Xbox (etapa 07)**: `/` virou **Início** (`Home`: hero + faixas
    "Continuar jogando" / "Adicionados recentemente"); a biblioteca completa é
    `/library` ("Meus jogos", grade vertical por sistema). Busca global (Y / `/`
    / campo centralizado), menu de contexto no cartão (☰ / clique-direito),
    dicas de botão cientes de contexto, `last_played_at` / `added_at` no
    `RomDto`. **`styles/xbox.css` migrado 100% pra Griffel** (`styles/xbox.ts`,
    `makeStyles` + `tokens`); doc novo `docs/design/fluent2.md`
    (<https://fluent2.microsoft.design/>).
  - **Foco do controle nos menus**: o anel só respondia a `:focus-visible`, que
    o WebKitGTK não marca no `.focus()` vindo de evento Tauri → foco movia
    invisível. Trocado por `:focus`; `focusNav` pula o campo de busca.
  - **Fatia 6 — parâmetros de shader na UI (2026-08-30)**: `FrameProcessor`
    guarda `param_meta` (dos `#pragma parameter`); `set_shader_param(name,val)`
    clampa e entra no uniform buffer sem rebuild. `ShaderChainStore` ganhou
    `set_parameter_override`/`clear_parameter_overrides` (tabela
    `shader_parameter_overrides` já existia; upsert por `assignment_id::key`).
    Comandos `get_shader_params` / `set_shader_param(scope?)` /
    `reset_shader_params(scope?)`. `apply_resolved_shader_ex` aplica os
    overrides do assignment ao carregar o jogo. Front: `<ShaderParams>`
    (sliders Fluent + "Restaurar padrões", debounce 200ms) em `SettingsVideo`
    (scope default) e `RomDetail` (scope rom; ganhou "Carregar .slangp…" por
    jogo). +1 teste (`shader_parameter_overrides_roundtrip`). **Etapa 04 →
    `done`.**
  - VERIFICADO: `cargo test --workspace --all-features` (todos ok) + `clippy -D
    warnings` limpos; `tsc -b` / `oxlint` / `vite build` limpos.
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
    p/ .so) — 8 testes de integração (load/run/frames, save state, save RAM,
    core options, rejeição de HW core, um-por-processo, not-found).
  - **2026-08-28** — descoberta de cores + core options + save RAM:
    - `discover.rs` — `discover_cores(dir)` varre `*_libretro.<suf>`, espia
      `retro_get_system_info`/`retro_api_version` (sem `retro_init`).
    - `coreopts.rs` — parse de `SET_VARIABLES` (v0) / `SET_CORE_OPTIONS` (v1) /
      `SET_CORE_OPTIONS_V2` + variantes `_INTL`; `GET_VARIABLE`/`GET_VARIABLE_UPDATE`
      agora funcionam (`GET_CORE_OPTIONS_VERSION` → 2). API livre de thread:
      `core_options()` / `core_option_values()` / `set_core_option()` /
      `set_pending_core_option_values()` (valores do DB aplicados no load).
    - `DesktopCore::{save_ram, restore_save_ram}` — `retro_get_memory_data`/
      `_size(RETRO_MEMORY_SAVE_RAM)`.
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
  - **2026-08-30 — pacing + build otimizado** (travadas no GBA): o `core_loop`
    (session.rs) passou a pacear por **acumulador** (`next_deadline`) + **spin**
    no último ~1.2ms em vez de `thread::sleep` puro (que passa do ponto no
    Linux e causa microstutter, pior em cores com fps ≠ 60 como GBA 59.73).
    `poll_frame` era chamado ~250×/s → o fetch loop da `PlayScreen` agora
    espera ~11ms após pegar um frame. `Cargo.toml` raiz ganhou
    `[profile.dev] opt-level=2` + deps em `-O3` (sem isso o pipeline não
    sustenta 60fps em `cargo tauri dev`) + `[profile.release]` lto/1-cgu.
  - **2026-08-30 — aplicar ao vivo (etapa 06 `done`)**: `Command::ReloadAudio(
    AudioSinkFactory, reply)` + `EmuSession::reload_audio()` — recria o
    `AudioSink` na thread do core (dropa o stream cpal antigo antes). O comando
    `update_audio_config` persiste E chama `reload_audio` com a config nova
    (spawn_blocking). Muda device/sample rate sem recarregar o jogo. Toast
    "Áudio salvo e aplicado".
  - Falta (backlog): validar sessão longa sem glitch (core real + ouvir —
    usuário); resampler linear → `rubato` se a qualidade não bastar.
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
  - **2026-08-28** — `gilrs`: `input-desktop::GamepadPoller` (poll de gamepad
    físico numa thread de `emu-session`, `enable_gamepad`), `Button` normalizado
    → RetroPad (convenção libretro), 1ª controle = porta 0; botão `Mode` →
    toggle de menu (via `take_menu_request` no loop de eventos). +4 testes.
  - **2026-08-28** — UI de captura de binding: `input_desktop::capture` (flag
    global; enquanto ligada, teclado/gamepad vão pro frontend por
    `emit("raw-input-captured")` em vez de irem pro jogo). Comandos
    `start_binding_capture` / `cancel_binding_capture` / `save_binding`
    (`target` = `system_hotkey` | `controller_mapping`, ambos com combinação
    hold+press) / `list_system_hotkeys` / `clear_system_hotkey` /
    `list_controller_mappings`. `db::SystemHotkeysRepo` +
    `db::ControllerMappingsRepo` (trigger/layout serializados em JSON). +3
    testes. Frontend: `useBindingCaptureStore` (Zustand transitório, janela de
    ~300ms), `<BindingCapture>` (diálogo único), seção "Atalhos de sistema" em
    Settings.
  - **2026-08-28** — `HotkeyResolver` ligado ao DB em runtime:
    `input_desktop::held` (conjunto segurado global, teclado + gamepad),
    `AppState.hotkeys: Mutex<ComboHotkeyResolver>` semeado do `system_hotkeys`
    no startup (default `ToggleMenuOverlay` = `F1`; `Esc` fica hardcoded como
    rede de segurança). `commands::poll_hotkeys` roda a cada frame no loop de
    eventos ANTES do roteamento pro jogo (prioridade), dispara 1×/aperto:
    `ToggleMenuOverlay` alterna o foco, `QuickSave`/`QuickLoad` emitem
    `hotkey-action` (frontend só avisa por toast — falta contexto de ROM).
    `save_binding`/`clear_system_hotkey` recompõem o resolver. `FocusController`
    limpa o `held` na transição. +2 testes.
  - **2026-08-28** — mapeamento de controle do DB em runtime:
    `input_desktop::mappings` (override global lido pela thread de gamepad;
    `set`/`resolve`, combinação suportada). `GamepadPoller` agora recompõe o
    RetroPad por porta a cada evento a partir dos índices físicos segurados
    (`down` por gamepad + diff contra `applied`), usando o override do `guid`
    se houver, senão o mapa fixo do `gilrs`. `PollOutcome.gamepads`
    (`(guid, nome)` conectados) → `EmuSession::connected_gamepads()`. Comandos
    `list_gamepads` / `clear_controller_mapping`; `save_binding` e o startup
    republicam o override. Frontend `<ControllerMappings>` (seção "Controles"
    em Settings): junta gamepad conectado + mapa salvo, grade de 16 botões
    RetroPad com rebind/`+`, "Limpar mapa". +1 teste.
  - **2026-08-28** — fechamento da etapa 05:
    - stick esquerdo → d-pad (`GamepadPoller` trata `AxisChanged`, limiar 0.5,
      alimenta a mesma recomposição de RetroPad). +1 teste.
    - `QuickSave`/`QuickLoad` reais: `AppState.current_rom` (setado por
      `load_game(rom_id)`), `QUICK_SLOT = 0`, `poll_hotkeys` dispara uma task
      async que grava/restaura via `save_state.rs` e devolve toast por
      `hotkey-action {action, ok, message}`.
    - `device_port_assignment`: `domain::input::DevicePortRepository` +
      `db::DevicePortsRepo` (cria linha vazia em `controller_mappings` pro FK) +
      `input_desktop::mappings::{set_ports,port_for}` (poller consulta antes da
      ordem de conexão). Comandos `set_device_port`/`clear_device_port`/
      `list_device_ports`; `<Select>` de porta por controle na UI. +1 teste.
    - rótulos amigáveis do botão físico na UI (`describeRawInput`: "A (baixo)",
      "D-pad ↑", "Guia"…).
    - `<IdleScreen>` — tela cheia opaca quando `session_state == Idle` (a
      webview transparente só faz sentido com jogo rodando).
  - **2026-08-29 — validado em hardware (DualSense/PS5)**: jogo reconhece o
    controle (RetroPad OK), áudio OK. **Bug**: os menus não respondiam ao
    gamepad — o `useGamepadNav` dependia da Gamepad API do navegador, que o
    WebKitGTK 2.52 nesse setup não expõe (nunca dispara `gamepadconnected`).
    - Correção: navegação de menu passou a ser resolvida pelo **gilrs no
      backend**. `input_desktop::gamepad`: novo `NavPulse` (Up/Down/Left/Right/
      Confirm/Back) com edge-detection + auto-repeat (delay 380ms, repeat
      150ms); `PollOutcome.nav`. `emu-session`: `Shared.nav` +
      `EmuSession::take_nav_pulses()`. Shell: emite `menu-nav` no loop de
      eventos, **só quando `session.state() != Running`** (Idle no launcher,
      Paused no menu de pausa — durante o jogo o d-pad vai só pro RetroPad).
    - Frontend: `lib/focusNav.ts` (`moveFocus` extraído do hook), `onMenuNav`
      em `tauri.ts`, `useGamepadNav` escuta `menu-nav` em vez de pollar a
      Gamepad API, `PlayScreen` trata `menu-nav` no menu de pausa (confirm =
      clica, back = continua, setas = move foco; foca o 1º item ao abrir).
    - **2ª rodada**: a 1ª versão não funcionou — o event loop do Tauri fica em
      `Wait` quando a webview está ociosa, então o `MainEventsCleared` (onde a
      ponte de input rodava) não tiquetaqueia no launcher. Movido pra uma
      **thread dedicada `spawn_input_bridge`** (~60Hz, independente do loop);
      d-pad-como-eixo (DualSense) agora tratado (`Axis::DPadX/DPadY` → `hat`).
    - Cartões da Library "piscando": `QueryClient` com `refetchOnWindowFocus:
      false` + `staleTime: 5min` (o WebKitGTK dispara foco espúrio).
    - **3ª rodada**: menu de pausa ainda não navegava — `moveFocus` filtrava
      por `offsetParent` que o WebKitGTK zera dentro de `position: fixed`.
      Trocado por `getBoundingClientRect()`. Gate `state != Running` removido
      da emissão do `menu-nav`. **Validado com DualSense**: launcher, seleção
      1-a-1 e menu de pausa todos navegando pelo controle.
  - **2026-08-29 — 2 correções menores**:
    - `load_game` agora registra o core em `installed_cores` (via `get`+
      `register`) antes do `replace_schema` — a FK de `core_options_schema`
      falhava porque a descoberta por disco não persistia nada (WARN
      "salvando schema de core options: FOREIGN KEY constraint failed").
    - Splashscreen: `PlayScreen` no estado `loading` agora mostra capa (ou
      iniciais) + título + sistema + spinner, no lugar do spinner solto.
      `RomDetail` passa `boxart`/`system` no state da navegação; `initials`
      extraído pra `lib/initials.ts` (compartilhado com `GameCard`).
  - **2026-08-29 — Etapa 04 fatia 1 (caminho GPU)**: usuário escolheu o
    pipeline slang completo do doc. Como a surface nativa está desligada nesse
    ambiente, o wgpu roda **headless** (sem surface → sem conflito com o GTK).
    `src-tauri/src/gpu.rs` — `FrameProcessor` (contexto wgpu + `video_surface::
    Renderer` + alvo offscreen + readback). `poll_frame` passa o frame por ele
    (blit 1:1, passthrough) e cai no CPU (`to_rgba8`) em qualquer falha.
    `AppState.gpu: Mutex<Option<FrameProcessor>>`, init no setup do `lib.rs`.
    Dep `wgpu = "30"` no shell. Verificado: adapter Vulkan (RTX 3060) sobe
    headless sem crash. `REEMU_NO_GPU=1` desliga (volta pro caminho CPU).
    - **1ª tentativa distorceu** (usar `video_surface::Renderer` aqui aplicava
      `letterbox_scale` — pra SNES, squish vertical + tarjas). Reescrito como
      **blit 1:1 próprio** em `gpu.rs` (fullscreen-triangle + `textureSample`,
      sampler nearest, sem uniforms/escala) — é a primitiva de cada passe da
      cadeia multi-passe. A proporção continua sendo do `<canvas>`/CSS.
    - **Fatias 2+3 (2026-08-29) — motor multi-passe + parser `.slangp`**:
      - `crates/shader-slang` (NOVO, membro do workspace) — parser isolado do
        `.slangp` (`parse_slangp`/`parse_slangp_file`, segue `#reference`,
        scale_type x/y, wrap, alias, parâmetros, texturas do usuário). 6 testes.
      - `gpu.rs` reescrito como **cadeia multi-passe**: N passes fullscreen-
        triangle, cada um amostrando a saída do anterior, com escala/filtro por
        passe e uniforms semânticos (`source_size`/`output_size`/`orig_size`/
        `frame`). Presets embutidos em WGSL: `plain` (1p), `crt` (2p:
        sangramento H → scanline+máscara+vinheta 2x), `lcd` (1p, grade 2x).
        `REEMU_SHADER=` escolhe; `set_preset()` troca em runtime.
      - Comandos `get_shader_info` / `set_shader`; aba **Configurações › Vídeo**
        (`SettingsVideo.tsx`) com os 3 presets.
      - Verificado: `plain` e `crt` (2 passes) compilam WGSL e sobem sem erro.
      - **Fatia 2b (2026-08-29) — compilador `.slang`** em `crates/shader-slang`:
        `preprocess.rs` (`#include` c/ guard, split de estágios, `#pragma
        name`/`parameter`) + `compile.rs` (rewrite push_constant→UBO e
        `sampler2D`→texture+sampler + call-sites → `naga` glsl-in/wgsl-out;
        `Feedback`/`OriginalHistory` → `Unsupported`). **11 testes**. Falta o
        wiring no `gpu.rs` (reflection do bloco uniforme) — fatia 3b.
      - **Tela cheia**: `tauri.conf.json` `fullscreen: true`; comandos
        `is_fullscreen`/`set_fullscreen`; `useFullscreen`/`useFullscreenSync`
        (F11 global), botão na topbar + no menu de pausa.
      - Tuning CRT/LCD depois dos prints do usuário (CRT 3x, máscara mais leve).
      - **Fatia 3b (2026-08-29) — `.slangp` roda na cadeia**: `gpu.rs` reescrito
        com quad em vertex buffer (Position vec4 + TexCoord, triangle-strip);
        builtins e slang no mesmo motor. `UniformMode::{Fixed, Slang(layout)}` —
        no modo slang o buffer é montado por reflection (`compile.rs::reflect`
        via IR do naga: offsets/tipos dos campos) preenchendo `MVP` (ortho
        [0,1]→[-1,1]), `SourceSize`/`OriginalSize`/`OutputSize`/
        `FinalViewportSize`, `FrameCount`/`FrameDirection`, e parâmetros por
        nome (defaults do `#pragma parameter` + override do `.slangp`).
        `REEMU_SHADER=/caminho/x.slangp` ou botão "Carregar .slangp…" em
        Configurações › Vídeo (`pickSlangp`). **Verificado**: preset slang CRT
        de teste (`~/.local/share/com.reemu.desktop/shaders/test-crt.slangp`)
        compila, valida e sobe sem erro; builtins seguem OK.
      - **UBO + Push (2026-08-29)**: shaders slang do RetroArch têm 2 blocos
        uniformes. `reflect_all` → `Vec<(binding, layout)>`; `rewrite` força
        `Push`→binding 0, `UBO`→binding 3 (`declares_block` casa a palavra
        inteira; BGL do `gpu.rs` ganhou binding 3, `Pass.ubuf: [Buffer; 2]`).
        **Validado**: `scanline.slangp` real do RetroBat compila e sobe sem
        fallback (1 passe, 2 parâmetros).
      - Nota: `/` estava 100% cheio → limpei `target/debug/incremental` (19G).
      - **Fatia 3c (2026-08-30) — preset por jogo/sistema no DB**: domain
        `ShaderChainStore` (upsert_preset / list_presets / set_assignment /
        clear_assignment) impl em `ShaderChainRepo` (troca a atribuição do
        escopo via DELETE+INSERT numa tx). +1 teste (cascata + replace).
        Builtins semeados no startup (`seed_builtin_shader_presets`).
        `set_shader(name, scope, rom_id)` — `scope`: session / `default` /
        `rom` (name vazio = limpar). `get_rom_shader(rom_id)` → preset
        resolvido + `from_rom`. `load_game` chama `apply_resolved_shader`
        (cascata rom→sistema→default, senão `plain`). `FrameProcessor.
        preset_source` pro dedup. Front: `SettingsVideo` persiste como
        `default`; `RomDetail` tem `<Select>` "Shader deste jogo".
      - **Fatia 4+5 (2026-08-30) — decoração / bezels**:
        `library_scan::scan_decoration_pack` + `viewport_for_image` (convenção
        Bezel Project/RetroBat + `.cfg` `custom_viewport_*`, 2 testes); domain
        `DecorationStore` impl em `DecorationRepo`; shell `decoration.rs`
        (`import_pack` mapeia stem→`rom_id`, `decode_png` RGB/RGBA8); comandos
        `import_decoration_pack`/`clear_decorations`; `load_game` →
        `apply_resolved_decoration` (cascata); **exclusão mútua** (pula se o
        shader ativo tem `includes_bezel`); `gpu.rs` passe de composição (jogo
        no viewport do `.cfg` ou centralizado + bezel PNG alpha-blend);
        `LoadedGame.aspect_ratio` vira a da moldura. Front: import/remover em
        Config › Vídeo.
    **Próxima fatia**: (6) parâmetros de shader ajustáveis na UI (os
    `#pragma parameter` já são lidos + buffer montado por nome).
  - **Falta / follow-up**: caso de dois controles idênticos (mesmo GUID SDL →
    colidem na porta — precisa usar o `GamepadId` do `gilrs`); saída de eixo
    analógico pro RetroPad (hoje só stick→d-pad).
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
  - **2026-08-28** — save RAM (battery): `emu-session` carrega
    `<saves>/<stem>.srm` no core logo após o load e regrava a cada 10s + no
    unload/troca/shutdown (`DesktopCore::{save_ram,restore_save_ram}`). 2 testes.
    QuickSave/QuickLoad no menu de pausa da `PlayScreen`.
  - **2026-08-30** — shutdown limpo: `RunEvent::ExitRequested` (`lib.rs`) faz
    `session.unload()` ao fechar (X da janela / Alt+F4 / `quit_app`), garantindo
    o flush final da `.srm` — antes só o flush periódico de 10s cobria.
    `flush_save_ram` virou escrita atômica (`.srm.tmp` + rename).
  - **2026-08-30 — thumbnail + painel (etapa 08 `done`)**: `poll_frame` guarda
    uma cópia throttled (1×/500ms) do último frame em `AppState.last_frame`
    (`CachedFrame`); no `save_state`/QuickSave o `thumbnail_png` (nearest →
    320px → PNG via crate `png`) é gravado ao lado do `.state` como `.png`.
    `SaveStateMetadata.thumbnail_path` (schema já tinha a coluna); `delete`
    remove os dois. `SaveStateDto.has_thumbnail` + comando
    `read_save_thumbnail` (PNG por IPC → blob URL). Front:
    `components/SaveStateThumb.tsx`, `RomDetail` lista com miniatura + "Jogar
    daqui" (`/play?loadState=<id>` → `PlayScreen` carrega o state após o boot),
    e o menu de pausa ganhou a lista completa de estados (miniatura + carregar).
    +0 teste novo (os 4 de `save_state.rs` seguem, com `None` no arg novo).
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
  - **2026-08-28** — capas via **thumbnails da libretro** (MVP, sem API key):
    `library_scan::libretro_boxart_url(system_id, título)` monta
    `https://thumbnails.libretro.com/<Sistema>/Named_Boxarts/<Nome>.png`
    (mapa de ~15 sistemas + sanitização de nome no padrão RetroArch). `RomDto`
    ganhou `boxart: Option<String>`; `<GameCard>` e `RomDetail` mostram como
    `<img>` com fallback pras iniciais no `onerror`. Casa melhor com ROMs
    No-Intro. +1 teste. URL verificada 200 no CDN.
  - **2026-08-30 — etapa 07 fechada**: `RomDetail` reescrito no estilo "página
    de jogo do Xbox" (hero com arte/capa da metadata + scrim, título grande,
    badges de sistema/ano/gênero, descrição, botão Jogar/Continuar grande,
    seções em painéis: shader do jogo, opções do core, save states, remover).
    Menu de pausa da `PlayScreen` migrado pro estilo Xbox (`usePauseStyles`,
    painel escuro + anel de foco próprio já que `/play/*` fica fora do
    `.xb-app`). `styles/xbox.ts` += `useDetailStyles` / `usePauseStyles`.
  - **2026-08-30 — MetadataProvider real (ScreenScraper)**: migration
    `0003_metadata.sql` (`scrape_matches.candidate_json`, índices únicos por
    rom, `metadata_config` singleton). domain: `GameMetadata`, `ScrapeQuery`,
    `PendingMatch`, `MetadataConfig`; trait `MetadataProvider::search` +
    `MetadataRepository` (get/upsert metadata, record_match, rom_ids_without_match,
    list_pending, resolve_pending). `db::MetadataRepo`. Shell `scraping.rs`:
    `query_screenscraper` (`jeuInfos.php` por CRC + systemeid, `ssid`/`sspassword`
    opcionais; parse título/synopsis/dates/genres/medias), `scrape_pending`
    (task em background, delay 1.2s, cancelável, `ScrapeProgress` atômico).
    **Hash exato → `auto_matched` + metadata aplicada; qualquer coisa por nome →
    `pending_review`** (Abordagem B respeitada). Comandos
    `get/set_metadata_config`, `start_metadata_scan`, `metadata_scan_progress`,
    `cancel_metadata_scan`, `get_rom_metadata`, `list_pending_matches`,
    `resolve_pending_match`. Front: aba **Configurações › Metadata**
    (`SettingsMetadata.tsx` — credenciais, botão escanear + progresso, lista de
    revisão aceitar/rejeitar); `RomDetail` mostra título/descrição/ano/gênero/
    capa da metadata quando existe. +1 teste db (21 agora).
  - **Falta**: multi-provider (IGDB/TheGamesDB) + cascata; rate-limit por
    provider; match por MD5 além de CRC; UI: badge de "N pendências" no rail.
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
  - **2026-08-28** — reestruturação em **rotas (modelo launcher, opção A)**:
    `react-router-dom` v7 + `createHashRouter` (webview, sem servidor).
    - `layouts/`: `RootLayout` (Outlet + `BindingCapture` + `ToastLayer`),
      `AppShell` (rail Biblioteca/Configurações/Cores, **fundo opaco**),
      `SettingsLayout` (abas → sub-rotas).
    - Rotas: `/` Library · `/rom/:romId` RomDetail (core picker + save states +
      "Jogar") · `/settings/{audio,hotkeys,controllers,cores}` · `/play/:romId`
      PlayScreen (**transparente**, HUD, Esc → menu de pausa com QuickSave/Load/
      Sair; `loadGame` on mount / `unloadGame` on unmount).
    - `Settings.tsx` quebrado em `screens/settings/*`; `MenuOverlay`,
      `IdleScreen`, `App.tsx` removidos. `lib/toast.ts` (`sysToast`).
    - Backend: comando `unload_game` (limpa `current_rom` + `session.unload()`);
      `load_game` ganhou `rom_id`. `index.css`: `body` transparente, cada rota
      pinta seu fundo.
  - **2026-08-28** — pré-requisitos de backend do frontend:
    - Diretório de dados **único** (`data_dir()` no `lib.rs` → SQLite + cores +
      saves + system no mesmo lugar; `dirs_or_temp` removido; `AppState` ganhou
      `cores_dir`).
    - `list_installed_cores` agora **varre `<dados>/cores/`** (`discover_cores`)
      e cruza com `installed_cores` (render backend). DTO ganhou `name` +
      `extensions`. `SettingsCores` + o core-picker do `RomDetail` usam isso
      (ordena por extensão que casa com a ROM).
    - Core options: comandos `get_core_options` / `set_core_option` (fonte da
      verdade = core carregado, senão DB); `load_game` semeia os valores salvos
      antes do load e persiste o schema depois. `<CoreOptions>` (Select por
      opção) no `RomDetail`. `CoreOptionsPanel` antigo removido.
  - **2026-08-28** — visual "modo Xbox" + catálogo de cores + navegação por
    controle:
    - `styles/xbox.css` — linguagem visual (rail de ícones, topbar com relógio,
      cartões arredondados, anel de foco forte). `AppShell` reescrito;
      `<GameCard>` / `<ButtonHints>` / `useClock`. `Library` agrupada por
      sistema em grade estilo Xbox.
    - **Catálogo de cores (etapa 10)**: `core_catalog.rs` — cores do buildbot
      oficial (`<stem>.so.zip` → extrai o dylib pra `<dados>/cores/`). Comandos
      `list_core_catalog` / `download_core` / `remove_core` (deps `reqwest`
      rustls + `zip`). Aba **Cores** em Settings ("Instalados" + "Catálogo").
      **2026-08-30 — ampliado pra 68 cores** cobrindo ~todos os sistemas (NES→
      PS2, home computers, arcade, fantasy consoles). `CatalogEntry.hw`:
      `Software` (roda hoje) ou `OpenGl` (baixa, mas `load_game` recusa até o
      contexto GL da etapa 02 — badge "precisa de GPU" na UI, ordenados por
      último). Cores exclusivamente Vulkan ficam de fora até a etapa 12.
      **Etapa 10 → `done`.**
    - **Navegação por controle**: `useGamepadNav` — setas do teclado sempre +
      Gamepad API do navegador (lazy, só após `gamepadconnected` — o WebKitGTK
      reclama se pollarmos sem gamepad). Foco geométrico (bom pra grade), `A`
      clica, `B` volta. Montado no `AppShell`.
  - **Bug resolvido — a webview nunca renderizou nessa máquina** (duas causas):
    (1) Vite 8/Rolldown travava o dev server → **Vite 7.3 + plugin-react 5**;
    (2) a **child window X11 pro vídeo** + `GDK_BACKEND=x11` fazem o WebKitGTK
    2.52 (NVIDIA/XWayland) montar o DOM mas não pintar → **padrão agora é sem
    child window X11**, GTK usa Wayland nativo (`REEMU_X11_VIDEO=1` volta o
    esquema antigo). O vídeo do jogo passou a ser **`<canvas>` na webview**
    (comando `poll_frame` → RGBA8; `to_rgba8` moveu pro `domain`). Confirmado
    visualmente pelo usuário: launcher renderiza.
  - Falta: **confirmação visual do vídeo** (precisa de core + tela; já tem 3
    cores instalados: fceumm/snes9x/gambatte); scraping/metadata (09); shader
    (04); thumbnails de save state; polir RomDetail/PlayScreen no estilo Xbox;
    caçar o ruído do WebKit no dev.
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
  - **2026-08-30 — readback com pipeline** (`gpu.rs`): o readback GPU→CPU fazia
    `device.poll(wait_indefinitely)` — bloqueava a thread do Tauri e serializava
    CPU/GPU (sem pipeline). Agora `ReadbackRing` com 2 staging buffers: o frame
    N copia pro slot N%2 + `map_async`, e lê o slot do frame anterior (já
    mapeado) sem bloquear (`poll(Poll)`). Fallback bloqueante só se o slot ainda
    não mapeou (raro). Atraso de exatamente 1 frame; sem stall no caminho
    normal. +1 teste (`pipelined_readback_has_one_frame_delay`, headless wgpu).
  - **2026-08-30 — Etapa 03 `done` (desvio aceito)**: a surface nativa não
    funciona no WebKitGTK+NVIDIA desse setup (`Gdk Error 71` na `wl_surface`;
    child X11 não pinta). O vídeo do desktop é um **`<canvas>` na webview**:
    `poll_frame` (RGBA8 por IPC, corpo vazio = sem frame novo) + `PlayScreen`
    com dois loops desacoplados (fetch async → ref; paint rAF sincronizado com
    vblank; 3ms rodando / 120ms pausado; freeze automático no menu). Resize =
    CSS. Critério de pronto atendido (SNES validado; foco pausa/resume áudio +
    frame). Caminho nativo fica no código (`REEMU_X11_VIDEO=1`, `#[cfg(not(
    linux))]` Win/macOS) sem verificação. Ver `docs/ai-context/03`.
    Follow-up não-bloqueante: medir latência canvas vs nativo; passo 4 da
    etapa 02 (contexto GL, `FrameOrigin::HardwareTexture` tem o encaixe).
