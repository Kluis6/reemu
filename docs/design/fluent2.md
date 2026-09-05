# Fluent 2 — linguagem de design (referência)

**Site oficial:** <https://fluent2.microsoft.design/>
Documentação da lib React: <https://react.fluentui.dev/> · Griffel: <https://griffel.js.org/>

O ReEmu usa **`@fluentui/react-components` v9** (Fluent 2) no frontend
(`packages/app-desktop`). Este doc resume o que importa pra manter a UI
coerente com o Fluent 2 e explica por que o CSS deve migrar pra **Griffel**.

---

## Princípios do Fluent 2

- **Tokens, não valores fixos.** Cor, espaçamento, raio, sombra, tipografia e
  motion vêm de _design tokens_. Em código: `import { tokens } from
  '@fluentui/react-components'` → `tokens.colorNeutralBackground1`,
  `tokens.spacingHorizontalM`, `tokens.borderRadiusMedium`,
  `tokens.shadow16`, `tokens.fontSizeBase300`, etc. Trocar o tema (claro/
  escuro/high-contrast) reajusta tudo automaticamente.
- **Tema.** `FluentProvider` recebe um `theme`. O ReEmu tem temas **próprios**
  em `src/styles/themes.ts` (`createDarkTheme(rampa)` + neutros escurecidos
  `consoleDark` + tokens custom `reemu*`), escolhidos por `useThemeStore`
  (persistido em `localStorage`, lido de forma síncrona → sem flash). A raiz
  (`src/App.tsx`) aplica `THEMES[themeId].theme`. Nada de cor hardcoded fora
  dos tokens.
- **Ramp tipográfica.** Componentes `Title1/Title2/Title3`, `Subtitle1/2`,
  `Body1/Body2`, `Caption1/2` — não montar `font-size`/`font-weight` na mão.
- **Espaçamento em múltiplos de 4px** via `spacing*` tokens.
- **Elevação** por `shadow2/4/8/16/28/64` (sombra + leve borda), não
  `box-shadow` custom.
- **Motion** por `durationFast/Normal/Slow` + `curveEasyEase*`.
- **Acessibilidade primeiro.** Foco visível (o Fluent já entrega
  `:focus-visible` com anel do tema), alvos ≥ 24px, contraste AA, suporte a
  teclado/leitor de tela. High-contrast é um tema de primeira classe.
- **Densidade.** Fluent 2 é mais "arejado" que o Fluent 1 — respeitar o
  espaçamento dos tokens em vez de comprimir.

## Griffel (o CSS-in-JS do Fluent 2)

`makeStyles` / `mergeClasses` — atomic CSS gerado em build/runtime, com os
tokens acima. É o jeito idiomático de estilizar num app Fluent 2.

```ts
import { makeStyles, tokens, shorthands } from '@fluentui/react-components'

const useStyles = makeStyles({
  card: {
    backgroundColor: tokens.colorNeutralBackground2,
    borderRadius: tokens.borderRadiusLarge,
    padding: tokens.spacingHorizontalL,
    // Griffel NÃO aceita shorthands ambíguos de `border`/`margin`/`padding`
    // com múltiplos valores em alguns casos — use as long-hands ou
    // `shorthands.border(...)` / `shorthands.padding(...)`.
    ...shorthands.border('1px', 'solid', tokens.colorNeutralStroke1),
  },
})

function Card() {
  const s = useStyles()
  return <div className={s.card} />
}
```

Gotchas conhecidos neste projeto:

- `makeStyles` rejeita `border: '1px solid #333'` (shorthand string) em
  algumas versões → usar `borderWidth`/`borderStyle`/`borderColor` separados
  ou `shorthands.border()`. Mesmo caso pra `padding`/`margin`/`inset` com
  4 valores, `gap`, `flex`, `grid-template`.
- Pseudo-seletores e media: aninhar como chave (`':hover'`, `'@media ...'`).
- Estado por classe (`aria-current`, `data-*`): `'&[aria-current="page"]'`.
- Ordem de `mergeClasses` importa (última ganha) — passar as condicionais
  depois da base.

