import { create } from 'zustand'
import type { Gamepad } from '../lib/tauri'

/** Controles conectados agora — alimentado por `useGamepadStatus` (lista
 *  inicial + eventos `gamepad-connected`/`gamepad-disconnected`). Só a
 *  contagem importa pro ícone da topbar; `ControllerMappings` usa a própria
 *  query pra listar/editar. */
interface GamepadState {
  devices: Gamepad[]
  setAll: (list: Gamepad[]) => void
  add: (g: Gamepad) => void
  remove: (guid: string) => void
}

export const useGamepadStore = create<GamepadState>((set) => ({
  devices: [],
  setAll: (list) => set({ devices: list }),
  add: (g) =>
    set((s) => (s.devices.some((d) => d.guid === g.guid) ? s : { devices: [...s.devices, g] })),
  remove: (guid) => set((s) => ({ devices: s.devices.filter((d) => d.guid !== guid) })),
}))
