import { create } from "zustand";
import type { UpdateInfo } from "../lib/tauri";

/**
 * Central de notificações do sino da barra lateral — avisos do próprio
 * sistema (versão nova disponível, novidades depois de atualizar). Diferente
 * dos toasts, que somem sozinhos: aqui fica até o usuário abrir.
 */
export type NotificationKind = "update" | "whatsNew";

export interface AppNotification {
  id: string;
  kind: NotificationKind;
  title: string;
  /** Uma linha, mostrada na lista do sino. */
  summary: string;
  /** Epoch ms. */
  at: number;
  read: boolean;
  update: UpdateInfo;
}

/** O que o modal mostra: oferecer a atualização ou só as novidades. */
export interface UpdateDialogState {
  mode: "update" | "whatsNew";
  info: UpdateInfo;
}

interface NotificationState {
  items: AppNotification[];
  dialog: UpdateDialogState | null;
  /** Substitui a notificação do mesmo `id` (ex.: `update` só existe uma). */
  upsert: (n: AppNotification) => void;
  markAllRead: () => void;
  openDialog: (d: UpdateDialogState) => void;
  closeDialog: () => void;
}

export const useNotificationStore = create<NotificationState>((set) => ({
  items: [],
  dialog: null,
  upsert: (n) =>
    set((s) => ({ items: [n, ...s.items.filter((i) => i.id !== n.id)] })),
  markAllRead: () =>
    set((s) => ({ items: s.items.map((i) => (i.read ? i : { ...i, read: true })) })),
  openDialog: (dialog) => set({ dialog }),
  closeDialog: () => set({ dialog: null }),
}));
