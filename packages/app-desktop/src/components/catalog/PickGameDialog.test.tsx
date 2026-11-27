// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CatalogSearchGame, GameMetadata } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  catalogSearch: vi.fn(),
  catalogPick: vi.fn(),
}))

vi.mock('../../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../../lib/tauri')>()),
  ...api,
}))

vi.mock('../FullscreenDialog', () => ({
  FullscreenDialog: (p: {
    open: boolean
    onClose: () => void
    title: string
    children: React.ReactNode
    actions?: React.ReactNode
  }) =>
    p.open ? (
      <div className="fui-DialogSurface" role="dialog">
        <h2>{p.title}</h2>
        {p.children}
        {p.actions}
      </div>
    ) : null,
}))

import '../../i18n'
import { PickGameDialog } from './PickGameDialog'
import { cleanSeedQuery } from './cleanSeedQuery'

const RESULTS: CatalogSearchGame[] = [
  {
    gameId: '01950f10-aaaa-7bbb-8ccc-ff66ff66ff66',
    title: 'Final Fantasy VI',
    platforms: ['Super Nintendo'],
    releaseDate: '1994-04-02',
    cover: 'https://cdn.alvanista.com/ff6/cover.webp',
  },
  {
    gameId: '01950f10-aaaa-7bbb-8ccc-ff44ff44ff44',
    title: 'Final Fantasy IV',
    platforms: ['Super Nintendo'],
    releaseDate: '1991-07-19',
    cover: null,
  },
]

const PICKED_META: GameMetadata = {
  title: 'Final Fantasy VI',
  description: null,
  coverUrl: 'https://cdn.alvanista.com/ff6/cover.webp',
  releaseDate: '1994-04-02',
  genre: 'RPG',
  providerSource: 'alvanista',
  gameUuid: '01950f10-aaaa-7bbb-8ccc-ff66ff66ff66',
  developer: 'Square',
  publisher: 'Square',
  players: '1',
  rating: null,
  ageRating: null,
  modes: null,
  screenshotUrl: null,
  titleScreenUrl: null,
  fanartUrl: null,
  logoUrl: null,
  videoUrl: null,
  gamekave: null,
}

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 6; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

async function mount(onClose = () => {}, initialQuery = 'Final Fantasy (USA)') {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () =>
    root.render(
      <QueryClientProvider client={qc}>
        <PickGameDialog
          romId="rom-1"
          systemId="snes"
          initialQuery={initialQuery}
          onClose={onClose}
        />
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.catalogSearch.mockResolvedValue(RESULTS)
  api.catalogPick.mockResolvedValue(PICKED_META)
})

afterEach(async () => {
  if (unmount) await unmount()
  host?.remove()
})

describe('PickGameDialog', () => {
  it('search_shows_results_with_cover_and_year', async () => {
    await mount()
    expect(api.catalogSearch).toHaveBeenCalledWith('Final Fantasy', 'snes')
    const items = host.querySelectorAll<HTMLButtonElement>('[data-testid="catalog-search-item"]')
    expect(items).toHaveLength(2)
    expect(items[0].textContent).toContain('Final Fantasy VI')
    expect(items[0].textContent).toContain('1994')
    expect(items[0].querySelector('img')?.getAttribute('src')).toBe(
      'https://cdn.alvanista.com/ff6/cover.webp',
    )
    expect(items[1].textContent).toContain('Final Fantasy IV')
    expect(items[1].textContent).toContain('1991')
  })

  it('pick_calls_catalog_pick_and_closes', async () => {
    const onClose = vi.fn()
    await mount(onClose)
    const first = host.querySelector<HTMLButtonElement>('[data-testid="catalog-search-item"]')!
    await act(async () => {
      first.click()
    })
    await settle()
    expect(api.catalogPick).toHaveBeenCalledWith(
      'rom-1',
      '01950f10-aaaa-7bbb-8ccc-ff66ff66ff66',
      expect.any(String),
    )
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('cleans_rom_and_iso_dump_tags_serials_and_inverted_articles_in_seed_query', () => {
    expect(cleanSeedQuery('0425 - Pokemon - Emerald Version (USA, Europe)')).toBe(
      'Pokemon - Emerald Version',
    )
    expect(cleanSeedQuery('Legend of Zelda, The - A Link to the Past (USA)')).toBe(
      'The Legend of Zelda - A Link to the Past',
    )
    expect(cleanSeedQuery('SLUS-20062 - Grand Theft Auto III (USA)')).toBe('Grand Theft Auto III')
    expect(cleanSeedQuery('ULUS10041_Lumines')).toBe('Lumines')
    expect(cleanSeedQuery('God.of.War.II.USA.PS2DVD-LIGHTFORCE')).toBe('God of War II')
    expect(cleanSeedQuery("Luigi's Mansion (USA).nkit")).toBe("Luigi's Mansion")
    expect(cleanSeedQuery('128-in-1 Junior Console (Chip 1) (PAL) [!]')).toBe(
      '128-in-1 Junior Console',
    )
    expect(cleanSeedQuery('Final Fantasy VII - Disc 1')).toBe('Final Fantasy VII')
    expect(cleanSeedQuery('007 - GoldenEye (USA)')).toBe('007 - GoldenEye')
  })
})

