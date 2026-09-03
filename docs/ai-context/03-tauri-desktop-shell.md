# 03 — Tauri Desktop Shell + Surface Nativa

## Objetivo desta etapa

Montar o app Tauri v2 que hospeda a webview React (menu Fluent) e a
surface nativa de renderização do jogo (fora da WebView), com o overlay
de menu sempre sobreposto.

## Decisões relevantes (não renegociar sem discutir)

- ~~**O jogo renderiza fora da WebView**, numa surface/child window nativa~~
  — **DESVIO ACEITO (2026-08-30), reconfirmado (2026-09-03)**: o vídeo do
  desktop é um **`<canvas>` dentro da webview** alimentado por `poll_frame`
  (RGBA8 por IPC). Três tentativas de surface nativa, todas barradas pelo
  WebKitGTG nesse combo NVIDIA+Wayland:
  1. `wgpu::Surface` na `wl_surface` do GTK → `Gdk Error 71`.
  2. child window X11 (`REEMU_X11_VIDEO=1`) → o backend X11 faz a webview não
     pintar.
  3. `wl_subsurface` `place_below` + webview transparente (`REEMU_NATIVE_VIDEO=1`,
     `src/video.rs`) → **o lado Rust funciona** (subsurface compõe, chain roda,
     present ok — provado com blit de teste magenta), mas o WebKitGTG **não
     entrega webview transparente**: compositing off → transparente vira preto;
     on → opaca; DMA-BUF renderer on → webview em branco.
  O código da opção 3 fica gated (deve funcionar em Mesa / WebKitGTG com alpha
  ok). **Plano B** se voltar ao assunto: `GtkGLArea` num `GtkOverlay` — a GTK
  compõe os widgets internamente, sem depender da transparência da `wl_surface`
  (reusa o interop dma_buf da etapa 02). ~1 semana, risco no reparent do widget
  da webview que o Tauri gerencia.
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

## Estado atual (2026-08-30 — `done`, com o desvio do canvas)

Fechado. O app abre com a webview React, carrega e roda jogos (SNES validado
pelo usuário), alterna foco pausando/resumindo o core (áudio + frame congelam),
e o vídeo desenha num `<canvas>`:

- **`poll_frame`** (`commands.rs`) — `session.take_latest_frame()` → caminho
  GPU (etapa 04) ou CPU (`to_rgba8`) → `[w u32][h u32][rgba8…]`. Corpo vazio
  quando não há frame novo (o `take` consome).
- **`PlayScreen`** — dois loops desacoplados: um `async` de **fetch** (IPC,
  guarda o último frame numa ref; 3ms rodando / 120ms pausado) e um **rAF de
  paint** (sincronizado com o vblank, `putImageData` do último frame). Pausa =
  o canvas segura o último frame (freeze). Resize = CSS (`height:100%`,
  `aspect-ratio` do core), sem passo nativo.
- `FocusController` pausa/resume a `EmuSession` na transição de foco; o único
  gatilho é o comando `toggle_focus` (via hotkey/gamepad/menu), nunca o React.
- Shutdown: `RunEvent::ExitRequested` → `session.unload()` (flush da `.srm`).

Caminho nativo (histórico, não é o padrão):

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
  aparece (via `<canvas>` — ver desvio) sem cobrir o menu de pausa.
- [x] Alternar foco via hotkey pausa/resume o core de forma perceptível
  (áudio para e volta, frame congela e descongela) — validado com SNES.

Follow-up (não bloqueia): medir a latência do canvas vs. nativo; verificar o
caminho nativo em Windows/macOS quando houver máquina; GL embutido.
