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
  Spinner,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import {
  ArrowUpRegular,
  DocumentRegular,
  FolderRegular,
  HardDriveRegular,
} from "@fluentui/react-icons";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { browseDir } from "../lib/tauri";
import { useFilePickerStore, type FilePickerRequest } from "../stores/useFilePickerStore";
import { useModalButtonStyles } from "../styles/buttons";

const useStyles = makeStyles({
  surface: { maxWidth: "720px", width: "calc(100vw - 96px)" },
  content: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalS },
  path: { color: tokens.colorNeutralForeground3, overflowWrap: "anywhere" },
  // a folga não deixa a lista cortar o anel de foco
  list: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalS,
    height: "min(50vh, 400px)",
    overflowY: "auto",
    padding: "8px",
    margin: "-8px",
  },
  row: {
    flexShrink: 0,
    justifyContent: "flex-start",
    textAlign: "left",
    columnGap: tokens.spacingHorizontalM,
    maxWidth: "none",
    width: "100%",
    overflowWrap: "anywhere",
  },
  message: { padding: tokens.spacingVerticalL },
});

/** Navegador de arquivos usado com o controle, no lugar da janela do sistema. */
export function FilePicker() {
  const request = useFilePickerStore((s) => s.request);
  const finish = useFilePickerStore((s) => s.finish);
  // Volta o foco a quem abriu: o controle continua de onde estava.
  const opener = useRef<HTMLElement | null>(null);
  if (request && !opener.current) opener.current = document.activeElement as HTMLElement;

  if (!request) return null;
  const done = (path: string | null) => {
    const el = opener.current;
    opener.current = null;
    finish(path);
    requestAnimationFrame(() => el?.focus({ preventScroll: true }));
  };
  return <Browser request={request} onDone={done} />;
}

function Browser({
  request,
  onDone,
}: {
  request: FilePickerRequest;
  onDone: (path: string | null) => void;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const [dir, setDir] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const listing = useQuery({
    queryKey: ["browse-dir", dir, request.directory, request.extensions],
    queryFn: () => browseDir(dir, request.extensions, request.directory),
    retry: false,
    gcTime: 0,
  });
  const data = listing.data;
  const row = mergeClasses(s.row, btn.secondary);

  // Ao entrar numa pasta, o foco vai para o primeiro item dela.
  useEffect(() => {
    if (data) list.current?.querySelector<HTMLElement>("button")?.focus({ preventScroll: true });
  }, [data]);

  return (
    <Dialog
      open
      onOpenChange={(_, d) => !d.open && onDone(null)}
      surfaceMotion={DIALOG_FADE_ONLY}
    >
      <DialogSurface className={s.surface}>
        <DialogBody>
          <DialogTitle>{request.title}</DialogTitle>
          <DialogContent className={s.content}>
            <Caption1 className={s.path}>{dir ?? t("filePicker.start")}</Caption1>
            <div ref={list} className={s.list}>
              {dir !== null && (
                <Button
                  className={row}
                  icon={<ArrowUpRegular />}
                  onClick={() => setDir(data?.parent ?? null)}
                >
                  {t("filePicker.up")}
                </Button>
              )}
              {listing.isLoading && <Spinner size="tiny" label={t("common.loading")} />}
              {listing.isError && <Body1 className={s.message}>{t("filePicker.error")}</Body1>}
              {data?.entries.length === 0 && (
                <Body1 className={s.message}>{t("filePicker.empty")}</Body1>
              )}
              {data?.entries.map((e) => (
                <Button
                  key={e.path}
                  className={row}
                  icon={
                    !e.isDir ? (
                      <DocumentRegular />
                    ) : dir === null ? (
                      <HardDriveRegular />
                    ) : (
                      <FolderRegular />
                    )
                  }
                  onClick={() => (e.isDir ? setDir(e.path) : onDone(e.path))}
                >
                  {e.name}
                </Button>
              ))}
            </div>
          </DialogContent>
          <DialogActions>
            {request.directory && (
              <Button appearance="primary" disabled={dir === null} onClick={() => onDone(dir)}>
                {t("filePicker.chooseFolder")}
              </Button>
            )}
            <Button className={btn.secondary} onClick={() => onDone(null)}>
              {t("common.cancel")}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
