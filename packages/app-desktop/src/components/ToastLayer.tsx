import {
  Button,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  MessageBarGroup,
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
import i18n from "../i18n";

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
    // Acima dos portais do Fluent (gaveta, diálogos, menus): o nó de
    // montagem deles tem `z-index: 1000000` (`usePortalMountNodeStyles` do
    // @fluentui/react-portal). Com 9999 o aviso ficava atrás da gaveta de
    // informações do jogo, que ocupa justo o canto dos toasts.
    zIndex: 1000001,
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

  // `MessageBarGroup animate="both"`: entrada com fade + deslize de cima e
  // saída com fade (`MessageBarMotion` do Fluent, `durationGentle`); o grupo
  // segura o toast que saiu da fila até o fade terminar. Respeita o
  // "reduzir movimento" do sistema (motion do Fluent).
  return (
    <MessageBarGroup animate="both" className={styles.layer}>
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
                icon={<DismissRegular />}
                aria-label={i18n.t("shell.close")}
                title={i18n.t("shell.close")}
                onClick={() => dismiss(t.id)}
              />
            }
          >
            {[...(t.action ? [t.action] : []), ...(t.moreActions ?? [])].map(
              (a) => (
                <Button
                  key={a.label}
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
    </MessageBarGroup>
  );
}
