import { create } from 'zustand'
import {
  cancelBindingCapture,
  startBindingCapture,
  type BindingTarget,
  type RawInputEvent,
} from '../lib/tauri'

/**
 * Estado **transitório** da UI de captura de binding (etapa 05, `docs/ai-context/05`).
 * Não persiste — a gravação real vai pro backend (`save_binding`).
 *
 * A janela hold+press que agrupa a combinação (~300ms) é responsabilidade
 * daqui: cada evento novo reinicia o timer de "assentou"; quando assenta, o
 * componente `BindingCapture` chama `save_binding` com o `events` acumulado.
 * O backend (`input_desktop::capture`) só decide "roteia normal" vs "manda
 * pra captura".
 */

export interface CaptureTargetInfo {
  target: BindingTarget
  /** `SystemAction::as_wire()` ou `"<guid>::<nome>::<Botão>"`. */
  targetKey: string
  /** Texto amigável mostrado no diálogo ("Abrir menu", "Controle → A"…). */
  label: string
}

const MAX_EVENTS = 4

function sameEvent(a: RawInputEvent, b: RawInputEvent): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

interface BindingCaptureState {
  active: CaptureTargetInfo | null
  events: RawInputEvent[]
  begin: (info: CaptureTargetInfo) => void
  addEvent: (ev: RawInputEvent) => void
  /** Sai do modo de captura (cancela no backend também). */
  reset: () => void
}

export const useBindingCaptureStore = create<BindingCaptureState>((set, get) => ({
  active: null,
  events: [],
  begin: (info) => {
    set({ active: info, events: [] })
    void startBindingCapture().catch(() => {})
  },
  addEvent: (ev) =>
    set((s) => {
      if (!s.active || s.events.length >= MAX_EVENTS) return s
      if (s.events.some((e) => sameEvent(e, ev))) return s
      return { events: [...s.events, ev] }
    }),
  reset: () => {
    if (get().active) void cancelBindingCapture().catch(() => {})
    set({ active: null, events: [] })
  },
}))
