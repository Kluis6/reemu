# 18 — Menus confortáveis para tela grande e controle, na TV e no PC

**Estado: planejado (revisado em 2026-10-30). Etapa 0 feita, a partir de vídeos em
360p.** Nada deste plano foi implementado ainda.

## Objetivo

O frontend atual fica, com ajustes. O objetivo é deixá-lo confortável em tela grande
e com controle, sem copiar o dashboard do console.

Regras do dono do projeto (revisão de 2026-10-30, substitui o espelhamento completo
do Xbox que estava aqui antes):

- **Layout da aplicação: app Xbox para Windows**, como já é hoje (barra lateral, busca
  no topo, fileiras).
- **Menus: as configurações do console Xbox são o modelo**, por serem as mais
  confortáveis com controle.
- **O card mantém o comportamento atual:** não cresce em layout nem empurra vizinhos.
- **Cada etapa fica só local.** Commit e push depois que o dono do projeto testar e
  aprovar aquela etapa.
- Alguns usuários seguem no PC, com teclado e mouse, em telas menores que uma TV.
- Sem atalho de teclado para trocar as abas do menu de pausa (pedido anterior).
- Avisos não aparecem durante o jogo: vão para o sino.
- Usar o comportamento como referência, não a marca: sem logotipos nem identidade do
  Xbox.

## Fontes e o quanto valem

