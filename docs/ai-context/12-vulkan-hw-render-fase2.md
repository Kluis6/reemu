# 12 — HW Render Vulkan Por-Core

**Status: EM ANDAMENTO (iniciado 2026-09-06).** Decisão do usuário: rodar o
core Vulkan **no processo pai** (in-process, junto do wgpu), zero-cópia.

**Alvos, em ordem** (decisão 2026-09-06):

1. **`libretro-samples/video/vulkan/vk_rendering`** — core de teste oficial
   (triângulo girando). Negociação `{ get_application_info, NULL }` → **o
   frontend cria o device** (= arquitetura escolhida). Sem BIOS, sem ROM
   (`SET_SUPPORT_NO_GAME`). Usa `set_command_buffers` (o core NÃO submete;
   entrega o cmd buffer pro frontend), `wait_sync_index`, `get_sync_index`.
   Imagem `COLOR_ATTACHMENT|SAMPLED|TRANSFER_SRC`, `R8G8B8A8_UNORM`, termina
   em `SHADER_READ_ONLY_OPTIMAL`. É o bring-up da fase A+B.
2. **Beetle PSX HW** (`mednafen_psx_hw` / parallel-psx) — 1º emulador real.
   `create_device` **coopera** (habilita `required_device_extensions`, usa
   `required_features`), imagem do scanout tem `TRANSFER_SRC`, implementa
   `wait_sync_index`, código endurecido pra frontends Vulkan não-RetroArch.
3. **flycast** — por último (fase C). `VkCreateDevice` v1 ignora
   `required_*`; imagem sem `TRANSFER_SRC`; Vulkan opt-in.

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

**Não chamar o `create_device` do flycast.** O frontend cria a
`VkInstance`/`VkDevice` (via `ash`) com TODAS as extensões/features que
precisa, e passa esse device pros dois lados:

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

Modelo do `vk_rendering` / Beetle PSX (core NÃO submete — usa
`set_command_buffers`):

1. Antes do `retro_run`: `current_sync_index = (current_sync_index + 1) % N`
   (N derivado do mask que devolvemos; começar com N=3).
2. `retro_run()` → core:
   - `wait_sync_index(handle)` — frontend faz CPU-wait no `fence[idx]` do
     ciclo anterior desse índice (no-op nos N primeiros frames), depois
     `vkResetFences`.
   - `get_sync_index(handle)` → devolvemos `current_sync_index`.
   - grava o cmd buffer (com o barrier de release pra `SHADER_READ_ONLY`
     dentro dele), `set_image(&image)`, `set_command_buffers(n, cmds)` —
     guardamos ambos no bridge.
   - `video_refresh(RETRO_HW_FRAME_BUFFER_VALID, w, h, 0)` → marca
     `hw_frame`/`had_new_frame` (igual GL).
3. Depois do `retro_run` (em `DesktopCore::next_hw_frame`): frontend submete
   os cmd buffers guardados na `VkQueue`, com `fence[current_sync_index]`.
   (flycast, fase C: o core já submeteu ele mesmo via `lock_queue`; aí o
   passo 3 vira só um barrier-only cmd buffer `COLOR_ATTACHMENT_WRITE` →
   `SHADER_READ`.)
4. `create_texture_from_hal` na `image.create_info.image` (1ª vez por índice;
   cacheia por sync index) → entrega como `Frame` pro compositor.
5. Sync fase B: `vkWaitForFences(fence[idx])` antes de amostrar (conservador,
   correto). Fase C: deixar a ordem de submissão + barrier na mesma
   `VkQueue` do wgpu resolver, sem CPU-wait; `set_signal_semaphore`.

## Fatiamento

- **Fase A** — `core-loader-desktop`: `vk_context.rs` (feito, `03eb89d`) +
  `vk_frame.rs` (bridge: os 8 callbacks + fence ring por sync index) +
  construção de `retro_hw_render_interface_vulkan` + wiring no `ffi_state.rs`
  (`GET_PREFERRED_HW_RENDER` opt-in via `REEMU_HW=vulkan`, aceitar
  `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`, responder
  `GET_HW_RENDER_INTERFACE`) + `loader.rs::setup_vk_context`. Critério:
  teste headless carrega a `.so` do `vk_rendering`, roda N frames,
  `retro_run` sem crash, `set_image`/`set_command_buffers` chegam.
- **Fase B** — backend in-process no `emu-session` (Vulkan não passa pelo
  `ChildProc`); wgpu do `gpu.rs` reconstruído sobre o `ash::Device`
  compartilhado quando um core Vulkan carrega; `FrameOrigin` novo
  (`HardwareVulkanImage`) + `create_texture_from_hal` no `gpu.rs`; sync
  conservador. Critério: triângulo do `vk_rendering` na tela, orientação
  certa; depois Beetle PSX HW.
- **Fase C** — sync fino (sem CPU-wait, barriers mínimos), validação sob
  carga (troca rápida de cena, resize, save/load state), flycast como 3º
  alvo, `provoking_vertex`/OIT reavaliados.

## Validação (ligar SEMPRE durante o desenvolvimento)

`REEMU_VK_VALIDATION=1` liga `VK_LAYER_KHRONOS_validation` +
`VK_EXT_debug_utils`; as mensagens saem pelo `log` (`[vulkan] ...`). Se o
layer não estiver instalado, avisa e segue sem validar:

```sh
sudo apt install vulkan-validationlayers   # Debian/Ubuntu
```

Erro de sincronização em HW render é silencioso sem o layer — é exatamente o
tipo de bug que esta etapa arrisca. Rode o teste com ele:

```sh
scripts/build-vk-test-core.sh
REEMU_VK_VALIDATION=1 RUST_LOG=debug \
  cargo test -p core-loader-desktop --test vk_hw_render -- --ignored --nocapture
# sem GPU: VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json (lavapipe)
```

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
