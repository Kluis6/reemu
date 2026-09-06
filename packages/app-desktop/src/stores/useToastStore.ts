import { create } from "zustand";

/**
 * Fila de toasts — camada independente da state machine de foco
 * (ver `crates/domain/src/focus.rs`, `ToastItem`). Nunca captura input,
 * nunca pausa o core. Origem tanto do backend (evento Tauri) quanto do
 * frontend. Fila, não substituição.
 */
export type ToastVariant = "Info" | "Success" | "Warning" | "Error";
export type ToastSource = "System" | "Core";

export interface ToastItem {
  id: string;
  message: string;
  variant: ToastVariant;
  durationMs: number;
  source: ToastSource;
  /**
   * Barra de progresso no toast. `undefined` = sem barra; `null` = barra
   * indeterminada; `0..1` = determinada. Usado no scan de ROMs, que empurra um
   * toast fixo (`durationMs: 0`) e vai atualizando via `update`.
   */
  progress?: number | null;
}

interface ToastState {
  queue: ToastItem[];
  push: (toast: ToastItem) => void;
  update: (id: string, patch: Partial<Omit<ToastItem, "id">>) => void;
  dismiss: (id: string) => void;
}

export const useToastStore = create<ToastState>((set) => ({
  queue: [],
  push: (toast) => set((s) => ({ queue: [...s.queue, toast] })),
  update: (id, patch) =>
    set((s) => ({
      queue: s.queue.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    })),
  dismiss: (id) => set((s) => ({ queue: s.queue.filter((t) => t.id !== id) })),
}));
