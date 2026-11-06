import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
  mergeClasses,
} from "@fluentui/react-components";
import type { ReactNode } from "react";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { CloseButton } from "./CloseButton";

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
  // com rodapé (`actions`): título, conteúdo que rola e botões embaixo
  bodyActions: { gridTemplateRows: "auto minmax(0, 1fr) auto" },
  content: { maxHeight: "none" },
});

/** Dialog de tela cheia com título e botão de fechar. Esc e o Voltar do controle
 *  também fecham. `actions` vira o rodapé, fixo embaixo do conteúdo. */
export function FullscreenDialog({
  open,
  onClose,
  title,
  children,
  actions,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  actions?: ReactNode;
}) {
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
        <DialogBody className={mergeClasses(s.body, actions != null && s.bodyActions)}>
          <DialogTitle
            action={<CloseButton onClick={onClose} />}
          >
            {title}
          </DialogTitle>
          <DialogContent className={s.content}>{children}</DialogContent>
          {actions != null && <DialogActions>{actions}</DialogActions>}
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
