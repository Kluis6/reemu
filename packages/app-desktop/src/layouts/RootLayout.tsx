import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Navigate, Outlet, useLocation } from 'react-router-dom'
import { BindingCapture } from '../components/BindingCapture'
import { OnScreenKeyboard } from '../components/OnScreenKeyboard'
import { Splash, SPLASH_EXIT_MS } from '../components/Splash'
import { FilePicker } from '../components/FilePicker'
import { ToastLayer } from '../components/ToastLayer'
import { useFullscreenSync } from '../hooks/useFullscreen'
import { useGamepadStatus } from '../hooks/useGamepadStatus'
import { useMenuNav } from '../hooks/useMenuNav'
import { useRaEvents } from '../hooks/useRetroAchievements'
import { getProfile, setTrayLabels } from '../lib/tauri'
import { getReduceMotion } from '../lib/uiPrefs'

/** Tempo mínimo da splash (estilo Xbox — não pisca em máquina rápida). */
const SPLASH_MIN_MS = 1100

/**
 * Envolve todas as rotas. Global: splash de abertura, gate de onboarding, fila
 * de toasts, captura de binding e o teclado na tela (aqui, e não no
 * `AppShell`, pra valer também no onboarding).
 */
export function RootLayout() {
  useMenuNav()
  useFullscreenSync()
  useGamepadStatus()
  useRaEvents()

  const { pathname } = useLocation()

  // Menu do ícone da bandeja (Configurações › Sistema) no idioma da
  // interface: o Rust só guarda os rótulos que vêm daqui.
  const { t, i18n } = useTranslation()
  useEffect(() => {
    void setTrayLabels(t('system.trayOpen'), t('system.trayQuit')).catch(() => {})
  }, [t, i18n.language])
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })

  // splash: tempo mínimo + espera o perfil resolver; depois faz o fade-out
  const [minElapsed, setMinElapsed] = useState(false)
  const [splashGone, setSplashGone] = useState(false)
  useEffect(() => {
    const t = setTimeout(() => setMinElapsed(true), SPLASH_MIN_MS)
    return () => clearTimeout(t)
  }, [])
  const ready = minElapsed && !profile.isLoading
  useEffect(() => {
    if (!ready) return
    const reduced = getReduceMotion() || window.matchMedia('(prefers-reduced-motion: reduce)').matches
    // O transitionend desmonta a splash. O timer cobre transições suprimidas.
    const t = setTimeout(() => setSplashGone(true), reduced ? 0 : SPLASH_EXIT_MS + 64)
    return () => clearTimeout(t)
  }, [ready])

  // fail-open: sem backend / erro → segue pro app (não prende no onboarding)
  const onboarded = (profile.data?.onboarded ?? false) || profile.isError
  const atOnboarding = pathname === '/onboarding'

  return (
    <>
      {ready && onboarded && atOnboarding && <Navigate to="/" replace />}
      {ready && !onboarded && !atOnboarding && <Navigate to="/onboarding" replace />}

      <Outlet />

      {!splashGone && <Splash leaving={ready} onExited={() => setSplashGone(true)} />}

      <BindingCapture />
      <ToastLayer />
      <OnScreenKeyboard />
      <FilePicker />
    </>
  )
}
