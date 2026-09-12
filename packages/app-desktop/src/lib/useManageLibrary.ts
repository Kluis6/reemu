import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  clearLibrary,
  listInstalledCores,
  listRomSources,
  listSystemCores,
  removeRomSource,
  removeRomSystem,
  setSystemCore,
} from "./tauri";
import { sysToast } from "./toast";
import { useToastStore } from "../stores/useToastStore";

/**
 * Lógica de "Gerenciar biblioteca": por plataforma, escolhe o core padrão e
 * permite apagar; também apaga por pasta de origem e limpa tudo. Compartilhada
 * entre o modal (`ManageLibraryDialog`, acionado pela Biblioteca) e a aba
 * "Gerenciar biblioteca" em Configurações — mesma funcionalidade, chrome
 * diferente (modal vs página cheia).
 *
 * `enabled` gate as queries (o modal só busca quando aberto; a aba de
 * Configurações passa sempre `true`, já que a tela inteira É o conteúdo).
 */
export function useManageLibrary(enabled: boolean) {
  const qc = useQueryClient();
  const push = useToastStore((s) => s.push);

  const cores = useQuery({
    queryKey: ["installed-cores"],
    queryFn: listInstalledCores,
    enabled,
  });
  const sysCores = useQuery({
    queryKey: ["system-cores"],
    queryFn: listSystemCores,
    enabled,
  });
  const sources = useQuery({
    queryKey: ["romSources"],
    queryFn: listRomSources,
    enabled,
  });

  // escolhas pendentes: systemId → coreId ("" = usar o padrão automático)
  const [pending, setPending] = useState<Record<string, string>>({});
  const [confirm, setConfirm] = useState<string | null>(null);

  const purge = useMutation({
    mutationFn: (target: string) => {
      if (target === "__all__") return clearLibrary();
      if (target.startsWith("sys:")) return removeRomSystem(target.slice(4));
      return removeRomSource(target);
    },
    onSuccess: (n) => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      qc.invalidateQueries({ queryKey: ["romSources"] });
      setConfirm(null);
      push(sysToast(`${n} jogo(s) removido(s) da biblioteca.`, "Success"));
    },
    onError: (e) => {
      setConfirm(null);
      push(sysToast(`Falha: ${e}`, "Error"));
    },
  });

  const save = useMutation({
    mutationFn: async () => {
      const entries = Object.entries(pending);
      for (const [sys, core] of entries) {
        if ((sysCores.data?.[sys] ?? "") !== core) await setSystemCore(sys, core);
      }
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["system-cores"] });
      setPending({});
      push(sysToast("Configurações da biblioteca salvas.", "Success"));
    },
    onError: (e) => push(sysToast(`Falha ao salvar: ${e}`, "Error")),
  });

  const coreValue = (sys: string) => pending[sys] ?? sysCores.data?.[sys] ?? "";

  const reset = () => setPending({});

  return {
    cores,
    sysCores,
    sources,
    pending,
    setPending,
    confirm,
    setConfirm,
    purge,
    save,
    coreValue,
    reset,
  };
}

export type ManageLibraryState = ReturnType<typeof useManageLibrary>;
