import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import i18n from "../i18n";
import { checkCoreUpdates } from "../lib/tauri";
import { useNotificationStore } from "../stores/useNotificationStore";

/** A tela de Cores usa a mesma chave: as duas enxergam a mesma lista. */
export const CORE_UPDATES_KEY = ["core-updates"];
export const CORE_UPDATES_STALE_MS = 6 * 60 * 60 * 1000;
/** Depois de abrir o app — ler a versão dos cores abre as DLLs. */
const FIRST_CHECK_MS = 20_000;
const NOTIFICATION_ID = "core-updates";

/**
 * Procura cores com versão nova e deixa o aviso no sino, com o total. Montado
 * no `AppShell` — no jogo o shell não existe, então nada aparece por cima da
 * partida.
 */
export function useCoreUpdates() {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setReady(true), FIRST_CHECK_MS);
    return () => window.clearTimeout(timer);
  }, []);

  const { data } = useQuery({
    queryKey: CORE_UPDATES_KEY,
    queryFn: checkCoreUpdates,
    enabled: ready,
    staleTime: CORE_UPDATES_STALE_MS,
    gcTime: CORE_UPDATES_STALE_MS,
    retry: false,
  });

  const count = data?.length;
  useEffect(() => {
    if (count === undefined) return;
    const { items, upsert, remove } = useNotificationStore.getState();
    if (count === 0) {
      remove(NOTIFICATION_ID);
      return;
    }
    // mesmo total já avisado: mantém lida/não lida como estava
    const prev = items.find((i) => i.id === NOTIFICATION_ID);
    const same = prev?.kind === "notice" && prev.count === count;
    upsert({
      id: NOTIFICATION_ID,
      kind: "notice",
      title: i18n.t("cores.updatesTitle", { count }),
      summary: i18n.t("cores.updatesSummary"),
      route: "/settings/cores",
      count,
      at: same && prev ? prev.at : Date.now(),
      read: same && prev ? prev.read : false,
    });
  }, [count]);
}
