# 27 — Qualidade e fluidez dos papéis de parede animados

**Estado** — Seis cenas atualizadas em 2026-11-20. Fliperama recriado em
2026-11-23. Validação em outros equipamentos em backlog.

**Objetivo** — Melhorar a nitidez e a continuidade do movimento dos seis fundos
embutidos, com limite de custo na navegação e no início dos jogos. O primeiro
piloto será no Windows com WebView2. O pôster é a referência de menor custo.

## Circuito lo-fi em 2026-11-23

O preset 1 mostra uma cidade-placa vista da sacada de um quarto ao fim
da tarde. Trilhas de cobre, vias e pontos de solda ligam prédios inspirados
em chips, capacitores, módulos de memória, resistores com faixas, bobinas
toroidais e dissipadores com aletas. A [ilustração revisada](../../scripts/wallpaper-video/circuito-lofi.png)
foi editada com o ImageGen integrado a partir da arte original.

Com a câmera fixa, o gerador pré-renderiza três faixas de nuvens com
deslocamentos independentes, partículas e pulsos nas trilhas e janelas.
O sol tem um halo âmbar suave, com reflexos localizados no processador,
nas trilhas de cobre e na borda da sacada.

As plantas foram divididas em quatro áreas de folhagem. As pontas oscilam
levemente em ritmos diferentes, com deslocamento menor perto da base de
cada planta. O movimento é incorporado ao vídeo durante a geração.

`python scripts/wallpaper-video/render.py --only 1` exporta o vídeo VP9 de
20 s em 1600×900 a 30 fps e seu pôster. O vídeo ocupa 2.373.112 bytes e o
catálogo de vídeos e pôsteres ocupa 14.884.643 bytes. A diferença média
por canal na emenda decodificada ficou em 1,79 na escala de 0 a 255.
A arte de origem fica entre as ferramentas de geração.

<details>
<summary>Prompt original da arte do Circuito lo-fi</summary>

```text
Create a single original finished 16:9 landscape illustration for a calm looping desktop wallpaper. The three attached images are mood references only: image 1 for the warm/cool contrast and cozy room framing, image 2 for the orange and coral dusk, image 3 for the hand-painted lo-fi anime atmosphere, lavender cloud shadows and balcony plants. Compose a new scene.

Inside a quiet bedroom looking from a fixed eye-level camera through a very wide, fully open balcony door onto a sprawling city built from a giant printed circuit board. The view out of the balcony is the main subject, taking up most of the image with lots of legible structure across the lower middle. The foreground room frames it at the edges and bottom with dark wood, a soft curtain, a small lamp, plants and the corner of a bed or desk. A thin railing or balcony sill sits low so the city remains visible. No person. Low sunset and layered painterly clouds over the city. Raspberry, coral, apricot and honey near the horizon, cooler indigo, slate blue and lavender in the shadows, and deep petrol blue PCB substrate.

Make the city clearly recognizable as a PCB at city scale: roads literally are metallic copper traces with paired lines, right-angle and 45-degree bends, visible circular via-pad roundabouts, solder pads and converging perspective. Dedicated large IC and CPU buildings have black ceramic housings, gold contact pins visibly reaching the streets on several sides and a few amber windows. Some buildings are cylindrical electrolytic capacitors with brushed metallic scored tops and colored dark sleeves. More small rectangular components, pins and pads recede into atmospheric haze. Keep wide contiguous portions of the blue circuit board ground visible between the foreground structures so the trace-as-street idea reads immediately. Show depth with three coherent scales of urban blocks. Every part of the city should feel integrated into the electronics motif. Keep copper traces and lights softly warm with a few turquoise accents.

Polished hand-painted lo-fi anime environment art, clean and varied linework, detailed but breathable architecture, warm sunset rim light on buildings and plants, subdued analog texture. Calm ambient composition suitable behind an app, with gentle color transitions and highlights short of white clipping. No text, logos, watermark, borders, letterboxing, decorative interface, full-screen neon or photorealistic render.
```

