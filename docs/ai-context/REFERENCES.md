# Referências externas oficiais

**Regra do projeto (ver `00-visao-geral.md`): consultar SEMPRE a doc oficial
antes de escrever binding FFI, usar API de terceiro, ou depurar plataforma.**
Não trabalhar de memória.

## libretro

| O quê | Onde |
|---|---|
| Guia da API (env callbacks, HW render, save state, core options) | https://docs.libretro.com/ |
| **Header canônico** — `#define`, structs, enums (fonte da verdade) | https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro.h |
| **Header Vulkan HW render** (etapa 12 — `retro_hw_render_interface_vulkan` v5, negotiation v2) | https://raw.githubusercontent.com/libretro/libretro-common/master/include/libretro_vulkan.h |
| Buildbot de cores (catálogo, etapa 10) | https://buildbot.libretro.com/ |
| slang-shaders (etapa 04, downloader do backlog) | https://github.com/libretro/slang-shaders |

Já mordido: `RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER` escrito como `42` de
cabeça; o certo é `56` (commit `9d633d7`). `sys.rs` inteiro reconferido lá.

**BIOS/arquivos de sistema por core** (`domain::bios`, 2026-09-04) — nomes de
arquivo e MD5 vêm da página de cada core, não tem convenção única:

| Core | Página |
|---|---|
| Beetle PSX (psx) | https://docs.libretro.com/library/beetle_psx/ |
| Kronos (Saturn) | https://docs.libretro.com/library/kronos/ |
| Flycast (Dreamcast) | https://docs.libretro.com/library/flycast/ |

**Flycast — integração Vulkan libretro (etapa 12)**, conferir sempre na fonte,
não de memória (`flyinghead/flycast@master`):

| O quê | Onde |
|---|---|
| Negociação v1 (`VkCreateDevice`/`VkGetApplicationInfo`), `set_image`, uso de `get_sync_index`/`lock_queue` | `core/rend/vulkan/vk_context_lr.cpp` + `vk_context_lr.h` |
| `SET_HW_RENDER` Vulkan, `GET_HW_RENDER_INTERFACE`, ramo de `GET_PREFERRED_HW_RENDER` | `shell/libretro/libretro.cpp` (`set_vulkan_hw_render`, `retro_vk_context_reset`) |
| FBNeo (arcade) | https://docs.libretro.com/library/fbneo/ |

`libretro-thumbnails` (org do GitHub, não a doc) segue sendo a fonte pros
nomes de pasta do `thumbnails.libretro.com` — `gh api
orgs/libretro-thumbnails/repos` lista os repos = pastas reais (`_` no nome do
repo = espaço na pasta).

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

**Veredito 2026-09-03:** com `transparent: true` + NVIDIA + WebKitGTK não há
combinação que dê webview transparente E estável — bug upstream sem solução.
**Superado 2026-09-04**: abordagem sem janela transparente (`wl_subsurface`
`place_above` a webview OPACA) — sem transparência, sem o bug. É o padrão
agora (ver `03`).

## wgpu / Rust GPU

| O quê | Onde |
|---|---|
| wgpu API | https://docs.rs/wgpu/latest/wgpu/ |
| wgpu-hal (interop, `texture_from_dmabuf_fd`, `as_hal`, `device_from_raw`) | https://docs.rs/wgpu-hal/latest/wgpu_hal/ |
| Fonte dos crates (sempre disponível) | `~/.cargo/registry/src/index.crates.io-*/` |

## OpenGL (etapa 02 passo 4 — HW render GL por core: N64, PSX-hw, Saturn, DS, Dreamcast)

**Consultar a doc oficial antes de mexer no contexto GL offscreen, nos FBOs do
core, ou na interop dma_buf — não de memória.**

