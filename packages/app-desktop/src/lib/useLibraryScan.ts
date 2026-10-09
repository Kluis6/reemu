import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { useToastStore } from "../stores/useToastStore";
import { rescanLibrary, scanLibrary, type ScanProgress } from "./tauri";
import { errorPatch } from "./toast";
import { useAutoMetadataScan } from "./useAutoMetadataScan";

/**
 * Varredura da biblioteca com aviso de progresso: `mutate(pasta)` adiciona uma
 * pasta; `mutate(null)` atualiza a biblioteca (varre de novo as pastas já
 * adicionadas — jogos novos entram, os que sumiram saem, a plataforma é
 * corrigida). Usada pela Biblioteca (adicionar ROMs) e por "Gerenciar
 * biblioteca" (modal e Configurações), que tem o "Atualizar biblioteca".
 */
export function useLibraryScan() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const push = useToastStore((s) => s.push);
  const updateToast = useToastStore((s) => s.update);
  const scanId = useRef<string | null>(null);
  const autoMetadata = useAutoMetadataScan();

  return useMutation({
    mutationFn: (path: string | null) => {
      const progress = (p: ScanProgress) => {
        if (!scanId.current) return;
        updateToast(scanId.current, {
          message: t("scan.progress", { current: p.current, total: p.total ? `/${p.total}` : "" }),
          progress: p.total ? p.current / p.total : null,
        });
      };
      return path === null ? rescanLibrary(progress) : scanLibrary(path, progress);
    },
    onMutate: () => {
      const id = crypto.randomUUID();
      scanId.current = id;
      push({
        id,
        message: t("scan.start"),
        variant: "Info",
        durationMs: 0,
        source: "System",
        progress: null,
      });
    },
    onSuccess: (r, path) => {
      if (scanId.current) {
        updateToast(scanId.current, {
          message:
            path === null
              ? t("scan.refreshResult", { added: r.added, removed: r.removed, reclassified: r.reclassified })
              : t("scan.result", { added: r.added, known: r.skippedKnown, skipped: r.skippedUnrecognized }),
          variant: r.errors > 0 ? "Warning" : "Success",
          durationMs: 5000,
          progress: undefined,
        });
      }
      qc.invalidateQueries({ queryKey: ["roms"] });
      qc.invalidateQueries({ queryKey: ["romSources"] });
      autoMetadata(r.added);
    },
    onError: (e) => {
      if (scanId.current) {
        updateToast(scanId.current, errorPatch(e, "scanFolder"));
      }
    },
    onSettled: () => {
      scanId.current = null;
    },
  });
}
