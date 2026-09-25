import {
  Button,
  Caption1,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  MessageBar,
  MessageBarBody,
  MessageBarTitle,
  ProgressBar,
  Subtitle2,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ArrowDownloadRegular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { rememberWhatsNew } from "../hooks/useUpdateCheck";
import { describeError } from "../lib/errors";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { parseReleaseNotes } from "../lib/releaseNotes";
import { installUpdate, onUpdateProgress } from "../lib/tauri";
import {
  useNotificationStore,
  type UpdateDialogState,
} from "../stores/useNotificationStore";
import { useTranslation } from "react-i18next";
import i18n from "../i18n";

const useStyles = makeStyles({
  surface: { maxWidth: "560px", width: "calc(100vw - 32px)" },
  content: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalM,
  },
  meta: { color: tokens.colorNeutralForeground3 },
  // notas longas rolam dentro do modal, os botões ficam sempre à vista
  notes: {
    maxHeight: "min(50vh, 420px)",
    overflowY: "auto",
    padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalL}`,
    backgroundColor: tokens.colorNeutralBackground3,
    borderRadius: tokens.borderRadiusMedium,
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalS,
    // rolável pelo teclado/controle (tabIndex) — contorno só no foco visível
    outlineStyle: "none",
    ":focus-visible": {
      outline: `${tokens.strokeWidthThick} solid ${tokens.colorStrokeFocus2}`,
    },
  },
  heading: {
    margin: 0,
    ":not(:first-child)": { marginTop: tokens.spacingVerticalS },
  },
  action: { whiteSpace: "nowrap" },
  list: {
    margin: 0,
    paddingLeft: tokens.spacingHorizontalXL,
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXS,
  },
  paragraph: { margin: 0 },
  empty: { color: tokens.colorNeutralForeground3, margin: 0 },
  progress: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXS,
  },
});

const mb = (bytes: number) =>
  (bytes / 1_048_576).toLocaleString(i18n.language, { maximumFractionDigits: 1, minimumFractionDigits: 1 });

function formatDate(iso: string | null): string | null {
  if (!iso) return null;
  const d = new Date(iso);
  return Number.isNaN(d.getTime())
    ? null
    : d.toLocaleDateString(i18n.language, { day: "2-digit", month: "long", year: "numeric" });
}

/**
 * Modal da atualização — aberto pelo toast ("Ver") ou pelo sino. Modo
 * `update`: resumo das mudanças + "Atualizar agora"/"Fechar". Modo
 * `whatsNew` (depois de atualizar): só as novidades.
 */
export function UpdateDialog() {
  const dialog = useNotificationStore((n) => n.dialog);
  // Fechar desmonta o conteúdo — progresso/erro recomeçam do zero a cada
  // abertura.
  return dialog ? <UpdateDialogOpen dialog={dialog} /> : null;
}

function UpdateDialogOpen({ dialog }: { dialog: UpdateDialogState }) {
  const { t } = useTranslation();
  const s = useStyles();
  const close = useNotificationStore((n) => n.closeDialog);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<{ downloaded: number; total: number | null } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!installing) return;
    let off: (() => void) | undefined;
    let alive = true;
    void onUpdateProgress((p) => setProgress(p)).then((u) => {
      if (alive) off = u;
      else u();
    });
    return () => {
      alive = false;
      off?.();
    };
  }, [installing]);

  const { mode, info } = dialog;
  const blocks = parseReleaseNotes(info.notes);
  const date = formatDate(info.date);

  const install = async () => {
    setError(null);
    setProgress(null);
    setInstalling(true);
    rememberWhatsNew(info);
    try {
      await installUpdate(); // se der certo, o app reinicia e não volta aqui
    } catch (e) {
      setError(String(e));
      setInstalling(false);
    }
  };

  const fraction =
    progress?.total && progress.total > 0 ? progress.downloaded / progress.total : undefined;

  return (
    <Dialog
      open
      // durante o download não fecha (Esc/fora) — o app vai reiniciar
      onOpenChange={(_, d) => {
        if (!d.open && !installing) close();
      }}
      surfaceMotion={DIALOG_FADE_ONLY}
    >
      <DialogSurface className={s.surface}>
        <DialogBody>
          <DialogTitle>
            {mode === "update"
              ? t("updates.dialogUpdate", { version: info.version })
              : t("updates.dialogWhatsNew", { version: info.version })}
          </DialogTitle>
          <DialogContent className={s.content}>
            <Caption1 className={s.meta}>
              {mode === "update"
                ? t("updates.currentVersion", { version: info.currentVersion })
                : t("updates.updatedFrom", { version: info.currentVersion })}
              {date ? t("updates.publishedOn", { date }) : ""}
            </Caption1>

            <div className={s.notes} tabIndex={0} aria-label={t("updates.notesLabel")}>
              {blocks.length === 0 && (
                <p className={s.empty}>{t("updates.noNotes")}</p>
              )}
              {blocks.map((b, i) =>
                b.type === "heading" ? (
                  <Subtitle2 key={i} as="h3" className={s.heading}>
                    {b.text}
                  </Subtitle2>
                ) : b.type === "list" ? (
                  <ul key={i} className={s.list}>
                    {b.items.map((it, j) => (
                      <li key={j}>{it}</li>
                    ))}
                  </ul>
                ) : (
                  <p key={i} className={s.paragraph}>
                    {b.text}
                  </p>
                ),
              )}
            </div>

            {installing && (
              <div className={s.progress}>
                <ProgressBar thickness="large" value={fraction} />
                <Caption1 className={s.meta}>
                  {progress
                    ? progress.total
                      ? t("updates.downloadingOf", { done: mb(progress.downloaded), total: mb(progress.total) })
                      : t("updates.downloadingMb", { done: mb(progress.downloaded) })
                    : t("updates.preparing")}{" "}
                  {t("updates.restarts")}
                </Caption1>
              </div>
            )}

            {error && (
              <MessageBar intent="error">
                <MessageBarBody>
                  <MessageBarTitle>{describeError(error, "updateReemu").title}</MessageBarTitle>
                  {describeError(error, "updateReemu").hint}
                  <Caption1 block className={s.meta}>
                    {error}
                  </Caption1>
                </MessageBarBody>
              </MessageBar>
            )}
          </DialogContent>
          <DialogActions>
            {mode === "update" && (
              <Button
                className={s.action}
                appearance="primary"
                icon={<ArrowDownloadRegular />}
                disabled={installing}
                onClick={() => void install()}
              >
                {installing ? t("updates.updating") : t("updates.updateNow")}
              </Button>
            )}
            <Button
              className={s.action}
              appearance="secondary"
              disabled={installing}
              onClick={close}
            >
              {t("common.close")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
