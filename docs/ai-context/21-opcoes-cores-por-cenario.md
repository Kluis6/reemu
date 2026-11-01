# Opções dos cores por cenário

Pesquisa de 2026-11-01 para as predefinições Desempenho e Qualidade. As chaves vêm dos
arquivos de opções de cada core e do docs.libretro.com; os efeitos vêm do que essas fontes
afirmam, salvo quando marcado como relato. Nenhum valor foi testado no ReEmu. Conferir as
chaves na versão instalada, porque mudam entre versões.

Decisão do dono do projeto: todos os cores mostram os botões. Quem não tem predefinição completa
muda só a resolução (`resolutionPreset` em `lib/corePresets.ts`).

## Padrão entre os cores

| Tipo de opção | Mais FPS | Mais compatibilidade | Mais qualidade |
|---|---|---|---|
| Resolução interna | 1x | 1x | Aumentar até o limite da GPU |
| Recompilador (dynarec, JIT) | Ligado | Interpretador, se o jogo falhar | Sem efeito direto |
| Pulo de quadros | Automático | Desligado | Desligado |
| Filtro de textura, MSAA, anisotrópico | Desligado ou baixo | Padrão do core | Aumentar |
| Correção de geometria (PGXP) | Desligada | Desligada | Ligada, com risco por jogo |
| Hacks (widescreen, overclock) | Evitar | Evitar | Só por jogo |

Hacks de velocidade e melhorias visuais podem quebrar jogos, então "mais compatibilidade" é
quase sempre o padrão do core. Overclock e widescreen ficam fora das predefinições gerais.

## PS1: Beetle PSX HW (`beetle_psx_hw_*`)

| Chave | Efeito |
|---|---|
| `renderer` | Software é o mais preciso; hardware rende mais a partir de 2x e libera as melhorias |
| `internal_resolution` | 1x a 16x; melhora o 3D, com custo de desempenho |
| `pgxp_mode` | "memory only" tem custo e problemas menores; "memory + CPU" é muito pesado e causa erros de geometria |
| `dither_mode` | Só visual; "1x(native)" pode ficar granulado em resolução alta |
| `filter` | "nearest" é o original; SABR, xBR e JINC2 custam mais |
| `msaa` | Só Vulkan; mais leve que supersampling, com qualidade menor |
| `cpu_dynarec` | Muito mais rápido que o interpretador, com temporização menos precisa |
| `frame_duping` | Ganho pequeno; pode perder quadros de animação, não recomendado sem necessidade |

## PS1: SwanStation (`swanstation_*`)

| Chave | Efeito |
|---|---|
| `GPU_ResolutionScale` | Maior é mais lento; alguns jogos exigem 1x |
| `GPU_TrueColor` | Tira o dithering; pode quebrar alguns jogos |
| `GPU_TextureFilter` | Suaviza texturas ampliadas |
| `GPU_PGXPEnable` | Reduz o tremor de polígonos; pode não ser compatível com todos os jogos |
| `GPU_MSAA` | Custo menor que aumentar a resolução; pode causar erros |
| `CPU_ExecutionMode` | `Recompiler` tem o melhor desempenho |
| `CPU_RecompilerICache` | Mais precisão, com custo pequeno |

## PS1 leve: PCSX ReARMed (`pcsx_rearmed_*`)

| Chave | Efeito |
|---|---|
| `drc` | Muito mais rápido que o interpretador; pode ser menos preciso |
| `frameskip_type` | Mais desempenho, menos suavidade |
| `gpu_thread_rendering` | Comandos da GPU em outra thread; "auto" liga com 2 núcleos ou mais |
| `dithering` | Suaviza bandas de cor; aumenta a exigência |
| `neon_enhancement_enable` | Dobra a resolução; mais qualidade e mais exigência |
| `icache_emulation` | Mais precisão com custo; necessário em alguns jogos de Fórmula 1 |

Opções como `gpu_unai_blending` e `gpu_unai_lighting` ganham desempenho com erros graves de
imagem. Não entram em predefinição.

## N64: Mupen64Plus-Next (`mupen64plus-*`)

| Chave | Efeito |
|---|---|
| `cpucore` | O recompilador dinâmico é o mais rápido |
| `43screensize`, `169screensize` | Maior resolução exige mais |
| `EnableFBEmulation` | Desligar remove efeitos; só com problema de desempenho |
| `MultiSampling`, `FXAA` | Suavização de bordas |
| `EnableN64DepthCompare` | Relato de erro gráfico em GLideN64 com HLE; não verificado |

