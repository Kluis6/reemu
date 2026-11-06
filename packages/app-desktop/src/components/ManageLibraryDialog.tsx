import {
  Button,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ManageLibraryFields } from "./ManageLibraryFields";
import { FullscreenDialog } from "./FullscreenDialog";
import { useManageLibrary } from "../lib/useManageLibrary";
import { useModalButtonStyles } from "../styles/buttons";
import { useTranslation } from "react-i18next";

const useStyles = makeStyles({
  // cards de plataforma/pasta com 12 entre si, como em Configurações; a lista
  // ocupa toda a largura do dialog
  content: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalM,
  },
});

/**
 * Modal "Gerenciar biblioteca": por plataforma, escolhe o core padrão e
 * permite apagar; também apaga por pasta de origem e limpa tudo. As escolhas
 * de core ficam pendentes até "Salvar". Mesma funcionalidade também vive numa
 * aba de Configurações (`SettingsLibrary`) — ambas usam `useManageLibrary` +
 * `ManageLibraryFields`.
 */
export function ManageLibraryDialog({
  open,
  onOpenChange,
  platforms,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** `[systemId, quantidade]` presentes na biblioteca. */
  platforms: readonly (readonly [string, number])[];
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const state = useManageLibrary(open);

  const close = () => {
    state.reset();
    onOpenChange(false);
  };

  return (
    <FullscreenDialog
      open={open}
      onClose={close}
      title={t("manage.title")}
      actions={
        <>
          <Button appearance="secondary" className={btn.secondary} onClick={close}>
            {t("common.close")}
          </Button>
          <Button
            appearance="primary"
            disabled={state.save.isPending || Object.keys(state.pending).length === 0}
            onClick={() => state.save.mutate(undefined, { onSuccess: () => onOpenChange(false) })}
          >
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className={s.content}>
        <ManageLibraryFields state={state} platforms={platforms} />
      </div>
    </FullscreenDialog>
  );
}
