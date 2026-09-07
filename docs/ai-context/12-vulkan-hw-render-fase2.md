# 12 — HW Render Vulkan Por-Core

**Status: EM ANDAMENTO (iniciado 2026-09-06).** Fase A + B (B1..B3b) feitas e
validadas end-to-end com o core de teste `vk_rendering` num RTX 3060
(2026-09-07). Falta o 1º emulador real (Beetle PSX HW) e a fase C (sync fino).
Decisão do usuário: rodar o core Vulkan **no processo pai** (in-process, junto
do wgpu), zero-cópia.

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
- **Fase B** — o core Vulkan usa o MESMO `VkDevice` do wgpu.
  - **B1 (feito)** — `domain::VulkanSharedDevice` (handles crus) +
    `VkContext::adopt` (não destrói nada; o compositor é o dono).
  - **B2 (feito)** — `FrameProcessor::vulkan_shared_device()`: lê os handles
    do wgpu com `as_hal::<Vulkan>()`.
    **Correção importante do plano:** NÃO é preciso inverter a init do wgpu
    (`Instance::from_hal`/`Adapter::device_from_raw`). O `wgpu-hal` 30 expõe
    `raw_device()`/`raw_physical_device()`/`raw_queue()`/
    `queue_family_index()`/`shared_instance().entry()` do device que ele
    mesmo criou — então o `FrameProcessor` continua sendo construído como
    sempre e o core só adota. Some o pedaço mais invasivo da etapa.
  - **B4 (feito)** — `FrameOrigin::HardwareVulkanImage` +
    `FrameProcessor::bind_vulkan_input`/`wrap_vulkan_image`:
    `Device::texture_from_raw` (`TextureMemory::External`, `drop_callback`
    no-op, `initial_state = RESOURCE`) cacheado por `sync_index`; alimenta
    `self.interop_view` — a chain trata igual ao interop GL.
    `core::next_vk_frame` devolve o `Frame` (era `None`): submete os cmd
    buffers do core + CPU-wait no fence + embrulha a `VkImage`.
    `DesktopCoreLoader::with_vulkan_shared_device()` faz o `adopt`.
  - **B3a (feito) — threading: opção 4 (a thread do core só GRAVA; o
    compositor SUBMETE).** O problema era `vkQueueSubmit` na MESMA `VkQueue`
    de duas threads (core-loop + render do wgpu) = UB (Vulkan exige sync
    externo na queue e não dá pra hookar o `wgpu::Queue::submit`). Descartadas:
    (1) mover o core pra thread do compositor — restruturação grande;
    (2) `Mutex` em volta de todo `queue.submit` — frágil (`write_buffer`/
    `write_texture` do wgpu submetem por fora); (3) aceitar a corrida — UB.
    **Opção 4:** o core, em `set_command_buffers`, só grava os cmd buffers;
    quem chama `vkQueueSubmit` é sempre a thread do compositor (junto do
    submit do wgpu). Peças:
    - `vk_frame.rs`: `VkFrameSync` (`Arc`, `Send + Sync`) — gate por-slot
      "geração já liberada". O core, no `wait_sync_index`, bloqueia num
      `Condvar` até o compositor (ou o descarte de frame) liberar o uso
      anterior daquele slot — senão o core reescreveria o command pool de um
      slot que o compositor ainda submete (UB). `take_pending()` (era
      `submit_pending`) pega os cmd buffers SEM submeter.
    - `VkImageFrame` carrega os cmd buffers + o fence; `Drop` → `release()`
      do slot (idempotente) — funciona pra quem largar o `Frame` (compositor
      após processar, ou `emu-session` descartando frame atrasado).
    - `domain::VulkanImageHandle` += `command_buffers()` / `fence()` /
      `release()`.
    - `gpu.rs::submit_vulkan_cmds`: na thread do compositor, `vkQueueSubmit`
      dos cmd buffers do core + `vkWaitForFences` (sync conservador da fase
      B). Chamado por `bind_vulkan_input` antes do `wrap`.
    Teste e2e (`#[ignore]`, `apps/desktop/src-tauri/src/gpu.rs`): `vk_rendering`
    adota o `VkDevice` do compositor → renderiza o triângulo → `FrameProcessor`
    amostra com `texture_from_raw` → chain → readback. Verde: frame 1,
    320×240, 1º pixel `[204, 153, 51, 255]` (= clear RGB 0.8, 0.6, 0.2), sem
    erros da validação Vulkan.
  - **B3b (feito, opt-in) — o `emu-session` roda o core Vulkan in-process.**
    O shell publica os handles do device na sessão
    (`EmuSession::attach_vulkan_device`, chamado depois que o `FrameProcessor`
    sobe, com `FrameProcessor::vulkan_shared_device()`). No `Command::Load`, se
    `REEMU_HW=vulkan` e o device está publicado, `session.rs::core_loop` tenta
    `crates/emu-session/src/local_core.rs::LocalCore::load` (=`DesktopCoreLoader
    ::with_vulkan_shared_device` + `open_core`) ANTES do `ChildProc::spawn`. Se
    o core não negociar Vulkan, `LocalCore::load` devolve `HwRenderUnsupported`
    e o loop cai pro processo filho (e memoiza o core em `known_non_vulkan` pra
    não fazer um 2º `retro_init` no processo pai — mataria um parallel_n64).
    O `LocalCore` tem seu próprio drive loop (pacing por acumulador, igual ao
    `reemu-core-host::run_one_frame`); `session.rs` publica o `Frame`/áudio
    direto em `Shared` (sem IPC, sem anel). O `VkFrameSync` viaja dentro do
    `Frame` (`VkImageFrame` carrega o `Arc`), então o descarte de frame
    atrasado no `emu-session`/compositor já libera o slot pelo `Drop` — não
    precisou de fiação extra.
    **Trade-off (aceito, gated no env var):** volta a valer "um core por
    processo" da API libretro pro processo pai — cores não re-entrantes podem
    cair numa 2ª carga local. Cores software/GL seguem no filho, intactos.
    Fase C traz o sync fino de qualquer jeito (`set_signal_semaphore` do core
    → `Queue::add_wait_semaphore` do wgpu-hal, que existe na 30).
  - **B3b validado end-to-end (2026-09-07, RTX 3060):** app com
    `REEMU_HW=vulkan` + `--features dev-autoload` + `scripts/build-vk-test-core.sh`
    → `contexto Vulkan ADOTADO do compositor` → `core Vulkan ... in-process:
    fps=60` (sem `reemu-core-host`) → **triângulo RGB do `vk_rendering` girando
    na surface nativa, orientação certa**, sobre o fundo `(0.8,0.6,0.2)`. Sem
    erro de validação Vulkan.
  - **Próximo:** Beetle PSX HW (`mednafen_psx_hw`) — 1º emulador real.
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
