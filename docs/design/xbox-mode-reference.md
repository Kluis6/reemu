# Referência de design — "Modo Xbox" (Windows 11 / Xbox full-screen)

O frontend do ReEmu (`packages/app-desktop`) segue a linguagem visual do **modo
Xbox** do app Xbox no Windows 11 e do dashboard do Xbox Series X. Os
screenshots de referência ficam em [`screenshots/`](screenshots/) (coloque os
`.png` lá — não são versionados por serem capturas de tela de terceiros; este
documento descreve o que cada padrão deve ser).

> **Marca:** usamos só o *layout* e as *interações*. Nada de logos, ícones ou
> tipografia da Microsoft/Xbox. O verde (`--xb-accent`) é usado com parcimônia.
>
> **Base de design:** a linguagem visual concreta (tokens, tipografia,
> elevação, Griffel) vem do **Fluent 2** — ver
> [`fluent2.md`](fluent2.md) e <https://fluent2.microsoft.design/>. Os estilos
> ficam em `packages/app-desktop/src/styles/xbox.ts` (Griffel `makeStyles` +
> `tokens`); o antigo `xbox.css` foi removido.

## Estrutura de telas (rotas)

- `/` → **Início** (`screens/Home`): hero + faixas curadas ("Continuar
  jogando", "Adicionados recentemente", …). É a home tipo modo Xbox — **não**
  é a biblioteca. Novas seções entram aqui.
- `/library` → **Meus jogos** (`screens/Library`): biblioteca completa, grade
  vertical por sistema, busca (Y), adicionar/remover ROMs e remover uma
  biblioteca inteira (`list_rom_sources`/`remove_rom_source`/`clear_library`).

---

## Escala (proporções do Xbox)

Fonte: Microsoft, [Designing for Xbox and TV](https://learn.microsoft.com/windows/apps/design/devices/designing-for-tv)
e [Gamepad and remote control interactions](https://learn.microsoft.com/windows/apps/design/input/gamepad-and-remote-interactions).

- O Xbox renderiza 1920×1080 a **200%**: a interface é desenhada em pixels
  efetivos (epx) numa tela lógica de **960×540** e escala por igual. O ReEmu
  faz o mesmo com o zoom nativo do webview (`lib/uiScale.ts`):
  `zoom = min(largura / 960, altura / 540)` → 1 px do CSS = 1 epx, zoom 2 em
  1080p, 4 em 4K. Proporções idênticas em qualquer resolução; telas mais
  largas (21:9) ou mais altas (16:10, 4:3) só ganham espaço na outra
  dimensão. **Nada de `vw`/`clamp()` pra tamanho de componente.**
- Métricas em `styles/metrics.ts`:
  - área segura da TV: 48 epx nas laterais, 27 em cima e embaixo;
  - alvo interativo ≥ 32 epx de altura (nada de `size="small"` em botão,
    `Select`, `Input` ou `TabList`);
  - texto principal ≥ 15 epx, secundário ≥ 12 epx (tokens `fontSizeBase300`
    e `fontSizeBase100` do tema sobem pra 15 e 12);
  - no máximo 6 cliques de borda a borda → 6 cards por fileira em 16:9
    (`CARD_W` ≈ 126 epx);
  - rampa tipográfica do Windows: Title 28, Subtitle 20, Body 15, Caption 12.
- Preferência do usuário (Aparência › Tamanho da interface) multiplica o
  zoom: Compacto 80%, Padrão 100%, Grande 115%, Maior 130%.
- Configurações seguem o Xbox: categorias numa lista vertical à esquerda.

## 1. Estrutura da tela

```
┌────┬──────────────────────────────────────────────────────────┐
│    │  ‹   [ 🔎  Pesquisar jogos…                    ]   ⏻  02:47 │  topbar
│ ▎🏠 ├──────────────────────────────────────────────────────────┤
│  📚 │                                                          │
│  ☁ │   ╔══════════════════ HERO ══════════════════╗            │
│  🎮 │   ║  imagem grande, 16:6, cantos 16px         ║           │
│  ⚙ │   ║  título + subtítulo embaixo-esquerda      ║           │
│    │   ╚══════════════════════════════════════════╝  • • • ○   │
│ ── │                                                          │
│  🔔 │   Continuar jogando  ›                                    │  shelf
│  ⏻ │   ┌──┐ ┌──┐ ┌──┐ ┌──┐ ┌──┐  ›                              │
│    │   └──┘ └──┘ └──┘ └──┘ └──┘                                 │
│    │                                                          │
│    │   SNES  ›                                                 │
│    │   subtítulo em cinza                                      │
│    │   ┌──┐ ┌──┐ ┌──┐ …                                        │
└────┴──────────────────────────────────────────────────────────┘
```

- **Rail** (`.xb-rail`, 64 epx, itens de 40): só ícones. Ativo = **barra vertical fina**
  (3 px) na borda esquerda do ícone + ícone branco; inativo = ícone cinza.
  Divisória (`border-top`) separando a navegação principal dos utilitários
  (notificações, sair). Ícone de marca (círculo "R") no topo.
- **Topbar** (começa na área segura de cima, controles de 32 epx): chevron
  *voltar*, campo de busca centralizado (360 epx), à direita utilitários +
  relógio `tabular-nums`.
- **Conteúdo**: rolagem vertical só aqui. Padding 24 epx depois da rail, 48
  (área segura) à direita, e embaixo área segura + barra de dicas.

## 2. Cores e tokens

| token | valor | uso |
|---|---|---|
| `--xb-bg` | `#0b0b0d` | fundo |
| `--xb-bg-elev` | `#17171b` | hover / superfícies |
| `--xb-bg-elev-2` | `#1f1f24` | placeholder de arte |
| `--xb-stroke` | `#2c2c33` | bordas 1 px |
| `--xb-text` | `#f3f3f4` | texto primário |
| `--xb-text-2` | `#a9a9b2` | texto secundário / ícones inativos |
| `--xb-accent` | `#6cc04a` | só realces pontuais |
| `--xb-focus` | `#ffffff` | anel de foco |

Fundo com **gradiente vertical** sutil no topo (`#121218 → #0b0b0d`) + um glow
radial fraco no canto sup-esquerdo. **Nada de `backdrop-filter`** nem
`radial-gradient` multicamada em elemento `position: fixed` (quebra o
WebKitGTC — ver `main.rs`).

## 3. Componentes

### Hero (`.xb-hero`)
Card grande, `aspect-ratio: 16/6` (ou altura fixa ~260 px), raio 16 px, imagem
`object-fit: cover`. Overlay `linear-gradient(90deg, rgba(0,0,0,.75), transparent 60%)`
pra legibilidade. Título (`clamp(20px, 2.4vw, 30px)`, peso 700) + subtítulo
cinza embaixo-esquerda. Se houver mais de um, **dots** centralizados abaixo.
No ReEmu o hero mostra o **último jogo jogado** ("Continuar") ou uma capa de
boas-vindas quando a biblioteca está vazia.

### Cabeçalho de seção (`.xb-section`)
`<h2>` + chevron `›` (o conjunto é clicável se a seção tiver uma tela própria) +
uma linha de **subtítulo em cinza** logo abaixo. Margem-topo ~28 px.

### Prateleira horizontal (`.xb-shelf`)
`display: flex; overflow-x: auto; scroll-snap-type: x`. Cartões com
`scroll-snap-align: start`. Um **chevron `›`** flutuante na borda direita
(gradiente de fade) que rola +1 página no clique. Sem barra de rolagem visível.
Cai pra **grade** (`.xb-grid`) quando a intenção é "ver tudo" (rota da seção).

### Cartão de jogo (`.xb-card`)
- Arte **retrato 3:4**, raio 12 px, `object-fit: cover`. Sem capa → iniciais
  grandes sobre `--xb-bg-elev-2`.
- Badge do sistema/plataforma: pílula pequena no canto inf-esq da arte
  (`rgba(0,0,0,.6)`).
- Abaixo: título (branco, 2 linhas máx) + subtítulo/estado em cinza
  (ex.: "Instalado", nome do core).
- Hover: `translateY(-3px)` na arte.

### Chips de filtro (`.xb-chip`)
Pílula (`--xb-stroke` 1 px, fundo `rgba(255,255,255,.04)`), texto + `▾`.
Linha de chips + contagem à direita (`N jogos`) + botão `+` (adicionar ROMs).

### Anel de foco
`outline: 3px solid #fff; outline-offset: 2px; border-radius: herda`. Tem que
aparecer forte em navegação por teclado/controle (`:focus-visible`).

### Barra de dicas de botão (`.xb-hints`)
Pílula flutuante no canto inf-direito: glifos A/B/X/Y coloridos (verde/vermelho/
azul/amarelo) + rótulo. `pointer-events: none`.

## 4. Estado vazio

Ícone/ilustração simples centralizada + título + 1 linha + um botão primário
("Adicionar ROMs…"). Nada de formulário grande jogado na cara.

## 5. Interação por controle

Toda a navegação funciona com d-pad/analógico + A (selecionar) + B (voltar) —
ver `hooks/useMenuNav.ts`. O foco é **sempre visível**. As prateleiras rolam
junto quando o foco chega na borda.
