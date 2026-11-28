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
import { ChevronLeftRegular } from "@fluentui/react-icons";
import { useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
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
  titleRow: {
    display: "inline-flex",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalM,
    minWidth: 0,
  },
  backBtn: {
    flexShrink: 0,
  },
  content: { maxHeight: "none" },
});

/** Dialog de tela cheia com botão Voltar, título e botão de fechar. Esc e o B
 *  do controle também fecham. `actions` vira o rodapé, fixo embaixo do conteúdo. */
export function FullscreenDialog({
  open,
  onClose,
  title,
  children,
  actions,
  contentClassName,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  actions?: ReactNode;
  contentClassName?: string;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const backRef = useRef<HTMLButtonElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  // Quando o conteúdo abre vazio ou carregando (ex.: Meu Log ou Conquistas sem
  // botões habilitados), garante foco dentro da superfície para teclado e controle.
  useEffect(() => {
    if (!open) return;
    const raf = requestAnimationFrame(() => {
      const surface = backRef.current?.closest(".fui-DialogSurface");
      const active = document.activeElement;
      if (
        surface &&
        active instanceof HTMLElement &&
        active !== surface &&
        !active.hasAttribute("data-tabster-dummy") &&
        surface.contains(active)
      ) {
        return;
      }
      const firstContentControl = contentRef.current?.querySelector<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      (firstContentControl ?? backRef.current)?.focus({ preventScroll: true });
    });
    return () => cancelAnimationFrame(raf);
  }, [open]);

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
            <span className={s.titleRow}>
              <Button
                ref={backRef}
                appearance="secondary"
                size="small"
                className={s.backBtn}
                icon={<ChevronLeftRegular />}
                onClick={onClose}
              >
                {t("hints.back")}
              </Button>
              <span>{title}</span>
            </span>
          </DialogTitle>
          <DialogContent ref={contentRef} className={mergeClasses(s.content, contentClassName)}>
            {children}
          </DialogContent>
          {actions != null && <DialogActions>{actions}</DialogActions>}
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
