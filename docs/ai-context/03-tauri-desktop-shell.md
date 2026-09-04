# 03 — Tauri Desktop Shell + Surface Nativa

## Objetivo desta etapa

Montar o app Tauri v2 que hospeda a webview React (menu Fluent) e a
surface nativa de renderização do jogo (fora da WebView), com o overlay
de menu sempre sobreposto.

## Decisões relevantes (não renegociar sem discutir)

- **O jogo renderiza fora da WebView**, numa `wl_subsurface` nativa — **padrão
  no Linux/Wayland desde 2026-09-04** (`src/video.rs`; `REEMU_NATIVE_VIDEO=0`
  volta pro `<canvas>`; fallback automático pro canvas se não for Wayland ou o
  attach falhar). O caminho até aqui — 4 tentativas:
  1. `wgpu::Surface` na `wl_surface` do GTK → `Gdk Error 71`.
  2. child window X11 → o backend X11 faz a webview não pintar (código removido).
  3. `wl_subsurface` `place_below` + webview **transparente** → o lado Rust
     funciona, mas o WebKitGTK **não entrega webview transparente** nesse combo
     NVIDIA (bug upstream tauri#14924, sem fix): compositing off → transparente
     vira preto; on → opaca; DMA-BUF renderer on → webview em branco.
  4. ✅ **`wl_subsurface` `place_above` a webview OPACA + esconder no menu**. Sem
     transparência = sem o bug. Jogando, a subsurface (wgpu, zero-cópia) cobre a
     webview; ao abrir o menu o Rust captura 1 frame, esconde a subsurface
     (`attach(None)`) e a webview reaparece com o print borrado (menu de pausa
     estilo RetroArch). Coreografia = máquina de estado `commands::VideoMenu` no
     `reemu-video-pump`. Posição da subsurface = `(0,0)` em fullscreen (o caso
     comum), deslocada pela espessura da CSD em janela (best-effort via
     `inner_position - outer_position`).

  Env do WebKitGTK (`main.rs`, sempre no Linux): `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  + `WEBKIT_DISABLE_COMPOSITING_MODE=1` — o SW renderer sem compositing é o
  único estável nesse combo (compositing on → loop de `internallyFailedLoadTimerFired`).
  Como a página agora é **opaca**, não precisamos de compositing pra nada.
- **O menu Fluent fica sempre sobreposto** (nunca escondido durante
  `MenuFocused`, e mesmo em `GameFocused` a webview continua viva, só sem
  captar input) — não implemente um modelo de "esconder a webview
  inteira" como alternativa.
- **Entrar em `MenuFocused` pausa o core** (emulação + `AudioSink`) — ver
  `domain::focus::FocusManager`. Isso é orquestrado aqui, no shell, porque
  é quem tem visão dos dois lados (core loop + UI).
- O estado de foco (`InputFocus`) é decidido no lado Rust e propagado pro
  React via evento Tauri (`emit("focus-changed", ...)`) — não tente
  decidir foco no lado JS.

## Estado atual (2026-09-04 — `done`, surface nativa é o padrão)

Fechado. O app abre com a webview React, carrega e roda jogos (SNES + N64 +
bezel validados), alterna foco pausando/resumindo o core, e o vídeo sai numa
**`wl_subsurface` nativa** (Linux/Wayland) ou num **`<canvas>`** (fallback):

- **Surface nativa** (`src/video.rs` + `gpu.rs::render_to_surface` +
  `reemu-video-pump` em `lib.rs`) — a shader chain desenha direto na imagem do
  swapchain da subsurface (zero cópia de CPU). Menu de pausa: captura de 1
  frame + `set_hidden` + webview opaca com o print borrado (CSS). Resize/CSD:
  `RunEvent::WindowEvent::Resized` → `VideoSurface::reconfigure`.
- **`poll_frame`** (`commands.rs`) — caminho `<canvas>`: `take_latest_frame()`
  → GPU (etapa 04) ou CPU (`to_rgba8`) → `[w u32][h u32][rgba8…]`. Corpo vazio
  quando não há frame novo. `PlayScreen` roda o loop de canvas só quando
  `native_video_active()` é `false`.
- `FocusController` pausa/resume a `EmuSession` na transição de foco; o único
  gatilho é o comando `toggle_focus` (via hotkey/gamepad/menu), nunca o React.
- Shutdown: `RunEvent::ExitRequested` → `session.unload()` (flush da `.srm`).

Caminho nativo (detalhes de implementação):

## Histórico da surface nativa (2026-08-27)

Spine testável pronto:
- `crates/emu-session` — `EmuSession` roda o core numa thread dedicada
  (`emu-core-loop`), API de comandos (`load`/`set_paused`/`save_state`/...),
  saída por `take_latest_frame()` / `drain_audio()` / `frame_seq()`.
- `FocusController` implementa `domain::focus::FocusManager`: `toggle()`/`set()`
  pausam/resumem a `EmuSession` na transição de foco.
- `apps/desktop/src-tauri/src/commands.rs` — `AppState` (managed), comandos
  `toggle_focus` (emite `focus-changed`), `load_game`, `current_focus`,
  `session_state`.

Renderer pronto e testado: `crates/video-surface` — `Renderer` wgpu que sobe o
`SoftwareRawBuffer` numa textura e desenha com letterbox. Testado headless
(render p/ textura offscreen + readback). `examples/play.rs` = player standalone
(winit+wgpu+emu-session), roda.

Integração feita: `video-surface::WindowTarget` cria a `wgpu::Surface` de raw
handles; `apps/desktop/src-tauri/src/video.rs` liga na janela; render na thread
principal a cada `RunEvent::MainEventsCleared`. Janela `transparent: true`.

**Linux — resolvido via child window X11**. `wgpu::Surface` Vulkan na
`wl_surface` da janela GTK (com OU sem webview) → `Gdk-Message: Error 71`,
crash: o GTK é dono da submissão de buffer daquela surface. Solução (como
mpv/RetroArch embutidos): sob X11/XWayland, `XCreateSimpleWindow` filha do XID
do GTK + `XLowerWindow` (fica atrás do conteúdo da webview) + `wgpu::Surface`
na child. Implementado em `src/video.rs` (mod `x11`, dep `x11-dl`) +
`video-surface::WindowTarget`. `main.rs` força `GDK_BACKEND=x11` e
`WEBKIT_DISABLE_DMABUF_RENDERER=1` (webkitgtk+XWayland+NVIDIA).

Verificado: child window criada, wgpu Vulkan anexado, `cargo tauri dev
--features dev-autoload` roda sem crash. **Não verificado visualmente** que o
jogo aparece atrás da webview transparente (sem acesso à tela) — o pipeline
está montado.

Windows/macOS: surface direto no handle da janela principal (webview
transparente compõe sobre a camada nativa). Código pronto, não verificado.

O player standalone (`cargo run -p video-surface --example play`) é o caminho
de vídeo puro-Wayland / sem shell.

## Estrutura sugerida

```
apps/desktop/src-tauri/src/
  main.rs
  video_surface.rs     -- cria e gerencia a surface nativa (wgpu)
  focus_manager.rs      -- implementa domain::focus::FocusManager
  game_loop.rs            -- orquestra retro_run + FrameSource + pause/resume
  commands.rs               -- comandos Tauri (#[tauri::command]) expostos ao frontend
```

## Pontos de atenção técnica

- Resize da janela precisa redimensionar a surface nativa em sincronia
  (sem delay de um frame) — trate o evento de resize do Tauri e propague
  pra surface antes do próximo frame ser desenhado.
- Ao pausar (`MenuFocused`), congele o último frame renderizado em vez de
  limpar a tela — evita salto visual feio atrás do menu.
- O comando Tauri que alterna foco deve ser o único ponto de entrada que
  aciona `FocusManager::toggle()` — não deixe o React decidir isso
  diretamente, só solicitar via `invoke`.

## Depende de

`02-core-loader-desktop.md` já funcional (pelo menos o caminho
software-only) — este passo não faz sentido sem um `FrameSource` real
produzindo frames.

## Critério de pronto

- [x] Janela abre com a webview React e, ao carregar um jogo, o vídeo do core
  aparece (surface nativa no Wayland, `<canvas>` no fallback) sem cobrir o menu
  de pausa.
- [x] Alternar foco via hotkey pausa/resume o core de forma perceptível
  (áudio para e volta, frame congela e descongela) — validado com SNES e N64.

Follow-up (não bloqueia): posição da subsurface no rect exato da área de jogo
em janela com CSD (hoje best-effort); verificar o caminho nativo em
Windows/macOS quando houver máquina; medir latência nativo vs. canvas.
