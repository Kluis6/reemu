# 12 — HW Render Vulkan Por-Core

**Status: EM ANDAMENTO (iniciado 2026-09-06).** Core-alvo: **flycast**
(Dreamcast). Decisão do usuário: rodar o core Vulkan **no processo pai**
(in-process, junto do wgpu), zero-cópia.

## Por que isso é o trecho mais arriscado do projeto inteiro

O core cria/usa `VkImage` próprias e gerencia parte da sincronização — o
frontend consome essas imagens sem introduzir hazard (barriers corretos,
nunca ler uma imagem que o core ainda escreve). Historicamente a parte mais
bugada de frontends libretro. Não subestime a validação.

## O que a fonte do flycast diz (conferido, não de memória)

`core/rend/vulkan/vk_context_lr.cpp` + `shell/libretro/libretro.cpp` do
`flyinghead/flycast@master`:

- **Só usa Vulkan se `GET_PREFERRED_HW_RENDER` devolver `RETRO_HW_CONTEXT_VULKAN`**
  (ou no fallback quando o preferido é `DUMMY`). Hoje devolvemos
  `OPENGL_CORE` → flycast roda em GL. Vulkan é **opt-in** nosso.
- Registra `retro_hw_render_context_negotiation_interface_vulkan` **v1**:
  `{ type, version, VkGetApplicationInfo, VkCreateDevice, nullptr }`.
  Não implementa v2 (`create_instance`/`create_device2`).
- `VkCreateDevice` (v1) do flycast **ignora** `required_device_extensions`,
  `required_device_layers` E `required_features` — cria o device só com o
  que ELE quer (swapchain + opcionais properties2/provoking_vertex/BDA).
  ⇒ Se deixarmos o flycast criar o device, **não temos como habilitar as
  extensões que o wgpu (ou o dma_buf import) precisa.**
- `VulkanContext::init` só lê `retro_render_if->{instance,gpu,device,queue,
  queue_index}` — **não exige ter chamado `create_device` ele mesmo.** Os
  checks de feature (`fragmentStoresAndAtomics`, `provokingVertex`, …) são
  `static bool` default `false`; se o frontend criar o device, ficam `false`
  e o flycast usa os fallbacks (perde OIT per-pixel e provoking-vertex nativo
  — perda de qualidade, não blocker).
- `set_image(handle, &retro_image, 0, nullptr, VK_QUEUE_FAMILY_IGNORED)` —
  **0 semáforos, sem transferência de queue family**. `retro_image` em
  `SHADER_READ_ONLY_OPTIMAL`, formato `R8G8B8A8_UNORM`, `create_info.image`
  carrega a `VkImage`. Imagem criada com usage `COLOR_ATTACHMENT | SAMPLED`
  (**sem `TRANSFER_SRC`** — viola a spec; não dá pra `vkCmdBlitImage` dela,
  só amostrar em shader).
- **Usa ativamente** (via `retro_render_if`):
  - `get_sync_index_mask` → dimensiona o nº de frames em voo do flycast.
  - `get_sync_index` → índice do slot em voo do frame atual (indexa command
    pool / framebuffers do flycast). Frontend avança antes de cada `retro_run`.
  - `lock_queue`/`unlock_queue` em volta do `queue.submit` do próprio flycast
    — **flycast submete os command buffers dele direto na nossa `VkQueue`.**
  - Não usa `set_command_buffers`, `wait_sync_index`, `set_signal_semaphore`
    (podem ser no-op; ainda vale implementar `wait_sync_index`).

## Arquitetura escolhida — device criado pelo frontend, compartilhado

**Não chamar o `create_device` do flycast.** O frontend cria a `VkInstance`
+ `VkDevice` (via `ash`) com TODAS as extensões/features que precisa, e passa
esse device pros dois lados:

```text
        ash::Instance + ash::Device (1 só, criado por nós)
              /                              \
   wgpu (Adapter::device_from_raw)      flycast (retro_hw_render_interface_vulkan)
   compositor / shader chain            renderiza, set_image(VkImage)
              \                              /
         create_texture_from_hal(VkImage do flycast)  ← zero-cópia
```

