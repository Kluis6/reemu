import {
  Badge,
  Body1,
  Body1Strong,
  Button,
  Caption1,
  Field,
  Input,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  ProgressBar,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AlvanistaButton } from '../../components/AlvanistaButton'
import { LoadingState } from '../../components/EmptyState'
import { BlockGroup, FieldBlock, ToggleBlock } from '../../components/SettingBlock'
import { useAlvanistaConnect } from '../../hooks/useAlvanistaConnect'
import { ConflictDialog } from '../../components/sync/ConflictDialog'
import { SYNC_STATUS_KEY, SYNC_ERRORS, syncErrorCode, useConflicts, useSync } from '../../hooks/useSync'
import { alvanistaOpenSite, type SyncMachine } from '../../lib/tauri'
import { relativeAge } from '../profile/relativeAge'
import { formatBytes, hardwareLine, machineColor } from './syncFormat'

const SITE_HOST = 'alvanista.com/sync'
const LABEL_MAX = 80

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXL },
  stack: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalS, alignItems: 'flex-start' },
  waiting: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalM, flexWrap: 'wrap' },
  actions: { display: 'flex', flexWrap: 'wrap', columnGap: tokens.spacingHorizontalS, rowGap: tokens.spacingVerticalS },
  muted: { color: tokens.colorNeutralForeground3, overflowWrap: 'anywhere' },
  quota: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXS },
  quotaLine: { display: 'flex', justifyContent: 'space-between', columnGap: tokens.spacingHorizontalL, flexWrap: 'wrap' },
  rows: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalS },
  machine: {
    display: 'flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalL,
    padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalL}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  avatar: {
    flexShrink: 0,
    width: '40px',
    height: '40px',
    borderRadius: tokens.borderRadiusCircular,
    display: 'grid',
    placeItems: 'center',
    color: '#fff',
    fontWeight: tokens.fontWeightSemibold,
  },
  machineText: { display: 'flex', flexDirection: 'column', rowGap: '2px', minWidth: 0, flexGrow: 1 },
  machineName: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap' },
  rowTitle: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap' },
  machineSide: { display: 'flex', flexDirection: 'column', alignItems: 'flex-end', rowGap: '2px', flexShrink: 0 },
  site: { display: 'flex', alignItems: 'center', flexWrap: 'wrap', columnGap: tokens.spacingHorizontalXS },
})

/** RetroArch só guarda saves: sem hardware nem configurações do ReEmu. */
const isSavesOnly = (m: SyncMachine) => m.clientKind === 'retroarch'
/** Só outra máquina com o ReEmu tem configurações do ReEmu para copiar (D119). */
const canCopyFrom = (m: SyncMachine) => m.clientKind === 'reemu' && !m.isThis

function MachineRow({ m, lang, nowMs }: { m: SyncMachine; lang: string; nowMs: number }) {
  const { t } = useTranslation()
  const s = useStyles()
  const savesOnly = isSavesOnly(m)
  const hw = hardwareLine(m.hardware, lang)
  const seen = m.lastUsedAt ? Date.parse(m.lastUsedAt) : NaN
  return (
    <div className={s.machine}>
      <span className={s.avatar} style={{ backgroundColor: machineColor(m.id) }} aria-hidden>
        {(m.label.trim()[0] ?? '?').toUpperCase()}
      </span>
      <span className={s.machineText}>
        <span className={s.machineName}>
          <Body1Strong>{m.label}</Body1Strong>
          {m.isThis && (
            <Badge appearance="tint" color="brand">
              {t('sync.this')}
            </Badge>
          )}
        </span>
        <Caption1 className={s.muted}>{savesOnly ? t('sync.onlySaves') : hw}</Caption1>
      </span>
      <span className={s.machineSide}>
        <Caption1 className={s.muted}>
          {Number.isNaN(seen)
            ? t('sync.neverSeen')
            : t('sync.seenAt', { when: relativeAge(seen / 1000, nowMs, lang) })}
        </Caption1>
        {canCopyFrom(m) && (
          // Chega com o PR das configurações.
          <Button size="small" appearance="secondary" disabled>
            {t('sync.copy')} ({t('sync.soon')})
          </Button>
        )}
      </span>
    </div>
  )
}

/**
 * Configurações › Sincronização: esta máquina como aparelho da conta do
 * Alvanista (saves, states e, depois, configurações), com as outras máquinas.
 */
