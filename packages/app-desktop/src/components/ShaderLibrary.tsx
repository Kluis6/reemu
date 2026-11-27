import {
  Body1,
  Button,
  Caption1,
  Input,
  Spinner,
  Subtitle2,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { ArrowLeftRegular, FolderRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useRef, useState } from 'react'
import {
  downloadShaderPack,
  listSlangpDir,
  pickFolder,
  shaderPackStatus,
  shaderPackUpdate,
  type SlangpEntry,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { usePanelSlideStyles } from '../lib/motion'
import { errorPatch } from '../lib/toast'
import { DownloadButton } from './DownloadButton'
import { ShaderCard, ShaderCategoryCard } from './ShaderCard'
import { Trans, useTranslation } from 'react-i18next'

const ROOT_KEY = 'reemu.shaderLibRoot'

/** Caminho do preset relativo à pasta do pacote (`crt/crt-geom.slangp`). */
const relPath = (path: string, root: string) => {
  const p = path.replace(/\\/g, '/')
  const r = root.replace(/\\/g, '/').replace(/\/+$/, '')
  return p.toLowerCase().startsWith(r.toLowerCase() + '/') ? p.slice(r.length + 1) : p
}

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  // `fill`: ocupa a altura que o pai der, e a lista rola dentro dela
  rootFill: { flexGrow: 1, minHeight: 0 },
  // sem a margem negativa em cima: a lista passava por baixo do campo de filtro
  listFill: { flexGrow: 1, minHeight: 0, maxHeight: 'none', marginTop: 0, marginBottom: 0 },
  bar: { display: 'flex', gap: tokens.spacingHorizontalS, alignItems: 'center' },
  panel: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  categoryTitle: { minWidth: 0, overflowWrap: 'anywhere' },
  list: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    maxHeight: '340px',
    overflowY: 'auto',
    // folga (8) pro anel de foco dos cards não ser cortado pela borda que rola
    padding: '8px',
    margin: '-8px',
    scrollPaddingTop: '8px',
    scrollPaddingBottom: '8px',
    '& > *': { flexShrink: 0 },
  },
})

/** A pasta e o download são compartilhados pela barra de abas e pela lista. */
export function useShaderPack(enabled: boolean) {
  const { t } = useTranslation()
  const [root, setRoot] = useState<string>(() => {
    try {
      return localStorage.getItem(ROOT_KEY) ?? ''
    } catch {
      return ''
    }
  })
  const push = useToastStore((st) => st.push)
  const updateToast = useToastStore((st) => st.update)
  const dlId = useRef<string | null>(null)

  const pack = useQuery({
    queryKey: ['shader-pack'],
    queryFn: shaderPackStatus,
    enabled,
    retry: false,
  })
  // Versão nova do pacote no `libretro/slang-shaders` (o backend guarda a
  // consulta por 6 h). Falha não mostra nada.
  const qc = useQueryClient()
  const packUpdate = useQuery({
    queryKey: ['shader-pack-update'],
    queryFn: () => shaderPackUpdate(),
    enabled: enabled && !!pack.data?.installed,
    retry: false,
    staleTime: 60 * 60 * 1000,
  })

  const dl = useMutation({
    mutationFn: () =>
      downloadShaderPack((p) => {
        if (!dlId.current) return
        const pct = p.total ? p.received / p.total : null
        const mb = (n: number) => (n / 1048576).toFixed(0)
        updateToast(dlId.current, {
          message:
            p.phase === 'extract'
              ? t('shaders.extracting')
              : t('shaders.downloadingMb', { received: mb(p.received), total: p.total ? `/${mb(p.total)}` : '' }),
          progress: p.phase === 'extract' ? null : pct,
        })
      }),
    onMutate: () => {
      const id = crypto.randomUUID()
      dlId.current = id
      push({
        id,
        message: t('shaders.downloadingPack'),
        variant: 'Info',
        durationMs: 0,
        source: 'System',
        progress: null,
      })
    },
    onSuccess: (path) => {
      try {
        localStorage.setItem(ROOT_KEY, path)
      } catch {
        /* modo privado */
      }
      setRoot(path)
      pack.refetch()
      qc.invalidateQueries({ queryKey: ['shader-pack-update'] })
      qc.invalidateQueries({ queryKey: ['slangp-dir', path] })
      if (dlId.current)
        updateToast(dlId.current, {
          message: t('shaders.installed'),
          variant: 'Success',
          durationMs: 4000,
          progress: undefined,
        })
    },
    onError: (e) => {
      if (dlId.current)
        updateToast(dlId.current, errorPatch(e, 'downloadShaderPack'))
    },
    onSettled: () => {
      dlId.current = null
    },
  })

  const chooseRoot = async () => {
    const p = await pickFolder(t('shaders.pickFolder'))
    if (!p) return
    try {
      localStorage.setItem(ROOT_KEY, p)
    } catch {
      /* modo privado — segue sem lembrar */
    }
    setRoot(p)
  }

  return {
    root,
    chooseRoot,
    downloading: dl.isPending,
    download: () => dl.mutate(),
    // A atualização é do pacote gerenciado pelo ReEmu, identificado pela pasta instalada.
    updateAvailable: !!packUpdate.data && pack.data?.path === root,
  }
}

