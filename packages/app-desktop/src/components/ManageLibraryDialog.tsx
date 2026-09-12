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
import { DIALOG_FADE_ONLY } from "../lib/motion";
import { useManageLibrary } from "../lib/useManageLibrary";

const useStyles = makeStyles({
  // Maior que o padrão do Fluent (600px, altura de sobra pro conteúdo) — a
  // linha de plataforma tem 4 colunas (nome, contagem, seletor de core,
  // remover) e a lista de plataformas cresce bastante.
  surface: { width: "90vw", maxWidth: "90vw", height: "80vh", maxHeight: "80vh" },
  // `DialogBody` não herda altura sozinho (Fluent deixa o grid encolher pro
  // conteúdo) — sem isto a área de conteúdo não teria altura fixa pra rolar
  // dentro dos 80vh do surface.
  body: { height: "100%" },
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
    <Dialog
      open={open}
      onOpenChange={(_, d) => onOpenChange(d.open)}
      surfaceMotion={DIALOG_FADE_ONLY}
    >
      <DialogSurface className={s.surface}>
        <DialogBody className={s.body}>
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
