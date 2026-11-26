import {
  Button,
  Caption1,
  Checkbox,
  Image,
  Input,
  Select,
  Spinner,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkRegular, DismissRegular, EditRegular } from '@fluentui/react-icons'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { memo, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ListIconAction } from '../../components/ListIconAction'
import { providerLabel } from '../../lib/metadataFormat'
import {
  resolvePendingMatch,
  resolvePendingMatchesBulk,
  type PendingMatch,
} from '../../lib/tauri'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'

const PAGE_STEP = 50

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
  },
  headerRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
  },
  filterBar: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
    '& .fui-Input': { width: 'min(380px, 100%)', flexGrow: 1 },
    '& .fui-Select': { minWidth: '210px' },
  },
  bulkBar: {
    position: 'sticky',
    top: `calc(-1 * ${tokens.spacingHorizontalXXL})`,
    zIndex: 2,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    boxShadow: tokens.shadow4,
  },
  bulkSelection: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  bulkActions: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  list: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
  },
  row: {
    display: 'flex',
    gap: tokens.spacingHorizontalM,
    alignItems: 'center',
    padding: tokens.spacingHorizontalM,
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  cover: {
    width: '36px',
    height: '48px',
    objectFit: 'cover',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusMedium,
  },
  coverPlaceholder: {
    width: '36px',
    height: '48px',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  matchDetails: {
    display: 'flex',
    flexDirection: 'column',
    flexGrow: 1,
    minWidth: 0,
    overflowWrap: 'anywhere',
  },
  matchActions: {
    display: 'flex',
    flexShrink: 0,
    gap: tokens.spacingHorizontalS,
  },
  moreRow: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
    paddingTop: tokens.spacingVerticalXS,
  },
  emptyFiltered: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalM} 0`,
  },
  dim: { color: tokens.colorNeutralForeground3 },
})

interface RowProps {
  match: PendingMatch
  selected: boolean
  busy: boolean
  onToggle: (romId: string) => void
  onReview: (match: PendingMatch) => void
  onResolve: (romId: string, accept: boolean) => void
}

const PendingMatchRow = memo(function PendingMatchRow({
  match: m,
  selected,
  busy,
  onToggle,
  onReview,
  onResolve,
}: RowProps) {
  const { t } = useTranslation()
  const s = useStyles()
  const details = [providerLabel(m.provider), m.releaseDate, m.genre].filter(Boolean).join(' · ')

  return (
    <div className={s.row} data-testid="pending-match-row">
      <Checkbox
        aria-label={m.title}
        checked={selected}
        disabled={busy}
        onChange={() => onToggle(m.romId)}
      />
      {m.coverUrl ? (
        <Image className={s.cover} src={m.coverUrl} alt="" loading="lazy" />
      ) : (
        <span className={s.coverPlaceholder} aria-hidden />
      )}
      <div className={s.matchDetails}>
        <Text weight="semibold">{m.title}</Text>
        <Caption1 className={s.dim}>{t('metadata.file', { name: m.fileStem })}</Caption1>
        {details && <Caption1 className={s.dim}>{details}</Caption1>}
      </div>
      <div className={s.matchActions}>
        <ListIconAction
          label={t('metadata.editMatch')}
          icon={<EditRegular />}
          disabled={busy}
          onClick={() => onReview(m)}
        />
        <ListIconAction
          label={t('metadata.accept')}
          icon={<CheckmarkRegular />}
          disabled={busy}
          onClick={() => onResolve(m.romId, true)}
        />
        <ListIconAction
          label={t('metadata.reject')}
          icon={<DismissRegular />}
          disabled={busy}
          onClick={() => onResolve(m.romId, false)}
        />
      </div>
    </div>
  )
})

export function PendingMatchesSection({
  matches,
  onReview,
}: {
  matches: PendingMatch[]
  onReview: (match: PendingMatch) => void
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const [filter, setFilter] = useState('')
  const [provider, setProvider] = useState<string>('all')
  const [selected, setSelected] = useState<Set<string>>(() => new Set())
  const [visibleCount, setVisibleCount] = useState(PAGE_STEP)
  const [confirmScope, setConfirmScope] = useState<string | null>(null)

  const providerCounts = useMemo(() => {
    const counts = new Map<string, number>()
    for (const m of matches) {
      counts.set(m.provider, (counts.get(m.provider) ?? 0) + 1)
    }
    return [...counts.entries()].sort(([a], [b]) =>
      (providerLabel(a) ?? a).localeCompare(providerLabel(b) ?? b),
    )
  }, [matches])

  const activeProvider =
    provider === 'all' || providerCounts.some(([key]) => key === provider) ? provider : 'all'

  const query = filter.trim().toLowerCase()
  const isFiltered = query !== '' || activeProvider !== 'all'

  const filteredMatches = useMemo(() => {
    return matches.filter((m) => {
      if (activeProvider !== 'all' && m.provider !== activeProvider) return false
      if (!query) return true
      const haystack = `${m.title} ${m.fileStem} ${m.genre ?? ''} ${m.releaseDate ?? ''} ${providerLabel(m.provider) ?? m.provider}`.toLowerCase()
      return haystack.includes(query)
    })
  }, [matches, activeProvider, query])

  const selectedFilteredIds = useMemo(
    () => filteredMatches.filter((m) => selected.has(m.romId)).map((m) => m.romId),
    [filteredMatches, selected],
  )

  const scopeKey = `${query}|${activeProvider}|${selectedFilteredIds.length}|${matches.length}`
  const confirmReject = confirmScope === scopeKey

  const refreshLists = async (removedIds: readonly string[]) => {
    if (removedIds.length > 0) {
      const gone = new Set(removedIds)
      setSelected((prev) => {
        let changed = false
        const next = new Set<string>()
        for (const id of prev) {
          if (gone.has(id)) changed = true
          else next.add(id)
        }
        return changed ? next : prev
      })
    }
    await Promise.all([
      qc.invalidateQueries({ queryKey: ['pending-matches'] }),
      qc.invalidateQueries({ queryKey: ['roms'] }),
      qc.invalidateQueries({ queryKey: ['rom-metadata'] }),
    ])
  }

  const resolveOne = useMutation({
    mutationFn: ({ romId, accept }: { romId: string; accept: boolean }) =>
      resolvePendingMatch(romId, accept),
    onSuccess: (_d, { romId }) => void refreshLists([romId]),
    onError: (e) => push(errorToast(e, 'resolveMatch')),
  })

  const resolveBulk = useMutation({
    mutationFn: ({ romIds, accept }: { romIds: string[]; accept: boolean }) =>
      resolvePendingMatchesBulk(romIds, accept),
    onSuccess: (resolvedCount, { romIds, accept }) => {
      setConfirmScope(null)
      void refreshLists(romIds)
      const count = resolvedCount > 0 ? resolvedCount : romIds.length
      push(
        sysToast(
          t(accept ? 'metadata.bulkAcceptedToast' : 'metadata.bulkRejectedToast', { count }),
          'Success',
        ),
      )
    },
    onError: (e) => push(errorToast(e, 'resolveMatch')),
  })

  const busy = resolveOne.isPending || resolveBulk.isPending
  const hasSelection = selectedFilteredIds.length > 0
  const allFilteredSelected =
    filteredMatches.length > 0 && selectedFilteredIds.length === filteredMatches.length
  const targetIds = hasSelection ? selectedFilteredIds : filteredMatches.map((m) => m.romId)

  const toggleOne = (romId: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(romId)) next.delete(romId)
      else next.add(romId)
      return next
    })
  }

  const toggleSelectAllFiltered = () => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (allFilteredSelected) {
        for (const m of filteredMatches) next.delete(m.romId)
      } else {
        for (const m of filteredMatches) next.add(m.romId)
      }
      return next
    })
  }

  const shownMatches = filteredMatches.slice(0, visibleCount)
  const remaining = filteredMatches.length - shownMatches.length

  const acceptLabel = hasSelection
    ? t('metadata.acceptSelected', { count: targetIds.length })
    : isFiltered
      ? t('metadata.acceptFiltered', { count: targetIds.length })
      : t('metadata.acceptAll', { count: targetIds.length })

  const rejectLabel = hasSelection
    ? t('metadata.rejectSelected', { count: targetIds.length })
    : isFiltered
      ? t('metadata.rejectFiltered', { count: targetIds.length })
      : t('metadata.rejectAll', { count: targetIds.length })

  return (
    <div className={s.root}>
      <div className={s.headerRow}>
        <Caption1>{t('metadata.review', { count: matches.length })}</Caption1>
        {isFiltered && (
          <Caption1 className={s.dim}>
            {t('metadata.showingCount', { shown: filteredMatches.length, total: matches.length })}
          </Caption1>
        )}
      </div>

      {(matches.length > 1 || isFiltered) && (
        <div className={s.filterBar}>
          <Input
            className="reemu-no-ring reemu-list-search"
            appearance="filled-darker"
            placeholder={t('metadata.filterPlaceholder')}
            value={filter}
            onChange={(_, d) => {
              setFilter(d.value)
              setVisibleCount(PAGE_STEP)
            }}
          />
          {providerCounts.length > 1 && (
            <Select
              appearance="filled-darker"
              aria-label={t('game.source')}
              value={activeProvider}
              onChange={(_, d) => {
                setProvider(d.value)
                setVisibleCount(PAGE_STEP)
              }}
            >
              <option value="all">{t('metadata.allProviders', { count: matches.length })}</option>
              {providerCounts.map(([key, count]) => (
                <option key={key} value={key}>
                  {t('metadata.providerOption', { provider: providerLabel(key) ?? key, count })}
                </option>
              ))}
            </Select>
          )}
        </div>
      )}

      {filteredMatches.length > 0 && (
        <div className={s.bulkBar} data-testid="pending-bulk-bar">
          <div className={s.bulkSelection}>
            <Checkbox
              checked={allFilteredSelected ? true : hasSelection ? 'mixed' : false}
              disabled={busy}
              label={
                hasSelection
                  ? t('metadata.selectedCount', { count: selectedFilteredIds.length })
                  : isFiltered
                    ? t('metadata.selectFiltered', { count: filteredMatches.length })
                    : t('metadata.selectAll')
              }
              onChange={toggleSelectAllFiltered}
            />
            {hasSelection && (
              <Button
                size="small"
                appearance="subtle"
                disabled={busy}
                onClick={() => setSelected(new Set())}
              >
                {t('metadata.clearSelection')}
              </Button>
            )}
          </div>

          <div className={s.bulkActions}>
            <Button
              appearance="primary"
              disabledFocusable={busy || targetIds.length === 0}
              icon={
                resolveBulk.isPending && resolveBulk.variables?.accept ? (
                  <Spinner size="tiny" />
                ) : (
                  <CheckmarkRegular />
                )
              }
              onClick={() => resolveBulk.mutate({ romIds: targetIds, accept: true })}
            >
              {acceptLabel}
            </Button>

            {confirmReject ? (
              <>
                <Button
                  appearance="secondary"
                  disabledFocusable={busy}
                  onClick={() => setConfirmScope(null)}
                >
                  {t('common.cancel')}
                </Button>
                <Button
                  appearance="primary"
                  disabledFocusable={busy}
                  icon={
                    resolveBulk.isPending && !resolveBulk.variables?.accept ? (
                      <Spinner size="tiny" />
                    ) : (
                      <DismissRegular />
                    )
                  }
                  onClick={() => resolveBulk.mutate({ romIds: targetIds, accept: false })}
                >
                  {t('metadata.confirmRejectBulk', { count: targetIds.length })}
                </Button>
              </>
            ) : (
              <Button
                appearance="secondary"
                disabledFocusable={busy || targetIds.length === 0}
                icon={<DismissRegular />}
                onClick={() => setConfirmScope(scopeKey)}
              >
                {rejectLabel}
              </Button>
            )}
          </div>
        </div>
      )}

      {filteredMatches.length === 0 ? (
        <div className={s.emptyFiltered}>
          <Caption1 className={s.dim}>{t('metadata.noFilteredMatches')}</Caption1>
          <Button
            size="small"
            appearance="secondary"
            onClick={() => {
              setFilter('')
              setProvider('all')
              setVisibleCount(PAGE_STEP)
            }}
          >
            {t('metadata.clearFilters')}
          </Button>
        </div>
      ) : (
        <div className={s.list}>
          {shownMatches.map((m) => (
            <PendingMatchRow
              key={m.romId}
              match={m}
              selected={selected.has(m.romId)}
              busy={busy}
              onToggle={toggleOne}
              onReview={onReview}
              onResolve={(romId, accept) => resolveOne.mutate({ romId, accept })}
            />
          ))}
        </div>
      )}

      {remaining > 0 && (
        <div className={s.moreRow}>
          <Button
            appearance="secondary"
            onClick={() => setVisibleCount((c) => c + PAGE_STEP)}
          >
            {t('metadata.showMore', { remaining })}
          </Button>
          {remaining > PAGE_STEP && (
            <Button
              appearance="subtle"
              onClick={() => setVisibleCount(filteredMatches.length)}
            >
              {t('metadata.showAll', { count: filteredMatches.length })}
            </Button>
          )}
        </div>
      )}
    </div>
  )
}
