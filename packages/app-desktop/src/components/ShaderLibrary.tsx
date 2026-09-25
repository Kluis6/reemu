import {
  Accordion,
  AccordionHeader,
  AccordionItem,
  AccordionPanel,
  Body1,
  Button,
  Caption1,
  Input,
  Spinner,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { ArrowDownloadRegular, FolderRegular } from '@fluentui/react-icons'
import { useMutation, useQuery } from '@tanstack/react-query'
import { useMemo, useRef, useState } from 'react'
import {
  downloadShaderPack,
  listSlangpDir,
  pickFolder,
  shaderPackStatus,
  type SlangpEntry,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { errorPatch } from '../lib/toast'
import { Trans, useTranslation } from 'react-i18next'

const ROOT_KEY = 'reemu.shaderLibRoot'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  bar: { display: 'flex', gap: tokens.spacingHorizontalS, alignItems: 'center' },
  path: {
    flexGrow: 1,
    minWidth: 0,
    overflowX: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    color: tokens.colorNeutralForeground3,
    fontVariantNumeric: 'tabular-nums',
  },
  list: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXXS,
    maxHeight: '340px',
    overflowY: 'auto',
    paddingRight: tokens.spacingHorizontalXS,
  },
  panel: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXXS },
  item: {
    justifyContent: 'flex-start',
    fontWeight: tokens.fontWeightRegular,
  },
  itemOn: {
    backgroundColor: tokens.colorBrandBackground2,
    ':hover': { backgroundColor: tokens.colorBrandBackground2Hover },
  },
})

/**
 * Navegador da pasta de shaders do usuário (RetroArch / RetroBat —
 * `shaders_slang`). Lista os `.slangp` agrupados por subpasta; clicar aplica
 * via `onPick`. A raiz escolhida fica no `localStorage` (conveniência de UI —
 * o shader ativo em si é persistido no banco pelo `set_shader`).
 */
export function ShaderLibrary({
  onPick,
  activePath,
  busy,
}: {
  onPick: (path: string) => void
  activePath: string
  busy: boolean
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const [root, setRoot] = useState<string>(() => {
    try {
      return localStorage.getItem(ROOT_KEY) ?? ''
    } catch {
      return ''
    }
  })
  const [filter, setFilter] = useState('')
  // grupos abertos (só relevante quando há > 3 grupos e sem filtro — senão
  // tudo fica aberto).
  const [openGroups, setOpenGroups] = useState<string[]>([])
  const push = useToastStore((st) => st.push)
  const updateToast = useToastStore((st) => st.update)
  const dlId = useRef<string | null>(null)

  const pack = useQuery({
    queryKey: ['shader-pack'],
    queryFn: shaderPackStatus,
    retry: false,
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
      const k = e.category || t('shaders.root')
      const arr = by.get(k)
      if (arr) arr.push(e)
      else by.set(k, [e])
    }
    return [...by.entries()]
  }, [q.data, filter, t])

  if (!root) {
    return (
      <div className={s.root}>
        <Caption1>
          <Trans i18nKey="shaders.intro" components={{ code: <code /> }} />
        </Caption1>
        <div className={s.bar}>
          <Button
            appearance="primary"
            icon={<ArrowDownloadRegular />}
            disabled={dl.isPending}
            onClick={() => dl.mutate()}
          >
            {dl.isPending ? t('shaders.downloading') : t('shaders.download')}
          </Button>
          <Button icon={<FolderRegular />} onClick={chooseRoot}>
            {t('shaders.chooseFolder')}
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className={s.root}>
      <div className={s.bar}>
        <span className={s.path} title={root}>
          {root}
        </span>
        <Button size="small" appearance="subtle" onClick={chooseRoot}>
          {t('shaders.change')}
        </Button>
      </div>

      {q.isLoading && <Spinner size="tiny" label={t('shaders.scanning')} />}
      {q.isError && <Body1>{t('shaders.readFailed', { error: String(q.error) })}</Body1>}

      {q.data && (
        <>
          <div className={s.bar}>
            <Input
              size="small"
              placeholder={t('shaders.filter')}
              value={filter}
              onChange={(_, d) => setFilter(d.value)}
              style={{ flex: 1 }}
            />
            <Caption1>{t('shaders.presets', { count: q.data.length })}</Caption1>
          </div>
          <div className={s.list}>
            {groups.length === 0 && <Caption1>{t('shaders.nothing')}</Caption1>}
            <Accordion
              multiple
              collapsible
              openItems={
                groups.length <= 3 || filter.trim()
                  ? groups.map(([cat]) => cat)
                  : openGroups
              }
              onToggle={(_, d) => setOpenGroups(d.openItems as string[])}
            >
              {groups.map(([cat, items]) => (
                <AccordionItem key={cat} value={cat}>
                  <AccordionHeader>
                    <Caption1>
                      {cat} · {items.length}
                    </Caption1>
                  </AccordionHeader>
                  <AccordionPanel className={s.panel}>
                    {items.map((e) => (
                      <Button
                        key={e.path}
                        appearance="subtle"
                        size="small"
                        disabled={busy}
                        className={mergeClasses(
                          s.item,
                          e.path === activePath && s.itemOn,
                        )}
                        onClick={() => onPick(e.path)}
                        title={e.path}
                      >
                        {e.name}
                      </Button>
                    ))}
                  </AccordionPanel>
                </AccordionItem>
              ))}
            </Accordion>
          </div>
        </>
      )}
    </div>
  )
}
