import { Button, Field, Input, ProgressBar, Spinner } from '@fluentui/react-components'
import { AddRegular, DeleteRegular, FolderRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { GameCard } from '../components/GameCard'
import { sysToast } from '../lib/toast'
import {
  clearLibrary,
  listRomSources,
  listRoms,
  pickFolder,
  removeRom,
  removeRomSource,
  removeRomSystem,
  scanLibrary,
  type RomEntry,
  type ScanProgress,
} from '../lib/tauri'
import { useSearchStore } from '../stores/useSearchStore'
import { useToastStore } from '../stores/useToastStore'
import { useBrowseStyles } from '../styles/xbox'

/**
 * "Meus jogos" — a biblioteca completa: grade vertical agrupada por sistema,
 * busca, adicionar/remover ROMs. A tela inicial (`/`) com hero e faixas
 * curadas fica em `screens/Home`.
 */
export function Library() {
  const s = useBrowseStyles()
  const qc = useQueryClient()
  const navigate = useNavigate()
  const push = useToastStore((s) => s.push)
  const query = useSearchStore((s) => s.query).trim().toLowerCase()
  const [dir, setDir] = useState('')
  const [showScan, setShowScan] = useState(false)
  const [showManage, setShowManage] = useState(false)
  const [progress, setProgress] = useState<ScanProgress | null>(null)

  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false })

  const scan = useMutation({
    mutationFn: (path: string) => scanLibrary(path, setProgress),
    onSettled: () => setProgress(null),
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: ['roms'] })
      qc.invalidateQueries({ queryKey: ['romSources'] })
      setShowScan(false)
      push(
        sysToast(
          `${r.added} adicionada(s) · ${r.skippedKnown} já conhecida(s) · ${r.skippedUnrecognized} ignorada(s)`,
          r.errors > 0 ? 'Warning' : 'Success',
        ),
      )
    },
    onError: (e) => push(sysToast(`Scan falhou: ${e}`, 'Error')),
  })

  const del = useMutation({
    mutationFn: (id: string) => removeRom(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['roms'] })
      push(sysToast('Removida da biblioteca.', 'Success'))
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })

  const sources = useQuery({
    queryKey: ['romSources'],
    queryFn: listRomSources,
    enabled: showManage,
    retry: false,
  })
  // alvo aguardando 2º clique: '__all__' | `sys:<id>` | `<path da pasta>`
  const [confirmPurge, setConfirmPurge] = useState<string | null>(null)
  const purge = useMutation({
    mutationFn: (target: string) => {
      if (target === '__all__') return clearLibrary()
      if (target.startsWith('sys:')) return removeRomSystem(target.slice(4))
      return removeRomSource(target)
    },
    onSuccess: (n) => {
      qc.invalidateQueries({ queryKey: ['roms'] })
      qc.invalidateQueries({ queryKey: ['romSources'] })
      setConfirmPurge(null)
      push(sysToast(`${n} jogo(s) removido(s) da biblioteca.`, 'Success'))
    },
    onError: (e) => {
      setConfirmPurge(null)
      push(sysToast(`Falha: ${e}`, 'Error'))
    },
  })
  // botão de remoção com confirmação em 2 cliques (usado nas listas e nos
  // cabeçalhos de sistema).
  const purgeBtn = (target: string, idle: string, confirm: string) => (
    <Button
      size="small"
      icon={<DeleteRegular />}
      appearance={confirmPurge === target ? 'primary' : 'secondary'}
      disabled={purge.isPending}
      onClick={() => (confirmPurge === target ? purge.mutate(target) : setConfirmPurge(target))}
    >
      {confirmPurge === target ? confirm : idle}
    </Button>
  )

  const all = useMemo(() => roms.data ?? [], [roms.data])
  const filtered = useMemo(
    () => (query ? all.filter((r) => r.title.toLowerCase().includes(query)) : all),
    [all, query],
  )
  const bySystem = useMemo(() => {
    const groups = new Map<string, RomEntry[]>()
    for (const r of filtered) {
      const list = groups.get(r.systemId) ?? []
      list.push(r)
      groups.set(r.systemId, list)
    }
    return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [filtered])

  const cardMenu = (r: RomEntry) =>
    [
      { label: 'Abrir', onClick: () => navigate(`/rom/${r.id}`) },
      { label: 'Remover da biblioteca', onClick: () => del.mutate(r.id) },
    ] as const

  const grid = (list: readonly RomEntry[]) => (
    <div className={s.grid}>
      {list.map((r) => (
        <GameCard
          key={r.id}
          title={r.title}
          badge={r.systemId}
          boxart={r.boxart}
          onClick={() => navigate(`/rom/${r.id}`)}
          menu={cardMenu(r)}
        />
      ))}
    </div>
  )

  const scanPanel = (
    <Field
      style={{ maxWidth: 640 }}
      validationState="none"
      validationMessage={
        scan.isPending && progress
          ? `${progress.current}${progress.total ? `/${progress.total}` : ''} — ${progress.file}`
          : undefined
      }
    >
      <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
        <Input
          style={{ flex: 1 }}
          value={dir}
          placeholder="/caminho/para/suas/ROMs"
          contentBefore={<FolderRegular />}
          onChange={(_, d) => setDir(d.value)}
        />
        <Button
          icon={<FolderRegular />}
          onClick={async () => {
            const p = await pickFolder()
            if (p) setDir(p)
          }}
        >
          Procurar…
        </Button>
        <Button appearance="primary" disabled={!dir || scan.isPending} onClick={() => scan.mutate(dir)}>
          {scan.isPending ? 'Escaneando…' : 'Escanear'}
        </Button>
      </div>
      {scan.isPending && (
        <ProgressBar
          style={{ marginTop: 10 }}
          value={progress && progress.total ? progress.current / progress.total : undefined}
        />
      )}
    </Field>
  )

  // --- busca ativa: grade única com o resultado ---
  if (query) {
    return (
      <div>
        <div className={s.sectionHead}>
          <h2 className={s.sectionTitle}>Resultados</h2>
        </div>
        <div className={s.sectionSub}>
          {filtered.length} {filtered.length === 1 ? 'jogo' : 'jogos'} para "{query}"
        </div>
        {filtered.length === 0 ? (
          <div className={s.empty}>
            <div className={s.emptyIcon}>🔍</div>
            <h2>Nada encontrado</h2>
          </div>
        ) : (
          grid(filtered)
        )}
      </div>
    )
  }

  return (
    <div>
      <div className={s.sectionHead}>
        <h2 className={s.sectionTitle}>Meus jogos</h2>
      </div>

      <div className={s.toolbar}>
        <button className={s.chip} onClick={() => setShowScan((v) => !v)}>
          <AddRegular /> Adicionar ROMs
        </button>
        <button className={s.chip} onClick={() => setShowManage((v) => !v)}>
          <DeleteRegular /> Gerenciar biblioteca
        </button>
        <span className={s.count}>
          {all.length} {all.length === 1 ? 'jogo' : 'jogos'}
        </span>
      </div>
      {showScan && <div style={{ marginTop: 6 }}>{scanPanel}</div>}
      {showManage && (
        <div className={s.libManage}>
          <div className={s.sectionSub}>Por sistema</div>
          {bySystem.length === 0 && <div className={s.count}>Biblioteca vazia.</div>}
          {bySystem.map(([system, list]) => (
            <div key={system} className={s.libRow}>
              <span className={s.libPath}>{system.toUpperCase()}</span>
              <span className={s.count}>{list.length}</span>
              {purgeBtn(`sys:${system}`, 'Remover', 'Confirmar')}
            </div>
          ))}

          {(sources.data?.length ?? 0) > 1 && (
            <>
              <div className={s.sectionSub} style={{ marginTop: 12 }}>
                Por pasta de origem
              </div>
              {sources.data!.map((src) => (
                <div key={src.path} className={s.libRow}>
                  <span className={s.libPath} title={src.path}>
                    {src.path}
                  </span>
                  <span className={s.count}>{src.count}</span>
                  {purgeBtn(src.path, 'Remover', 'Confirmar')}
                </div>
              ))}
            </>
          )}

          <div className={s.libRow} style={{ marginTop: 12 }}>
            <span className={s.libPath}>Toda a biblioteca ({all.length} jogos)</span>
            {purgeBtn('__all__', 'Limpar tudo', 'Confirmar: apagar tudo')}
          </div>
        </div>
      )}

      {roms.isLoading && <Spinner style={{ marginTop: 40 }} label="Carregando biblioteca…" />}
      {roms.isError && (
        <div className={s.empty}>
          <div className={s.emptyIcon}>⚠</div>
          <h2>Biblioteca indisponível</h2>
          <span>O backend não conseguiu abrir o banco de dados.</span>
        </div>
      )}
      {!roms.isLoading && !roms.isError && all.length === 0 && (
        <div className={s.empty}>
          <div className={s.emptyIcon}>🕹</div>
          <h2>Nenhuma ROM ainda</h2>
          <span>Aponte a pasta das suas ROMs pra começar.</span>
          <Button appearance="primary" icon={<AddRegular />} onClick={() => setShowScan(true)}>
            Adicionar ROMs…
          </Button>
          {showScan && <div style={{ marginTop: 12 }}>{scanPanel}</div>}
        </div>
      )}

      {bySystem.map(([system, list]) => (
        <section className={s.section} key={system}>
          <div className={s.sectionHead}>
            <h2 className={s.sectionTitle}>{system.toUpperCase()}</h2>
            <span className={s.count}>
              {list.length} {list.length === 1 ? 'jogo' : 'jogos'}
            </span>
            <span style={{ flex: 1 }} />
            {purgeBtn(`sys:${system}`, 'Remover sistema', 'Confirmar remoção')}
          </div>
          {grid(list)}
        </section>
      ))}
    </div>
  )
}
