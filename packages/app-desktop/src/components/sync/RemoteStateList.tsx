import { Body1, Button, Caption1, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { ArrowDownloadRegular, WarningRegular } from '@fluentui/react-icons'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { stateDownloadError, useRemoteStates } from '../../hooks/useSync'
import { sysToast } from '../../lib/toast'
import { syncDownloadState, type RemoteState, type SaveState } from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { SaveStateThumb } from '../SaveStateThumb'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: '8px' },
  title: { color: tokens.colorNeutralForeground3 },
  row: {
    display: 'flex',
    alignItems: 'center',
    columnGap: '12px',
    padding: '8px',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground1,
  },
  // O que não carrega aqui fica apagado e sem ação: não é alvo do foco.
  dimmed: { opacity: 0.6 },
  info: { display: 'flex', flexDirection: 'column', rowGap: '4px', flexGrow: 1, minWidth: 0 },
  actions: { display: 'flex', flexWrap: 'wrap', columnGap: '8px', rowGap: '4px' },
  // No menu estreito o botão tem o tamanho do conteúdo, como o Carregar dos locais.
  fit: { alignSelf: 'flex-start' },
  // O aviso dos que não carregam aqui. Não é um `Badge` (altura fixa): num menu
  // estreito o texto quebra, e a borda tem de acompanhar as linhas.
  pill: {
    display: 'flex',
    alignItems: 'flex-start',
    columnGap: '4px',
    minWidth: 0,
    maxWidth: '100%',
    padding: '2px 8px',
    borderRadius: tokens.borderRadiusMedium,
    border: `1px solid ${tokens.colorPaletteYellowBorder1}`,
    backgroundColor: tokens.colorPaletteYellowBackground1,
    color: tokens.colorPaletteYellowForeground1,
    fontSize: tokens.fontSizeBase200,
    lineHeight: tokens.lineHeightBase200,
    overflowWrap: 'anywhere',
  },
  pillIcon: { flexShrink: 0, marginTop: '2px' },
  muted: { color: tokens.colorNeutralForeground3, overflowWrap: 'anywhere' },
})

/**
 * Os save states das outras máquinas para um jogo, depois dos locais (D116). O
 * que tem o mesmo core e a mesma versão baixa para uma vaga daqui e é
 * entregue a `onDownloaded` (a página abre o jogo com ele; o menu de pausa o
 * carrega no core aberto). O resto fica apagado, com o motivo e sem ação. Sem
 * sync, sem states ou sem resposta da nuvem nada aparece.
 */