</details>


<details>
<summary>Prompt da revisão da cidade e do pôr do sol</summary>

```text
Use the first supplied image as the exact edit target. Create a polished revision of this existing lo-fi anime Circuito desktop wallpaper. Preserve the 16:9 framing, still camera, room and plants, open balcony, railing, sunset palette and existing city perspective. Leave the major processor at lower center-right, the copper streets attached to it, the near road junctions and the positions of the large capacitors visible in their original places. Preserve the general cloud bank positions for later gentle movement.

Enrich the city itself, concentrating in the unobscured middle distance and across both sides of the main processor: show more believable PCB component variety at architectural scale. Add a few tall rows of RAM-like modules with gold edge contacts, ceramic resistors with subtle colored bands, silvery ferrite chokes and copper inductor coils, heat-sink towers with visible cooling fins, tiny solder pads and surface-mounted components, white silkscreen outlines, and occasional turquoise board details. Leave visible open substrate and coherent traced routes between defined districts. The silhouettes must read as actual electronics in perspective, legible and varied, and sustain the cozy hand-painted city environment. Keep sunset lighting atmospheric and ordinary, with believable shading and tiny amber window lights; no giant labels, no decals with words, and no busy techno neon.

Refine the sun in place above the same horizon. Give it a more convincing warm-white center, subtle amber gradient at the rim, a smooth atmospheric halo and a cloud wisp softly backlit in front of it. Let the sunlight illuminate the cloud edges with early-evening pink and apricot. Add a believable broken path of specular sunlight reflected toward the viewer, limited to the appropriate surfaces and angles: small glints on copper traces, the brushed top of the large CPU under the sun, rounded capacitor rims and a warm narrow highlight on the balcony sill. Reflections follow individual material surfaces and perspective, with a smooth falloff, breathable dark shadows, natural soft bloom and no flat light veil across the room. Keep the hand-painted lo-fi art and balanced highlights. No camera motion, people, text, watermark, or letterboxing.
```

</details>

## Controles com túnel em perspectiva

O preset 2 usa o movimento de `uhd_30fps.mp4` como referência.
Triângulos, círculos, cruzes e quadrados crescem à medida que se aproximam
da câmera, com rotação suave e rastros de luz. A cena conserva a paleta
violeta, rosa e azul do fundo anterior.

`python scripts/wallpaper-video/render.py --only 2` gera o WebM VP9
de 20 s, em 1600×900 a 30 fps, e o pôster. O vídeo ocupa 2.630.268 bytes
com quantização fixa de 166. O catálogo ocupa 14.837.611 bytes. A
diferença média por canal na emenda decodificada ficou em 9,49, ante
7,92 entre os dois primeiros quadros, na escala de 0 a 255.

## Floresta 8-bit com parallax

O preset 3 usa a rolagem lateral do vídeo de referência como modelo.
A composição em pixel art usa uma grade de 400×225 e escala de
vizinho mais próximo. A paleta conserva o céu azul-petróleo, os verdes
da floresta e a luz dourada do pôr do sol. Nuvens, montanhas, pinheiros
e margem usam faixas repetíveis com períodos espaciais diferentes.
Ondulações, reflexos e vaga-lumes animam a superfície do lago.

`python scripts/wallpaper-video/render.py --only 3` gera o WebM VP9
de 20 s, em 1600×900 a 30 fps, e o pôster. O vídeo ocupa 939.826 bytes
com quantização fixa de 158. O catálogo ocupa 14.929.340 bytes.
Em 0,6 s, os recortes do vídeo decodificado registraram deslocamento
de 32 px nas nuvens e 80 px na margem. A diferença média por canal
na emenda foi de 3,40 na escala de 0 a 255.

## Espaço com nebulosa e grade

O preset 5 usa `spaço.mp4` como referência para a composição e o
movimento. A nebulosa violeta e azul é desenhada proceduralmente.
Estrelas cintilam, feixes horizontais deslizam perto do horizonte e
a grade avança devagar em perspectiva.

