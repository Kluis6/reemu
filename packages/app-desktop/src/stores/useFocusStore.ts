import { create } from 'zustand'

/**
 * `InputFocus` espelhado do backend Rust (ver `crates/domain/src/focus.rs`).
 *
 * O foco é decidido no lado Rust e propagado via evento Tauri
 * (`emit("focus-changed", ...)`); o React só reflete — nunca decide
 * (ver docs/ai-context/03 e 07). Store pequena e específica de propósito.
 */
export type InputFocus = 'GameFocused' | 'MenuFocused'

interface FocusState {
  focus: InputFocus
  /** Chamado só pelo listener do evento Tauri, não pela UI diretamente. */
  setFocus: (focus: InputFocus) => void
}

export const useFocusStore = create<FocusState>((set) => ({
  focus: 'GameFocused',
  setFocus: (focus) => set({ focus }),
}))
