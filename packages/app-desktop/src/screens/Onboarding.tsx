import { Body1, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Navigate, useNavigate } from 'react-router-dom'
import { AnimatedBackground } from '../components/AnimatedBackground'
import { AppLogo } from '../components/AppLogo'
import { LoadingState } from '../components/EmptyState'
import { ProfileForm } from '../components/ProfileForm'
import { getProfile } from '../lib/tauri'
import { useTranslation } from 'react-i18next'

// Primeira abertura: o fundo acende devagar, o cartão entra (fade + subida
// curta + leve crescimento) logo depois do splash e o conteúdo vem em
// cascata. Só opacity/transform — baratos no WebKitGTK e no WebView2 — e
// `fill-mode: backwards`: terminada a entrada nada fica aplicado (com
// `both` o texto perdia o ClearType no Windows, ver RouteTransition).
const EXIT_MS = 380
const bgIn = { from: { opacity: 0 }, to: { opacity: 1 } }
const cardIn = {
  from: { opacity: 0, transform: 'translateY(24px) scale(0.97)' },
  to: { opacity: 1, transform: 'none' },
}
const partIn = {
  from: { opacity: 0, transform: 'translateY(10px)' },
  to: { opacity: 1, transform: 'none' },
}
const cardOut = {
  from: { opacity: 1, transform: 'none' },
  to: { opacity: 0, transform: 'translateY(-8px) scale(1.015)' },
}
const noMotion = { '@media (prefers-reduced-motion: reduce)': { animationName: 'none' } }

const useStyles = makeStyles({
  bg: {
    position: 'absolute',
    inset: 0,
    animationName: bgIn,
    animationDuration: '1100ms',
    animationTimingFunction: tokens.curveEasyEase,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  root: {
    position: 'fixed',
    inset: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: tokens.spacingHorizontalXXL,
    overflowY: 'auto',
  },
  card: {
    position: 'relative',
    zIndex: 1,
    width: '100%',
    maxWidth: '480px',
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
    backgroundColor: tokens.colorNeutralBackground1,
    borderRadius: tokens.borderRadiusXLarge,
    padding: tokens.spacingHorizontalXXL,
    // Fluent 2 elevation: este card faz o papel de um painel/modal centrado
    // (não uma side-nav ou bottom sheet, que é o que shadow28 cobre) — a
    // tier certa é a mesma que o Dialog do Fluent usa, shadow64.
    boxShadow: tokens.shadow64,
    animationName: cardIn,
    animationDuration: '720ms',
    animationDelay: '180ms',
    animationTimingFunction: tokens.curveDecelerateMax,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  // saída ao concluir: o cartão sobe e some antes de trocar de tela
  leaving: {
    animationName: cardOut,
    animationDuration: `${EXIT_MS}ms`,
    animationDelay: '0ms',
    animationTimingFunction: tokens.curveAccelerateMid,
    animationFillMode: 'forwards',
    pointerEvents: 'none',
    ...noMotion,
  },
  head: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  // conteúdo em cascata depois do cartão (logo → texto → formulário)
  part: {
    animationName: partIn,
    animationDuration: '560ms',
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  d1: { animationDelay: '380ms' },
  d2: { animationDelay: '470ms' },
  d3: { animationDelay: '560ms' },
})

/**
 * Fluxo de 1ª execução — define nome, bio e avatar do perfil local (um por
 * instalação). Fica FORA da `AppShell` (sem rail/topbar). Ao concluir,
 * `setProfile` marca `onboarded` e a `/` passa a abrir normalmente.
 */
export function Onboarding() {
  const { t } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
  const [leaving, setLeaving] = useState(false)

  // Concluído: anima a saída do cartão e só então troca de tela (a Home
  // entra com a transição de rota dela).
  const finish = () => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      navigate('/', { replace: true })
      return
    }
    setLeaving(true)
    window.setTimeout(() => navigate('/', { replace: true }), EXIT_MS)
  }

  if (profile.isLoading) return <LoadingState />
  // já passou pelo onboarding, ou sem backend pra persistir → manda pra Home
  if (profile.data?.onboarded || profile.isError) return <Navigate to="/" replace />

  return (
    <div className={s.root}>
      <div className={s.bg}>
        <AnimatedBackground />
      </div>
      <div className={mergeClasses(s.card, leaving && s.leaving)}>
        <div className={s.head}>
          <div className={mergeClasses(s.part, s.d1)}>
            <AppLogo height={132} />
          </div>
          <Body1 className={mergeClasses(s.part, s.d2)}>
            {t('shell.welcome')}
          </Body1>
        </div>
        <div className={mergeClasses(s.part, s.d3)}>
          <ProfileForm
            initial={{
              name: profile.data?.name ?? '',
              bio: profile.data?.bio ?? null,
              avatar: profile.data?.avatar ?? 'preset:1',
            }}
            submitLabel={t('shell.start')}
            onDone={finish}
          />
        </div>
      </div>
    </div>
  )
}
