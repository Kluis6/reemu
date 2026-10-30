import { create } from "zustand";
import type { ShaderFailure, UpdateInfo } from "../lib/tauri";

/**
 * Central de notificações do sino da barra lateral — avisos do próprio
 * sistema (versão nova disponível, novidades depois de atualizar). Diferente
 * dos toasts, que somem sozinhos: aqui fica até o usuário abrir.
 */
/** `shaderFailed`: shader que impediu o jogo de aparecer e saiu da
 *  atribuição (`hooks/useShaderFailures.ts`). */
export type NotificationKind = "update" | "whatsNew" | "shaderFailed" | "notice";

interface NotificationBase {
  id: string;
  title: string;
  /** Uma linha, mostrada na lista do sino. */
  summary: string;
  /** Epoch ms. */
  at: number;
  read: boolean;
}
export type AppNotification =
  | (NotificationBase & { kind: "update" | "whatsNew"; update: UpdateInfo })
  | (NotificationBase & { kind: "shaderFailed"; shader: ShaderFailure })
  // aviso simples: o texto basta, sem modal; `route` = tela que abre ao clicar
  | (NotificationBase & { kind: "notice"; route?: string; count?: number });

/** O que o modal mostra: oferecer a atualização ou só as novidades. */
export interface UpdateDialogState {
  mode: "update" | "whatsNew";
  info: UpdateInfo;
}

interface NotificationState {
  items: AppNotification[];
  dialog: UpdateDialogState | null;
  /** Modal com os detalhes de um shader removido. */
  shaderDialog: ShaderFailure | null;
  /** Substitui a notificação do mesmo `id` (ex.: `update` só existe uma). */
  upsert: (n: AppNotification) => void;
  remove: (id: string) => void;
  markAllRead: () => void;
  openDialog: (d: UpdateDialogState) => void;
  closeDialog: () => void;
  openShaderDialog: (f: ShaderFailure) => void;
  closeShaderDialog: () => void;
}

export const useNotificationStore = create<NotificationState>((set) => ({
  items: [],
  dialog: null,
  shaderDialog: null,
  upsert: (n) =>
    set((s) => ({ items: [n, ...s.items.filter((i) => i.id !== n.id)] })),
  remove: (id) => set((s) => ({ items: s.items.filter((i) => i.id !== id) })),
  markAllRead: () =>
    set((s) => ({ items: s.items.map((i) => (i.read ? i : { ...i, read: true })) })),
  openDialog: (dialog) => set({ dialog }),
  closeDialog: () => set({ dialog: null }),
  openShaderDialog: (shaderDialog) => set({ shaderDialog }),
  closeShaderDialog: () => set({ shaderDialog: null }),
}));
