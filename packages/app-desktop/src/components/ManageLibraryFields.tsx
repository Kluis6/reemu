import { Button, Select, Spinner, Text, makeStyles, tokens } from "@fluentui/react-components";
import { platformLabel } from "../lib/platform";
import type { ManageLibraryState } from "../lib/useManageLibrary";

const useStyles = makeStyles({
  section: {
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
    marginTop: tokens.spacingVerticalM,
    marginBottom: tokens.spacingVerticalXS,
  },
  row: {
    display: "grid",
    gridTemplateColumns: "1fr auto minmax(160px, 1.2fr) auto",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
    "&:first-of-type": { borderTop: "none" },
  },
  srcRow: {
    display: "flex",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalM,
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  path: {
    flexGrow: 1,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: tokens.fontSizeBase200,
  },
  count: { color: tokens.colorNeutralForeground3, fontSize: tokens.fontSizeBase200 },
});

/**
 * Conteúdo de "Gerenciar biblioteca" (plataformas + core padrão, pastas de
 * origem, limpar tudo) — sem chrome nenhum ao redor, pra caber tanto num
 * `DialogContent` (`ManageLibraryDialog`) quanto direto numa página de
 * Configurações (`SettingsLibrary`). O estado/mutações vêm de
 * `useManageLibrary`.
 */
export function ManageLibraryFields({
  state,
  platforms,
}: {
  state: ManageLibraryState;
  /** `[systemId, quantidade]` presentes na biblioteca. */
  platforms: readonly (readonly [string, number])[];
}) {
  const s = useStyles();
  const { cores, sysCores, sources, setPending, confirm, setConfirm, purge, coreValue } = state;

  const purgeBtn = (target: string, idle: string, confirmLabel: string) => (
    <Button
      size="small"
      appearance={confirm === target ? "primary" : "secondary"}
      disabled={purge.isPending}
      onClick={() => (confirm === target ? purge.mutate(target) : setConfirm(target))}
    >
      {confirm === target ? confirmLabel : idle}
    </Button>
  );

  return (
    <>
      {cores.isLoading || sysCores.isLoading ? (
        <Spinner label="Carregando…" />
      ) : platforms.length === 0 ? (
        <Text>Biblioteca vazia.</Text>
      ) : (
        <>
          <div className={s.section}>Plataformas — core padrão e remoção</div>
          {platforms.map(([sys, n]) => (
            <div key={sys} className={s.row}>
              <Text>{platformLabel(sys)}</Text>
              <span className={s.count}>
                {n} {n === 1 ? "jogo" : "jogos"}
              </span>
              <Select
                value={coreValue(sys)}
                onChange={(_, d) => setPending((p) => ({ ...p, [sys]: d.value }))}
              >
                <option value="">Automático (por extensão)</option>
                {(cores.data ?? []).map((c) => (
                  <option key={c.coreId} value={c.coreId}>
                    {c.name}
                  </option>
                ))}
              </Select>
              {purgeBtn(`sys:${sys}`, "Remover", "Confirmar")}
            </div>
          ))}
        </>
      )}

      {(sources.data?.length ?? 0) > 1 && (
        <>
          <div className={s.section}>Pastas de origem</div>
          {sources.data!.map((src) => (
            <div key={src.path} className={s.srcRow}>
              <span className={s.path} title={src.path}>
                {src.path}
              </span>
              <span className={s.count}>{src.count}</span>
              {purgeBtn(src.path, "Remover", "Confirmar")}
            </div>
          ))}
        </>
      )}

      {platforms.length > 0 && (
        <div className={s.srcRow} style={{ marginTop: 12 }}>
          <span className={s.path}>Toda a biblioteca</span>
          {purgeBtn("__all__", "Limpar tudo", "Confirmar: apagar tudo")}
        </div>
      )}
    </>
  );
}
