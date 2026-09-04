# 02 — Core Loader Desktop (Caminho GL)

## Objetivo desta etapa

Implementar `domain::core_loader::CoreLoader` e `domain::frame_source::FrameSource`
pra desktop, cobrindo primeiro o caminho software-only (core entrega buffer
de pixels crus) e depois o caminho GL de hardware render
(`retro_hw_render_callback`). **Vulkan por-core fica pra depois** — ver
`12-vulkan-hw-render-fase2.md`.

## Estado atual (2026-09-02 — `done`)

Crate `crates/core-loader-desktop`. Passos 1-4 **feitos**:
- `src/sys.rs` — FFI libretro (conferido contra `libretro.h`).
- `RawCore` (libloading), `ffi_state` (estado global + callbacks — libretro é
  **um core por processo**), `DesktopCore` (impl `FrameSource` +
  `domain::core_loader::LoadedCore`), `DesktopCoreLoader` (impl `CoreLoader`;
  `load_core()` devolve o tipo concreto com os extras).
- Caminho **software-only** validado ponta a ponta com um core-fake em C
  (`fixtures/testcore.c`, compilado pelo `build.rs`).
- **HW render GL** (`src/gl_context.rs`, `src/dmabuf.rs`): contexto EGL
  offscreen (`DynamicInstance`, surfaceless Mesa → fallback pbuffer), FBO
  RGBA8 + `DEPTH24_STENCIL8` opcional. `SET_HW_RENDER` escreve
  `get_current_framebuffer`/`get_proc_address` de volta no struct do core;
  `setup_gl_context` publica o FBO e roda `context_reset`; `Drop` chama
  `context_destroy`. **Validado com Super Mario 64** (2026-09-02, NVIDIA).
  - Frame sai por **readback** (`glReadPixels` → `SoftwareRawBuffer::Rgba8888`,
    default) ou **interop zero-cópia** dma_buf (GBM aloca, EGL/wgpu importam;
    `REEMU_GL_INTEROP=1`, ainda não validado).
- ROM em `.zip` extraída pra arquivo temporário no load (`src/archive.rs`).
- Save state: só o ponto de extensão (`request_save_state`/`poll_save_state`).

**Vulkan por-core**: segue recusado com `CoreLoadError::HwRenderUnsupported`
(etapa 12).

## Isolamento em processo filho (2026-09-04)

`core-loader-desktop` (`RawCore`/`DesktopCore`/`ffi_state`/`gl_context`/
`dmabuf`/`loader`) não muda — mas quem o usa agora é **exclusivamente** o novo
binário `crates/core-host-desktop` (`reemu-core-host`), não mais
`emu-session` diretamente. Motivo: alguns cores (parallel_n64) não são
re-entrantes — o "um core por processo" do parágrafo acima virou "um core por
**vida do processo**": um 2º `retro_init` no MESMO processo, depois de um
core não re-entrante já ter rodado ali, derruba o processo sem erro visível
(estado global em C sujo sobrevivendo ao `dlclose`).

`emu-session` agora mata o processo filho e sobe um **novo**
`reemu-core-host` a cada `load` (nunca reusa, mesmo pro mesmo core) — memória
sempre parte limpa, a re-entrância do core deixa de importar. IPC sobre
`crates/core-ipc` (socket Unix `SOCK_SEQPACKET` via `socketpair`, sem
bind/endereço; fds de dma_buf/memfd passados por `SCM_RIGHTS`). Ver a memória
`n64-reload-crash` e o cabeçalho de `emu-session/src/session.rs`.

O caminho GL/interop dma_buf não muda nada do lado de dentro — `gpu.rs`
continua importando o mesmo jeito (`GpuTextureHandle::take_plane()`), só que
o fd agora atravessa um `sendmsg`/`recvmsg` em vez de ficar no mesmo processo.

Mudanças no `domain`: `frame_source::SoftwarePixelFormat` (+ campo `format` no
`SoftwareRawBuffer`); `core_loader::SystemAvInfo` e o trait `LoadedCore`
(substituiu o marker `LoadedCoreHandle`); `CoreLoadError::HwRenderUnsupported`.

## Ordem de implementação (não pule etapas)

1. **Carregamento básico**: `libloading` pra abrir o `.so`/`.dll`/`.dylib`
   e resolver os símbolos `retro_init`, `retro_load_game`, `retro_run`,
   `retro_deinit`, `retro_get_system_av_info`.
2. **Core software-only primeiro**: valide o pipeline inteiro (carregar →
   rodar → `retro_video_refresh` recebendo buffer cru → virar `FrameSource`)
   com um core simples, tipo um core de NES conhecido por ser bem
   comportado. Não tente um core hardware-accelerated ainda.
3. **Detecção de HW render**: implemente o handler de
   `RETRO_ENVIRONMENT_SET_HW_RENDER` — quando o core chamar isso, capture
   os requisitos (`context_type`, `version_major/minor`, profile) e
   preencha `CoreRenderRequirements`, persistindo via
   `installed_cores_repo` (etapa 01) no primeiro load.
4. **Negociação GL real**: só depois do passo 3 funcionando, implemente a
   criação de contexto GL compatível e os callbacks
   (`get_current_framebuffer`, `get_proc_address`, `context_reset`,
   `context_destroy`).

## Decisões relevantes

- Detecção de `render_backend`/`gl_version_min`/etc é sempre em runtime,
  no primeiro load — nunca peça pro usuário preencher isso manualmente.
- Save state (`retro_serialize`) só pode ser chamado entre frames, nunca
  no meio de um `retro_run` — isso importa aqui porque é este crate que
  vai expor o método `save_state_pending: Arc<AtomicBool>` (ou similar)
  que o loop principal checa a cada frame. A implementação completa do
  save state em si é outra etapa (`08-save-states.md`) — aqui você só
  precisa deixar o ponto de extensão pronto.
- `LoadedCoreHandle` é opaco no domínio — a struct concreta
  (`DesktopCoreHandle` ou nome equivalente) fica só neste crate, contendo
  o `libloading::Library` e os ponteiros de função resolvidos.

## Referências de protocolo

Use `https://docs.libretro.com/` como fonte pra assinatura exata das
funções `retro_*` e do struct `retro_hw_render_callback` — não invente
assinatura de função por suposição.

## Critério de pronto — ✅ atingido

- ✅ Um core software-only carrega, roda, e produz frames consumíveis via
  `FrameSource::next_frame()` sem panics
- ✅ Um core GL (N64 — `mupen64plus_next` / equivalente) completa a
  negociação de contexto e renderiza (Super Mario 64, 2026-09-02)

## Backlog deste crate

- Interop zero-cópia dma_buf: validar `REEMU_GL_INTEROP=1` em hardware, trocar
  `glFinish` grosso por semáforo cross-API, depois tirar o gate.
- `.7z` no `archive.rs` (hoje só `.zip`).
- GLES-only: `eglBindAPI(EGL_OPENGL_ES_API)` já implementado, sem core pra testar.
