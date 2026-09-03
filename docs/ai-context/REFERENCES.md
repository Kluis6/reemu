# Referências externas oficiais

**Regra do projeto (ver `00-visao-geral.md`): consultar SEMPRE a doc oficial
antes de escrever binding FFI, usar API de terceiro, ou depurar plataforma.**
Não trabalhar de memória.

## libretro

| O quê | Onde |
|---|---|
| Guia da API (env callbacks, HW render, save state, core options) | https://docs.libretro.com/ |
| **Header canônico** — `#define`, structs, enums (fonte da verdade) | https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h |
| Buildbot de cores (catálogo, etapa 10) | https://buildbot.libretro.com/ |
| slang-shaders (etapa 04, downloader do backlog) | https://github.com/libretro/slang-shaders |

Já mordido: `RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER` escrito como `42` de
cabeça; o certo é `56` (commit `9d633d7`). `sys.rs` inteiro reconferido lá.

## Tauri v2

| O quê | Onde |
|---|---|
| Docs gerais | https://v2.tauri.app/ |
| **Linux graphics / NVIDIA / WebKitGTK** (env vars, Error 71) | https://v2.tauri.app/develop/debug/linux-graphics/ |
| Customização de janela (transparent, decorations) | https://v2.tauri.app/learn/window-customization/ |
| Configuração (`tauri.conf.json`) | https://v2.tauri.app/develop/configuration-files/ |
| Reference da API Rust | https://docs.rs/tauri/latest/tauri/ |
| Issue aberta: transparent + NVIDIA (sem fix) | https://github.com/tauri-apps/tauri/issues/14924 |

**Env vars Linux (da doc oficial), na ordem de tentativa:**
1. `nvidia_drm.modeset=1` (parâmetro de kernel, drivers NVIDIA < 545)
2. `__NV_DISABLE_EXPLICIT_SYNC=1` — costuma resolver o Error 71 sem custo de
   perf; **mas causa ghosting** (frame anterior fica preso — foi o que o
   usuário viu no vídeo nativo).
3. `WEBKIT_DISABLE_DMABUF_RENDERER=1` — resolve o erro de framebuffer DMABUF;
   custa o caminho de render rápido. **Tira a transparência (cantos pretos).**
4. `WEBKIT_DISABLE_COMPOSITING_MODE=1` — último recurso pro crash silencioso no
   resize; desliga o compositing acelerado inteiro.

**Veredito ReEmu (2026-09-03):** com `transparent: true` + NVIDIA + WebKitGTK
não há combinação que dê webview transparente E estável. Vídeo nativo
arquivado, `<canvas>` é o padrão (ver `03`). É bug upstream sem solução.

## wgpu / Rust GPU

| O quê | Onde |
|---|---|
| wgpu API | https://docs.rs/wgpu/latest/wgpu/ |
| wgpu-hal (interop, `texture_from_dmabuf_fd`, `as_hal`) | https://docs.rs/wgpu-hal/latest/wgpu_hal/ |
| Fonte dos crates (sempre disponível) | `~/.cargo/registry/src/index.crates.io-*/` |

## Design (frontend)

| O quê | Onde |
|---|---|
| Fluent 2 | https://fluent2.microsoft.design/ |
| Fluent UI React | https://react.fluentui.dev/ |
| Griffel (`makeStyles` + `tokens`) | https://griffel.js.org/ |

## Wayland / EGL / GBM / DRM (vídeo nativo — arquivado mas o código existe)

| O quê | Onde |
|---|---|
| `EGL_EXT_image_dma_buf_import` | https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_image_dma_buf_import.txt |
| Protocolo Wayland core (`wl_subsurface`, `wl_subcompositor`) | https://wayland.freedesktop.org/docs/html/apa.html |
| DRM format modifiers / fourcc | `<drm_fourcc.h>` do libdrm |
