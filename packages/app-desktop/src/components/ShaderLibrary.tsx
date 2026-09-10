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
  const push = useToastStore((t) => t.push)
  const updateToast = useToastStore((t) => t.update)
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
              ? 'Extraindo pacote de shaders…'
              : `Baixando shaders ${mb(p.received)}${p.total ? `/${mb(p.total)}` : ''} MB…`,
          progress: p.phase === 'extract' ? null : pct,
        })
      }),
    onMutate: () => {
      const id = crypto.randomUUID()
      dlId.current = id
      push({
        id,
        message: 'Baixando pacote de shaders…',
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
          message: 'Pacote de shaders instalado.',
          variant: 'Success',
          durationMs: 4000,
          progress: undefined,
        })
    },
    onError: (e) => {
      if (dlId.current)
        updateToast(dlId.current, {
          message: `Falha no download: ${e}`,
          variant: 'Error',
          durationMs: 6000,
          progress: undefined,
        })
    },
    onSettled: () => {
      dlId.current = null
    },
  })

  const chooseRoot = async () => {
    const p = await pickFolder('Escolha a pasta de shaders (shaders_slang)')
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
      const k = e.category || '(raiz)'
      const arr = by.get(k)
      if (arr) arr.push(e)
      else by.set(k, [e])
    }
    return [...by.entries()]
  }, [q.data, filter])

  if (!root) {
    return (
      <div className={s.root}>
        <Caption1>
          Baixe o pacote de slang shaders — os mesmos do RetroArch (Online
          Updater → Update Slang Shaders) — ou aponte pra pasta{' '}
          <code>shaders_slang</code> do RetroArch/RetroBat.
        </Caption1>
        <div className={s.bar}>
          <Button
            appearance="primary"
            icon={<ArrowDownloadRegular />}
            disabled={dl.isPending}
            onClick={() => dl.mutate()}
          >
            {dl.isPending ? 'Baixando…' : 'Baixar pacote de shaders'}
          </Button>
          <Button icon={<FolderRegular />} onClick={chooseRoot}>
            Escolher pasta…
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
          Trocar
        </Button>
      </div>

      {q.isLoading && <Spinner size="tiny" label="Varrendo…" />}
      {q.isError && <Body1>Falha ao ler a pasta: {String(q.error)}</Body1>}

      {q.data && (
        <>
          <div className={s.bar}>
            <Input
              size="small"
              placeholder="Filtrar…"
              value={filter}
              onChange={(_, d) => setFilter(d.value)}
              style={{ flex: 1 }}
            />
            <Caption1>{q.data.length} presets</Caption1>
          </div>
          <div className={s.list}>
            {groups.length === 0 && <Caption1>Nada encontrado.</Caption1>}
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
