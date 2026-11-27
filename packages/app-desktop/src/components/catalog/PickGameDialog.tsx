import {
  Body1,
  Body1Strong,
  Button,
  Caption1,
  Input,
  MessageBar,
  MessageBarBody,
  Spinner,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { SearchRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CATALOG_STATUS_KEY } from '../../hooks/useCatalog'
import { catalogPick, catalogSearch, type CatalogSearchGame } from '../../lib/tauri'
import { sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { FullscreenDialog } from '../FullscreenDialog'
import { cleanSeedQuery } from './cleanSeedQuery'

const DEBOUNCE_MS = import.meta.env.MODE === 'test' ? 0 : 220

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalM,
    maxWidth: '760px',
  },
  hint: { color: tokens.colorNeutralForeground3 },
  results: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalS,
  },
  row: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'flex-start',
    columnGap: tokens.spacingHorizontalM,
    width: '100%',
    minHeight: '68px',
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
    color: tokens.colorNeutralForeground1,
    border: 'none',
    textAlign: 'left',
    cursor: 'pointer',
    ':hover': { backgroundColor: 'var(--reemuControlFillHover)' },
    ':active': { backgroundColor: 'var(--reemuControlFillPressed)' },
  },
  cover: {
    width: '40px',
    height: '54px',
    objectFit: 'cover',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  coverEmpty: {
    width: '40px',
    height: '54px',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  info: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: '2px',
    minWidth: 0,
    flexGrow: 1,
  },
})

export function PickGameDialog({
  romId,
  systemId,
  initialQuery = '',
  onClose,
}: {
  romId: string
  systemId: string
  initialQuery?: string
  onClose: () => void
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const seed = cleanSeedQuery(initialQuery)
  const [text, setText] = useState(seed)
  const [debounced, setDebounced] = useState(seed)
  const [pickError, setPickError] = useState(false)

  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // Foco inicial direto no campo de busca (navegação por controle, doc 18).
  useEffect(() => {
    const id = window.setTimeout(() => inputRef.current?.focus(), 0)
    return () => window.clearTimeout(id)
  }, [])

  useEffect(() => {
    const next = text.trim()
    if (DEBOUNCE_MS === 0) {
      setDebounced(next)
      return
    }
    const id = window.setTimeout(() => setDebounced(next), DEBOUNCE_MS)
    return () => window.clearTimeout(id)
  }, [text])

  const search = useQuery({
    queryKey: ['catalog-search', systemId, debounced],
    queryFn: () => catalogSearch(debounced, systemId),
    enabled: debounced.length > 0,
    retry: false,
  })

  const pick = useMutation({
    mutationFn: (game: CatalogSearchGame) => catalogPick(romId, game.gameId, i18n.language),
    onMutate: () => setPickError(false),
    onSuccess: async (freshMeta) => {
      const resolvedLang = i18n.resolvedLanguage ?? i18n.language
      qc.setQueryData(['rom-metadata', romId, resolvedLang], freshMeta)
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['roms'] }),
        qc.invalidateQueries({ queryKey: ['rom-metadata', romId] }),
        qc.invalidateQueries({ queryKey: ['alvanista-collection'] }),
        qc.invalidateQueries({ queryKey: ['gamelog-state', romId] }),
        qc.invalidateQueries({ queryKey: ['gamelog-items'] }),
        qc.invalidateQueries({ queryKey: ['pending-matches'] }),
        qc.invalidateQueries({ queryKey: CATALOG_STATUS_KEY }),
      ])
      push(sysToast(t('catalog.pick.applied'), 'Success'))
      onClose()
    },
    onError: () => setPickError(true),
  })

  const games = search.data ?? []

  return (
    <FullscreenDialog
      open
      onClose={() => {
        if (!pick.isPending) onClose()
      }}
      title={t('catalog.pick.title')}
      actions={
        <Button appearance="secondary" disabled={pick.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
      }
    >
      <div className={s.root}>
        <Input
          ref={inputRef}
          appearance="filled-darker"
          size="large"
          contentBefore={<SearchRegular />}
          aria-label={t('catalog.pick.title')}
          placeholder={t('catalog.pick.placeholder')}
          value={text}
          disabled={pick.isPending}
          onChange={(_, d) => setText(d.value)}
          onKeyDown={(ev) => {
            if (ev.key === 'ArrowDown') {
              const first = listRef.current?.querySelector<HTMLButtonElement>('button')
              if (first) {
                ev.preventDefault()
                first.focus()
              }
            }
          }}
        />
        <Caption1 className={s.hint}>{t('catalog.pick.hint')}</Caption1>

        {(search.isError || pickError) && (
          <MessageBar intent="warning">
            <MessageBarBody>{t('catalog.pick.error')}</MessageBarBody>
          </MessageBar>
        )}

        {search.isFetching && (
          <Body1 role="status">
            <Spinner size="tiny" label={t('catalog.pick.searching')} />
          </Body1>
        )}

        {!search.isFetching && debounced.length > 0 && search.isSuccess && games.length === 0 && (
          <Caption1 className={s.hint}>{t('catalog.pick.empty', { query: debounced })}</Caption1>
        )}

        {games.length > 0 && (
          <div ref={listRef} className={s.results} role="list">
            {games.map((g) => {
              const year = g.releaseDate?.match(/^\d{4}/)?.[0] ?? null
              const subtitle = [year, g.platforms.join(', ')].filter(Boolean).join(' · ')
              return (
                <button
                  key={g.gameId}
                  type="button"
                  role="listitem"
                  data-testid="catalog-search-item"
                  className={mergeClasses('reemu-block', s.row)}
                  disabled={pick.isPending}
                  onClick={() => pick.mutate(g)}
                >
                  {g.cover ? (
                    <img className={s.cover} src={g.cover} alt="" loading="lazy" />
                  ) : (
                    <span className={s.coverEmpty} aria-hidden />
                  )}
                  <span className={s.info}>
                    <Body1Strong>{g.title}</Body1Strong>
                    {subtitle && <Caption1 className={s.hint}>{subtitle}</Caption1>}
                  </span>
                </button>
              )
            })}
          </div>
        )}
      </div>
    </FullscreenDialog>
  )
}

