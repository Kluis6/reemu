import {
  ArrowLeftRegular,
  FullScreenMaximizeRegular,
  FullScreenMinimizeRegular,
  HomeRegular,
  LibraryRegular,
  PowerRegular,
  PuzzlePieceRegular,
  SearchRegular,
  SettingsRegular,
  XboxControllerRegular,
} from '@fluentui/react-icons'
import { mergeClasses } from '@fluentui/react-components'
import { useEffect, useRef } from 'react'
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import { ButtonHints } from '../components/ButtonHints'
import { useClock } from '../hooks/useClock'
import { useFullscreen } from '../hooks/useFullscreen'
import { quitApp } from '../lib/tauri'
import { useSearchStore } from '../stores/useSearchStore'
import { useShellStyles } from '../styles/xbox'

const RAIL = [
  { to: '/', end: true, icon: <HomeRegular />, label: 'Início' },
  { to: '/library', end: true, icon: <LibraryRegular />, label: 'Meus jogos' },
  { to: '/settings/cores', end: false, icon: <PuzzlePieceRegular />, label: 'Cores' },
  { to: '/settings/controllers', end: false, icon: <XboxControllerRegular />, label: 'Controles' },
  { to: '/settings', end: true, icon: <SettingsRegular />, label: 'Configurações' },
]

export function AppShell() {
  const s = useShellStyles()
  const navigate = useNavigate()
  const { pathname } = useLocation()
  const clock = useClock()
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen()
  const atRoot = pathname === '/'
  const atBrowse = pathname === '/' || pathname === '/library'

  const search = useSearchStore()
  const searchRef = useRef<HTMLInputElement>(null)
  // Y no controle / "/" no teclado marcam `open` → foca o campo.
  useEffect(() => {
    if (search.open) searchRef.current?.focus()
  }, [search.open])

  const hints = atBrowse
    ? ([
        { glyph: 'A', label: 'Selecionar' },
        { glyph: 'Y', label: 'Buscar' },
        { glyph: 'MENU', label: 'Opções' },
      ] as const)
    : ([
        { glyph: 'A', label: 'Selecionar' },
        { glyph: 'B', label: 'Voltar' },
      ] as const)

  return (
    <div className={s.app}>
      <nav className={s.rail}>
        <div className={s.railBrand} title="ReEmu">
          R
        </div>
        {RAIL.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            end={it.end}
            className={s.railItem}
            title={it.label}
            aria-label={it.label}
          >
            {it.icon}
          </NavLink>
        ))}
        <div className={s.railSpacer} />
        <div className={s.railSep} />
        <button
          className={mergeClasses(s.railItem, s.railQuit)}
          onClick={() => void quitApp()}
          title="Fechar o ReEmu"
          aria-label="Fechar o ReEmu"
        >
          <PowerRegular />
        </button>
      </nav>

      <div className={s.main}>
        <div className={s.topbar}>
          <button
            className={s.iconBtn}
            onClick={() => (atRoot ? undefined : navigate(-1))}
            disabled={atRoot}
            aria-label="Voltar"
          >
            <ArrowLeftRegular />
          </button>
          <label className={s.search} data-nav-skip onClick={() => searchRef.current?.focus()}>
            <SearchRegular />
            <input
              ref={searchRef}
              value={search.query}
              placeholder="Buscar na biblioteca…"
              onFocus={() => {
                if (pathname !== '/library') navigate('/library')
                search.setOpen(true)
              }}
              onBlur={() => search.setOpen(false)}
              onChange={(e) => search.setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') {
                  search.reset()
                  searchRef.current?.blur()
                }
              }}
            />
          </label>
          <div className={s.topbarSpacer} />
          <button
            className={s.iconBtn}
            onClick={() => void toggleFullscreen()}
            aria-label={fullscreen ? 'Sair da tela cheia (F11)' : 'Tela cheia (F11)'}
            title={fullscreen ? 'Sair da tela cheia (F11)' : 'Tela cheia (F11)'}
          >
            {fullscreen ? <FullScreenMinimizeRegular /> : <FullScreenMaximizeRegular />}
          </button>
          <span className={s.clock}>{clock}</span>
        </div>
        <div className={s.scroll}>
          <Outlet />
        </div>
      </div>

      <ButtonHints hints={hints} />
    </div>
  )
}
