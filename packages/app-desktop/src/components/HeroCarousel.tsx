import {
  Carousel,
  CarouselCard,
  CarouselNav,
  CarouselNavButton,
  CarouselNavContainer,
  CarouselSlider,
  CarouselViewport,
  makeStyles,
  tokens,
  useCarouselContext_unstable as useCarouselContext,
} from '@fluentui/react-components'
import { useEffect } from 'react'
import { initials } from '../lib/initials'
import { platformLabel } from '../lib/platform'
import type { RomEntry } from '../lib/tauri'
import { useTranslation } from 'react-i18next'

const ADVANCE_MS = 7000

const useStyles = makeStyles({
  wrap: {
    position: 'relative',
    width: '100%',
    minWidth: 0,
    // Altura dos destaques do app Xbox ("Em destaque", 324 epx) menos um
    // pouco: o destaque e a 1ª fileira de jogos aparecem juntos na tela de
    // referência (768 epx).
    height: '288px',
    // janela baixa: os jogos continuam aparecendo sem rolar
    '@media (max-height: 700px)': { height: '220px' },
    borderRadius: tokens.borderRadiusMedium,
    backgroundImage: 'linear-gradient(135deg, #1a1a1f 0%, #101014 100%)',
    // viewport/slider/card do `Carousel` ocupam a altura toda. O recorte
    // (cantos e o que passa da borda) fica no viewport, não na raiz: o
    // brilho do foco (`::before` da raiz, index.css) vai por fora dela.
    '& .fui-CarouselViewport, & .fui-CarouselSlider': { height: '100%' },
    '& .fui-CarouselViewport': {
      overflow: 'hidden',
      borderRadius: tokens.borderRadiusMedium,
    },
  },
  card: { height: '100%' },
  slide: {
    position: 'relative',
    display: 'block',
    width: '100%',
    height: '100%',
    border: 'none',
    padding: 0,
    textAlign: 'left',
    color: 'inherit',
    cursor: 'pointer',
    backgroundColor: 'transparent',
  },
  img: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'cover',
  },
  fallback: {
    position: 'absolute',
    inset: 0,
    display: 'grid',
    placeItems: 'center',
    fontSize: '72px',
    fontWeight: tokens.fontWeightBold,
    color: 'rgba(255,255,255,0.12)',
  },
  scrim: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'linear-gradient(90deg, rgba(0,0,0,0.82) 0%, rgba(0,0,0,0.2) 55%, transparent 78%), linear-gradient(0deg, rgba(0,0,0,0.6), transparent 42%)',
  },
  body: {
    position: 'absolute',
    left: '24px',
    bottom: '24px',
    maxWidth: '62%',
    zIndex: 1,
    // kicker / título / sistema são <span> (dentro de <button>) — em coluna,
    // senão saíam colados na mesma linha ("ContinuarJogo 2Super Nintendo").
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    textAlign: 'left',
    // Sempre sobre a arte + véu escuro (`scrim`), em qualquer tema — texto
    // claro fixo. Herdando a cor do tema, nos temas claros o título saía
    // quase preto sobre o escuro.
    color: '#ffffff',
  },
  // fluent2.microsoft.design/typography: "use sentence case, nunca all
  // caps" — tracking largo (0.08em) era calibrado pra maiúsculas, reduzido
  // junto (senão "Continuar"/"Destaque" minúsculo ficava esparramado).
  kicker: {
    fontSize: tokens.fontSizeBase200,
    fontWeight: tokens.fontWeightRegular,
    letterSpacing: '0.02em',
    // Mesma cor do nome do jogo (o branco do `body`, sobre o véu escuro da
    // arte), em qualquer tema — pedido do usuário; antes era a cor de marca.
    color: 'inherit',
  },
  title: {
    // "Title" da rampa tipográfica do Windows (28 epx).
    fontSize: '28px',
    fontFamily: 'var(--reemuFontDisplay)',
    fontWeight: tokens.fontWeightRegular,
    lineHeight: 1.12,
    margin: '4px 0 2px',
    // no máximo 2 linhas (com "…"): nome muito comprido subia até a seta da
    // esquerda, que fica no meio da altura
    display: '-webkit-box',
    WebkitBoxOrient: 'vertical',
    WebkitLineClamp: 2,
    overflow: 'hidden',
  },
  sub: {
    fontSize: tokens.fontSizeBase300,
    color: 'rgba(255, 255, 255, 0.82)',
  },
  // Pontos no canto de baixo à DIREITA (pedido do usuário; o Fluent centraliza
  // com `justify-content: center`): o texto do jogo fica embaixo à esquerda e
  // nome comprido não encosta nos controles. 24 da borda, como o texto.
  navContainer: {
    justifyContent: 'flex-end !important',
    paddingRight: '24px',
    boxSizing: 'border-box',
  },
  nav: { marginLeft: 'auto !important', marginRight: '0 !important' },
  // Setas do `CarouselNavContainer` (layout "overlay-expanded": nas laterais,
  // no meio da altura). 40 epx com ícone de 24: a seta das prateleiras do app
  // Xbox tem 45 epx e um chevron de ~23 epx; com 32/16 ficava pequena.
  // Formato dos outros botões do ReEmu (pedido do usuário): quadrado com o
  // raio padrão (`borderRadiusMedium`), não o círculo do Fluent.
  arrow: {
    borderRadius: `${tokens.borderRadiusMedium} !important`,
    backgroundColor: 'rgba(0,0,0,0.45) !important',
    color: '#ffffff !important',
    width: '40px !important',
    height: '40px !important',
    minWidth: '0 !important',
    maxWidth: '40px !important',
    fontSize: '24px !important',
    '& .fui-Button__icon': { fontSize: '1em', width: '1em', height: '1em' },
    ':hover': { backgroundColor: 'rgba(0,0,0,0.72) !important' },
  },
})

