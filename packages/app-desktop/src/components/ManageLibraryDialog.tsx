import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  makeStyles,
} from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { ManageLibraryFields } from "./ManageLibraryFields";
import { useManageLibrary } from "../lib/useManageLibrary";

const useStyles = makeStyles({
  // Mais largo que o padrão do Fluent (600px) — a linha de plataforma tem 4
  // colunas (nome, contagem, seletor de core, remover) e ficava apertada.
  surface: { maxWidth: "820px" },
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
  const s = useStyles();
  const state = useManageLibrary(open);

  const close = () => {
    state.reset();
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={(_, d) => onOpenChange(d.open)}>
      <DialogSurface className={s.surface}>
        <DialogBody>
          <DialogTitle
            action={
              <Button
                appearance="subtle"
                aria-label="Fechar"
                icon={<DismissRegular />}
                onClick={close}
              />
            }
          >
            Gerenciar biblioteca
          </DialogTitle>
          <DialogContent>
            <ManageLibraryFields state={state} platforms={platforms} />
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={close}>
              Fechar
            </Button>
            <Button
              appearance="primary"
              disabled={state.save.isPending || Object.keys(state.pending).length === 0}
              onClick={() => state.save.mutate(undefined, { onSuccess: () => onOpenChange(false) })}
            >
              Salvar
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