/**
 * Navegador da pasta de shaders do usuário (RetroArch / RetroBat —
 * `shaders_slang`). Lista os `.slangp` agrupados por subpasta.
 */
export function ShaderLibrary({
  pack,
  onPick,
  activePath,
  busy,
  settingsFor,
  fill = false,
}: {
  pack: ReturnType<typeof useShaderPack>
  onPick: (path: string) => void
  activePath: string
  busy: boolean
  /** Botão de ajustes no card do shader, igual à lista do ReEmu. */
  settingsFor?: (path: string) => (() => void) | undefined
  /** Lista na altura que sobrar no pai (flex em coluna), em vez da fixa. */
  fill?: boolean
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const slide = usePanelSlideStyles()
  const { root, chooseRoot, downloading, download } = pack
  const [filter, setFilter] = useState('')
  const [group, setGroup] = useState<{ category: string; open: boolean } | null>(null)
  const filterRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const categoryRef = useRef<HTMLDivElement>(null)
  const backRef = useRef<HTMLButtonElement>(null)
  const categoriesScroll = useRef(0)

  useEffect(() => {
    if (!group) return
    if (group.open) {
      backRef.current?.focus({ preventScroll: true })
    } else {
      if (listRef.current) listRef.current.scrollTop = categoriesScroll.current
      const target = categoryRef.current ?? filterRef.current
      target?.focus({ preventScroll: true })
    }
  }, [group])

  const q = useQuery({
    queryKey: ['slangp-dir', root],
    queryFn: () => listSlangpDir(root),
    enabled: root.length > 0,
    retry: false,
  })

  const groups = useMemo(() => {
    const f = filter.trim().toLowerCase()
    const hit = (e: SlangpEntry) =>
      !f || e.name.toLowerCase().includes(f) || e.category.toLowerCase().includes(f)
    const by = new Map<string, SlangpEntry[]>()
    for (const e of q.data ?? []) {
      if (!hit(e)) continue
      const arr = by.get(e.category)
      if (arr) arr.push(e)
      else by.set(e.category, [e])
    }
    return by
  }, [q.data, filter])

  const category = group?.open ? group.category : null
  const presets = category === null ? null : (groups.get(category) ?? [])

  if (!root) {
    return (
      <div className={s.root}>
        <Caption1>
          <Trans i18nKey="shaders.intro" components={{ code: <code /> }} />
        </Caption1>
        <div className={s.bar}>
          <DownloadButton
            appearance="primary"
            label={downloading ? t('shaders.downloading') : t('shaders.download')}
            busy={downloading}
            onClick={download}
          />
          <Button icon={<FolderRegular />} onClick={chooseRoot}>
            {t('shaders.chooseFolder')}
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className={mergeClasses(s.root, fill && s.rootFill)}>
      {q.isLoading && <Spinner size="tiny" label={t('shaders.scanning')} />}
      {q.isError && <Body1>{t('shaders.readFailed', { error: String(q.error) })}</Body1>}

      {q.data && (
        <>
          <div className={s.bar}>
            <Input
              ref={filterRef}
              aria-label={t('shaders.filter')}
              appearance="filled-darker"
              placeholder={t('shaders.filter')}
              value={filter}
              onChange={(_, d) => setFilter(d.value)}
              style={{ flex: 1 }}
            />
            <Caption1>{t('shaders.presets', { count: presets?.length ?? q.data.length })}</Caption1>
          </div>
          <div
            key={category === null ? 'categories' : 'presets'}
            className={mergeClasses(
              s.panel,
              fill && s.rootFill,
              slide.transition,
              !!group && (group.open ? slide.forward : slide.backward),
            )}
          >
            {category !== null && (
              <div className={s.bar}>
                <Button
                  ref={backRef}
                  appearance="subtle"
                  icon={<ArrowLeftRegular />}
                  aria-label={t('shaders.backToCategories')}
                  onClick={() => setGroup({ category, open: false })}
                />
                <Subtitle2 className={s.categoryTitle}>{category || t('shaders.root')}</Subtitle2>
              </div>
            )}
            <div ref={listRef} className={mergeClasses(s.list, fill && s.listFill)}>
              {(presets?.length ?? groups.size) === 0 && <Caption1>{t('shaders.nothing')}</Caption1>}
              {presets ? presets.map((entry) => (
                <ShaderCard
                  key={entry.path}
                  title={entry.name}
                  description={relPath(entry.path, root)}
                  selected={entry.path === activePath}
                  disabled={busy}
                  onSelect={() => onPick(entry.path)}
                  onSettings={settingsFor?.(entry.path)}
                />
              )) : Array.from(groups, ([cat, items]) => (
                <ShaderCategoryCard
                  key={cat}
                  ref={cat === group?.category ? categoryRef : undefined}
                  title={cat || t('shaders.root')}
                  count={items.length}
                  onOpen={() => {
                    categoriesScroll.current = listRef.current?.scrollTop ?? 0
                    setGroup({ category: cat, open: true })
                  }}
                />
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  )
}