| O quê | Onde |
|---|---|
| Índice da documentação OpenGL (Khronos) | https://www.opengl.org/Documentation/Documentation.html |
| **Registry** — índice de specs core + GLSL + TODAS as extensões + `gl.xml` (enums canônicos) | https://registry.khronos.org/OpenGL/index_gl.php |
| Wiki (FBO, contextos, sync objects, `GL_ARB_*`) | https://www.khronos.org/opengl/wiki/ |
| Referência de funções (`glTexImage2D`, `glFramebufferTexture`, …) | https://registry.khronos.org/OpenGL-Refpages/gl4/ |
| EGL (contexto/superfície offscreen, `EGL_KHR_surfaceless_context`) | https://registry.khronos.org/EGL/ |

O core GL renderiza num FBO que o frontend dá; por padrão o resultado sai por
`glReadPixels` (readback, caminho estável). Com `REEMU_GL_INTEROP=1` sai por
interop dma_buf zero-cópia (`EGL_EXT_image_dma_buf_import`, ver seção
Wayland/EGL) — opt-in, ainda não validado o bastante em hardware pra ser
padrão (achado 2026-09-12: tela preta com `parallel_n64_libretro` quando
ligado).

**Specs de extensão que valem pro `gl_context.rs` (conferir o enum/semântica na
fonte, não de memória):**

| Extensão | Pra quê no ReEmu |
|---|---|
| `GL_OES_EGL_image` | `glEGLImageTargetTexture2DOES` — restrições de target/formato da textura respaldada por `EGLImage` |
| `EGL_EXT_image_dma_buf_import` / `_modifiers` | enums `EGL_LINUX_DMA_BUF_EXT`, `EGL_DMA_BUF_PLANE0_*` (hoje hard-coded em `gl_context.rs`) |
| `EGL_MESA_platform_surfaceless` | `PLATFORM_SURFACELESS_MESA = 0x31DD` |
| `EGL_KHR_fence_sync` + `EGL_ANDROID_native_fence_sync` **ou** `GL_EXT_semaphore_fd` | handoff GL→Vulkan SEM `glFinish` (o "sync fino" pendente, ver `12`) |
| `GL_KHR_debug` | `glDebugMessageCallback` atrás de `REEMU_GL_DEBUG` (diagnóstico de "tela preta" em core GL) |

`gl.xml` é a fonte dos valores de enum; o `glow` em uso já é gerado dele.

## Vulkan (etapa 12 — HW render por-core)

**Consultar a spec/guia oficial antes de qualquer código Vulkan — sync e
external memory são fáceis de errar.**

| O quê | Onde |
|---|---|
| Portal / índice da documentação | https://www.vulkan.org/ · https://docs.vulkan.org/ |
| Spec (sincronização, barriers, external memory) | https://docs.vulkan.org/spec/latest/chapters/synchronization.html |
| Guia (Vulkan Guide — sync, memory, wsi) | https://docs.vulkan.org/guide/latest/ |
| **Synchronization Examples** (padrões `VkSubmitInfo`/semáforo/barrier prontos) | https://github.com/KhronosGroup/Vulkan-Docs/wiki/Synchronization-Examples |
| **Vulkan Samples** (`timeline_semaphore`, `synchronization_2`, `hpp_*`) | https://github.com/KhronosGroup/Vulkan-Samples |
| `ash` (binding em uso — fonte vendorizada é a verdade) | https://docs.rs/ash/0.38.0/ash/ · `~/.cargo/registry/src/index.crates.io-*/ash-0.38.0*/` |

**Ferramentas do LunarG SDK que valem pro que falta na etapa 12:**

| Ferramenta | Pra quê no ReEmu |
|---|---|
| **Synchronization validation** (`VK_VALIDATION_FEATURE_ENABLE_SYNCHRONIZATION_VALIDATION_EXT`) | pega a classe de bug que sobrou: `VkQueue` submetida de 2 threads, semáforo faltando entre o submit do core e o do wgpu. Ligar via `vk_layer_settings.txt` (aplica mesmo no device ADOTADO, que não passa pelo nosso `vk_context.rs`). |
| `vkconfig` (GUI) | liga/desliga validação + sync-val sem env var, por app |
| `gfxreconstruct` | captura 1 frame do Beetle e replay offline — debugar o scanout `A1R5G5B5` sem o jogo rodando |
| `synchronization2` (`vkQueueSubmit2` + `VkSemaphoreSubmitInfo`) | o `submit_vulkan_cmds` usa `VkSubmitInfo` legado + fence binário; o wgpu-hal 30 já usa timeline por dentro — migrar alinha os dois |

