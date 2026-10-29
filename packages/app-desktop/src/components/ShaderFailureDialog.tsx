import {
  Body1,
  Button,
  Caption1,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { useTranslation } from "react-i18next";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { platformLabel } from "../lib/platform";
import { useNotificationStore } from "../stores/useNotificationStore";
import { useModalButtonStyles } from "../styles/buttons";

const useStyles = makeStyles({
  surface: { maxWidth: "560px", width: "calc(100vw - 32px)" },
  content: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalM },
  label: { color: tokens.colorNeutralForeground3 },
  // mensagem técnica do compilador/da GPU: monoespaçada, rola se for longa
  detail: {
    margin: 0,
    maxHeight: "200px",
    overflow: "auto",
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    fontFamily: tokens.fontFamilyMonospace,
    fontSize: tokens.fontSizeBase200,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
  },
});

/**
 * Detalhes de um shader que impediu o jogo de aparecer e saiu da atribuição
 * (`shader_failure.rs`): o que aconteceu, de onde ele saiu e a mensagem
 * técnica. Abre pelo aviso no sino.
 */
export function ShaderFailureDialog() {
  const failure = useNotificationStore((n) => n.shaderDialog);
  const close = useNotificationStore((n) => n.closeShaderDialog);
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  if (!failure) return null;
  // onde o shader estava atribuído
  const scope =
    failure.scope === "rom"
      ? t("shaderFailure.scopeRom")
      : failure.scope === "system"
        ? t("shaderFailure.scopeSystem", { platform: platformLabel(failure.systemId ?? "") })
        : failure.scope === "default"
          ? t("shaderFailure.scopeDefault")
          : t("shaderFailure.scopeNone");
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && close()} surfaceMotion={DIALOG_FADE_ONLY}>
      <DialogSurface className={s.surface}>
        <DialogBody>
          <DialogTitle>{t("shaderFailure.dialogTitle", { name: failure.name })}</DialogTitle>
          <DialogContent className={s.content}>
            <Body1>{t(`shaderFailure.kind.${failure.kind}`)}</Body1>
            <Body1>{scope}</Body1>
            <div>
              <Caption1 className={s.label}>{t("shaderFailure.file")}</Caption1>
              <pre className={s.detail}>{failure.source}</pre>
            </div>
            <div>
              <Caption1 className={s.label}>{t("shaderFailure.error")}</Caption1>
              <pre className={s.detail}>{failure.error}</pre>
            </div>
          </DialogContent>
          <DialogActions>
            <Button className={btn.secondary} onClick={close}>
              {t("common.close")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
