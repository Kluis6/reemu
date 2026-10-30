import {
  Avatar,
  Badge,
  Body1Strong,
  Button,
  Caption1,
  MessageBar,
  MessageBarBody,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AlvanistaButton } from '../../components/AlvanistaButton'
import { LoadingState } from '../../components/EmptyState'
import { ProfileAvatar } from '../../components/ProfileAvatar'
import { ProfileForm } from '../../components/ProfileForm'
import { ALVANISTA_CACHED_KEY } from '../../hooks/useEffectiveProfile'
import { sysToast } from '../../lib/toast'
import {
  alvanistaCachedStatus,
  alvanistaCancel,
  alvanistaConnect,
  alvanistaDisconnect,
  alvanistaOpenProfile,
  alvanistaStatus,
  getProfile,
  type AlvanistaStatus,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  block: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    gap: tokens.spacingVerticalS,
  },
  waiting: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalM },
  identity: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalL },
  actions: { display: 'flex', flexWrap: 'wrap', gap: tokens.spacingHorizontalS },
})

const KEY = ['alvanista-status'] as const

const DISCONNECTED: AlvanistaStatus = {
  connected: false,
  username: null,
  avatar_url: null,
  hero_url: null,
  bio: null,
  profile_url: null,
  offline: false,
  insecure_storage: false,
  ended: false,
}

/** Códigos que o `alvanista_connect` devolve ao falhar. */
const OUTCOMES = ['denied', 'timeout', 'offline', 'failed'] as const
type Outcome = (typeof OUTCOMES)[number]

function outcomeOf(err: unknown): Outcome | 'cancelled' {
  const code = typeof err === 'string' ? err : err instanceof Error ? err.message : ''
  if (code === 'cancelled') return 'cancelled'
  return (OUTCOMES as readonly string[]).includes(code) ? (code as Outcome) : 'failed'
}

/**
 * Configurações › Perfil. Sem conexão, edita o perfil local (o mesmo do
 * onboarding) e oferece "Entrar com Alvanista". Conectado, mostra nome e
 * avatar do Alvanista, só leitura; o perfil local não é tocado e volta ao
 * desconectar.
 */
