import { useEffect } from "react";
import { appVersion, checkForUpdate, type UpdateInfo } from "../lib/tauri";
import { useNotificationStore } from "../stores/useNotificationStore";
import { useToastStore } from "../stores/useToastStore";

/** Depois de abrir o app — não disputa rede/CPU com o carregamento inicial. */
const FIRST_CHECK_MS = 8_000;
const RECHECK_MS = 6 * 60 * 60 * 1000;
/**
 * Novidades da versão sendo instalada, gravadas antes de reiniciar — ao
 * abrir de novo, se a versão bater, viram a notificação "atualizado".
 */
const WHATS_NEW_KEY = "reemu.whatsNew";

// Uma vez por execução do app, mesmo que o AppShell remonte (ir pro jogo e
// voltar desmonta o shell).
let started = false;
let toastedVersion: string | null = null;

export function rememberWhatsNew(info: UpdateInfo) {
  try {
    localStorage.setItem(WHATS_NEW_KEY, JSON.stringify(info));
  } catch {
    // sem storage: só não aparece o "atualizado" depois
  }
}

function takeWhatsNew(): UpdateInfo | null {
  try {
    const raw = localStorage.getItem(WHATS_NEW_KEY);
    if (!raw) return null;
    localStorage.removeItem(WHATS_NEW_KEY);
    return JSON.parse(raw) as UpdateInfo;
  } catch {
    return null;
  }
}

/**
 * Procura versão nova ao abrir e a cada 6 h. Achou: notificação no sino +
 * toast com "Ver" (abre o modal). Também mostra as novidades uma vez depois
 * de uma atualização. Montado no `AppShell` — no jogo o shell não existe,
 * então nada aparece por cima da partida.
 */
export function useUpdateCheck() {
  useEffect(() => {
    if (started) return;
    started = true;
    const { upsert, openDialog } = useNotificationStore.getState();
    const push = useToastStore.getState().push;

    void (async () => {
      const done = takeWhatsNew();
      if (!done) return;
      const current = await appVersion().catch(() => null);
      if (current !== done.version) return; // instalação falhou/cancelada
      upsert({
        id: `whats-new-${done.version}`,
        kind: "whatsNew",
        title: `ReEmu atualizado para a versão ${done.version}`,
        summary: "Veja o que mudou",
        at: Date.now(),
        read: false,
        update: done,
      });
      push({
        id: crypto.randomUUID(),
        message: `ReEmu atualizado para a versão ${done.version}.`,
        variant: "Success",
        durationMs: 8000,
        source: "System",
        action: {
          label: "Novidades",
          onClick: () => openDialog({ mode: "whatsNew", info: done }),
        },
      });
    })();

    const check = async () => {
      let info: UpdateInfo | null;
      try {
        info = await checkForUpdate();
      } catch (e) {
        // sem rede, GitHub fora, etc. — tenta de novo no próximo ciclo
        console.warn("verificação de atualização falhou:", e);
        return;
      }
      if (!info) return;
      // mesma versão já avisada: mantém lida/não lida como estava
      const prev = useNotificationStore
        .getState()
        .items.find((i) => i.id === "update");
      upsert({
        id: "update",
        kind: "update",
        title: `Nova versão disponível: ${info.version}`,
        summary: `Você está na ${info.currentVersion}`,
        at: prev?.update.version === info.version ? prev.at : Date.now(),
        read: prev?.update.version === info.version ? prev.read : false,
        update: info,
      });
      if (toastedVersion === info.version) return;
      toastedVersion = info.version;
      const found = info;
      push({
        id: crypto.randomUUID(),
        message: `Nova versão do ReEmu disponível: ${info.version}.`,
        variant: "Info",
        durationMs: 10000,
        source: "System",
        action: {
          label: "Ver",
          onClick: () => openDialog({ mode: "update", info: found }),
        },
      });
    };
    window.setTimeout(() => void check(), FIRST_CHECK_MS);
    window.setInterval(() => void check(), RECHECK_MS);
  }, []);
}