`python scripts/wallpaper-video/render.py --only 5` gera o WebM VP9
de 20 s, em 1600×900 a 30 fps, e o pôster. O vídeo ocupa 1.865.888 bytes
com quantização fixa de 112. O catálogo ocupa 14.649.524 bytes. A
diferença média por canal na emenda decodificada ficou em 1,43 na escala
de 0 a 255.

## Fundo ReEmu em 2026-11-23

O preset 6 foi recriado a partir do movimento e da profundidade vistos no
vídeo `partc.mp4`. A composição procedural usa partículas finas, névoa e
círculos desfocados em verde `#3DDC2F`, azul `#1E8BFF` e ciano `#27B8C9`,
as cores do [símbolo do ReEmu](../../branding/reemu-mark.svg).

`python scripts/wallpaper-video/render.py --only 6` gera o WebM VP9 sem
áudio e o pôster em 1600×900. O vídeo tem 20 s a 30 fps e ocupa
3.181.369 bytes. A diferença média por canal na emenda decodificada foi
de 1,00 na escala de 0 a 255. O catálogo ocupa 12.996.006 bytes.

## Fliperama em 2026-11-23

O preset 4 passa por uma estrada vazia entre montanhas em grade em direção
a uma cidade em silhueta à frente do sol. O sol listrado e a perspectiva
vêm do vídeo de referência enviado pelo usuário. As referências complementares foram a composição de
estrada e palmeiras de Bangjey e o horizonte com montanhas geométricas de
Alp K. Şenlik. A cena do ReEmu é desenhada proceduralmente.
[Estrada ao pôr do sol](https://elements.envato.com/retro-sunset-palm-trees-highway-video-7GTJL3Q) ·
[Retrowave 3D Environment](https://www.behance.net/gallery/247011545/Retrowave-3D-Environment)

O [gerador](../../scripts/wallpaper-video/render.py) exporta um WebM VP9
de 1600×900 a 30 fps, sem áudio, com duração de 30 s. Rode
`python scripts/wallpaper-video/render.py --only 4` para gerar o vídeo e
seu pôster a partir da cena Canvas 2D. O vídeo ocupa 3.506.534 bytes e
passa de 86,4 para 43,2 megapixels de vídeo por segundo em relação ao
piloto a 60 fps. A diferença média por canal na emenda decodificada ficou
em 2,22 numa escala de 0 a 255 com quantização fixa de 142. A mesma
codificação com taxa de bits variável ocupou 4.225.807 bytes e teve
diferença de 2,96 na emenda. Esses dados descrevem o vídeo e a emenda.
O consumo de CPU e GPU depende do decodificador e do equipamento.
[VP9 no WebCodecs](https://www.w3.org/TR/webcodecs-vp9-codec-registration/)

O player da proteção de tela completou uma volta no Chromium local com
backend simulado. Em 32,5 s, o contador do navegador registrou 971
quadros, com zero descartes. O intervalo medido entre os quadros
apresentados na virada foi de 66,7 ms. Na tela inicial, a preferência de
movimento reduzido trocou o vídeo pelo pôster. A soma dos seis vídeos e
dos pôsteres ficou em 13.035.734 bytes.
[Media Playback Quality](https://w3c.github.io/media-playback-quality/)

A revisão do asfalto adicionou halos rosa e ciano no contorno e
reflexos quentes em faixas móveis, desenhados no vídeo e no pôster.
Com o mesmo perfil de 30 s a 1600×900 e 30 fps, o vídeo atualizado
ocupa 3.559.134 bytes. A diferença média por canal na emenda é 2,34
(escala 0–255) e o conjunto ocupa 13.095.525 bytes.

A comparação com os quadros da referência levou a troncos curvos, copas
assimétricas com folhas arqueadas e uma cidade em silhueta atrás das
montanhas. Esses detalhes são desenhados na geração do vídeo e do pôster.
Com o mesmo perfil de vídeo, o WebM passou a 3.291.651 bytes e a diferença
média por canal na emenda ficou em 2,21. O conjunto passou a ocupar
12.825.855 bytes.

A revisão seguinte retirou as palmeiras e destacou a cidade diante do
sol. A silhueta combina bairros de fundo, torres, antenas e
janelas discretas, com névoa na base. A quantização 134 preserva mais
detalhes da cidade que a 142. O WebM ocupa 2.905.837 bytes e a diferença
média por canal na emenda é de 1,66. O conjunto ocupa 12.428.032 bytes.

A revisão com `testsca.mp4` ampliou o disco fixo atrás da cidade e
adicionou um degradê de amarelo quente a rosa, um halo magenta e uma
faixa de luz azul no horizonte. As scanlines da estética CRT ficam
fixas no quadro e perdem intensidade do céu para a pista. Os efeitos
fazem parte do vídeo e do pôster gerados pelo mesmo comando.
O WebM ocupa 2.868.754 bytes e o catálogo ocupa 14.892.663 bytes.
A diferença média por canal na emenda decodificada é de 2,20
na escala de 0 a 255.

A malha das montanhas ganhou linhas mais espessas, em rosa à esquerda
e azul à direita, que suavizam em direção ao horizonte. A quantização
fixa 174 mantém o WebM em 2.847.133 bytes e o catálogo em 14.908.226
bytes. A diferença média por canal na emenda decodificada ficou em
4,01, ante 2,69 entre os dois primeiros quadros, na escala de 0 a 255.

## Comparação de 900p e 1080p a 30 fps em 2026-11-23

Fliperama (30 s) e ReEmu (20 s) foram exportados em 1920×1080 com a
mesma cena, duração, codec VP9 e quantização de cada versão em 1600×900
(134 e 88, respectivamente). O 1080p deixou mais nítidas as linhas finas
do Fliperama e as partículas menores do ReEmu nos recortes em escala 1:1.

Foi medida uma rodada de 60 s por versão na prévia da proteção de tela do
ReEmu, em perfil de teste isolado, com WebView2 157, Ryzen 7 3700X e
RTX 3060 (driver 32.0.16.2124). A área útil foi de 1280×749 pixels CSS.
As versões foram fornecidas ao mesmo player via `Blob`. A CPU corresponde
à participação do ReEmu e seus processos WebView2 na capacidade total da
máquina. Os dados de GPU são médias de 15 amostras dos contadores dos
motores 3D e Video Decode associados a esses processos.

| Fundo | Resolução | WebM | CPU | GPU 3D | GPU Video Decode | Quadros descartados | Maior intervalo na emenda |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Fliperama | 900p | 2.905.837 B | 0,51% | 1,07% | 4,47% | 3 de 1.799 | 66,9 ms |
| Fliperama | 1080p | 3.703.142 B | 0,62% | 2,93% | 6,33% | 3 de 1.801 | 83,6 ms |
| ReEmu | 900p | 3.181.369 B | 0,55% | 1,80% | 4,93% | 3 de 1.799 | 67,0 ms |
| ReEmu | 1080p | 4.051.635 B | 0,55% | 3,47% | 6,27% | 2 de 1.800 | 33,5 ms |

Ambas as amostras 1080p ficaram abaixo de 1% de descarte. O ReEmu ficou
dentro da meta de 67 ms nas três viradas. O Fliperama excedeu a meta em
uma das duas viradas. Substituir os dois pares de vídeo e pôster levaria
o catálogo a 14.750.308 bytes. Distribuir as duas resoluções de ambos
levaria o total a 21.049.209 bytes. O perfil de distribuição é 900p.
Ainda falta medir GPU integrada, navegação e abertura de jogos com 1080p.
[Contadores de reprodução](https://w3c.github.io/media-playback-quality/)

## Piloto implementado em 2026-11-20

No piloto, Fliperama usou uma cena procedural do Three.js renderizada
fora do app. O gerador produziu um pôster e um WebM VP9 de 1600×900,
60 fps, 20 s e sem áudio. O vídeo ocupava
2.626.927 bytes. O papel de parede e a proteção de tela do ReEmu reproduzem esse
arquivo pelo player de vídeo.

O primeiro piloto usava 30 fps e 12 s. Na comparação dos quadros
decodificados, a diferença média por canal RGB na emenda caiu de 2,74
para 1,26 numa escala de 0 a 255. O ajuste usa quantização fixa na geração
para manter a qualidade mais uniforme. O fluxo nominal passou de 43,20
para 86,40 megapixels/s.

Em **Configurações › Aparência › Proteção de tela**, o usuário escolhe um
preset e um intervalo de inatividade ou abre uma prévia. Durante a
proteção, o player do fundo normal é desmontado. O primeiro comando de
retorno fecha a proteção sem acionar a interface atrás. O player usa
pôster com redução de movimento ou no Linux. A proteção funciona dentro
da janela do ReEmu e fica fora da rota de jogo.

### Medição do piloto no WebView2 em 2026-11-20

ReEmu desktop de desenvolvimento em sessão isolada, com Ryzen 7 3700X
(16 processadores lógicos), GeForce RTX 3060, driver 32.0.16.2124 e
WebView2 157.0.4341.60. Tela 1366×768 a 59 Hz e área do app de 1281×720
em pixels CSS. Foram duas rodadas alternadas de 24 s por condição, com
a prévia em foco e apenas o player da proteção de tela em reprodução.
Os dois vídeos usam VP9 a 1600×900. A referência de 30 fps foi carregada
no mesmo elemento de vídeo.

| Proteção de tela | CPU do ReEmu e WebView2 (% do total da máquina) | GPU 3D média | GPU Video Decode média (pico) | FPS apresentados | Quadros descartados |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pôster estático | 0,02% | 0% | 0% (0%) | — | — |
| Fliperama a 30 fps, 12 s | 0,44% | 0,98% | 4,48% (7%) | 30,00 | 2 de 1.441 (0,14%) |
| Fliperama a 60 fps, 20 s | 0,67% | 2,85% | 9,07% (12%) | 59,25 | 34 de 2.876 (1,18%) |

O painel Media registrou `D3D11VideoDecoder` com decodificação de
plataforma para ambos os vídeos. O loop de 20 s reiniciou nas duas
rodadas. O player registrou um evento `waiting` por volta em ambos
os vídeos. A duração dessa pausa não foi medida. O teste de consumo
cobre a proteção de tela nessa máquina. O fundo normal foi conferido
carregando o vídeo de 20 s. A comparação com a referência a 1080p/24
e com GPU integrada segue nas etapas abaixo.
[Media Playback Quality](https://w3c.github.io/media-playback-quality/) ·
[Callback de quadro de vídeo](https://wicg.github.io/video-rvfc/) ·
[Painel Media do WebView2](https://learn.microsoft.com/en-us/microsoft-edge/devtools/media/)

## Atualização dos demais fundos em 2026-11-20

Cinco cenas foram recriadas no gerador Canvas 2D. Cada uma usa VP9 a
1600×900, 30 fps e 20 s. Desenho e codificação acontecem fora do app.
O perfil de 30 fps passa de 49,77 para 43,20 megapixels de vídeo por
segundo frente à referência de 1080p/24. O teto para distribuição é
15 MB decimais. A soma dos seis WebM e seus pôsteres passou de 14,81
para **12,13 MB**. Naquele momento, Fliperama ainda usava o perfil de 60 fps.

| Fundo | Mudança de arte e movimento | Emenda RGB antes → depois |
| --- | --- | ---: |
| Circuito | Placa com processador e contatos, LEDs graduais e pulsos que desaparecem no fim das trilhas | 2,235 → 0,538 |
| Controles | Gamepad flutuante com contorno iluminado, comandos orbitais e cor suave nos botões | 2,169 → 0,619 |
| Floresta 8-bit | Camadas de pixel art em paralaxe, pinheiros, lago, reflexos e vaga-lumes | 1,843 → 0,788 |
| Espaço | Nebulosa, anéis, faixas atmosféricas, lua em órbita e meteoros que entram e saem em fade | 1,511 → 0,554 |
| ReEmu | Símbolos com relevo luminoso, grade em perspectiva e transição gradual das linhas | 1,912 → 0,759 |

A distância na emenda é a diferença absoluta média por canal RGB
(0 a 255) entre o último e o primeiro quadro decodificados e
comparados a 1600×900. Ela caiu de 57% a 76% nas cinco cenas.
A quantização fixa é 64 para Circuito, Floresta 8-bit e Espaço,
88 para Controles e ReEmu e 96 para Fliperama.
[Gerador](../../scripts/wallpaper-video/render.py) ·
[Registro VP9 no WebCodecs](https://www.w3.org/TR/webcodecs-vp9-codec-registration/)

### Comparação na proteção de tela

Na mesma máquina do piloto, foi feita uma rodada de 22 s por vídeo,
depois do aquecimento, com a janela em foco. Os originais a 1080p/24 foram
carregados no mesmo elemento de vídeo que os novos a 900p/30. O WebView2
usou `D3D11VideoDecoder`. Cada prévia rodou com um vídeo ativo e completou
uma volta sem erro. Os novos apresentaram 29,96 fps e descartaram de
0,15% a 0,45% dos quadros. A CPU abaixo soma os processos do ReEmu
e WebView2 como porcentagem da CPU total da máquina.

| Fundo | CPU antes → depois | GPU 3D média antes → depois | GPU Video Decode média antes → depois |
| --- | ---: | ---: | ---: |
| Circuito | 0,35% → 0,52% | 3,11% → 1,00% | 4,78% → 4,78% |
| Controles | 0,36% → 0,35% | 3,22% → 1,00% | 4,28% → 4,33% |
| Floresta 8-bit | 0,41% → 0,49% | 2,56% → 1,00% | 4,22% → 4,53% |
| Espaço | 0,38% → 0,33% | 3,17% → 1,00% | 4,89% → 4,72% |
| ReEmu | 0,35% → 0,50% | 3,28% → 1,00% | 5,17% → 4,72% |

A medição cobre a prévia da proteção de tela na RTX 3060. A validação
em GPU integrada, telas de maior resolução, navegação e abertura de
jogos permanece nas etapas abaixo.

## Avaliação dos arquivos antes do piloto

Os seis WebM embutidos têm uma faixa de vídeo VP9 sem áudio, 1920×1080,
288 quadros em 12 s e quadros-chave em 0, 4 e 8 s. As marcas de tempo
avançam 41 ou 42 ms, como esperado a 24 fps com resolução de 1 ms. Os
vídeos somavam **13,96 MB** e os seis pôsteres somavam **0,53 MB**
(MB decimais).

Foram conferidos os contêineres e os pôsteres. Também foram comparados os
quadros 286, 287, 0 e 1 das cenas no Canvas e dos WebM decodificados em
Chromium headless. A tabela mostra a diferença absoluta média por canal
RGB de 8 bits. Para os vizinhos, foi usada a maior diferença entre
286→287 e 0→1. O teste mede a imagem da emenda. O tempo de apresentação
pelo WebView2 e o consumo foram comparados na seção de atualização.

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

- [x] Atualizar a grade de Fliperama, os contornos de Controles e as
  outras quatro cenas conforme a comparação do catálogo.
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

- [x] Aplicar os perfis às seis cenas mantendo os IDs, medir o pacote
  final e conferir as prévias no WebView2 com GPU dedicada.
- [ ] Repetir os testes em GPU integrada e telas de maior resolução.
  Validar contraste na Início e em Configurações e medir o pacote do instalador.
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
