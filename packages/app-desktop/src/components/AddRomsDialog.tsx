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
                aria-label="Fechar"
                icon={<DismissRegular />}
                onClick={() => onOpenChange(false)}
              />
            }
          >
            Adicionar ROMs à biblioteca
          </DialogTitle>
          <DialogContent>
            <Field hint="A pasta é varrida recursivamente. Extensões desconhecidas são ignoradas.">
              <div className={s.row}>
                <Input
                  className={s.input}
                  value={dir}
                  placeholder="/caminho/para/suas/ROMs"
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
                  Procurar…
                </Button>
              </div>
            </Field>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => onOpenChange(false)}>
              Cancelar
            </Button>
            <Button
              appearance="primary"
              disabled={!dir.trim()}
              onClick={() => {
                onScan(dir.trim());
                setDir("");
              }}
            >
              Escanear
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