/**
 * Carrossel de destaque da Início (modelo modo XBOX) com o `Carousel` do
 * Fluent 2: 1 slide por vez em fade (`motion="fade"`), setas nas laterais e
 * pontos embaixo (`layout="overlay-expanded"`), avanço automático a cada 7 s
 * e o anúncio do slide pra leitor de tela. Cada slide abre o jogo.
 *
 * Sem o botão de pausar/retomar (pedido do usuário: "o carousel deve ser
 * automático") — o Fluent 2 recomenda ter um ("the user must always have the
 * option of pausing the auto-play feature"). O avanço liga por
 * `enableAutoplay` do contexto do Carousel (`AutoAdvance`), o mesmo que o
 * botão chama; o Fluent pausa sozinho com o mouse ou o foco no carrossel.
 */
function AutoAdvance() {
  const enableAutoplay = useCarouselContext((ctx) => ctx.enableAutoplay)
  useEffect(() => {
    enableAutoplay(true)
    return () => enableAutoplay(false)
  }, [enableAutoplay])
  return null
}

/** Carrossel de destaque da Início (ver `AutoAdvance`). */
export function HeroCarousel({
  items,
  onOpen,
}: {
  items: readonly RomEntry[]
  onOpen: (id: string) => void
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const n = items.length
  if (n === 0) return null

  return (
    <Carousel
      className={s.wrap}
      groupSize={1}
      circular
      motion="fade"
      autoplayInterval={ADVANCE_MS}
      aria-roledescription={t('shell2.carousel')}
      announcement={(index, total) => t('shell2.slideOf', { n: index + 1, total })}
    >
      {n > 1 && <AutoAdvance />}
      <CarouselViewport>
        <CarouselSlider>
          {items.map((r, i) => (
            <CarouselCard
              key={r.id}
              className={s.card}
              aria-label={t('shell2.slideOf', { n: i + 1, total: n })}
            >
              <button type="button" className={s.slide} onClick={() => onOpen(r.id)}>
                {r.boxart ? (
                  <img className={s.img} src={r.boxart} alt="" />
                ) : (
                  <span className={s.fallback}>{initials(r.title)}</span>
                )}
                <span className={s.scrim} />
                <span className={s.body}>
                  <span className={s.kicker}>
                    {r.lastPlayedAt ? t('shell2.continue') : t('shell2.featured')}
                  </span>
                  <span className={s.title}>{r.title}</span>
                  <span className={s.sub}>{platformLabel(r.systemId)}</span>
                </span>
              </button>
            </CarouselCard>
          ))}
        </CarouselSlider>
      </CarouselViewport>
      {n > 1 && (
        <CarouselNavContainer
          className={s.navContainer}
          layout="overlay-expanded"
          next={{ className: s.arrow, 'aria-label': t('shell2.next') }}
          prev={{ className: s.arrow, 'aria-label': t('shell2.previous') }}
        >
          <CarouselNav className={s.nav}>
            {(index) => (
              <CarouselNavButton aria-label={t('shell2.goToSlide', { n: index + 1 })} />
            )}
          </CarouselNav>
        </CarouselNavContainer>
      )}
    </Carousel>
  )
}
