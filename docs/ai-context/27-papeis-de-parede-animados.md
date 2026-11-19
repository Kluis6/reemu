# 27 — Qualidade e fluidez dos papéis de parede animados

**Estado** — Backlog desde 2026-11-18.

**Objetivo** — Melhorar a nitidez e a continuidade do movimento dos seis fundos
embutidos, com limite de custo na navegação e no início dos jogos. O primeiro
piloto será no Windows com WebView2. O pôster é a referência de menor custo.

## Avaliação dos arquivos atuais

Os seis WebM embutidos têm uma faixa de vídeo VP9 sem áudio, 1920×1080,
288 quadros em 12 s e quadros-chave em 0, 4 e 8 s. As marcas de tempo
avançam 41 ou 42 ms, como esperado a 24 fps com resolução de 1 ms. Os
vídeos somam **13,96 MB** e os seis pôsteres somam **0,53 MB** (MB
decimais). O catálogo acerta os 24 fps. O cabeçalho do gerador ainda
diz 30 fps e deve acompanhar a configuração executada.

Foram conferidos os contêineres e os pôsteres. Também foram comparados os
quadros 286, 287, 0 e 1 das cenas no Canvas e dos WebM decodificados em
Chromium headless. A tabela mostra a diferença absoluta média por canal
RGB de 8 bits. Para os vizinhos, foi usada a maior diferença entre
286→287 e 0→1. O teste mede a imagem da emenda. O tempo de apresentação
pelo WebView2 e o consumo do aplicativo ainda precisam ser medidos.

| Papel de parede | Emenda na arte | Emenda no WebM | Maior vizinho no WebM | Trabalho por cena |
| --- | ---: | ---: | ---: | --- |
| 1. Circuito | 0,16 | 2,30 | 0,25 | Preservar trilhas finas e pontos de luz. Atenuar cada pulso perto das pontas da trilha em que ele reaparece. |
| 2. Controles | 0,15 | 2,25 | 0,23 | Usar como piloto de contornos em movimento. Conferir as espessuras e a legibilidade dos ícones em 720p e 900p. |
| 3. Floresta 8-bit | 0,33 | 1,89 | 0,75 | Manter os contornos pixelados e distribuir no tempo os saltos de 12 px das nuvens e do terreno. Conferir a repetição das árvores nas bordas. |
| 4. Fliperama | 0,21 | 2,85 | 0,42 | Usar como piloto de emenda. Fazer as linhas da grade entrarem e saírem com opacidade suave. Conferir o sol listrado e o horizonte com a UI por cima. |
| 5. Espaço | 0,05 | 1,55 | 0,18 | Preservar as estrelas pequenas e os gradientes da nebulosa. Conferir as trajetórias das estrelas cadentes nas extremidades. |
| 6. ReEmu | 0,17 | 1,99 | 0,38 | Suavizar a reciclagem da grade e preservar os contornos em néon e os detalhes das áreas escuras. |

O salto entre o último e o primeiro quadro foi maior no vídeo decodificado
do que na arte nos seis arquivos. Fliperama teve a maior diferença média
na emenda. Em Floresta 8-bit, a diferença entre vizinhos já varia na arte,
onde há deslocamentos arredondados para a grade de pixels. Esses resultados
orientam duas frentes de trabalho. A primeira corrige as trajetórias e a
segunda mede como a codificação muda o aspecto da volta.

O app usa um único vídeo em loop no Windows. A galeria de Aparência usa
miniaturas estáticas. A janela desfocada ou minimizada pede pausa.
Movimento reduzido e Linux usam o pôster. O fundo é desmontado na rota de
jogo. Na partida do vídeo há `autoplay` junto com `play()` pelo efeito,
`preload="auto"` e a rejeição de `play()` é descartada. Esses pontos entram
no teste de ciclo de vida, inclusive a primeira montagem com a janela já
sem foco. [Componente do fundo](../../packages/app-desktop/src/components/AnimatedBackground.tsx) ·
[Galeria](../../packages/app-desktop/src/screens/settings/SettingsAppearance.tsx) ·
[Cenas](../../scripts/wallpaper-video/scenes.js) ·
[Gerador](../../scripts/wallpaper-video/render.py) ·
[Histórico](../historico.md)

