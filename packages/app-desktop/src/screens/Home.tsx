import { Button, Spinner } from '@fluentui/react-components'
import { AddRegular, ChevronRightRegular, GridRegular } from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useMemo, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { GameCard } from '../components/GameCard'
import { Shelf } from '../components/Shelf'
import { listRoms, type RomEntry } from '../lib/tauri'
import { useBrowseStyles, useHeroStyles } from '../styles/xbox'

/** Uma faixa curada da Início (título + "ver mais" opcional + prateleira). */
function Row({
  title,
  items,
  onMore,
  render,
}: {
  title: string
  items: readonly RomEntry[]
  onMore?: () => void
  render: (r: RomEntry) => ReactNode
}) {
  const s = useBrowseStyles()
  if (items.length === 0) return null
  return (
    <section className={s.section}>
      <div className={s.sectionHead}>
        <h2 className={s.sectionTitle}>{title}</h2>
        {onMore && (
          <button className={s.sectionChevron} onClick={onMore} tabIndex={-1} aria-hidden>
            <ChevronRightRegular />
          </button>
        )}
      </div>
      <Shelf>{items.map(render)}</Shelf>
    </section>
  )
}

/**
 * Tela inicial no estilo "modo Xbox": um hero + faixas curadas (Continuar
 * jogando, Adicionados recentemente…). A biblioteca completa ("Meus jogos")
 * fica em `/library`. Novas seções entram aqui.
 */
export function Home() {
  const s = useBrowseStyles()
  const h = useHeroStyles()
  const navigate = useNavigate()
  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false })
  const all = useMemo(() => roms.data ?? [], [roms.data])

  const recent = useMemo(
    () =>
      all
        .filter((r) => r.lastPlayedAt != null)
        .sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
        .slice(0, 16),
    [all],
  )
  const added = useMemo(() => [...all].sort((a, b) => b.addedAt - a.addedAt).slice(0, 16), [all])
  const hero = recent[0] ?? all.find((r) => r.boxart) ?? all[0]

  const card = (r: RomEntry) => (
    <GameCard
      key={r.id}
      title={r.title}
      badge={r.systemId}
      boxart={r.boxart}
      onClick={() => navigate(`/rom/${r.id}`)}
      menu={[
        { label: 'Abrir', onClick: () => navigate(`/rom/${r.id}`) },
        { label: 'Ver biblioteca', onClick: () => navigate('/library') },
      ]}
    />
  )

  if (roms.isLoading) return <Spinner style={{ marginTop: 40 }} label="Carregando…" />

  if (!roms.isError && all.length === 0) {
    return (
      <div className={s.empty}>
        <div className={s.emptyIcon}>🕹</div>
        <h2>Bem-vindo ao ReEmu</h2>
        <span>Adicione suas ROMs pra montar a biblioteca.</span>
        <Button appearance="primary" icon={<AddRegular />} onClick={() => navigate('/library')}>
          Adicionar ROMs…
        </Button>
      </div>
    )
  }

  return (
    <div>
      {hero && (
        <button className={h.hero} onClick={() => navigate(`/rom/${hero.id}`)}>
          {hero.boxart && <img src={hero.boxart} alt="" />}
          <span className={h.body}>
            <span className={h.kicker}>{hero.lastPlayedAt ? 'Continuar' : 'Destaque'}</span>
            <span className={h.title}>{hero.title}</span>
            <span className={h.sub}>{hero.systemId.toUpperCase()}</span>
          </span>
        </button>
      )}

      <Row title="Continuar jogando" items={recent} onMore={() => navigate('/library')} render={card} />
      <Row
        title="Adicionados recentemente"
        items={added}
        onMore={() => navigate('/library')}
        render={card}
      />

      <div className={s.toolbar} style={{ marginTop: 28 }}>
        <button className={s.chip} onClick={() => navigate('/library')}>
          <GridRegular /> Ver todos os jogos ({all.length})
        </button>
      </div>
    </div>
  )
}
