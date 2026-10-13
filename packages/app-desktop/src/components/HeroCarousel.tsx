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
} from '@fluentui/react-components'
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
    borderRadius: tokens.borderRadiusXLarge,
    overflow: 'hidden',
    backgroundImage: 'linear-gradient(135deg, #1a1a1f 0%, #101014 100%)',
    // viewport/slider/card do `Carousel` ocupam a altura toda
    '& .fui-CarouselViewport, & .fui-CarouselSlider': { height: '100%' },
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
    // acima da linha de controles do `CarouselNavContainer` (pausar e
    // pontos, ~32 epx a partir de 12 da borda)
    bottom: '56px',
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
    fontWeight: tokens.fontWeightBold,
    letterSpacing: '0.02em',
    // Mesma cor do nome do jogo (o branco do `body`, sobre o véu escuro da
    // arte), em qualquer tema — pedido do usuário; antes era a cor de marca.
    color: 'inherit',
  },
  title: {
    // "Title" da rampa tipográfica do Windows (28 epx).
    fontSize: '28px',
    fontWeight: tokens.fontWeightBold,
    lineHeight: 1.12,
    margin: '4px 0 2px',
  },
  sub: {
    fontSize: tokens.fontSizeBase300,
    color: 'rgba(255, 255, 255, 0.82)',
  },
  // Setas do `CarouselNavContainer` (layout "overlay-expanded": nas laterais,
  // no meio da altura). 40 epx com ícone de 24: a seta das prateleiras do app
  // Xbox tem 45 epx e um chevron de ~23 epx; com 32/16 ficava pequena.
  arrow: {
    borderRadius: tokens.borderRadiusCircular,
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
  // Pausar/retomar o avanço automático (Fluent 2 › Carousel: "the user must
  // always have the option of pausing the auto-play feature") — sobre a arte.
  autoplay: {
    backgroundColor: 'rgba(0,0,0,0.45) !important',
    color: '#ffffff !important',
    ':hover': { backgroundColor: 'rgba(0,0,0,0.72) !important' },
  },
})

/**
 * Carrossel de destaque da Início (modelo modo XBOX) com o `Carousel` do
 * Fluent 2: 1 slide por vez em fade (`motion="fade"`), setas nas laterais e
 * pontos embaixo (`layout="overlay-expanded"`), avanço automático a cada 7 s
 * com botão de pausar (o Fluent pede: "the user must always have the option
 * of pausing the auto-play feature") e o anúncio do slide pra leitor de tela.
 * Cada slide abre o jogo.
 */
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
          layout="overlay-expanded"
          next={{ className: s.arrow, 'aria-label': t('shell2.next') }}
          prev={{ className: s.arrow, 'aria-label': t('shell2.previous') }}
          autoplay={{
            className: s.autoplay,
            defaultChecked: true,
            'aria-label': t('shell2.autoplay'),
          }}
        >
          <CarouselNav>
            {(index) => (
              <CarouselNavButton aria-label={t('shell2.goToSlide', { n: index + 1 })} />
            )}
          </CarouselNav>
        </CarouselNavContainer>
      )}
    </Carousel>
  )
}