## Estado no ReEmu

Tudo em Griffel (`makeStyles` + `tokens`): `SettingsLayout`, `ToastLayer`,
`BindingCapture`, `ControllerMappings`, `SettingsAudio`, e as telas do "modo
Xbox" via **`packages/app-desktop/src/styles/xbox.ts`** (hooks
`useShellStyles` / `useHeroStyles` / `useBrowseStyles` / `useShelfStyles` /
`useCardStyles` / `useHintStyles`, consumidos por `AppShell` / `Home` /
`Library` / `GameCard` / `Shelf` / `ButtonHints`). O antigo `styles/xbox.css`
foi removido (2026-08-30).

`styles/xbox.ts` — decisões:

- Texto, stroke, spacing, motion (`durationFaster`/`curveEasyEase`) e
  `fontFamilyBase` vêm de `tokens`.
- **Cor de marca e elevações vêm do tema**, não mais de um `brand` local:
  `tokens.colorBrand*` + `tokens.colorNeutralBackground2..4` (escurecidos em
  `themes.ts`) + tokens custom `var(--reemuAppBg)` / `var(--reemuAccentSoft)`
  / `var(--reemuBrandSolid)` / `var(--reemuOnBrand)`. Só o que não é cor
  (raio 12/16px, rail 64px) fica num objeto `shell` local.
- Cores que NÃO devem seguir o tema, mantidas fixas: glifos A/B/X/Y do
  controle (verde/vermelho/azul/amarelo reais do Xbox) e os scrims
  preto/branco translúcidos sobre imagens (hero, capa).
- Só `index.css` tem CSS global (reset: `body`/`#root` 100%, fundo opaco,
  `kbd`). O resto é Griffel.
- Restrições WebKitGTK mantidas: 1 camada de `radial-gradient` no
  `'::before'` do `.app`, nada de `backdrop-filter`.

### Temas de cor (`src/styles/themes.ts`)

- 3 paletas hoje: **Verde Xbox** (padrão), **Roxo**, **Âmbar**. Cada uma é
  uma rampa `BrandVariants` (16 tons, gerada por interpolação HSL — script
  abaixo — ou no Fluent Theme Designer, semente ~tom 80).
- `make(rampa)` = `createDarkTheme(rampa)` + `consoleDark` (neutros
  `#0b0b0d`/`#14141a`/`#17171b`/`#1f1f24`, casam com o antigo `brand`) +
  4 tokens custom `reemu*`. O `FluentProvider` emite **toda chave do objeto
  de tema** como CSS var (`createCSSRuleFromTheme`), inclusive as custom.
- Falta (próximo passo): tela **Configurações › Aparência** com swatches,
  variante **clara** (`createLightTheme`) e **alto contraste**
  (`createHighContrastTheme`), e persistência no lado Rust.

```js
// regen de rampa (node): semente -> 16 tons
const S=[10,20,30,40,50,60,70,80,90,100,110,120,130,140,150,160];
const L=[4,8,13,18,24,30,37,44,52,58,64,71,78,85,91,96];
// h,s da semente em HSL; sat = clamp(12,92, s*(0.55+0.75*sin(pi*t))); hsl(h,sat,L[i])
```

Gotcha aplicado: `borderStyle`/`borderColor` sozinhos são barrados pelo tipo
do Griffel (forçam declarar o `border` inteiro) → usar `border: '...'`
completo ou os 3 long-hands (`borderXxxWidth/Style/Color`). `outlineStyle`
sozinho passa. Seletores aninhados por chave: `'::before'`, `'&::after'`,
`'&[aria-current="page"]::before'`, `'& :focus-visible'`, `'& > *'`,
`'&:hover [data-art]'`, `'::-webkit-scrollbar'`.

**Regra:** todo componente Fluent novo usa `makeStyles`+`tokens`. Se precisar
de um seletor pai→filho no hover, marcar o filho com um `data-*` e usar
`'&:hover [data-x]'` (como `data-art` no `GameCard`).
