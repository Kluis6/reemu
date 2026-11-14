import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import i18n from "../i18n";
import { bezelUpdates, shaderPackUpdate } from "../lib/tauri";
import { useNotificationStore } from "../stores/useNotificationStore";

/** Depois do aviso dos cores (20 s): as duas consultas vão ao GitHub. */
const FIRST_CHECK_MS = 25_000;
/** O backend guarda cada consulta por 6 h; aqui a mesma janela. */
const STALE_MS = 6 * 60 * 60 * 1000;
const BEZEL_ID = "bezel-updates";
const SHADER_ID = "shader-pack-update";

/** Põe, troca ou tira o aviso `id` do sino. Mesmo total já avisado: mantém
 *  lida/não lida como estava (como o aviso dos cores). */
function notice(id: string, count: number, title: string, summary: string, route: string) {
  const { items, upsert, remove } = useNotificationStore.getState();
  if (count === 0) {
    remove(id);
    return;
  }
  const prev = items.find((i) => i.id === id);
  const same = prev?.kind === "notice" && prev.count === count;
  upsert({
    id,
    kind: "notice",
    title,
    summary,
    route,
    count,
    at: same && prev ? prev.at : Date.now(),
    read: same && prev ? prev.read : false,
  });
}

/**
 * Versão nova das molduras baixadas (The Bezel Project) e do pacote de shaders
 * (`libretro/slang-shaders`) no sino, como o aviso dos cores. Usa as mesmas
 * chaves das telas de Molduras e Shaders: baixar lá invalida a consulta e o
 * aviso some. Montado no `AppShell` (nada aparece durante o jogo).
 */
export function usePackUpdates() {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const timer = window.setTimeout(() => setReady(true), FIRST_CHECK_MS);
    return () => window.clearTimeout(timer);
  }, []);

  const bezels = useQuery({
    queryKey: ["bezel-updates"],
    queryFn: () => bezelUpdates(),
    enabled: ready,
    staleTime: STALE_MS,
    retry: false,
  });
  const shaders = useQuery({
    queryKey: ["shader-pack-update"],
    queryFn: () => shaderPackUpdate(),
    enabled: ready,
    staleTime: STALE_MS,
    retry: false,
  });

  const bezelCount = bezels.data?.length;
  useEffect(() => {
    if (bezelCount === undefined) return;
    notice(
      BEZEL_ID,
      bezelCount,
      i18n.t("notifications.bezelUpdatesTitle", { count: bezelCount }),
      i18n.t("notifications.bezelUpdatesSummary"),
      "/settings/video/molduras",
    );
  }, [bezelCount]);

  const shaderPack = shaders.data;
  useEffect(() => {
    if (shaderPack === undefined) return;
    notice(
      SHADER_ID,
      shaderPack ? 1 : 0,
      i18n.t("notifications.shaderPackUpdateTitle"),
      i18n.t("notifications.shaderPackUpdateSummary"),
      "/settings/video/shaders",
    );
  }, [shaderPack]);
}
