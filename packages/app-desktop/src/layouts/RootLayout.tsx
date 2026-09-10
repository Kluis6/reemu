import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { Navigate, Outlet, useLocation } from 'react-router-dom'
import { BindingCapture } from '../components/BindingCapture'
import { Splash } from '../components/Splash'
import { ToastLayer } from '../components/ToastLayer'
import { useFullscreenSync } from '../hooks/useFullscreen'
import { useMenuNav } from '../hooks/useMenuNav'
import { getProfile } from '../lib/tauri'

/** Tempo mínimo da splash (estilo Xbox — não pisca em máquina rápida). */
const SPLASH_MIN_MS = 1800
const SPLASH_FADE_MS = 320

/**
 * Envolve todas as rotas. Global: splash de abertura, gate de onboarding, fila
 * de toasts, captura de binding.
 */
export function RootLayout() {
  useMenuNav()
  useFullscreenSync()

  const { pathname } = useLocation()
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
    const t = setTimeout(() => setSplashGone(true), SPLASH_FADE_MS)
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

      {!splashGone && <Splash leaving={ready} />}

      <BindingCapture />
      <ToastLayer />
    </>
  )
}
