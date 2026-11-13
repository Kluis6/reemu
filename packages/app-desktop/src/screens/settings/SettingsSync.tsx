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
import { SYNC_STATUS_KEY, SYNC_ERRORS, syncErrorCode, useSync } from '../../hooks/useSync'
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
  machineSide: { display: 'flex', flexDirection: 'column', alignItems: 'flex-end', rowGap: '2px', flexShrink: 0 },
  site: { display: 'flex', alignItems: 'center', flexWrap: 'wrap', columnGap: tokens.spacingHorizontalXS },
})

/** RetroArch só guarda saves: sem hardware nem configurações do ReEmu. */
const isSavesOnly = (m: SyncMachine) => m.clientKind === 'retroarch'

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
        {!m.isThis && !savesOnly && (
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
  const { status: query, machines, busy, error, setEnabled, setLabel, setPaused, now, cancel } = useSync()
  const { connect, cancel: cancelConnect, connecting, outcome } = useAlvanistaConnect()
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

  // O nome em edição (null: o do estado). Enviado, volta a seguir o estado.
  const [edited, setEdited] = useState<string | null>(null)
  const label = st?.label ?? ''
  const draft = edited ?? label
  const commitLabel = () => {
    const next = draft.trim()
    setEdited(null)
    if (next === label || next.length === 0 || next.length > LABEL_MAX) return
    void setLabel(next)
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

  const offline = st.lastError === 'offline' || query.isError || machines.isError
  const openSite = (page: 'sync' | 'plans') => void alvanistaOpenSite(page).catch(() => {})
  const failed = error ?? (st.lastError && (SYNC_ERRORS as readonly string[]).includes(st.lastError) ? syncErrorCode(st.lastError) : null)
  const showError = failed && failed !== 'offline' ? failed : null

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
              <Caption1>{t('alvanista.explain')}</Caption1>
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
          <Button appearance="primary" onClick={() => openSite('plans')}>
            {t('sync.seePlans')}
          </Button>
        </div>
      </div>
    )
  }

  const enabling = busy === 'enable' && !st.enabled
  const working = busy !== null
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
          <MessageBarBody>{t(`sync.err.${showError}`)}</MessageBarBody>
        </MessageBar>
      )}

      <BlockGroup title={t('sync.thisMachine')}>
        <ToggleBlock
          label={t('sync.toggle')}
          description={t('sync.toggleHint')}
          checked={st.enabled}
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
                  disabled={pausedSite || busy === 'label'}
                  onChange={(_, d) => setEdited(d.value)}
                  onBlur={commitLabel}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') commitLabel()
                  }}
                />
              </Field>
              <Caption1 className={s.muted}>{hardwareLine(st.hardware, lang)}</Caption1>
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
                  <ProgressBar value={quota.limitBytes > 0 ? used / quota.limitBytes : 0} />
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
        {(['saves', 'states', 'settings'] as const).map((k) => (
          <FieldBlock key={k} wide>
            <Body1>{t(`sync.${k}`)}</Body1>
            <Caption1 className={s.muted}>{t(`sync.${k}Hint`)}</Caption1>
          </FieldBlock>
        ))}
      </BlockGroup>

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
