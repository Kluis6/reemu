import {
  Button,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  MessageBarTitle,
  ProgressBar,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { useEffect } from "react";
import {
  useToastStore,
  type ToastItem,
  type ToastVariant,
} from "../stores/useToastStore";

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
  body: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalXS,
  },
  // texto técnico do erro: menor, apagado, no máximo 3 linhas
  detail: {
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
    overflowWrap: "anywhere",
    display: "-webkit-box",
    WebkitLineClamp: "3",
    WebkitBoxOrient: "vertical",
    overflow: "hidden",
  },
  progress: { marginTop: tokens.spacingVerticalXXS },
});

/** Toast só informativo (sem botão nem progresso) some sozinho em poucos
 *  segundos, mesmo que quem criou tenha pedido mais tempo ou `0`. */
const INFO_MAX_MS = 5000;

function lifetime(t: ToastItem): number {
  const informative =
    !t.action && !t.moreActions?.length && t.progress === undefined;
  if (!informative) return t.durationMs;
  return t.durationMs > 0 ? Math.min(t.durationMs, INFO_MAX_MS) : INFO_MAX_MS;
}

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

  // Auto-dismiss por `lifetime` (0 = fica até fechar no X ou ser atualizado).
  useEffect(() => {
    const timers = queue
      .map((t) => [t.id, lifetime(t)] as const)
      .filter(([, ms]) => ms > 0)
      .map(([id, ms]) => window.setTimeout(() => dismiss(id), ms));
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
          layout={t.action || t.title ? "multiline" : "auto"}
        >
          <MessageBarBody className={styles.body}>
            {t.title && <MessageBarTitle>{t.title}</MessageBarTitle>}
            {t.message}
            {t.detail && <span className={styles.detail}>{t.detail}</span>}
            {t.progress !== undefined && (
              <ProgressBar
                className={styles.progress}
                thickness="large"
                value={t.progress ?? undefined}
              />
            )}
          </MessageBarBody>
          <MessageBarActions
            containerAction={
              <Button
                appearance="transparent"
                size="small"
                icon={<DismissRegular />}
                aria-label="Fechar"
                title="Fechar"
                onClick={() => dismiss(t.id)}
              />
            }
          >
            {[...(t.action ? [t.action] : []), ...(t.moreActions ?? [])].map(
              (a) => (
                <Button
                  key={a.label}
                  size="small"
                  onClick={() => {
                    dismiss(t.id);
                    a.onClick();
                  }}
                >
                  {a.label}
                </Button>
              ),
            )}
          </MessageBarActions>
        </MessageBar>
      ))}
    </div>
  );
}
