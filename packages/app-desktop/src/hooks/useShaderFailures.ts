import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import i18n from "../i18n";
import { onShaderFailed, onWideBezelOff } from "../lib/tauri";
import { useNotificationStore } from "../stores/useNotificationStore";

/**
 * Shader que impediu o jogo de aparecer (`shader_failure.rs`): o backend já
 * voltou pro padrão e tirou o shader da atribuição; aqui o aviso entra no sino
 * (sem aviso por cima do jogo) e as telas de shader recarregam. Montado uma
 * vez no `AppShell`.
 */
export function useShaderFailures() {
  const qc = useQueryClient();
  useEffect(() => {
    let off: (() => void) | undefined;
    let offBezel: (() => void) | undefined;
    let alive = true;
    // Moldura fora por causa da imagem larga: também só no sino, e um aviso
    // só (mesmo `id`) por mais jogos que passem por isso.
    void onWideBezelOff(() => {
      useNotificationStore.getState().upsert({
        id: "wide-bezel-off",
        kind: "notice",
        title: i18n.t("game.image.bezelOffTitle"),
        summary: i18n.t("game.image.bezelOffSummary"),
        at: Date.now(),
        read: false,
      });
    }).then((fn) => {
      if (alive) offBezel = fn;
      else fn();
    });
    void onShaderFailed((f) => {
      useNotificationStore.getState().upsert({
        id: crypto.randomUUID(),
        kind: "shaderFailed",
        title: i18n.t("shaderFailure.title", { name: f.name }),
        summary: i18n.t(
          f.scope ? "shaderFailure.summaryRemoved" : "shaderFailure.summary",
        ),
        at: Date.now(),
        read: false,
        shader: f,
      });
      qc.invalidateQueries({ queryKey: ["rom-shader"] });
      qc.invalidateQueries({ queryKey: ["shader-info"] });
    }).then((fn) => {
      if (alive) off = fn;
      else fn();
    });
    return () => {
      alive = false;
      off?.();
      offBezel?.();
    };
  }, [qc]);
}
