import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
} from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { DIALOG_FADE_ONLY } from "../lib/motion";

const useStyles = makeStyles({
  // Ocupa a janela toda; em % para acompanhar o zoom da interface.
  surface: {
    width: "100%",
    height: "100%",
    maxWidth: "none",
    maxHeight: "none",
    boxSizing: "border-box",
    borderRadius: 0,
    // na cor da página: nestes temas `Background1` é igual ao fundo dos blocos
    backgroundColor: "var(--reemuPage)",
  },
  body: { height: "100%", maxHeight: "none", gridTemplateRows: "auto minmax(0, 1fr)" },
  content: { maxHeight: "none" },
});

/** Dialog de tela cheia com título e botão de fechar. Esc e o Voltar do controle
 *  também fecham. */
export function FullscreenDialog({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  return (
    <Dialog
      open={open}
      onOpenChange={(_, d) => {
        if (!d.open) onClose();
      }}
      surfaceMotion={DIALOG_FADE_ONLY}
    >
      <DialogSurface className={s.surface}>
        <DialogBody className={s.body}>
          <DialogTitle
            action={
              <Button
                appearance="subtle"
                icon={<DismissRegular />}
                aria-label={t("common.close")}
                onClick={onClose}
              />
            }
          >
            {title}
          </DialogTitle>
          <DialogContent className={s.content}>{children}</DialogContent>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
