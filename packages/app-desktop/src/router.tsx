import type { ComponentType } from 'react'
import { createHashRouter, Navigate } from 'react-router-dom'
import { AppShell } from './layouts/AppShell'
import { RootLayout } from './layouts/RootLayout'
import { Home } from './screens/Home'
import { Library } from './screens/Library'
import { RouteError } from './components/RouteError'
import { PlatformLibrary } from './screens/PlatformLibrary'

/**
 * Rota carregada sob demanda (`lazy` do React Router 7): o código da tela só
 * baixa na primeira visita, e o pacote inicial fica com a casca, a Início e a
 * Biblioteca. Enquanto o módulo chega, a tela anterior continua na tela.
 * `pick` escolhe o componente exportado; `props` fixa as do caminho.
 */
function page<M, P extends object>(
  load: () => Promise<M>,
  pick: (m: M) => ComponentType<P>,
  props?: P,
) {
  return async () => {
    const C = pick(await load())
    return { Component: props ? () => <C {...props} /> : (C as ComponentType) }
  }
}


/**
 * Hash router (a webview do Tauri não tem servidor, então nada de history
 * mode). Modelo "launcher": Biblioteca/Configurações são telas cheias e
 * opacas (`AppShell`); só `/play/:romId` fica transparente pro vídeo nativo
 * aparecer atrás.
 */
export const router = createHashRouter([
  {
    element: <RootLayout />,
    errorElement: <RouteError />,
    children: [
      {
        element: <AppShell />,
        children: [
          {
            // Erro numa tela aparece DENTRO da casca (rail/topbar seguem de
            // pé) — ver components/RouteError.
            errorElement: <RouteError />,
            children: [
              { index: true, element: <Home /> },
              { path: 'library', element: <Library /> },
              { path: 'library/:platform', element: <PlatformLibrary /> },
              { path: 'rom/:romId', lazy: page(() => import('./screens/RomDetail'), (m) => m.RomDetail) },
              { path: 'conquistas', lazy: page(() => import('./screens/Achievements'), (m) => m.Achievements) },
              { path: 'perfil', lazy: page(() => import('./screens/ProfilePage'), (m) => m.ProfilePage) },
              {
                path: 'settings',
                lazy: page(() => import('./layouts/SettingsLayout'), (m) => m.SettingsLayout),
                children: [
                  { index: true, lazy: page(() => import('./screens/settings/SettingsHome'), (m) => m.SettingsHome) },
                  { path: 'perfil', lazy: page(() => import('./screens/settings/SettingsProfile'), (m) => m.SettingsProfile) },
                  { path: 'sincronizacao', lazy: page(() => import('./screens/settings/SettingsSync'), (m) => m.SettingsSync) },
                  { path: 'aparencia', lazy: page(() => import('./screens/settings/SettingsAppearance'), (m) => m.SettingsAppearance) },
                  { path: 'aparencia/temas', lazy: page(() => import('./screens/settings/SettingsAppearance'), (m) => m.SettingsAppearance, { section: 'temas' as const }) },
                  { path: 'aparencia/papel-de-parede', lazy: page(() => import('./screens/settings/SettingsAppearance'), (m) => m.SettingsAppearance, { section: 'papel-de-parede' as const }) },
                  { path: 'biblioteca', lazy: page(() => import('./screens/settings/SettingsLibrary'), (m) => m.SettingsLibrary) },
                  { path: 'audio', lazy: page(() => import('./screens/settings/SettingsAudio'), (m) => m.SettingsAudio) },
                  { path: 'video', lazy: page(() => import('./screens/settings/SettingsVideo'), (m) => m.SettingsVideo) },
                  { path: 'video/shaders', lazy: page(() => import('./screens/settings/SettingsVideo'), (m) => m.SettingsVideo, { section: 'shaders' as const }) },
                  { path: 'video/molduras', lazy: page(() => import('./screens/settings/SettingsVideo'), (m) => m.SettingsVideo, { section: 'molduras' as const }) },
                  { path: 'metadata', lazy: page(() => import('./screens/settings/SettingsMetadata'), (m) => m.SettingsMetadata) },
                  { path: 'hotkeys', lazy: page(() => import('./screens/settings/SettingsHotkeys'), (m) => m.SettingsHotkeys) },
                  { path: 'controllers', lazy: page(() => import('./screens/settings/SettingsControllers'), (m) => m.SettingsControllers) },
                  { path: 'cores', lazy: page(() => import('./screens/settings/SettingsCores'), (m) => m.SettingsCores) },
                  { path: 'cores/:coreId', lazy: page(() => import('./screens/settings/SettingsCoreDetail'), (m) => m.SettingsCoreDetail) },
                  { path: 'bios', lazy: page(() => import('./screens/settings/SettingsBios'), (m) => m.SettingsBios) },
                  { path: 'sistema', lazy: page(() => import('./screens/settings/SettingsSystem'), (m) => m.SettingsSystem) },
                  { path: 'sistema/opcoes', lazy: page(() => import('./screens/settings/SettingsSystem'), (m) => m.SettingsSystem, { section: 'opcoes' as const }) },
                  { path: 'sistema/hardware', lazy: page(() => import('./screens/settings/SettingsSystem'), (m) => m.SettingsSystem, { section: 'hardware' as const }) },
                  { path: 'sistema/sobre', lazy: page(() => import('./screens/settings/SettingsSystem'), (m) => m.SettingsSystem, { section: 'sobre' as const }) },
                ],
              },
            ],
          },
        ],
      },
      { path: 'onboarding', lazy: page(() => import('./screens/Onboarding'), (m) => m.Onboarding) },
      { path: 'play/:romId', lazy: page(() => import('./screens/PlayScreen'), (m) => m.PlayScreen) },
      { path: '*', element: <Navigate to="/" replace /> },
    ],
  },
])
