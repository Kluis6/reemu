import { createHashRouter, Navigate } from 'react-router-dom'
import { AppShell } from './layouts/AppShell'
import { RootLayout } from './layouts/RootLayout'
import { SettingsLayout } from './layouts/SettingsLayout'
import { Home } from './screens/Home'
import { Library } from './screens/Library'
import { PlayScreen } from './screens/PlayScreen'
import { RomDetail } from './screens/RomDetail'
import { SettingsAudio } from './screens/settings/SettingsAudio'
import { SettingsControllers } from './screens/settings/SettingsControllers'
import { SettingsCores } from './screens/settings/SettingsCores'
import { SettingsHotkeys } from './screens/settings/SettingsHotkeys'
import { SettingsMetadata } from './screens/settings/SettingsMetadata'
import { SettingsVideo } from './screens/settings/SettingsVideo'

/**
 * Hash router (a webview do Tauri não tem servidor, então nada de history
 * mode). Modelo "launcher": Biblioteca/Configurações são telas cheias e
 * opacas (`AppShell`); só `/play/:romId` fica transparente pro vídeo nativo
 * aparecer atrás.
 */
export const router = createHashRouter([
  {
    element: <RootLayout />,
    children: [
      {
        element: <AppShell />,
        children: [
          { index: true, element: <Home /> },
          { path: 'library', element: <Library /> },
          { path: 'rom/:romId', element: <RomDetail /> },
          {
            path: 'settings',
            element: <SettingsLayout />,
            children: [
              { index: true, element: <Navigate to="audio" replace /> },
              { path: 'audio', element: <SettingsAudio /> },
              { path: 'video', element: <SettingsVideo /> },
              { path: 'metadata', element: <SettingsMetadata /> },
              { path: 'hotkeys', element: <SettingsHotkeys /> },
              { path: 'controllers', element: <SettingsControllers /> },
              { path: 'cores', element: <SettingsCores /> },
            ],
          },
        ],
      },
      { path: 'play/:romId', element: <PlayScreen /> },
      { path: '*', element: <Navigate to="/" replace /> },
    ],
  },
])