export function RemoteStateList({
  romId,
  coreId,
  variant = 'page',
  onDownloaded,
}: {
  romId: string
  /** O core escolhido. `null` (só o menu de pausa): o que está aberto. */
  coreId: string | null
  /** `pause`: o menu estreito, com o botão debaixo do texto, na coluna dos
   *  botões dos states locais (o controle desce do último local para ele). */
  variant?: 'page' | 'pause'
  onDownloaded: (state: SaveState) => void | Promise<void>
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const push = useToastStore((st) => st.push)
  const remote = useRemoteStates(romId, coreId)
  // A vaga ocupada pergunta antes de trocar; só uma pergunta por vez.
  const [asking, setAsking] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const busyRef = useRef(false)
  const cancelRef = useRef<HTMLButtonElement>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  // A linha cuja pergunta acabou de fechar: o foco volta ao botão dela, que
  // reaparece no lugar do par Substituir/Cancelar.
  const back = useRef<string | null>(null)
  // Um download que termina depois de a lista sair da tela (a pessoa fechou o
  // painel ou abriu o jogo) não abre nem carrega nada: a tela já é outra.
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  // A pergunta abre com o foco no "Cancelar" (o controle não confirma sem querer).
  useEffect(() => {
    if (asking) cancelRef.current?.focus()
  }, [asking])
  useEffect(() => {
    if (back.current === null || asking !== null || busy !== null) return
    const id = back.current
    back.current = null
    const button = [...(rootRef.current?.querySelectorAll<HTMLElement>('[data-download]') ?? [])].find(
      (b) => b.dataset.download === id,
    )
    button?.focus()
  }, [asking, busy])

  const states = remote.data?.states ?? []
  if (remote.data === undefined || remote.data.offline || states.length === 0) return null
  const installed = remote.data.installedVersion

  const download = async (st: RemoteState, overwrite: boolean) => {
    if (busyRef.current) return
    busyRef.current = true
    setBusy(st.id)
    try {
      const saved = await syncDownloadState(romId, coreId, st.id, overwrite)
      if (!mounted.current) return
      setAsking(null)
      await onDownloaded(saved)
    } catch (err) {
      if (err === 'slot_occupied' || (err instanceof Error && err.message === 'slot_occupied')) {
        setAsking(st.id)
      } else {
        setAsking(null)
        back.current = st.id
        push(sysToast(t(`sync.state.err.${stateDownloadError(err)}`), 'Error'))
      }
    } finally {
      busyRef.current = false
      setBusy(null)
    }
  }

  const machine = (st: RemoteState) => st.machine || t('sync.state.unknownMachine')
  const slotName = (st: RemoteState) =>
    st.slot === 0 ? t('play.quickSave') : st.slot != null ? t('game.slot', { n: st.slot }) : t('game.auto')
  const when = (st: RemoteState) => new Date(st.createdAt * 1000).toLocaleString(i18n.language)
  const why = (st: RemoteState) => {
    const core = st.core
    if (st.reason === 'other_core') return t('sync.state.otherCore', { core, machine: machine(st) })
    // A versão de lá é conhecida: mostra as duas, e diz quando a daqui não é.
    if (st.coreVersion) {
      return installed
        ? t('sync.state.otherVersion', { core, version: st.coreVersion, machine: machine(st), installed })
        : t('sync.state.otherVersionUnknown', { core, version: st.coreVersion, machine: machine(st) })
    }
    return t('sync.state.unversioned', { core, machine: machine(st) })
  }

  const action = (st: RemoteState) =>
    asking === st.id ? (
      <div className={mergeClasses(s.actions, variant === 'pause' && s.fit)}>
        <Button appearance="primary" disabledFocusable={busy !== null} onClick={() => void download(st, true)}>
          {t('sync.state.replace')}
        </Button>
        <Button ref={cancelRef} disabledFocusable={busy !== null} onClick={() => {
            back.current = st.id
            setAsking(null)
          }}>
          {t('sync.state.cancel')}
        </Button>
      </div>
    ) : (
      <Button
        className={variant === 'pause' ? s.fit : undefined}
        data-download={st.id}
        icon={<ArrowDownloadRegular />}
        disabledFocusable={busy !== null}
        onClick={() => void download(st, false)}
      >
        {t('sync.state.download')}
      </Button>
    )

  return (
    <div ref={rootRef} className={s.root} data-testid="remote-states">
      <Caption1 className={s.title}>{t('sync.state.title')}</Caption1>
      {states.map((st) =>
        st.available ? (
          <div key={st.id} className={s.row} data-remote-state="available">
            <SaveStateThumb stateId={st.id} hasThumbnail={st.hasThumbnail} remote />
            <div className={s.info}>
              <Body1>{slotName(st)}</Body1>
              <Caption1 className={s.muted}>
                {t('sync.state.sameCore', { machine: machine(st), when: when(st) })}
              </Caption1>
              {asking === st.id && <Caption1>{t('sync.state.confirm')}</Caption1>}
              {variant === 'pause' && action(st)}
            </div>
            {variant === 'page' && action(st)}
          </div>
        ) : (
          <div key={st.id} className={mergeClasses(s.row, s.dimmed)} data-remote-state="unavailable">
            <SaveStateThumb stateId={st.id} hasThumbnail={st.hasThumbnail} remote />
            <div className={s.info}>
              <Body1>{slotName(st)}</Body1>
              <Caption1 className={s.muted}>{when(st)}</Caption1>
              <span className={s.pill}>
                <WarningRegular className={s.pillIcon} aria-hidden />
                <span>{why(st)}</span>
              </span>
            </div>
          </div>
        ),
      )}
    </div>
  )
}