`vk_layer_settings.txt` na raiz do repo liga sync-val: rodar o caminho Beetle
com `VK_LAYER_SETTINGS_PATH=$PWD VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`.

Barrier do `set_image` (produtor color-attachment → consumidor sampler, mesma
queue, sem semáforo): `src COLOR_ATTACHMENT_OUTPUT / COLOR_ATTACHMENT_WRITE`
→ `dst FRAGMENT_SHADER / SHADER_READ`, layout `SHADER_READ_ONLY_OPTIMAL` →
`SHADER_READ_ONLY_OPTIMAL` (flycast já entrega transicionada). Barrier
explícito é **obrigatório** — ordem de submissão numa queue não basta.

**Pra o handoff GL→Vulkan sem CPU-wait** (item pendente, ver `02` §OpenGL):
consumidor importa o fence-fd como `VkSemaphore` com `VK_KHR_external_semaphore_fd`
(`vkImportSemaphoreFdKHR`, `VK_EXTERNAL_SEMAPHORE_HANDLE_TYPE_SYNC_FD_BIT`) e
passa em `wgpu_hal::vulkan::Queue::add_wait_semaphore`. O wgpu-hal 30 **não**
habilita `VK_KHR_external_semaphore_fd` sozinho — precisa criar o device pelo
caminho manual de `wgpu-hal` (como o `from_adopted_vulkan` já faz).

## Design (frontend)

| O quê | Onde |
|---|---|
| Fluent 2 | https://fluent2.microsoft.design/ |
| Fluent UI React | https://react.fluentui.dev/ |
| Griffel (`makeStyles` + `tokens`) | https://griffel.js.org/ |

## Wayland / EGL / GBM / DRM (vídeo nativo — padrão desde 2026-09-04)

| O quê | Onde |
|---|---|
| `EGL_EXT_image_dma_buf_import` | https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_image_dma_buf_import.txt |
| Protocolo Wayland core (`wl_subsurface`, `wl_subcompositor`) | https://wayland.freedesktop.org/docs/html/apa.html |
| DRM format modifiers / fourcc | `<drm_fourcc.h>` do libdrm |

## IPC entre processos (isolamento de core, 2026-09-04)

`crates/core-ipc` — o pai (`emu-session`) fala com o processo filho
(`reemu-core-host`) por socket Unix `SOCK_SEQPACKET` (`socketpair`) +
`SCM_RIGHTS` (fd de memfd/dma_buf) + `memfd_create`/`mmap` pro anel de frame.
Sem crate de binding pronto pra isso em alto nível — usamos `rustix`
diretamente (o `libc`/FFI cru ficaria arriscado de acertar na primeira,
mensagens de controle têm alinhamento/`CMSG_*` chatos de montar à mão).

| O quê | Onde |
|---|---|
| `rustix` (docs.rs, mas a fonte vendorizada é a fonte da verdade — ver abaixo) | https://docs.rs/rustix/latest/rustix/ |
| **Fonte exata da versão em uso** (`sendmsg`/`recvmsg`/`SendAncillaryBuffer`/`memfd_create`/`mmap`/`fcntl_setfd` — conferido ali, não de memória) | `~/.cargo/registry/src/index.crates.io-*/rustix-<versão>/src/{net/send_recv/msg.rs,net/socketpair.rs,fs/memfd_create.rs,mm/mmap.rs,io/fcntl.rs}` |
| `socketpair(2)` | https://man7.org/linux/man-pages/man2/socketpair.2.html |
| `unix(7)` (`SCM_RIGHTS`, passagem de fd) | https://man7.org/linux/man-pages/man7/unix.7.html |
| `memfd_create(2)` | https://man7.org/linux/man-pages/man2/memfd_create.2.html |
| `bincode` 2 (serialização do protocolo) | https://docs.rs/bincode/latest/bincode/ |
