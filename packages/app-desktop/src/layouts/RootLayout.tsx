import { Outlet } from 'react-router-dom'
import { BindingCapture } from '../components/BindingCapture'
import { ToastLayer } from '../components/ToastLayer'
import { useFullscreenSync } from '../hooks/useFullscreen'
import { useMenuNav } from '../hooks/useMenuNav'

/**
 * Envolve todas as rotas. Só coisas globais que não dependem de layout:
 * a fila de toasts e o diálogo de captura de binding (pode ser disparado de
 * qualquer tela de Configurações).
 */
export function RootLayout() {
  useMenuNav()
  useFullscreenSync()
  return (
    <>
      <Outlet />
      <BindingCapture />
      <ToastLayer />
    </>
  )
}
