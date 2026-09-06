import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Select,
  Spinner,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { platformLabel } from "../lib/platform";
import { sysToast } from "../lib/toast";
import {
  clearLibrary,
  listInstalledCores,
  listRomSources,
  listSystemCores,
  removeRomSource,
  removeRomSystem,
  setSystemCore,
} from "../lib/tauri";
import { useToastStore } from "../stores/useToastStore";

const useStyles = makeStyles({
  section: {
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
    marginTop: tokens.spacingVerticalM,
    marginBottom: tokens.spacingVerticalXS,
  },
  row: {
    display: "grid",
    gridTemplateColumns: "1fr auto minmax(160px, 1.2fr) auto",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
    "&:first-of-type": { borderTop: "none" },
  },
  srcRow: {
    display: "flex",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  path: {
    flexGrow: 1,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: tokens.fontSizeBase200,
  },
  count: { color: tokens.colorNeutralForeground3, fontSize: tokens.fontSizeBase200 },
});

/**
 * Modal "Gerenciar biblioteca": por plataforma, escolhe o core padrão e
 * permite apagar; também apaga por pasta de origem e limpa tudo. As escolhas
 * de core ficam pendentes até "Salvar".
 */
export function ManageLibraryDialog({
  open,
  onOpenChange,
  platforms,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** `[systemId, quantidade]` presentes na biblioteca. */
  platforms: readonly (readonly [string, number])[];
}) {
  const s = useStyles();
  const qc = useQueryClient();
  const push = useToastStore((s) => s.push);

  const cores = useQuery({
    queryKey: ["installed-cores"],
    queryFn: listInstalledCores,
    enabled: open,
  });
  const sysCores = useQuery({
    queryKey: ["system-cores"],
    queryFn: listSystemCores,
    enabled: open,
  });
  const sources = useQuery({
    queryKey: ["romSources"],
    queryFn: listRomSources,
    enabled: open,
  });

  // escolhas pendentes: systemId → coreId ("" = usar o padrão automático)
  const [pending, setPending] = useState<Record<string, string>>({});
  const close = () => {
    setPending({});
    onOpenChange(false);
  };

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
      onOpenChange(false);
    },
    onError: (e) => push(sysToast(`Falha ao salvar: ${e}`, "Error")),
  });

  const purgeBtn = (target: string, idle: string, confirmLabel: string) => (
    <Button
      size="small"
      appearance={confirm === target ? "primary" : "secondary"}
      disabled={purge.isPending}
      onClick={() =>
        confirm === target ? purge.mutate(target) : setConfirm(target)
      }
    >
      {confirm === target ? confirmLabel : idle}
    </Button>
  );

  const coreValue = (sys: string) =>
    pending[sys] ?? sysCores.data?.[sys] ?? "";

  return (
    <Dialog open={open} onOpenChange={(_, d) => onOpenChange(d.open)}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle
            action={
              <Button
                appearance="subtle"
                aria-label="Fechar"
                icon={<DismissRegular />}
                onClick={close}
              />
            }
          >
            Gerenciar biblioteca
          </DialogTitle>
          <DialogContent>
            {cores.isLoading || sysCores.isLoading ? (
              <Spinner label="Carregando…" />
            ) : platforms.length === 0 ? (
              <Text>Biblioteca vazia.</Text>
            ) : (
              <>
                <div className={s.section}>
                  Plataformas — core padrão e remoção
                </div>
                {platforms.map(([sys, n]) => (
                  <div key={sys} className={s.row}>
                    <Text>{platformLabel(sys)}</Text>
                    <span className={s.count}>
                      {n} {n === 1 ? "jogo" : "jogos"}
                    </span>
                    <Select
                      value={coreValue(sys)}
                      onChange={(_, d) =>
                        setPending((p) => ({ ...p, [sys]: d.value }))
                      }
                    >
                      <option value="">Automático (por extensão)</option>
                      {(cores.data ?? []).map((c) => (
                        <option key={c.coreId} value={c.coreId}>
                          {c.name}
                        </option>
                      ))}
                    </Select>
                    {purgeBtn(`sys:${sys}`, "Remover", "Confirmar")}
                  </div>
                ))}
              </>
            )}

            {(sources.data?.length ?? 0) > 1 && (
              <>
                <div className={s.section}>Pastas de origem</div>
                {sources.data!.map((src) => (
                  <div key={src.path} className={s.srcRow}>
                    <span className={s.path} title={src.path}>
                      {src.path}
                    </span>
                    <span className={s.count}>{src.count}</span>
                    {purgeBtn(src.path, "Remover", "Confirmar")}
                  </div>
                ))}
              </>
            )}

            {platforms.length > 0 && (
              <div className={s.srcRow} style={{ marginTop: 12 }}>
                <span className={s.path}>Toda a biblioteca</span>
                {purgeBtn("__all__", "Limpar tudo", "Confirmar: apagar tudo")}
              </div>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={close}>
              Fechar
            </Button>
            <Button
              appearance="primary"
              disabled={save.isPending || Object.keys(pending).length === 0}
              onClick={() => save.mutate()}
            >
              Salvar
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
