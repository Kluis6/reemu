import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowClockwiseRegular,
  ArrowExitRegular,
  PowerRegular,
} from "@fluentui/react-icons";
import { useMutation } from "@tanstack/react-query";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { useModalButtonStyles } from "../styles/buttons";
import { quitApp, restartSystem, shutdownSystem } from "../lib/tauri";
import { errorToast } from "../lib/toast";
import { useToastStore } from "../stores/useToastStore";
import { useTranslation } from "react-i18next";

const useStyles = makeStyles({
  surface: { maxWidth: "360px" },
  // 8 entre as opções: com o contorno visível (`btn.secondary`) os 4 de
  // antes deixavam os botões grudados (print do usuário, 2026-10-29)
  content: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalS },
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
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const push = useToastStore((st) => st.push);

  const shutdown = useMutation({
    mutationFn: shutdownSystem,
    onError: (e) => push(errorToast(e, "shutdownComputer")),
  });
  const restart = useMutation({
    mutationFn: restartSystem,
    onError: (e) => push(errorToast(e, "restartComputer")),
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
          <DialogTitle>{t("shell2.power")}</DialogTitle>
          <DialogContent className={s.content}>
            <Button
              className={mergeClasses(s.row, btn.secondary)}
              icon={<ArrowExitRegular />}
              disabled={busy}
              onClick={() => void quitApp()}
            >
              {t("shell2.quit")}
            </Button>
            <Button
              className={mergeClasses(s.row, btn.secondary)}
              icon={<ArrowClockwiseRegular />}
              disabled={busy}
              onClick={() => restart.mutate()}
            >
              {t("shell2.restart")}
            </Button>
            <Button
              className={mergeClasses(s.row, btn.secondary)}
              icon={<PowerRegular />}
              disabled={busy}
              onClick={() => shutdown.mutate()}
            >
              {t("shell2.shutdown")}
            </Button>
          </DialogContent>
          <DialogActions>
            <Button className={btn.secondary} onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
