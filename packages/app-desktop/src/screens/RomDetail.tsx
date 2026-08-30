import { Body1, Button, Caption1, Select, Spinner } from '@fluentui/react-components'
import { ArrowLeftRegular, DeleteRegular, PlayRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { CoreOptions } from '../components/CoreOptions'
import { ShaderParams } from '../components/ShaderParams'
import { sysToast } from '../lib/toast'
import {
  deleteSaveState,
  getRomMetadata,
  getRomShader,
  getShaderInfo,
  listInstalledCores,
  listRoms,
  listSaveStates,
  pickSlangp,
  removeRom,
  setShader,
} from '../lib/tauri'
import { useDetailStyles } from '../styles/xbox'
import { useToastStore } from '../stores/useToastStore'

export function RomDetail() {
  const s = useDetailStyles()
  const navigate = useNavigate()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const { romId = '' } = useParams()

  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false })
  const cores = useQuery({ queryKey: ['installed-cores'], queryFn: listInstalledCores, retry: false })
  const states = useQuery({
    queryKey: ['save-states', romId],
    queryFn: () => listSaveStates(romId),
    retry: false,
  })
  const meta = useQuery({
    queryKey: ['rom-metadata', romId],
    queryFn: () => getRomMetadata(romId),
    retry: false,
  })
  const shaderInfo = useQuery({ queryKey: ['shader-info'], queryFn: getShaderInfo, retry: false })
  const romShader = useQuery({
    queryKey: ['rom-shader', romId],
    queryFn: () => getRomShader(romId),
    retry: false,
  })
  const shaderPick = useMutation({
    mutationFn: (name: string) => setShader(name, 'rom', romId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['rom-shader', romId] })
      qc.invalidateQueries({ queryKey: ['shader-info'] })
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })
  const currentGameShader =
    romShader.data?.fromRom && romShader.data.sourcePath
      ? (romShader.data.sourcePath.split(/[/\\]/).pop() ?? '')
      : ''

  const rom = roms.data?.find((r) => r.id === romId)
  const ext = rom?.filePath.split('.').pop()?.toLowerCase() ?? ''
  const coreList = [...(cores.data ?? [])].sort((a, b) => {
    const am = a.extensions.includes(ext) ? 0 : 1
    const bm = b.extensions.includes(ext) ? 0 : 1
    return am - bm || a.name.localeCompare(b.name)
  })
  const [coreId, setCoreId] = useState('')
  const chosenCore = coreId || coreList[0]?.coreId || ''
  const [confirmRemove, setConfirmRemove] = useState(false)

  const del = useMutation({
    mutationFn: (id: string) => deleteSaveState(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['save-states', romId] }),
    onError: (e) => push(sysToast(`Falha ao apagar: ${e}`, 'Error')),
  })
  const remove = useMutation({
    mutationFn: () => removeRom(romId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['roms'] })
      push(sysToast(`"${rom?.title ?? 'ROM'}" removida da biblioteca.`, 'Success'))
      navigate('/library')
    },
    onError: (e) => push(sysToast(`Falha ao remover: ${e}`, 'Error')),
  })

  if (roms.isLoading) return <Spinner label="Carregando…" />
  if (!rom)
    return (
      <Body1>
        ROM não encontrada.{' '}
        <Button appearance="transparent" onClick={() => navigate('/library')}>
          Voltar
        </Button>
      </Body1>
    )

  const cover = meta.data?.coverUrl ?? rom.boxart
  const title = meta.data?.title ?? rom.title
  const hasQuick = states.data?.some((st) => st.slot === 0) ?? false

  const play = () => {
    if (!chosenCore) return
    navigate(`/play/${rom.id}?core=${encodeURIComponent(chosenCore)}`, {
      state: {
        coreId: chosenCore,
        romPath: rom.filePath,
        title,
        boxart: cover,
        system: rom.systemId,
      },
    })
  }

  return (
    <div className={s.root}>
      <button className={s.back} onClick={() => navigate(-1)}>
        <ArrowLeftRegular /> Voltar
      </button>

      <div className={s.hero}>
        {cover && (
          <img
            className={s.heroArt}
            src={cover}
            alt=""
            onError={(e) => {
              e.currentTarget.style.display = 'none'
            }}
          />
        )}
        <div className={s.heroScrim} />
        <div className={s.heroBody}>
          <h1 className={s.title}>{title}</h1>
          <div className={s.badges}>
            <span className={s.badge}>{rom.systemId}</span>
            {meta.data?.releaseDate && <span className={s.badge}>{meta.data.releaseDate}</span>}
            {meta.data?.genre && <span className={s.badge}>{meta.data.genre}</span>}
          </div>
          {meta.data?.description && <p className={s.desc}>{meta.data.description}</p>}
          <div className={s.path}>{rom.filePath}</div>
        </div>
      </div>

      <div className={s.actions}>
        <label className={s.field}>
          <Caption1>Core</Caption1>
          <Select
            value={chosenCore}
            disabled={coreList.length === 0}
            onChange={(_, d) => setCoreId(d.value)}
          >
            {coreList.length === 0 && <option value="">nenhum instalado</option>}
            {coreList.map((c) => (
              <option key={c.coreId} value={c.coreId}>
                {c.name}
                {c.extensions.includes(ext) ? ' ✓' : ''}
              </option>
            ))}
          </Select>
        </label>
        <Button
          appearance="primary"
          size="large"
          icon={<PlayRegular />}
          disabled={!chosenCore}
          onClick={play}
        >
          {hasQuick ? 'Continuar' : 'Jogar'}
        </Button>
      </div>
      {coreList.length === 0 && (
        <Caption1>Instale um core em Configurações → Cores pra poder jogar.</Caption1>
      )}

      {shaderInfo.data?.gpu && (
        <section className={s.section}>
          <h2 className={s.sectionTitle}>Shader deste jogo</h2>
          <div className={s.panel}>
            <Select
              value={currentGameShader}
              disabled={shaderPick.isPending}
              onChange={(_, d) => shaderPick.mutate(d.value)}
            >
              <option value="">Padrão da biblioteca</option>
              {shaderInfo.data.available.map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
              {romShader.data?.fromRom &&
                romShader.data.sourcePath &&
                !shaderInfo.data.available.includes(romShader.data.sourcePath) && (
                  <option value={romShader.data.sourcePath}>{currentGameShader}</option>
                )}
            </Select>
            <Button
              size="small"
              appearance="subtle"
              disabled={shaderPick.isPending}
              onClick={async () => {
                const p = await pickSlangp()
                if (p) shaderPick.mutate(p)
              }}
            >
              Carregar .slangp…
            </Button>
            {romShader.data?.fromRom && (
              <ShaderParams scope="rom" romId={romId} reloadKey={currentGameShader} />
            )}
          </div>
        </section>
      )}

      {chosenCore && (
        <section className={s.section}>
          <h2 className={s.sectionTitle}>Opções do core</h2>
          <div className={s.panel}>
            <CoreOptions coreId={chosenCore} />
          </div>
        </section>
      )}

      <section className={s.section}>
        <h2 className={s.sectionTitle}>Save states</h2>
        <div className={s.panel}>
          {states.data?.length === 0 && <Caption1>Nenhum save state pra esta ROM.</Caption1>}
          {states.data?.map((st) => (
            <div key={st.id} className={s.stateRow}>
              <Body1>
                {st.slot === 0 ? 'QuickSave' : st.slot != null ? `Slot ${st.slot}` : 'Auto'}{' '}
                <Caption1>· {new Date(st.createdAt * 1000).toLocaleString()}</Caption1>
              </Body1>
              <Button
                size="small"
                appearance="subtle"
                disabled={del.isPending}
                onClick={() => del.mutate(st.id)}
              >
                Apagar
              </Button>
            </div>
          ))}
          <Caption1 className={s.hint}>
            Carregar um save state é feito de dentro do jogo (menu de pausa).
          </Caption1>
        </div>
      </section>

      <section className={s.section}>
        <Button
          appearance="subtle"
          icon={<DeleteRegular />}
          disabled={remove.isPending}
          onClick={() => {
            if (confirmRemove) remove.mutate()
            else {
              setConfirmRemove(true)
              window.setTimeout(() => setConfirmRemove(false), 3000)
            }
          }}
        >
          {confirmRemove ? 'Clique de novo para confirmar' : 'Remover da biblioteca'}
        </Button>
        <Caption1 className={s.hint}>
          Remove só da lista — o arquivo em disco fica e um novo scan readiciona.
        </Caption1>
      </section>
    </div>
  )
}
