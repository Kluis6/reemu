import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState, type MutableRefObject } from "react";
import { useSearchParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { conflictFor } from "../../hooks/useSync";
import type { AppError } from "../../lib/bindings/AppError";
import { errorToast, sysToast } from "../../lib/toast";
import {
  currentFocus,
  loadGame,
  loadSaveState,
  onHotkeyAction,
  toggleFocus,
  unloadGame,
  type SyncConflict,
} from "../../lib/tauri";
import { useFocusStore } from "../../stores/useFocusStore";
import { useToastStore } from "../../stores/useToastStore";
import type { LaunchInfo } from "./useLaunchInfo";

/** `conflict`: a checagem da nuvem achou o save deste jogo mudado aqui e em
 *  outra máquina, e o jogo espera a pessoa escolher (D118). */
export type SessionStatus = "loading" | "ready" | { error: unknown } | { conflict: SyncConflict };

/** O que a pessoa respondeu ao conflito: um lado (o diálogo já resolveu) ou
 *  "Decidir depois". */
export type ConflictAnswer = "local" | "remote" | "later";

/** O load parou antes do core porque a checagem da nuvem achou um conflito. */
function isSyncConflict(e: unknown): boolean {
  return !!e && typeof e === "object" && (e as Partial<AppError>).code === "syncConflict";
}

/**
 * Carrega o jogo ao entrar e descarrega ao sair. Devolve o estado da sessão e
 * a proporção de exibição (a do core, que o vídeo pode refinar depois).
 * `declaredAspect` guarda a declarada no carregamento, para o vídeo comparar
 * com a dos pixels. Um conflito do save achado antes de abrir vira o estado
 * `conflict`; `answerConflict` tenta abrir de novo com a resposta.
 */
export function useGameSession(launch: LaunchInfo | null, romId: string) {
  const [params] = useSearchParams();
  const push = useToastStore((s) => s.push);
  const setFocus = useFocusStore((s) => s.setFocus);
  const [status, setStatus] = useState<SessionStatus>("loading");
  // Cada tentativa de abrir. `later`: a pessoa já disse "Decidir depois" (na
  // página do jogo, pela URL, ou aqui), e o jogo abre com o save daqui.
  const [attempt, setAttempt] = useState(() => ({ n: 0, later: params.get("sync") === "later" }));
  const [aspect, setAspect] = useState(4 / 3);
  // AR de exibição declarada pelo core (respeita PAR ≠ 1). O paint loop usa
  // isto quando a orientação do frame bate; se veio rotacionado (SET_ROTATION),
  // usa a AR dos pixels.
  const declaredAspect = useRef(4 / 3) as MutableRefObject<number>;

  // O unload no cleanup é ADIADO: o StrictMode (dev) monta→desmonta→remonta,
  // e um `load → unload → load` seguido quebra o core (áudio do mupen, contexto
  // GL). Se o effect re-rodar pro MESMO jogo em <200ms, o unload pendente é
  // cancelado e nada recarrega.
  const unloadTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const loadedKey = useRef<string | null>(null);
  useEffect(() => {
    if (!launch) return;
    const key = `${launch.coreId} ${launch.romPath} ${romId} ${attempt.n}`;

    if (unloadTimer.current) {
      clearTimeout(unloadTimer.current);
      unloadTimer.current = null;
    }
    const scheduleUnload = () => {
      unloadTimer.current = setTimeout(() => {
        unloadTimer.current = null;
        loadedKey.current = null;
        void unloadGame().catch(() => {});
      }, 200);
    };

    // Já é este jogo (remonta do StrictMode, ou o load em voo ainda vale) —
    // não recarrega; a promise em voo checa `loadedKey` e continua.
    if (loadedKey.current !== key) {
      loadedKey.current = key;
      setStatus("loading");
      void (async () => {
        try {
          const g = await loadGame(launch.coreId, launch.romPath, romId || undefined, attempt.later);
          if (loadedKey.current !== key) return; // navegou pra outro jogo
          const a = g.aspectRatio > 0 ? g.aspectRatio : g.baseWidth / g.baseHeight || 4 / 3;
          declaredAspect.current = a;
          setAspect(a);

          // "Jogar daqui": carrega o save state pedido na URL antes de mostrar.
          const wantState = params.get("loadState");
          if (wantState) {
            await loadSaveState(wantState).catch((e) => push(errorToast(e, "loadSaveState")));
          }
          // Garante que o jogo começa com foco (não no menu de pausa).
          const f = await currentFocus().catch(() => "GameFocused" as const);
          if (f === "MenuFocused") await toggleFocus().catch(() => {});
          setFocus("GameFocused");
          setStatus("ready");
        } catch (e) {
          if (loadedKey.current !== key) return;
          // D118: o save mudou aqui e em outra máquina, e nenhum core abriu.
          // Sem o conflito para mostrar, abre com o save daqui: a checagem da
          // nuvem nunca prende o jogo.
          if (isSyncConflict(e)) {
            const conflict = await conflictFor(romId).catch(() => null);
            if (loadedKey.current !== key) return;
            if (conflict) setStatus({ conflict });
            else setAttempt((a) => ({ n: a.n + 1, later: true }));
            return;
          }
          setStatus({ error: e });
        }
      })();
    }
    return scheduleUnload;
  }, [launch, romId, setFocus, params, push, attempt]);

  /** Abre de novo depois do diálogo: com um lado escolhido a checagem roda
   *  outra vez; com "Decidir depois" o jogo abre com o save daqui. */
  const answerConflict = useCallback((answer: ConflictAnswer) => {
    setAttempt((a) => ({ n: a.n + 1, later: answer === "later" }));
  }, []);

  return { status, aspect, setAspect, declaredAspect, answerConflict };
}

/**
 * QuickSave/QuickLoad pelo atalho de sistema: o backend faz e avisa por
 * `hotkey-action`; o aviso (já traduzido) sai daqui.
 */
export function useQuickStateToasts(romId: string) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const push = useToastStore((s) => s.push);
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    void onHotkeyAction((e) => {
      if (e.action === "quick_save" || e.action === "quick_load") {
        const save = e.action === "quick_save";
        if (e.ok) {
          qc.invalidateQueries({ queryKey: ["save-states", romId] });
          push(sysToast(t(save ? "play.quickSaved" : "play.quickLoaded"), "Success"));
        } else {
          push(errorToast(e.message, save ? "quickSave" : "quickLoad"));
        }
        return;
      }
      void qc.invalidateQueries({ queryKey: ["gamelog-state"] });
      void qc.invalidateQueries({ queryKey: ["gamelog-items"] });
      void qc.invalidateQueries({ queryKey: ["gamelog-usage"] });
      if (e.ok) {
        push(sysToast(e.message, "Success"));
      } else {
        push(errorToast(e.message, "gamelogAction"));
      }
    }).then((fn) => {
      if (alive) off = fn;
      else fn();
    });
    return () => {
      alive = false;
      off?.();
    };
  }, [push, qc, romId, t]);
}
