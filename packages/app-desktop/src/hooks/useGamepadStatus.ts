import { useEffect } from 'react'
import { listGamepads, onGamepadConnected, onGamepadDisconnected, type Gamepad } from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useGamepadStore } from '../stores/useGamepadStore'
import { useToastStore } from '../stores/useToastStore'

/**
 * Mantém `useGamepadStore` em dia com o backend — lista inicial (controles já
 * plugados quando o app abriu) + eventos `gamepad-connected`/
 * `gamepad-disconnected` (plug a quente). Toast só ao conectar; desconexão só
 * atualiza o ícone da topbar, sem interromper o usuário.
 */
export function useGamepadStatus() {
  const setAll = useGamepadStore((s) => s.setAll)
  const add = useGamepadStore((s) => s.add)
  const remove = useGamepadStore((s) => s.remove)
  const push = useToastStore((s) => s.push)

  useEffect(() => {
    let disposed = false
    void listGamepads()
      .then((list) => {
        if (!disposed) setAll(list)
      })
      .catch(() => {})

    let disposeConnected: (() => void) | undefined
    let disposeDisconnected: (() => void) | undefined
    void onGamepadConnected((g: Gamepad) => {
      add(g)
      push(sysToast(`Controle conectado: ${g.name}`, 'Success'))
    }).then((d) => {
      if (disposed) d()
      else disposeConnected = d
    })
    void onGamepadDisconnected((guid: string) => {
      remove(guid)
    }).then((d) => {
      if (disposed) d()
      else disposeDisconnected = d
    })

    return () => {
      disposed = true
      disposeConnected?.()
      disposeDisconnected?.()
    }
  }, [setAll, add, remove, push])
}
