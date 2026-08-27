# 03 — Tauri Desktop Shell + Surface Nativa

## Objetivo desta etapa

Montar o app Tauri v2 que hospeda a webview React (menu Fluent) e a
surface nativa de renderização do jogo (fora da WebView), com o overlay
de menu sempre sobreposto.

## Decisões relevantes (não renegociar sem discutir)

- **O jogo renderiza fora da WebView**, numa surface/child window nativa,
  criada via `wgpu::Surface` ou binding GL direto sobre o `WindowHandle`
  do Tauri — decisão tomada especificamente por causa de input lag.
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

## Estado atual (2026-08-27 — `in-progress`)

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

- Janela abre com a webview React e, ao carregar um jogo, a surface
  nativa aparece sobreposta corretamente, sem sobrepor incorretamente o
  menu
- Alternar foco via hotkey pausa/resume o core de forma perceptível
  (áudio para e volta, frame congela e descongela)
