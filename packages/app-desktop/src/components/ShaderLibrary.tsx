import {
  Body1,
  Button,
  Caption1,
  Input,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { FolderRegular } from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { listSlangpDir, pickFolder, type SlangpEntry } from '../lib/tauri'

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
  group: { marginTop: tokens.spacingVerticalXS },
  groupHead: {
    cursor: 'pointer',
    color: tokens.colorNeutralForeground2,
    paddingTop: tokens.spacingVerticalXXS,
    paddingBottom: tokens.spacingVerticalXXS,
    userSelect: 'none',
  },
  item: {
    display: 'block',
    width: '100%',
    textAlign: 'left',
    paddingTop: tokens.spacingVerticalXS,
    paddingBottom: tokens.spacingVerticalXS,
    paddingLeft: tokens.spacingHorizontalS,
    paddingRight: tokens.spacingHorizontalS,
    borderRadius: tokens.borderRadiusMedium,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground1,
    color: 'inherit',
    cursor: 'pointer',
  },
  itemOn: {
    border: `1px solid ${tokens.colorBrandStroke1}`,
    backgroundColor: tokens.colorBrandBackground2,
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
          Aponte pra pasta <code>shaders_slang</code> do RetroArch/RetroBat pra
          navegar os presets aqui dentro.
        </Caption1>
        <Button icon={<FolderRegular />} onClick={chooseRoot}>
          Escolher pasta de shaders…
        </Button>
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
            {groups.map(([cat, items]) => (
              <details key={cat} className={s.group} open={groups.length <= 3 || !!filter.trim()}>
                <summary className={s.groupHead}>
                  <Caption1>
                    {cat} · {items.length}
                  </Caption1>
                </summary>
                {items.map((e) => (
                  <button
                    key={e.path}
                    type="button"
                    disabled={busy}
                    className={`${s.item} ${e.path === activePath ? s.itemOn : ''}`}
                    onClick={() => onPick(e.path)}
                    title={e.path}
                  >
                    {e.name}
                  </button>
                ))}
              </details>
            ))}
          </div>
        </>
      )}
    </div>
  )
}