- **Microsoft, diretrizes de TV e controle:** a base mais sólida para
  ergonomia. São de 2020 e escritas para UWP, então valem como referência, não
  como regra atual do console.
  - [Designing for Xbox and TV](https://learn.microsoft.com/en-us/windows/apps/design/devices/designing-for-tv)
  - [Gamepad and remote control interactions](https://learn.microsoft.com/en-us/windows/apps/design/input/gamepad-and-remote-interactions)
- **Xbox Wire, Home do console:**
  [redesenho de 2023](https://news.xbox.com/en-us/2023/07/26/welcome-to-your-new-xbox-home/)
  e [personalização de 2025](https://news.xbox.com/en-us/2025/05/16/xbox-insiders-get-new-ways-to-personalize-home/).
  Não achei mudança da Home em 2026.
- **Suporte do Xbox, guia:**
  [uso](https://support.xbox.com/en-IN/help/hardware-network/getting-started-set-up/using-the-guide)
  e [abas](https://support.xbox.com/en-AU/help/hardware-network/getting-started-set-up/customize-guide-tabs).
  As páginas não abriram na ferramenta: o conteúdo veio dos resultados de busca.
- **Xbox mode no Windows:**
  [PCWorld](https://www.pcworld.com/article/3098980/xbox-fullscreen-experience-is-now-xbox-mode-for-windows.html)
  e [Thurrott](https://www.thurrott.com/games/xbox/329563/hands-on-windows-11-full-screen-experience).
  Análises de terceiros.

**Sem fonte escrita:** os valores de elevação e de animação do console e os atalhos
do teclado virtual dele. A etapa 0 mede o que os vídeos permitem.

## Xbox × ReEmu (auditoria de 2026-10-30)

| Tema | Xbox | ReEmu |
|---|---|---|
| Início | Fundo grande, que pode seguir o jogo em destaque; recentes embaixo; acesso rápido no topo | Carrossel de 288 px com troca automática; "Continuar" depois de "Adicionados" |
| Menu global | Botão Xbox abre o guia; LB/RB trocam as abas | Barra lateral, sino e drawer de perfil separados |
| Botões | A, B, Y busca, Menu opções; LT/RT paginam; View abre a navegação | LB/RB só no menu de pausa; LT, RT e View sem uso |
| Foco | Sempre visível; começa no item principal | Sem memória ao voltar; Início e Configurações sem foco inicial |
| Tooltips | Evitar na TV | Ações principais só com ícone e tooltip |
| Biblioteca | Pesquisa no Y; filtros e fixados | Filtros por menus; sem pular páginas nem letras |
| Som | Parte do retorno na TV | Nenhum som nem vibração |
| Elevação | Camada escura atrás de painéis; foco inconfundível | Card sem sombra nem elevação (só a imagem cresce 4,5%); painéis com sombra padrão |

### Tamanhos em 1080p (TV)

| Item | Microsoft | ReEmu |
|---|---|---|
| Texto principal | 30 px (15 epx a 200%) | ~20 px |
| Texto secundário | 24 px (12 epx a 200%) | ~17 px |
| Altura mínima de controle | 64 px (32 epx a 200%) | ~45 px |
| Margem segura | 5% | 3,4% na horizontal, 4,2% na vertical; barra lateral colada à borda |

### Telas menores no PC

`lib/uiScale.ts` mantém uma tela lógica de 1366×768 e encolhe tudo por zoom,
então a janela menor não reorganiza nada: só reduz o texto (valores calculados
pela regra do código).

| Janela (tamanho lógico) | Zoom | 14 px vira | 12 px vira |
|---|---|---|---|
| 1920×1080 | 1,41 | 19,7 px | 16,9 px |
| 1536×864 (1080p a 125%) | 1,13 | 15,8 px | 13,5 px |
| 1366×768 | 1,00 | 14 px | 12 px |
| 1024×640 | 0,75 | 10,5 px | 9 px |
| 960×1040 (meia tela) | 0,70 | 9,8 px | 8,4 px |
| 800×600 (mínimo do app) | 0,59 | 8,2 px | 7 px |

Medido com o zoom desligado e sem texto maior (`ui-smoke.py`, `ui-render.py`):

- Início, Biblioteca e jogo em 1024×640 e 800×600: sem rolagem horizontal. No
  Início, 5 elementos ficam fora da tela nos dois tamanhos, provavelmente slides
  do carrossel (não confirmado).
- Configurações (8 telas): só `/settings/video/shaders` quebra em 800×600.
- Início em 800×600: o carrossel ocupa quase metade da altura.

## Decisões

- **Mesmos layout e componentes, com dois perfis de densidade (TV e PC).**
  Nasceu como "uma interface só", mas o texto que serve para a TV fica grande
  demais num monitor.
- **Barra lateral fica:** segue o modelo do app Xbox para Windows e foi pedida
  pelo dono do projeto. O acesso rápido do console fica de fora.
- **Comportamento primeiro, visual depois:** as etapas 1 e 2 sustentam o resto.
- **View abre a barra lateral**, levando o foco até ela. O guia global saiu do plano.

## Etapas

### 0. Referência do Xbox

**Primeira leitura feita em 2026-10-30**, com os 4 vídeos locais de `videosxbox/`
(fora do git): console em versões recentes e Insider, e o app Xbox no Windows.

**Limites:** vídeos do YouTube em 360p, editados e a ~25 quadros por segundo. As
medidas são porcentagem da tela, com erro de cerca de 1 ponto, e os tempos têm
erro de ±40 ms. O tempo de abertura do guia e dos menus não pôde ser medido, porque
os cortes da edição escondem a transição. Sombra fina não aparece nessa resolução.

**Medido na Home do console:**

| Item | Valor |
|---|---|
| Fileira de recentes | de 59% a 73% da altura |
| Cards da segunda linha | de 77% a 97% da altura; ~21% de largura |
| Margens laterais | ~5,5% |
| Card normal | ~8% de largura × ~14% de altura |
| Card em foco | ~13,5% × ~24%, ou seja, **~1,65× maior** |
| Como cresce | **Empurra os vizinhos e sobe**, com a base quase fixa; não é sobreposição |
| Conteúdo do card em foco | Só nele aparecem título e estado |
| Tempo da troca de foco | **~120 a 160 ms**, com o card antigo encolhendo junto |
| Pulo longo com reordenação | ~170 ms no foco; cards entram com cor sólida e a imagem surge em ~300 a 500 ms |

**Observado:**

- **Home:** fundo em tela cheia; o conteúdo só ocupa os 40% de baixo. No topo, perfil
  à esquerda, atalhos em ícones no centro e status e relógio à direita.
- **Foco:** contorno na cor de destaque do usuário (verde, laranja, azul), com leve
  brilho. Os botões de lista e configurações seguem a mesma cor; no menu de contexto
  o contorno aparece branco.
- **Dica contextual:** só a ação do item em foco, numa pílula (ex.: `X Ejetar`).
- **Guia:** painel flutuante à esquerda, com ~28% de largura, separado das bordas.
  Abas só com ícones no topo, tela escurecida atrás, relógio grande à direita.
  Painéis podem abrir por cima, como o de perfil.
- **Menu de contexto:** painel centralizado, ~23% de largura, com o nome do jogo no
  topo, ícone mais texto em cada item e o primeiro item em foco; fundo escurecido.
- **Biblioteca e configurações:** coluna à esquerda de ~23% com ícone e texto, contagem
  e item ativo marcado; conteúdo à direita. Nas configurações, o item ativo é
  preenchido com a cor de destaque.
- **Teclado virtual:** painel flutuante de ~52% de largura, centralizado embaixo,
  deixando ver o campo. Tem sugestões, números na grade, setas de cursor com ícone de
  gatilho e os ícones X em apagar e Y em espaço.
- **App Xbox no Windows:** barra lateral, busca no topo, `Jump back in`, `Featured` e
  fileiras. Em tela muito larga o conteúdo fica centralizado, com largura limitada. A
  biblioteca tem abas e filtros em pílulas. Há um botão `Xbox mode` e um diálogo que
  explica os controles ao entrar nesse modo.

**O que vale para o plano:** só o que se refere a menus (guia e Home ficam como
registro). O modelo principal são as configurações do console: coluna de categorias à
esquerda, item ativo preenchido com a cor de destaque, opções em blocos grandes à
direita, descrição junto da opção e rótulos sempre visíveis. Menu de contexto e teclado
virtual seguem as observações acima. A elevação de menus vem de contorno e fundo
escurecido, sem sombra forte.

### 1. Tirar os becos sem saída

Os itens "(confirmar)" são deduções da leitura do código: começar por um roteiro
com controle.

- B no sino ou num menu aberto volta a página por trás (`useMenuNav.ts`).
- B no Início também tenta voltar o histórico.
- O foco pode sair de modais abertos, porque `focusNav.ts` busca no documento
  inteiro (confirmar).
- Sliders não mudam de valor pelo controle: matiz, HDR, parâmetros de shader
  (confirmar).
- Seletores nativos podem não abrir, como o de core (confirmar).
- Menu de contexto pode abrir no canto da tela (confirmar).
- Campos numéricos não abrem o teclado virtual, que também não tem `@`, `\` e `_`.

**Pronto quando:** todas as telas e modais funcionarem só com o controle.

### 2. Mapa único de botões e foco

- LB/RB trocam abas em toda parte; LT/RT paginam; View leva o foco à barra lateral.
- Memória de foco ao voltar e foco inicial definido em toda tela.
- Dicas de botão conforme o item em foco, inclusive em diálogos, com ícones de
  LB, RB, LT e RT.
- O visual segue a última entrada usada:
  - **Controle:** esconde cursor, hover e botão Voltar.
  - **Mouse:** cursor, hover e Voltar visíveis; tooltips mantidos; botão direito
    liberado para os menus do app (hoje bloqueado em `main.tsx`); setas e roda do
    mouse nas fileiras.
  - **Teclado:** Esc volta, Enter confirma, setas navegam, foco visível.

**Pronto quando:** voltar de um jogo cair no mesmo card, e a dica na tela sempre
corresponder ao que o botão faz.

### 3. Dois perfis de densidade

| | Perfil TV | Perfil PC |
|---|---|---|
| Tamanho | Tela lógica fixa, texto na medida da TV | Segue o DPI do Windows |
| Janela menor | Não se aplica | Reorganiza a tela, sem encolher o texto |
| Texto principal | ~30 px em 1080p | 14 px, com piso de 12 px |
| Margem segura | 5%, ajustável | Só a margem normal |

- **Escolha:** automática, com opção manual no onboarding e em Configurações. O
  automático usa TV quando o app abre em tela cheia com controle.
- **Sem troca no meio do uso:** muda só ao abrir o app ou sair da tela cheia.
- **Limite:** o app não tem como saber com certeza se a tela é uma TV.
- Barra lateral com rótulos ao receber foco, na TV.

Pontos de quebra do perfil PC (aplicados nas etapas 3 a 7):

- Abaixo de ~1100 px: cards menores e carrossel/fundo mais baixos.
- Abaixo de ~900 px: barra lateral só com ícones, linhas de configuração
  empilhadas.
- Largura máxima de leitura em ultrawide, nas telas de texto e formulário.

**Pronto quando:** a leitura estiver confortável numa TV real, a cerca de 3 m, em
tema claro e escuro, e nos tamanhos de PC abaixo.

### 4. Configurações no modelo do console

- Coluna de categorias sempre visível à esquerda, em vez de voltar ao hub para trocar de
  seção; item ativo preenchido com a cor de destaque.
- Opções em blocos grandes, com descrição, dois ou três por linha; subpáginas com o mesmo
  padrão.
- Sliders e seletores que funcionem com controle.
- No perfil PC, a mesma estrutura, mais compacta, com a coluna recolhendo em janela
  estreita.

### 5. Demais menus

- Menu do jogo (botão Menu no card): centralizado, com o nome do jogo no topo, ícone e
  texto em cada item, primeiro item em foco e fundo escurecido.
- Menu de pausa, drawer de perfil, sino e diálogos: mesmos tamanhos, foco, botões e
  fundo escurecido.
- Dicas de botão só com ações do item em foco.

- **Mensagens de erro (pedido do dono do projeto, 2026-10-30):** redesenhar a tela de
  erro ao abrir um jogo e os avisos de erro, seguindo o padrão dos menus.

### 6. Biblioteca e página do jogo

Sem mudar o layout. Biblioteca: pular por letra, aviso de busca ativa e lista otimizada
para acervos grandes. Jogo: "Jogar" como foco inicial, nomes visíveis nas ações e
confirmação ao apagar save state. Texto longo: rolagem pelo analógico direito.

### 7. Entrada de dados

- Navegador de arquivos dentro do app: as janelas do Windows não funcionam com controle.
- Teclado virtual flutuante, deixando o campo visível, com cursor, sugestões e layout
  conforme o idioma.

### 8. Acabamento e identidade

Sons e vibração com opção de desligar, animações consistentes, opção de reduzir
movimento, revisão de todos os temas numa TV.

## Fora do plano (decisão de 2026-10-30)

- Card que cresce e empurra os vizinhos: o card atual fica.
- Home no estilo do console: fundo em tela cheia e conteúdo nos 40% de baixo. O Início
  atual fica.
- Guia global: barra lateral, drawer de perfil e sino continuam como estão, só ganhando
  os ajustes da etapa 5.

## Como aprovar cada etapa

1. A etapa é implementada só localmente, sem commit.
2. O dono do projeto recebe o roteiro de teste e testa.
3. Com a aprovação, commit e push daquela etapa. Só então começa a próxima.

## Como testar cada etapa

- `teste-visual` (`scripts/ui-render.py`) e `scripts/ui-smoke.py`, na porta 1430.
- Tamanhos: 800×600, 1024×640, 1366×768, 1920×1080 e ultrawide.
- Entradas: mouse, teclado e controle. Temas: pelo menos um claro e um escuro.
- A etapa 3 também precisa de teste numa TV real.
- Mudança visual: consultar a documentação do Fluent 2 antes. Texto novo: i18n
  em pt-BR, en e es.