- **1 device, 1 processo** ⇒ a `VkImage` que o flycast entrega no `set_image`
  vira `wgpu::Texture` direto (`Device::create_texture_from_hal::<Vulkan>`),
  sem cópia, sem dma_buf, sem IPC.
- Extensões do device = `Adapter::required_device_extensions(features)` do
  wgpu-hal + features de `Adapter::physical_device_features()`. flycast não
  exige nada além disso (se adapta).
- `get_instance_proc_addr` / `get_device_proc_addr`: os do loader do `ash`.

### Trade-off aceito

flycast **perde o isolamento anti-crash do processo-filho** (`core-ipc` /
`reemu-core-host`) — só os cores Vulkan. Cores software/GL seguem no filho,
inalterados. (A alternativa — manter no filho + exportar dma_buf — exige um
device com `VK_EXT_external_memory_dma_buf`, que o `create_device` do flycast
não habilita, então precisaria também do device criado por nós, e aí some a
vantagem de reusar o `create_device` do core. Com a cópia GPU inevitável de
qualquer jeito, in-process zero-cópia é o melhor custo/benefício.)

## Sequência por frame (thread do core, processo pai)

1. Antes do `retro_run`: `current_sync_index = frame_count % N`
   (N = `popcount`-ish do mask que devolvemos; começar com N=2 ou 3).
2. `retro_run()` → flycast renderiza, `queue.submit` (dentro de
   `lock_queue`/`unlock_queue`), `set_image(retro_vulkan_image)`.
3. Frontend: barrier na imagem do flycast
   (`srcStage=COLOR_ATTACHMENT_OUTPUT`, `srcAccess=COLOR_ATTACHMENT_WRITE` →
   `dstStage=FRAGMENT_SHADER`, `dstAccess=SHADER_READ`) — a spec do
   `set_image` manda exatamente isso quando não há semáforo. Mesma queue ⇒
   ordem de submissão garante o resto.
4. `create_texture_from_hal` (1ª vez por slot; cacheia por
   `get_sync_index`) → entrega como `Frame` pro compositor.
5. Sync fase B: `vkQueueWaitIdle` / fence antes de amostrar (conservador,
   correto). Fase C: fence ring por sync index + `set_signal_semaphore`.

## Fatiamento

- **Fase A** — `vk_context.rs` no `core-loader-desktop`: `ash` instance +
  device (extensões do wgpu), negociação (`GET_PREFERRED_HW_RENDER` opt-in,
  aceitar `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`, responder
  `GET_HW_RENDER_INTERFACE` com a struct preenchida), callbacks
  (`set_image`/`get_sync_index`/`get_sync_index_mask`/`lock_queue`/
  `unlock_queue`/`wait_sync_index`), `context_reset`. Critério: flycast
  carrega, negocia, roda `retro_run` sem crash (frame ainda não exibido).
- **Fase B** — backend in-process no `emu-session` (Vulkan não passa pelo
  `ChildProc`); wgpu do `gpu.rs` reconstruído sobre o `ash::Device`
  compartilhado quando um core Vulkan carrega; `FrameOrigin` novo
  (`HardwareVulkanImage`) + `create_texture_from_hal` no `gpu.rs`; sync
  conservador. Critério: imagem do flycast na tela, orientação certa,
  parâmetros de shader ainda funcionam.
- **Fase C** — sync fino (fence ring por sync index, barriers mínimos),
  validação sob carga (troca rápida de cena, resize, save/load state),
  `provoking_vertex`/OIT reavaliados.

## Cores que dependem disso (enquanto não fecha)

Aparecem no catálogo normalmente, marcados **"requer Vulkan HW render"** na
UI via `render_backend` detectado em runtime no 1º load. Não excluir da
listagem.

## Critério de pronto

flycast (render Vulkan) carrega, negocia, e produz frames sem corrupção/
flickering de hazard, inclusive sob carga.

## Referências

`docs/ai-context/REFERENCES.md` §libretro (headers `libretro.h` +
`libretro_vulkan.h`) e §flycast. FFI base já in-tree:
`crates/core-loader-desktop/src/vk_sys.rs` (commit `78aa2bf`).
