import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowClockwiseRegular,
  ArrowExitRegular,
  PowerRegular,
} from "@fluentui/react-icons";
import { useMutation } from "@tanstack/react-query";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { quitApp, restartSystem, shutdownSystem } from "../lib/tauri";
import { sysToast } from "../lib/toast";
import { useToastStore } from "../stores/useToastStore";

const useStyles = makeStyles({
  surface: { maxWidth: "360px" },
  content: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalXS },
  row: {
    justifyContent: "flex-start",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalM,
    paddingBottom: tokens.spacingVerticalM,
  },
});

/**
 * Menu de energia do botão de sair do rail — 3 opções, estilo "segurar o
 * botão de energia" do Xbox: fechar só o app, desligar ou reiniciar a
 * máquina (útil pro ReEmu rodando full-screen numa TV/gabinete dedicado).
 */
export function PowerMenuDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const s = useStyles();
  const push = useToastStore((t) => t.push);

  const shutdown = useMutation({
    mutationFn: shutdownSystem,
    onError: (e) => push(sysToast(`Falha ao desligar: ${e}`, "Error")),
  });
  const restart = useMutation({
    mutationFn: restartSystem,
    onError: (e) => push(sysToast(`Falha ao reiniciar: ${e}`, "Error")),
  });

  const busy = shutdown.isPending || restart.isPending;

  return (
    <Dialog
      open={open}
      onOpenChange={(_, d) => onOpenChange(d.open)}
      surfaceMotion={DIALOG_FADE_ONLY}
    >
      <DialogSurface className={s.surface}>
        <DialogBody>
          <DialogTitle>Encerrar</DialogTitle>
          <DialogContent className={s.content}>
            <Button
              className={s.row}
              appearance="subtle"
              icon={<ArrowExitRegular />}
              disabled={busy}
              onClick={() => void quitApp()}
            >
              Fechar o ReEmu
            </Button>
            <Button
              className={s.row}
              appearance="subtle"
              icon={<ArrowClockwiseRegular />}
              disabled={busy}
              onClick={() => restart.mutate()}
            >
              Reiniciar o computador
            </Button>
            <Button
              className={s.row}
              appearance="subtle"
              icon={<PowerRegular />}
              disabled={busy}
              onClick={() => shutdown.mutate()}
            >
              Desligar o computador
            </Button>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => onOpenChange(false)}>
              Cancelar
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
