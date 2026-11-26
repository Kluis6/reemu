# Qualidade e desempenho dos shaders nativos do ReEmu

**Status** `backlog` · 2026-11-24 · [Tarefa principal](../../TASKS.md#vídeo--gpu)

## Estado atual

O ReEmu oferece os presets WGSL `plain`, `crt` e `lcd` e distribui o port do
AMD FidelityFX CAS em Slang. Os quatro funcionam sem baixar o pacote de
shaders. A mesma tela também oferece atalhos para efeitos do pacote, como
CRT Guest, NTSC, xBR, ScaleFX, FSR e NIS. Esses efeitos servem de referência
visual e de regressão para futuras mudanças no caminho compartilhado.
[Presets nativos e curados](../../apps/desktop/src-tauri/src/gpu/presets.rs) ·
[CAS embutido](../../apps/desktop/src-tauri/src/bundled_shaders.rs)

| Preset | Implementação | Oportunidade |
|---|---|---|
| `plain` | Um passe de cópia em resolução de origem. O blit final da superfície usa filtro linear. | Medir a eliminação da cópia intermediária em casos simples. Testar uma opção de escala nítida para pixel art, incluindo escala fracionária. |
| `crt` | Filtro horizontal de três amostras na origem. Segundo passe com escala fixa de 3× por eixo, scanlines, máscara RGB, vinheta e compensação de brilho. | Ajustar scanlines à escala do conteúdo e ancorar a máscara nos pixels físicos do retângulo do jogo. Evitar filtrá-la de novo e verificar cintilação, moiré e estouro em altas luzes. |
| `lcd` | Um passe em 2× por eixo, com amostragem vizinho mais próximo, grade, transformação tonal e ganho fixo. | Fazer a grade sobreviver à amostragem e ao redimensionamento. Preservar paleta e legibilidade em telas pequenas. |
| `curated:cas` | Um passe na resolução da origem, cinco leituras por pixel, aproximação de gama 2,0 e nitidez ajustável. | Revisar a conversão de cor de acordo com a entrada e limitar halos em textos e bordas. Manter o caso principal de nitidez sem ampliação. |

O CRT gera 9 pixels no segundo passe para cada pixel do quadro recebido.
Uma entrada de 1280×720 vira um alvo de 3840×2160, mesmo se a imagem for
exibida em 1080p. Os dois alvos RGBA8 totalizam aproximadamente 35,2 MiB
de armazenamento lógico nesse exemplo. O valor exclui entrada, composição
e superfície e serve para dimensionar o trabalho, sem estimar tempo de GPU
ou tráfego de memória real. [Escala dos presets](../../apps/desktop/src-tauri/src/gpu/specs.rs) ·
[Renderização dos passes](../../apps/desktop/src-tauri/src/gpu/chain.rs) ·
[Blit final](../../apps/desktop/src-tauri/src/gpu/surface.rs)

No LCD, com a imagem alinhada e o passe em 2×, as duas amostras por eixo
caem nas fases 0,25 e 0,75 de cada texel. Ambas ficam além do limite de 0,12
usado em `smoothstep`, de modo que a grade fica neutra nesse caso. No CAS,
o teste atual valida a matemática do port com uma imagem em cinza, útil
como referência para ampliar a cobertura a cores saturadas, sombras e
transições de alto contraste. [WGSL dos presets](../../apps/desktop/src-tauri/src/gpu/presets.rs) ·
[Slang do CAS](../../apps/desktop/src-tauri/shaders/cas/cas.slang) ·
[Testes de GPU](../../apps/desktop/src-tauri/src/gpu/tests.rs)

O diagnóstico `REEMU_PERF` cobre cadência, tempo observado na CPU, espera
da superfície e perdas de quadro. A referência existente da RTX 3060
inclui CRT Guest, um efeito externo. A primeira entrega deve medir os
presets nativos individualmente. A prévia usa uma instância de
`FrameProcessor` separada, quatro quadros e uma leitura para PNG. Seu
tempo de resposta precisa de uma medida própria. [Roteiro de captura](../perf/README.md) ·
[Prévia nativa](../../apps/desktop/src-tauri/src/commands/shader_preview.rs)

## Etapas em backlog

### 1. Medir o estado atual

- [ ] Capturar `plain`, `crt`, `lcd` e CAS sobre as mesmas sequências
  determinísticas. Cobrir entradas de 160×144 e 320×240, um core 3D com
  escala interna e saídas a 1080p e 4K. Registrar resolução da origem e do
  retângulo exibido, GPU, driver, backend, Hz, SDR/HDR e escopo do preset.
- [ ] Reaproveitar `REEMU_PERF` e o relatório atual. Medir GPU por passe e
  no blit final via `TIMESTAMP_QUERY` quando o dispositivo oferecer a
  capacidade. Converter os ticks com `Queue::get_timestamp_period` e ler
  resultados em buffers rotativos, sem bloquear a fila de apresentação.
  Nos dispositivos sem a capacidade, registrar tempo de CPU, cadência e
  perdas e indicar que a duração na GPU está indisponível.
  [Consultas por passe no wgpu](https://wgpu.rs/doc/wgpu/struct.RenderPassTimestampWrites.html) ·
  [Período dos timestamps](https://wgpu.rs/doc/wgpu/struct.Queue.html#method.get_timestamp_period)
- [ ] Registrar sessões repetidas de pelo menos 60 s após aquecimento.
  Publicar p50, p95 e p99 da GPU, ritmo e perdas de quadros, custo na CPU,
  tamanho dos alvos e tempo da primeira e das próximas prévias. Usar a
  RTX 3060 de referência e uma GPU integrada, como Intel ou AMD. Quando
  houver contador disponível, registrar memória e consumo como medições
  distintas do cálculo lógico dos alvos.

### 2. Estabilizar a imagem do CRT

- [ ] Criar uma variante leve para comparar com o CRT atual. Calcular a
  fase da scanline nas linhas da fonte e a máscara de fósforo no espaço
  do retângulo final. Usar cobertura de pixel ou derivadas de coordenadas
  contínuas para atenuar detalhes em escala fracionária ou reduzida.
  Comparar redimensionamento, rotação, conteúdo 240p/480i, se o core
  expuser essa distinção, e fontes 3D de maior resolução.
  [Derivadas em WGSL](https://www.w3.org/TR/WGSL/#derivative-builtin-functions) ·
  [Amostragem e escala Libretro](https://docs.libretro.com/development/shader/slang-shaders/)
- [ ] Comparar um passe final integrado à apresentação com uma variante
  que mantém o filtro horizontal em baixa resolução. Medir ambas a
  1080p e 4K. Testar duas amostras bilineares do filtro horizontal apenas
  para o caso de alinhamento que reproduz os pesos 0,25/0,50/0,25. A
  qualidade da máscara e o custo medido decidem a configuração.
  [Amostras bilineares combinadas](https://developer.nvidia.com/gpugems/gpugems2/part-iii-high-quality-rendering/chapter-20-fast-third-order-texture-filtering)
- [ ] Avaliar controle de scanlines, máscara e brilho. Comparar rampas e
  altas luzes em SDR e HDR. Diferenciar a variante com efeitos mais caros
  pela escolha do usuário e pelo orçamento medido.

### 3. Melhorar LCD, escala e CAS

- [ ] **LCD.** Fazer a grade pela distância simétrica à borda do texel,
  com cobertura estimada no pixel de saída. Atenuá-la onde o retângulo
  exibido tiver resolução insuficiente. Conferir a presença da grade em
  2× exato e estabilidade com escala fracionária, janela e tela cheia.
  Comparar contraste de leitura e cores de Game Boy, GBC e GBA.
- [ ] **Escala.** Comparar vizinho mais próximo em escala inteira e
  filtragem consciente do texel em escala fracionária. Usar texto fino,
  barras, diagonais e movimento lento. Conferir proporção, letterbox,
  molduras e rotação antes de expor a opção ao usuário.
  [Regras de filtragem Libretro](https://docs.libretro.com/development/shader/slang-shaders/)
- [ ] **CAS.** Mapear a codificação da textura de origem, o formato dos
  alvos e a apresentação final SDR/HDR. Validar conversão de ida e volta
  ao linear sem duplicá-la para texturas sRGB. Cobrir pretos, cores
  saturadas, halos e os extremos de nitidez. Registrar diferenças em
  relação ao port atual antes de trocar o padrão. Se houver protótipo de
  ampliação, limitar ao intervalo documentado pela referência adotada.
  O cabeçalho original define até 4× na **área**.
  [Integração AMD CAS](https://gpuopen.com/manuals/fidelityfx_sdk/techniques/contrast-adaptive-sharpening/) ·
  [Referência `ffx_cas.h`](https://github.com/GPUOpen-Effects/FidelityFX-CAS/blob/master/ffx-cas/ffx_cas.h) ·
  [Formatos de textura WebGPU](https://www.w3.org/TR/webgpu/#texture-formats)

### 4. Reduzir trabalho por quadro e na prévia

- [ ] Medir a cópia de `plain`, o `BindGroup` criado no blit a cada
  quadro e a reconstrução dos grupos quando muda o slot de entrada.
  Prototipar reutilização conforme a identidade dos recursos e um blit
  direto para o caminho sem efeito. Conferir invalidação por resize,
  troca de preset, HDR e recuperação de surface. Preservar recursos
  intermediários quando a leitura seguinte depende deles, incluindo
  composição, feedback, histórico, captura e canvas.
  [Validação WebGPU](https://www.w3.org/TR/webgpu/) ·
  [Custo dos anexos de renderização, Khronos](https://docs.vulkan.org/samples/latest/samples/performance/render_passes/README.html)
- [ ] Medir compilação, desenho, cópia para CPU e compressão da prévia
  separadamente. Para presets sem histórico ou feedback, testar o menor
  número de quadros necessário ao readback. Reusar a prévia por preset,
  cena, parâmetros e tamanho, com limite de memória. Coalescer pedidos
  de sliders e manter o resultado mais recente, reaproveitando o debounce
  existente. Preservar o aquecimento necessário aos efeitos temporais.
- [ ] Validar a equivalência entre a prévia e a tela do jogo no mesmo
  tamanho e modo de cor. Comparar recortes em escala de pixel e uma
  sequência curta em movimento para revelar cintilação.

### 5. Validar e preparar a entrega

- [ ] Comparar lado a lado as sequências de pixel art, texto, movimento
  diagonal, 3D e rampas de cinza. Registrar perda de detalhe, moiré,
  cintilação, halos, alteração de paleta e clipping, incluindo escala
  inteira e fracionária. Para caminhos equivalentes, usar comparação de
  pixels com tolerância explícita. Para efeitos com aparência nova,
  revisar também movimento e legibilidade no tamanho real.
- [ ] Executar a matriz no Windows com cores de software, OpenGL e Vulkan.
  Conferir janela, tela cheia, moldura, rotação, SDR/HDR, captura, pausa,
  troca de jogo e resolução dinâmica. Usar alguns presets do pacote com
  múltiplos passes e feedback para cobrir o caminho compartilhado.
  A matriz Linux está no [backlog de Linux x64](../../TASKS.md#testes-em-linux-x64).
- [ ] Manter os IDs e parâmetros persistidos por padrão, plataforma e
  jogo ao oferecer uma variante nova. Se os resultados justificarem mais
  de um nível de qualidade, usar controles Fluent UI 2 com i18n, opção
  explícita de escolha e navegação por controle. Testar recuperação com
  `plain` se a GPU rejeitar o preset escolhido.

## Critérios de aceite

- A melhora visual deve aparecer nas capturas e nas sequências da matriz.
  CRT e LCD devem conservar texto legível e padrões estáveis na escala
  fracionária. CAS deve passar o teste existente e os novos casos de cor.
- Aplicar o [portão de desempenho do projeto](19-plano-unificado.md#portão-de-desempenho)
  e o [relatório de comparação](../perf/README.md), incluindo p99 de
  cadência, perdas e áudio. Como meta inicial sujeita à primeira medição,
  buscar acréscimo de até **1 ms no p95 de GPU** sobre `plain` para a
  variante leve em 1080p na GPU integrada escolhida. Registrar o resultado
  correspondente a 4K e validar à parte cada variante mais cara.
- Registrar a documentação oficial e a versão da API usadas em cada
  mudança. A revisão da integração inclui os recursos do `wgpu 30.0.2`
  fixado no `Cargo.lock` e os limites do dispositivo real. A especificação
  e os exemplos de Khronos e W3C descrevem contratos e técnicas. Ganhos de
  desempenho dependem da medição no ReEmu.
  [Regra do projeto](../../CLAUDE.md#regra-de-documentação-oficial-para-apis-gráficas) ·
  [Capacidades wgpu](https://wgpu.rs/doc/wgpu/struct.Features.html)

## Evidência desta análise

No Windows com Vulkan, passaram os testes existentes da prévia 2D/3D com
CRT, do CAS contra a referência CPU do port e do mapeamento SDR para HDR
em scRGB. Eles fornecem uma verificação funcional. Tempos por preset,
quantificação de artefatos e a matriz de hardware ficam na etapa 1.
[Testes de GPU](../../apps/desktop/src-tauri/src/gpu/tests.rs)