export function SettingsProfile() {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
  // Primeiro o que está no aparelho (instantâneo); o status completo fala
  // com o Alvanista (até ~15 s) e substitui a tela quando chega.
  const cached = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })
  const alva = useQuery({ queryKey: KEY, queryFn: alvanistaStatus, retry: false })
  const [connecting, setConnecting] = useState(false)
  const [disconnecting, setDisconnecting] = useState(false)
  const disconnectingRef = useRef(false)
  const [outcome, setOutcome] = useState<Outcome | null>(null)
  // Só o resultado da tentativa mais recente conta (cancelar e tentar de
  // novo deixa a antiga rejeitando "cancelled" depois).
  const attempt = useRef(0)
  // Síncrono: um segundo clique antes do re-render não abre outra tentativa.
  const connectingRef = useRef(false)
  const endedShown = useRef(false)

  const status = alva.data ?? cached.data ?? DISCONNECTED

  // O status completo (ou conectar, ou desconectar, que o substituem) mostra
  // outra identidade que a guardada: o rail, o card do hub e a saudação
  // (`useEffectiveProfile`) releem o estado guardado.
  useEffect(() => {
    const full = alva.data
    const kept = cached.data
    if (!full || !kept) return
    if (
      full.connected !== kept.connected ||
      full.username !== kept.username ||
      full.avatar_url !== kept.avatar_url
    ) {
      void qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY })
    }
  }, [alva.data, cached.data, qc])

  useEffect(() => {
    if (!status.ended || endedShown.current) return
    endedShown.current = true
    push(sysToast(t('alvanista.ended'), 'Warning'))
    qc.setQueryData(KEY, DISCONNECTED)
  }, [status.ended, push, qc, t])

  // Sair da tela com uma conexão em andamento a cancela: nenhuma tentativa
  // termina sem ninguém olhando.
  useEffect(
    () => () => {
      if (connectingRef.current) {
        attempt.current++
        void alvanistaCancel().catch(() => {})
      }
    },
    [],
  )

  const connect = useCallback(() => {
    if (connectingRef.current) return
    connectingRef.current = true
    const mine = ++attempt.current
    setOutcome(null)
    setConnecting(true)
    alvanistaConnect(i18n.language)
      .then((next) => {
        if (mine !== attempt.current) return
        connectingRef.current = false
        void qc.cancelQueries({ queryKey: KEY })
        qc.setQueryData(KEY, next)
        setConnecting(false)
      })
      .catch((err: unknown) => {
        if (mine !== attempt.current) return
        connectingRef.current = false
        const out = outcomeOf(err)
        setConnecting(false)
        setOutcome(out === 'cancelled' ? null : out)
      })
  }, [i18n.language, qc])

  const cancel = useCallback(() => {
    void alvanistaCancel().catch(() => {})
  }, [])

  const disconnect = useCallback(async () => {
    // Síncrono, como no conectar: um segundo clique não desconecta de novo
    // (nem mostra um segundo aviso).
    if (disconnectingRef.current) return
    disconnectingRef.current = true
    setDisconnecting(true)
    try {
      await alvanistaDisconnect()
      // Um status em voo ainda veria a sessão antiga: descarta.
      await qc.cancelQueries({ queryKey: KEY })
      qc.setQueryData(KEY, DISCONNECTED)
      push(sysToast(t('alvanista.disconnected'), 'Success'))
    } catch {
      push(sysToast(t('alvanista.disconnectFailed'), 'Error'))
    } finally {
      disconnectingRef.current = false
      setDisconnecting(false)
    }
  }, [push, qc, t])

  const openProfile = useCallback(() => {
    void alvanistaOpenProfile().catch(() => {})
  }, [])

  if (profile.isLoading || (cached.isLoading && alva.isLoading)) return <LoadingState />

  if (status.connected) {
    return (
      <div className={s.root}>
        <div className={s.identity}>
          {status.avatar_url ? (
            <Avatar
              size={96}
              name={status.username ?? t('alvanista.unknownUser')}
              image={{ src: status.avatar_url }}
            />
          ) : (
            <ProfileAvatar
              profile={{ name: status.username ?? t('alvanista.unknownUser'), avatar: 'preset:1' }}
              size={96}
            />
          )}
          <div className={s.block}>
            <Body1Strong>{status.username ?? t('alvanista.unknownUser')}</Body1Strong>
            {status.offline && (
              <Badge appearance="tint" color="warning">
                {t('alvanista.offlineBadge')}
              </Badge>
            )}
          </div>
        </div>
        <Caption1>{t('alvanista.connectedHint')}</Caption1>
        {status.insecure_storage && (
          <MessageBar intent="warning">
            <MessageBarBody>{t('alvanista.insecureStorage')}</MessageBarBody>
          </MessageBar>
        )}
        <div className={s.actions}>
          <Button appearance="secondary" onClick={openProfile} disabled={!status.profile_url}>
            {t('alvanista.editOnAlvanista')}
          </Button>
          <Button
            appearance="outline"
            onClick={() => void disconnect()}
            // `disabledFocusable`: o foco (do controle, inclusive) fica no
            // botão enquanto desconecta.
            disabledFocusable={disconnecting}
            icon={disconnecting ? <Spinner size="tiny" /> : undefined}
          >
            {t('alvanista.disconnect')}
          </Button>
        </div>
      </div>
    )
  }

  // Sem banco o Alvanista não tem onde guardar a sessão (o estado falha):
  // só o perfil local.
  const available = !cached.isError

  return (
    <div className={s.root}>
      <Caption1>{t('profile.intro')}</Caption1>
      {available && (
        <div className={s.block}>
          {connecting ? (
            <>
              <div className={s.waiting}>
                <Spinner size="tiny" />
                <Body1Strong>{t('alvanista.waiting')}</Body1Strong>
                <Button appearance="secondary" onClick={cancel}>
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
      )}
      <ProfileForm
        initial={{
          name: profile.data?.name ?? '',
          bio: profile.data?.bio ?? null,
          avatar: profile.data?.avatar ?? 'preset:1',
        }}
        submitLabel={t('common.save')}
        // campos de um lado, avatares do outro (pedido do usuário)
        layout="columns"
        onDone={() => {
          qc.invalidateQueries({ queryKey: ['profile'] })
          push(sysToast(t('profile.updated'), 'Success'))
        }}
      />
    </div>
  )
}
