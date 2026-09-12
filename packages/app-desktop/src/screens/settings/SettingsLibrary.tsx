import { Button, makeStyles, tokens } from "@fluentui/react-components";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { LoadingState } from "../../components/EmptyState";
import { ManageLibraryFields } from "../../components/ManageLibraryFields";
import { platformLabel } from "../../lib/platform";
import { listRoms } from "../../lib/tauri";
import { useManageLibrary } from "../../lib/useManageLibrary";

const useStyles = makeStyles({
  root: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalM },
  saveRow: { display: "flex", justifyContent: "flex-end" },
});

/**
 * Aba "Gerenciar biblioteca" em Configurações — mesma funcionalidade do
 * modal acionado pela Biblioteca (`ManageLibraryDialog`), só que como página
 * cheia em vez de modal. Reusa `useManageLibrary`/`ManageLibraryFields`.
 */
export function SettingsLibrary() {
  const s = useStyles();
  const roms = useQuery({ queryKey: ["roms"], queryFn: listRoms, retry: false });
  const platforms = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of roms.data ?? []) m.set(r.systemId, (m.get(r.systemId) ?? 0) + 1);
    return [...m.entries()].sort(([a], [b]) => platformLabel(a).localeCompare(platformLabel(b)));
  }, [roms.data]);

  const state = useManageLibrary(true);

  if (roms.isLoading) return <LoadingState label="Carregando biblioteca…" />;

  return (
    <div className={s.root}>
      <ManageLibraryFields state={state} platforms={platforms} />
      <div className={s.saveRow}>
        <Button
          appearance="primary"
          disabled={state.save.isPending || Object.keys(state.pending).length === 0}
          onClick={() => state.save.mutate()}
        >
          Salvar
        </Button>
      </div>
    </div>
  );
}
