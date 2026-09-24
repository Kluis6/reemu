import {
  Button,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  ProgressBar,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { useEffect } from "react";
import { useToastStore, type ToastVariant } from "../stores/useToastStore";

const useStyles = makeStyles({
  // Camada independente da state machine de foco: sempre por cima, nunca
  // captura input (`pointerEvents: none`), nunca pausa o core.
  layer: {
    position: "fixed",
    top: tokens.spacingVerticalM,
    right: tokens.spacingHorizontalM,
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalM,
    zIndex: 9999,
    pointerEvents: "none",
    width: "360px",
    maxWidth: "calc(100vw - 32px)",
  },
  // O `MessageBar` da Fluent vem bem justo — mais respiro em volta do texto.
  bar: {
    pointerEvents: "auto",
    overflowWrap: "anywhere",
    paddingTop: tokens.spacingVerticalM,
    paddingBottom: tokens.spacingVerticalM,
    paddingLeft: tokens.spacingHorizontalL,
    paddingRight: tokens.spacingHorizontalL,
  },
  body: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalXS },
  progress: { marginTop: tokens.spacingVerticalXXS },
});

const INTENT: Record<ToastVariant, "info" | "success" | "warning" | "error"> = {
  Info: "info",
  Success: "success",
  Warning: "warning",
  Error: "error",
};

export function ToastLayer() {
  const styles = useStyles();
  const queue = useToastStore((s) => s.queue);
  const dismiss = useToastStore((s) => s.dismiss);

  // Auto-dismiss por `durationMs` (0 = fica até ser removido/atualizado).
  useEffect(() => {
    const timers = queue
      .filter((t) => t.durationMs > 0)
      .map((t) => window.setTimeout(() => dismiss(t.id), t.durationMs));
    return () => timers.forEach(window.clearTimeout);
  }, [queue, dismiss]);

  return (
    <div className={styles.layer}>
      {queue.map((t) => (
        <MessageBar
          key={t.id}
          className={styles.bar}
          intent={INTENT[t.variant]}
          // com botão: texto em cima e ação embaixo — em uma linha só o botão
          // estourava a largura fixa da camada e saía cortado
          layout={t.action ? "multiline" : "auto"}
        >
          <MessageBarBody className={styles.body}>
            {t.message}
            {t.progress !== undefined && (
              <ProgressBar
                className={styles.progress}
                thickness="large"
                value={t.progress ?? undefined}
              />
            )}
          </MessageBarBody>
          {t.action && (
            <MessageBarActions>
              <Button
                size="small"
                onClick={() => {
                  dismiss(t.id);
                  t.action?.onClick();
                }}
              >
                {t.action.label}
              </Button>
            </MessageBarActions>
          )}
        </MessageBar>
      ))}
    </div>
  );
}
