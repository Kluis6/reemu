import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Input,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { DismissRegular, FolderRegular } from "@fluentui/react-icons";
import { useState } from "react";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { pickFolder } from "../lib/tauri";
import { useTranslation } from "react-i18next";

const useStyles = makeStyles({
  row: {
    display: "flex",
    gap: tokens.spacingHorizontalS,
    alignItems: "flex-end",
  },
  input: { flexGrow: 1 },
});

/**
 * Modal pra apontar uma pasta de ROMs. Ao confirmar, chama `onScan(dir)` — o
 * chamador (`Library`) fecha o modal e toca o scan com toast de progresso.
 */
export function AddRomsDialog({
  open,
  onOpenChange,
  onScan,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onScan: (dir: string) => void;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const [dir, setDir] = useState("");

  return (
    <Dialog
      open={open}
      onOpenChange={(_, d) => onOpenChange(d.open)}
      surfaceMotion={DIALOG_FADE_ONLY}
    >
      <DialogSurface>
        <DialogBody>
          <DialogTitle
            action={
              <Button
                appearance="subtle"
                aria-label={t("common.close")}
                icon={<DismissRegular />}
                onClick={() => onOpenChange(false)}
              />
            }
          >
            {t("shell2.addRomsTitle")}
          </DialogTitle>
          <DialogContent>
            <Field hint={t("shell2.addRomsHint")}>
              <div className={s.row}>
                <Input
                  className={s.input}
                  value={dir}
                  placeholder={t("shell2.addRomsPlaceholder")}
                  contentBefore={<FolderRegular />}
                  onChange={(_, d) => setDir(d.value)}
                />
                <Button
                  icon={<FolderRegular />}
                  onClick={async () => {
                    const p = await pickFolder();
                    if (p) setDir(p);
                  }}
                >
                  {t("shell2.browse")}
                </Button>
              </div>
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => onOpenChange(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              appearance="primary"
              disabled={!dir.trim()}
              onClick={() => {
                onScan(dir.trim());
                setDir("");
              }}
            >
              {t("shell2.scan")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