## Etapas e tarefas

### 1. Medir a referência no desktop

- [ ] Medir três rodadas de pelo menos 60 s na mesma máquina, resolução,
  frequência e cena, com referência de pôster e do VP9 atual. Usar um
  Windows com GPU integrada e outro com dedicada. Incluir 60 Hz e ao menos
  uma configuração de TV ou tela de alta resolução.
- [ ] Registrar versão do WebView2, GPU, driver, modo de energia, tempo
  até o primeiro quadro, contagem de quadros descartados e a maior lacuna
  na virada do loop. Usar `getVideoPlaybackQuality()` e
  `requestVideoFrameCallback()` quando disponíveis, em modo de diagnóstico.
  Confirmar o decodificador pelo painel Media e pelo motor Video Decode.
  [Media Playback Quality](https://w3c.github.io/media-playback-quality/) ·
  [Callback de quadro de vídeo](https://wicg.github.io/video-rvfc/) ·
  [DevTools do WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-devtools) ·
  [Painel Media](https://learn.microsoft.com/en-us/microsoft-edge/devtools/media/) ·
  [GPUView](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/using-gpuview)
- [ ] Repetir a navegação pela Início, pela biblioteca e por Aparência, e
  abrir um jogo saindo da Início. Registrar CPU, GPU, memória e latência
  de entrada. Usar o [roteiro de desempenho de jogos](../perf/README.md)
  para comparar os tempos do core e do vídeo.

### 2. Melhorar a arte e a volta com o formato atual

- [ ] Corrigir em primeiro lugar a grade de Fliperama e testar os contornos
  de Controles. Depois aplicar as tarefas da tabela às outras cenas.
- [ ] Comparar quadros renderizados antes e depois de codificar, incluindo
  os quadros vizinhos dos pontos 0, 4 e 8 s. Separar saltos da arte, mudanças
  entre quadros-chave e preditivos, e pausas de reprodução. Inspecionar os
  gradientes escuros a 100% para ver faixas de cor, blocos e perda de brilho.
- [ ] Evitar ruído que muda a cada quadro. Se for preciso reduzir faixas de
  cor, testar dithering discreto e estável na geração e comparar tamanho e
  nitidez. Gerar o pôster da mesma primeira imagem que alimenta o vídeo e
  verificar a transição, o recorte em tela larga e a leitura nos temas claro
  e escuro. Em temas claros, manter o papel de parede com as cores da arte.
  [WebCodecs](https://www.w3.org/TR/webcodecs/) ·
  [WebM Container Guidelines](https://www.webmproject.org/docs/container/)

### 3. Comparar três perfis VP9

- [ ] Tornar resolução, fps, bitrate e intervalo de quadros-chave
  reprodutíveis pelo gerador. Gerar os candidatos em diretório temporário,
  com a mesma cena e duração, registrando versão do navegador e parâmetros.
- [ ] Usar 1080p/24 como referência. Comparar **900p/30** como candidato
  principal, **720p/30** como candidato de menor custo e **1080p/30** como
  controle de qualidade. Em 60 Hz, 30 fps pode distribuir um quadro a cada
  duas atualizações. 24 fps exige distribuição desigual nessa frequência.
  Confirmar a cadência no app e em telas com outras frequências.
- [ ] Comparar bitrate e intervalo de quadros-chave em ensaios separados.
  Para 1080p/30, testar primeiro o mesmo orçamento de bits por segundo,
  depois medir o efeito de subir o bitrate. Ajustar a codificação por cena
  se a qualidade exigir. Só abrir estudo de outro codec se a comparação em
  VP9 falhar e houver ganho medido no WebView alvo.

| Perfil | Megapixels de vídeo/s | Variação sobre o atual |
| --- | ---: | ---: |
| 1920×1080 / 24 | 49,77 | Referência |
| 1600×900 / 30 | 43,20 | −13,2% |
| 1280×720 / 30 | 27,65 | −44,4% |
| 1920×1080 / 30 | 62,21 | +25,0% |

Pixels por segundo descrevem o volume de imagem. A escolha depende das
medições de decodificação, escala, composição e consumo na máquina real.

### 4. Tornar a reprodução previsível

- [ ] Adotar uma única decisão de reprodução com base na rota, no foco,
  na visibilidade e no movimento reduzido. Mostrar o pôster até o primeiro
  quadro. Medir opções de preload e manter uma instância de vídeo ativa.
- [ ] Em erro de mídia ou falha de `play()`, exibir o pôster. Validar perda e
  retorno de foco, minimizar e restaurar, troca de preset e de escopo,
  alternância de movimento reduzido e entrada e saída do jogo. Confirmar
  que nenhum vídeo continua decodificando nessas rotas ou estados.
  [Reprodução no HTML](https://html.spec.whatwg.org/multipage/media.html#playing-the-media-resource) ·
  [Movimento reduzido](https://www.w3.org/TR/mediaqueries-5/#prefers-reduced-motion)
- [ ] Se os resultados divergirem entre máquinas, avaliar um perfil leve
  ou o pôster como alternativa. `MediaCapabilities.decodingInfo()` pode
  estimar suporte, fluidez e eficiência. Validar essa indicação com o
  decodificador e os contadores reais. [Media Capabilities](https://www.w3.org/TR/media-capabilities/)

### 5. Validar e liberar

- [ ] Aplicar o perfil aprovado aos seis presets mantendo os IDs salvos.
  Repetir as medidas do piloto, validar o contraste na Início e em
  Configurações e medir o pacote final. Se o candidato falhar, manter o
  perfil atual com as melhorias de arte e de ciclo de vida aprovadas.
- [ ] Avaliar animação no Linux em tarefa separada, com GPU e composição
  comprovadas no WebKitGTK/GStreamer. Manter o pôster no caminho
  padrão enquanto faltar evidência. Incluir AMD/Intel Mesa, NVIDIA
  proprietário e o caso de composição desabilitada. Consultar a
  documentação oficial antes de qualquer mudança de plataforma.
  [Inicialização Linux](../../apps/desktop/src-tauri/src/main.rs) ·
  [WebKitGTK e aceleração de vídeo](https://webkitgtk.org/2024/03/27/webkigit-2.44.html)

## Critérios para aprovar o piloto

Limites iniciais propostos, a confirmar com a variação observada nas três
rodadas. Comparar em primeiro plano com o pôster e com os vídeos atuais.
As medidas da rota de jogo seguem o limite já definido pelo projeto.

| Critério | Aprovação |
| --- | --- |
| Aparência | Sem salto, faixa de cor nova, sumiço de traço ou cintilação perceptível em cinco voltas consecutivas. Comparar também o escore da emenda com a referência de cada cena. |
| Cadência | Menos de 1% de quadros descartados durante os 60 s visíveis após aquecimento. Lacuna de apresentação na emenda de até dois intervalos nominais, cerca de 67 ms a 30 fps. |
| Interface | p95 de latência entre entrada e próxima apresentação medida até 2 ms acima do pôster. Medir navegação com controle e teclado. |
| Custo ativo | CPU total do app e processos WebView até 2 pontos percentuais acima do pôster, normalizada na máquina. Ganho de qualidade sem aumento de CPU/GPU acima da variação do vídeo atual. Registrar motores 3D e Video Decode separadamente e o pico de memória do conjunto. |
| Inativo e jogo | Nenhum avanço da mídia enquanto estiver pausada ou desmontada. Volta do custo ao nível do pôster dentro da variação observada. Passar pelo portão de desempenho do jogo, incluindo abertura com fundo ativo. |
| Distribuição | Até 15 MB decimais para vídeos e pôsteres embutidos. Medir o instalador também. Se houver perfil alternativo, ele entra na mesma conta. |

[Portão de desempenho do plano unificado](19-plano-unificado.md#portão-de-desempenho)