export function SettingsSync() {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const { status: query, machines, busy, error, setEnabled, setLabel, setPaused, setSaves, setStates, uploadOldStates, now, cancel } = useSync()
  // Quem conecta por aqui quer sincronizar: os dois escopos num consentimento só.
  const { connect, cancel: cancelConnect, connecting, outcome } = useAlvanistaConnect({ saves: true })
  const st = query.data
  const lang = i18n.language
  // Fixo por visita à tela: o "visto há" não precisa andar com a renderização.
  const [nowMs] = useState(() => Date.now())

  // Conectar por aqui muda a disponibilidade: relê o estado ao terminar.
  const wasConnecting = useRef(false)
  useEffect(() => {
    if (wasConnecting.current && !connecting) void qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
    wasConnecting.current = connecting
  }, [connecting, qc])

  // Voltou da página de planos: o foco da janela relê o estado, que confere o
  // plano no Alvanista. O foco automático do app é desligado (o WebKitGTK manda
  // foco espúrio), então só esta tela escuta, só depois de "Ver planos" e com
  // uma leitura por vez.
  const [plansOpened, setPlansOpened] = useState(false)
  const noTier = st?.availability === 'no_tier'
  useEffect(() => {
    if (!plansOpened || !noTier) return
    const recheck = () => {
      if (qc.isFetching({ queryKey: SYNC_STATUS_KEY }) === 0) {
        void qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
      }
    }
    window.addEventListener('focus', recheck)
    return () => window.removeEventListener('focus', recheck)
  }, [plansOpened, noTier, qc])

  // O nome em edição (null: o do estado). Enviado, fica na tela até a resposta
  // e volta a seguir o estado.
  const [edited, setEdited] = useState<string | null>(null)

  // "Resolver": os conflitos, um depois do outro. A fila guarda as chaves do
  // momento do clique; cada resolvido sai dela e o diálogo mostra a próxima.
  const conflictCount = query.data?.conflicts ?? 0
  const conflicts = useConflicts(conflictCount > 0)
  const [queue, setQueue] = useState<string[]>([])
  // A primeira da fila que ainda existe: uma resolvida em outro lugar é pulada.
  const asking = conflicts.data?.find((c) => queue.includes(c.key)) ?? null
  const deletedOnSite = conflicts.data?.filter((c) => c.remote.deleted).length ?? 0
  const twoMachines = (conflicts.data?.length ?? 0) - deletedOnSite
  const label = st?.label ?? ''
  const draft = edited ?? label
  const commitLabel = () => {
    // Outra ação em curso: o texto digitado fica até poder ser enviado.
    if (busy !== null || edited === null) return
    const next = draft.trim()
    if (next === label || next.length === 0 || next.length > LABEL_MAX) {
      setEdited(null)
      return
    }
    void setLabel(next).finally(() => setEdited((cur) => (cur === next ? null : cur)))
  }

  if (!st) {
    if (query.isError) {
      return (
        <MessageBar intent="warning">
          <MessageBarBody>{t(`sync.err.${syncErrorCode(query.error)}`)}</MessageBarBody>
        </MessageBar>
      )
    }
    return <LoadingState />
  }

  const offline = st.lastError === 'offline' || error === 'offline' || query.isError || machines.isError
  const openSite = (page: 'sync' | 'plans') => void alvanistaOpenSite(page).catch(() => {})
  const failed = error ?? (st.lastError && (SYNC_ERRORS as readonly string[]).includes(st.lastError) ? syncErrorCode(st.lastError) : null)
  // O banner de revogada já diz o mesmo.
  const showError = failed && failed !== 'offline' && !(failed === 'revoked' && st.revoked) ? failed : null

  if (st.availability === 'no_account') {
    return (
      <div className={s.root}>
        <div className={s.stack}>
          {connecting ? (
            <>
              <div className={s.waiting}>
                <Spinner size="tiny" />
                <Body1Strong>{t('alvanista.waiting')}</Body1Strong>
                <Button appearance="secondary" onClick={cancelConnect}>
                  {t('alvanista.cancel')}
                </Button>
              </div>
              <Caption1>{t('alvanista.waitingHint')}</Caption1>
            </>
          ) : (
            <>
              <AlvanistaButton onClick={connect} />
              <Caption1>{t('sync.explain')}</Caption1>
            </>
          )}
          {outcome && (
            <MessageBar intent="warning">
              <MessageBarBody>{t(`alvanista.${outcome}`)}</MessageBarBody>
            </MessageBar>
          )}
        </div>
      </div>
    )
  }

  if (st.availability === 'no_tier') {
    return (
      <div className={s.root}>
        <div className={s.stack}>
          <Body1>{t('sync.upgrade')}</Body1>
          <Button
            appearance="primary"
            onClick={() => {
              setPlansOpened(true)
              openSite('plans')
            }}
          >
            {t('sync.seePlans')}
          </Button>
        </div>
      </div>
    )
  }

  const enabling = busy === 'enable' && !st.enabled
  const working = busy !== null
  // A passada dos states pode estar rodando por trás destas duas ações: o
  // interruptor dos states segue valendo, e desligá-lo a para no meio.
  const statesFree = busy === null || busy === 'now' || busy === 'oldStates'
  const pausedSite = st.pausedSite
  const quota = st.quota
  const used = quota ? Math.min(quota.usedBytes, quota.limitBytes) : 0
  const list = machines.data ?? []

  return (
    <div className={s.root}>
      {offline && (
        <Badge appearance="tint" color="warning" style={{ alignSelf: 'flex-start' }}>
          {t('alvanista.offlineBadge')}
        </Badge>
      )}
      {st.revoked && (
        <MessageBar intent="warning">
          <MessageBarBody>{t('sync.revoked')}</MessageBarBody>
          <MessageBarActions>
            <Button disabledFocusable={working} onClick={() => void setEnabled(true)}>
              {t('sync.relink')}
            </Button>
          </MessageBarActions>
        </MessageBar>
      )}
      {st.enabled && pausedSite && (
        <MessageBar intent="warning">
          <MessageBarBody>{t('sync.pausedSite')}</MessageBarBody>
        </MessageBar>
      )}
      {showError && (
        <MessageBar intent="warning">
          <MessageBarBody>
            {t(`sync.err.${showError}`)}
            {/* Um save que a nuvem recusou: a página diz qual. */}
            {showError === st.lastError && st.lastErrorFile && (
              <>
                <br />
                {t('sync.errFile', { file: st.lastErrorFile })}
              </>
            )}
          </MessageBarBody>
        </MessageBar>
      )}

      <BlockGroup title={t('sync.thisMachine')}>
        <ToggleBlock
          label={t('sync.toggle')}
          description={t('sync.toggleHint')}
          checked={st.enabled}
          busy={working}
          onChange={(on) => {
            if (!working) void setEnabled(on)
          }}
        />
        {enabling && (
          <FieldBlock wide>
            <div className={s.waiting}>
              <Spinner size="tiny" />
              <Body1Strong>{t('alvanista.waiting')}</Body1Strong>
              <Button appearance="secondary" onClick={cancel}>
                {t('alvanista.cancel')}
              </Button>
            </div>
            <Caption1>{t('alvanista.waitingHint')}</Caption1>
          </FieldBlock>
        )}
        {st.enabled && (
          <>
            <FieldBlock wide>
              <Field label={t('sync.name')} hint={t('sync.nameHint')}>
                <Input
                  appearance="filled-darker"
                  value={draft}
                  maxLength={LABEL_MAX}
                  // Servidor responde 423 a tudo, menos a revogação, enquanto
                  // o aparelho está pausado no site.
                  disabled={pausedSite}
                  // Durante o próprio envio o campo segue focável, como o toggle:
                  // o foco do controle não some.
                  aria-busy={busy === 'label' || undefined}
                  onChange={(_, d) => setEdited(d.value)}
                  onBlur={commitLabel}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') commitLabel()
                  }}
                />
              </Field>
              {hardwareLine(st.hardware, lang) !== '' && (
                <Caption1 className={s.muted}>{hardwareLine(st.hardware, lang)}</Caption1>
              )}
              <Caption1 className={s.muted}>{t('sync.hardwareNote')}</Caption1>
            </FieldBlock>
            {quota && (
              <FieldBlock wide>
                <div className={s.quota}>
                  <div className={s.quotaLine}>
                    <Body1>{t('sync.space')}</Body1>
                    <Caption1 className={s.muted}>
                      {t('sync.quotaOf', {
                        used: formatBytes(used, lang),
                        limit: formatBytes(quota.limitBytes, lang),
                      })}
                      {machines.data ? ` · ${t('sync.machineCount', { count: list.length })}` : ''}
                    </Caption1>
                  </div>
                  <ProgressBar aria-label={t('sync.space')} value={quota.limitBytes > 0 ? used / quota.limitBytes : 0} />
                </div>
              </FieldBlock>
            )}
            <FieldBlock wide>
              <div className={s.actions}>
                <Button
                  appearance="primary"
                  disabledFocusable={working || st.pausedLocal || pausedSite}
                  icon={busy === 'now' ? <Spinner size="tiny" /> : undefined}
                  onClick={() => void now()}
                >
                  {t('sync.now')}
                </Button>
                <Button
                  appearance="secondary"
                  disabledFocusable={working || pausedSite}
                  onClick={() => void setPaused(!st.pausedLocal)}
                >
                  {st.pausedLocal ? t('sync.resume') : t('sync.pause')}
                </Button>
              </div>
            </FieldBlock>
          </>
        )}
      </BlockGroup>

      <BlockGroup title={t('sync.what')}>
        <ToggleBlock
          label={t('sync.saves')}
          description={t('sync.savesHint')}
          checked={st.savesEnabled}
          disabled={!st.enabled}
          busy={working}
          onChange={(on) => {
            if (!working) void setSaves(on)
          }}
        />
        <ToggleBlock
          label={t('sync.states')}
          description={t('sync.statesHint')}
          checked={st.statesEnabled}
          disabled={!st.enabled}
          busy={!statesFree}
          onChange={(on) => {
            if (statesFree) void setStates(on)
          }}
        />
        {st.enabled && st.statesEnabled && st.statesLocalOnly > 0 && (
          <FieldBlock wide>
            <Body1>{t('sync.statesLocalOnly', { count: st.statesLocalOnly })}</Body1>
            {/* "Enviar os states antigos também" (D127) só age nos apagados no
                site; os outros ficam só aqui e não ganham o botão. */}
            {st.statesWiped > 0 && (
              <div className={s.actions}>
                <Button
                  appearance="secondary"
                  disabledFocusable={working}
                  icon={busy === 'oldStates' ? <Spinner size="tiny" /> : undefined}
                  onClick={() => void uploadOldStates()}
                >
                  {t('sync.statesResend')}
                </Button>
              </div>
            )}
          </FieldBlock>
        )}
        {/* As configurações chegam com o PR que as sincroniza; até lá, "em breve". */}
        <FieldBlock wide>
          <span className={s.rowTitle}>
            <Body1>{t('sync.settings')}</Body1>
            <Badge appearance="tint" color="informative">
              {t('sync.soon')}
            </Badge>
          </span>
          <Caption1 className={s.muted}>{t('sync.settingsHint')}</Caption1>
        </FieldBlock>
      </BlockGroup>

      {st.enabled && conflictCount > 0 && (
        <BlockGroup title={t('sync.conflicts.title')}>
          <FieldBlock wide>
            {conflicts.data ? (
              // Duas perguntas diferentes: as duas máquinas mudaram, ou o site
              // apagou e esta máquina mudou.
              <>
                {twoMachines > 0 && <Body1>{t('sync.conflicts.count', { count: twoMachines })}</Body1>}
                {deletedOnSite > 0 && <Body1>{t('sync.conflicts.deletedCount', { count: deletedOnSite })}</Body1>}
              </>
            ) : (
              <Body1>{t('sync.conflicts.count', { count: conflictCount })}</Body1>
            )}
            <Caption1 className={s.muted}>
              {conflicts.data && deletedOnSite > 0
                ? twoMachines === 0
                  ? t('sync.conflict.deletedFooter')
                  : t('sync.conflicts.hintMixed')
                : t('sync.conflicts.hint')}
            </Caption1>
            <div className={s.actions}>
              <Button
                appearance="primary"
                disabledFocusable={!conflicts.data || conflicts.data.length === 0}
                onClick={() => setQueue((conflicts.data ?? []).map((c) => c.key))}
              >
                {t('sync.conflicts.resolve')}
              </Button>
            </div>
          </FieldBlock>
        </BlockGroup>
      )}

      {asking && (
        <ConflictDialog
          conflict={asking}
          onDone={(outcome) => {
            // Resolvido: a próxima (por chave: a lista também se atualiza sozinha).
            // Adiado ou fechado: para a fila.
            if (outcome === 'local' || outcome === 'remote') setQueue((q) => q.filter((k) => k !== asking.key))
            else setQueue([])
          }}
        />
      )}

      {st.enabled && !st.revoked && (
        <section className={s.rows}>
          <Body1 as="h3" style={{ margin: 0 }}>
            {t('sync.machines')}
          </Body1>
          <Caption1 className={s.muted}>{t('sync.machinesHint')}</Caption1>
          {machines.isLoading && <Spinner size="tiny" label={t('sync.machinesLoading')} labelPosition="after" />}
          {list.map((m) => (
            <MachineRow key={m.id} m={m} lang={lang} nowMs={nowMs} />
          ))}
        </section>
      )}

      <div className={s.site}>
        <Caption1 className={s.muted}>{t('sync.manageOnSite')}</Caption1>
        <Button appearance="transparent" size="small" onClick={() => openSite('sync')}>
          {SITE_HOST}
        </Button>
      </div>
    </div>
  )
}