O modo RSP LLE tende a ser mais preciso e exige mais. Relatos e wikis colocam GLideN64 como o
mais rápido e ParaLLEl-RDP como o preciso. As chaves do RSP e do plugin não foram conferidas.

## Dreamcast: Flycast (prefixo a confirmar)

| Chave | Efeito |
|---|---|
| `_internal_resolution` | Resolução de renderização; exige reiniciar |
| `_threaded_rendering` | CPU e GPU em threads separadas; muito recomendado pela documentação |
| `_auto_skip_frame` | Pula quadros quando fica lento; depende do threaded rendering |
| `_cable_type` | TV (Composite) é o mais suportado; VGA é indicado em máquina fraca |
| `_alpha_sorting` | Per-Strip é rápido e menos preciso; Per-Pixel é preciso e o mais lento |
| `_anisotropic_filtering` | Melhora texturas em ângulos oblíquos |
| `_delay_frame_swapping` | Evita tela piscando; não recomendado em máquina lenta |

## PSP: PPSSPP (`ppsspp_*`)

| Chave | Efeito |
|---|---|
| `internal_resolution` | Grande impacto na GPU |
| `auto_frameskip`, `frameskip` | Mais velocidade, com possível travamento visual |
| `skip_buffer_effects` | Mais rápido; alguns jogos podem não desenhar nada |
| `texture_scaling_type`, `texture_scaling_level` | Feito pela CPU, com grande impacto; xBRZ é o melhor geral |
| `texture_anisotropic_filtering` | Melhora texturas do horizonte |
| `lower_resolution_for_effects` | Reduz artefatos |

Na documentação, Vulkan é o mais rápido e OpenGL o mais compatível.

## DS: melonDS (`melonds_*`)

| Chave | Valores e padrão |
|---|---|
| `jit_enable` | Ligado por padrão; exige reiniciar |
| `threaded_renderer` | Renderizador por software em outra thread; desligado por padrão |
| `opengl_renderer` | Desligado por padrão; exige reiniciar |
| `opengl_resolution` | 1x a 8x; depende do renderizador OpenGL |
| `opengl_filtering` | `nearest` ou `linear` |

O arquivo não descreve o impacto dessas opções.

## PS2: LRPS2 (`pcsx2_libretro` no catálogo)

Na documentação: ParaLLEl-GS tem alta precisão e é recomendado se rodar em velocidade total, com
GPU dedicada; integrados Intel não seriam rápidos o bastante quando o texto foi escrito. Software
serve aos jogos que exigem mais precisão, e no modo por hardware os erros são mais comuns. A
GameIndex embutida aplica correções por jogo.

**Não verificado:** as chaves das opções. O caminho do repositório não respondeu e a página não
as lista. Deve ser o último core a receber predefinição.

## Fontes

- Opções: [PPSSPP](https://github.com/hrydgard/ppsspp/blob/master/libretro/libretro_core_options.h), [PCSX ReARMed](https://github.com/libretro/pcsx_rearmed/blob/master/frontend/libretro_core_options.h), [SwanStation](https://github.com/libretro/swanstation/blob/main/src/libretro/libretro_core_options.h), [melonDS](https://github.com/libretro/melonDS/blob/master/src/libretro/libretro_core_options.h), [Beetle PSX](https://github.com/libretro/beetle-psx-libretro/blob/master/libretro_core_options.h), [Mupen64Plus-Next](https://github.com/libretro/mupen64plus-libretro-nx/blob/develop/libretro/libretro_core_options.h), [Flycast](https://github.com/flyinghead/flycast/blob/master/shell/libretro/libretro_core_options.h)
- Documentação: [Beetle PSX HW](https://docs.libretro.com/library/beetle_psx_hw/), [Mupen64Plus-Next](https://docs.libretro.com/library/mupen64plus/), [Flycast](https://docs.libretro.com/library/flycast/), [PPSSPP](https://docs.libretro.com/library/ppsspp/), [LRPS2](https://docs.libretro.com/library/lrps2/)
- Comunidade: [SwanStation mais rápido que Beetle](https://www.libretro.com/index.php/category/playstation/), [erro com depth compare](https://forums.libretro.com/t/mupen64-next-gliden64-gliches/28090), [GLideN64 e ParaLLEl-RDP](https://wiki.batocera.org/_export/raw/systems:n64), [Beetle a 2x e 4x](https://forums.libretro.com/t/poor-performance-on-beetle-psx-i5-4590-gtx-1650-solved/26433)
