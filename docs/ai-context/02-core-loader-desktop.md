# 02 — Core Loader Desktop (Caminho GL)

## Objetivo desta etapa

Implementar `domain::core_loader::CoreLoader` e `domain::frame_source::FrameSource`
pra desktop, cobrindo primeiro o caminho software-only (core entrega buffer
de pixels crus) e depois o caminho GL de hardware render
(`retro_hw_render_callback`). **Vulkan por-core fica pra depois** — ver
`12-vulkan-hw-render-fase2.md`.

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

## Critério de pronto

- Um core software-only carrega, roda, e produz frames consumíveis via
  `FrameSource::next_frame()` sem panics
- Um core GL hardware-accelerated (ex: um core de N64) consegue completar
  a negociação de contexto sem erro — não precisa estar performático
  ainda, só funcionalmente correto
