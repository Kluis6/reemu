# 12 — HW Render Vulkan Por-Core (Fase 2 — Backlog)

**Não iniciar antes do restante do pipeline (02 a 10) estar estável.**
Esta é deliberadamente a última etapa — decisão tomada de priorizar GL
primeiro por ser o caminho mais maduro e cobrir a maioria dos cores.

## Gatilho de início (critério de maturidade, não data fixa)

Só comece esta etapa quando **todos** os pontos abaixo forem verdade —
não abra por calendário/pressão de roadmap:

1. Caminho GL (`02-core-loader-desktop.md`) estável em produção, sem
   regressão crítica de renderização por pelo menos 2-3 releases seguidas
2. `ShaderChain`/`DecorationResolver`/`OverlayCompositor`
   (`04-shader-chain-decoracao.md`) já validados contra o pipeline de
   pós-processamento — a fundação de cima precisa estar sólida antes de
   empilhar a parte mais arriscada do projeto em cima dela
3. Existe uma lista explícita de cores-alvo que justificam o esforço
   (ex: PCSX2, Flycast) — não inicie "porque sim"

## Cores que dependem disso, enquanto a fase 2 não existe

Decisão: eles **aparecem no catálogo normalmente** (consistente com o
requisito de listagem completa via buildbot, `10-core-catalog.md`), mas
ficam marcados como **"requer Vulkan HW render (não suportado ainda)"**
na UI — usando a mesma metadata técnica (`render_backend`) que já é
detectada em runtime no primeiro load (`02-core-loader-desktop.md`). Não
os exclua da listagem, e não precisa de curadoria manual extra pra saber
quais bloquear — a detecção em runtime já resolve isso: o app tenta
carregar, descobre que o core pede Vulkan, e exibe o bloqueio com a
mensagem apropriada em vez de deixar falhar de forma confusa.

## Objetivo (quando chegar a hora)

Implementar `retro_hw_render_interface_vulkan` pra cores que exigem
Vulkan (ex: PS2/PCSX2, versões modernas de outros cores pesados),
reaproveitando a instância/dispositivo Vulkan já criados na camada global
de pós-processamento (`04-shader-chain-decoracao.md`).

## Por que isso é o trecho mais arriscado do projeto inteiro

O core cria suas próprias `VkImage` e gerencia sua própria sincronização
— o frontend precisa consumir essas imagens sem introduzir hazard de
sincronização (barriers corretos, nunca ler uma imagem que o core ainda
está escrevendo). É historicamente a parte mais bugada de frontends
libretro. Não subestime o esforço de validação aqui.

## Antes de começar

- Confirme que `crates/core-loader-desktop` já suporta o caminho GL de
  forma estável e testada — Vulkan por-core reaproveita a mesma
  abstração (`CoreLoader`, `FrameSource`), só adiciona um novo tipo de
  `FrameOrigin::HardwareTexture`.
- Releia `docs.libretro.com` especificamente sobre
  `RETRO_HW_RENDER_INTERFACE_VULKAN` — não implemente de memória, a
  especificação de sincronização é detalhada e fácil de errar.

## Critério de pronto

- Um core Vulkan-first (ex: um core de PS2) carrega, negocia o contexto
  corretamente, e produz frames sem corrupção visual/flickering causado
  por hazard de sincronização, incluindo sob carga (várias trocas de
  cena rápidas)
