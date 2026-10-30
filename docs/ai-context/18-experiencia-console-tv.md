# 18 — Experiência de console: referência do Xbox, na TV e no PC

**Estado: planejado (2026-10-30). Próximo passo: etapa 0, que depende das
capturas do console.** Nada deste plano foi implementado ainda.

## Objetivo

O ReEmu deve se comportar como o dashboard do Xbox Series X|S quando usado na
TV com controle, e continuar confortável no PC com mouse e teclado.

Regras do dono do projeto:

- **Elevação, layout e interações espelham o Xbox** em todos os perfis. O PC só
  adapta tamanho e formas de entrada, não inventa outro comportamento.
- Alguns usuários seguem no PC, com teclado e mouse, em telas menores que uma
  TV: a interface precisa responder bem a isso.
- Sem atalho de teclado para trocar as abas do menu de pausa (pedido anterior).
- Avisos não aparecem durante o jogo: vão para o sino.
- Espelhar o comportamento, não a marca: sem logotipos nem identidade do Xbox.

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

**Não confirmado em nenhuma fonte:** os valores de elevação e de animação do
console e os atalhos do teclado virtual dele. A etapa 0 existe por isso.

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
  pelo dono do projeto. O acesso rápido no topo fica de fora, para não haver
  dois menus.
- **Comportamento primeiro, visual depois:** as etapas 1 e 2 sustentam o resto.
- **Guia abre pelo View.** O botão Xbox fica como extra, porque o Windows pode
  capturá-lo para a Game Bar.

## Etapas

### 0. Referência do Xbox

Medir em capturas e vídeos do console e do app Xbox: elevação, tempos de
animação, proporção dos cards e posição do foco. Fontes: gravação do dono do
projeto ou vídeos do Series X|S a partir de 2023, com a data conferida.
Resultado: uma tabela de valores nesta página, marcando o que é medição e o que
é estimativa.

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

- LB/RB trocam abas em toda parte; LT/RT paginam; View abre o guia.
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

### 4. Guia global

Painel com abas: Início e recentes, Notificações, Perfil e sistema (com desligar).
Substitui o popover do sino e o drawer de perfil, com o mesmo componente do menu
de pausa.

### 5. Novo Início

- Fundo em tela cheia, que segue o jogo em foco.
- Primeira fileira: "Continuar", com o jogo mais recente em destaque.
- Fileiras com rolagem de verdade e um card final "Ver tudo".
- Até 3 jogos fixados e opção de menos cards, como a Home de 2025.

### 6. Biblioteca e página do jogo

- Biblioteca: pular por letra, filtros em painel, aviso de busca ativa e lista
  otimizada para acervos grandes.
- Jogo: "Jogar" como foco inicial, ações secundárias no botão Menu e confirmação
  ao apagar save state.
- Texto longo: rolagem pelo analógico direito.

### 7. Configurações e entrada de dados

- Lista de seções sempre visível, sem voltar ao hub para trocar de categoria.
- Navegador de arquivos dentro do app: as janelas do Windows não funcionam com
  controle.
- Teclado virtual com cursor e layout conforme o idioma.

### 8. Acabamento e identidade

Sons e vibração com opção de desligar, animações consistentes, opção de reduzir
movimento, revisão de todos os temas numa TV.

## Elevação, layout e interações: o que espelhar

| Tema | Em todos os perfis |
|---|---|
| Elevação | Quatro níveis: fundo, card em foco, painel, modal com camada escura. Valores vêm da etapa 0 |
| Layout | Fundo do jogo, fileiras na mesma ordem, guia com abas, jogo em destaque |
| Interações | Mesma ação e mesma ordem em controle, mouse e teclado; B, Esc e Voltar sempre fazem o mesmo |

## Como testar cada etapa

- `teste-visual` (`scripts/ui-render.py`) e `scripts/ui-smoke.py`, na porta 1430.
- Tamanhos: 800×600, 1024×640, 1366×768, 1920×1080 e ultrawide.
- Entradas: mouse, teclado e controle. Temas: pelo menos um claro e um escuro.
- A etapa 3 também precisa de teste numa TV real.
- Mudança visual: consultar a documentação do Fluent 2 antes. Texto novo: i18n
  em pt-BR, en e es.
