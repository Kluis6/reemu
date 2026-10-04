import {
  Button,
  Select,
  Spinner,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ArrowSyncRegular } from "@fluentui/react-icons";
import { platformLabel } from "../lib/platform";
import type { ManageLibraryState } from "../lib/useManageLibrary";
import { useTranslation } from "react-i18next";

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
  count: {
    color: tokens.colorNeutralForeground3,
    fontSize: tokens.fontSizeBase200,
  },
  refreshRow: {
    display: "flex",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalM,
    paddingBottom: tokens.spacingVerticalS,
  },
  hint: { flexGrow: 1, color: tokens.colorNeutralForeground3 },
  refreshBtn: { flexShrink: 0, whiteSpace: "nowrap" },
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
  part,
}: {
  state: ManageLibraryState;
  /** `[systemId, quantidade]` presentes na biblioteca. */
  platforms: readonly (readonly [string, number])[];
  /** Só uma parte (abas das Configurações): `platforms` = core e remoção
   *  por plataforma; `folders` = atualizar, pastas de origem e limpar tudo.
   *  Sem isso (modal da biblioteca), tudo junto. */
  part?: "platforms" | "folders";
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const {
    cores,
    sysCores,
    sources,
    setPending,
    confirm,
    setConfirm,
    purge,
    rescan,
    coreValue,
  } = state;

  const purgeBtn = (target: string, idle: string, confirmLabel: string) => (
    <Button
      appearance={confirm === target ? "primary" : "secondary"}
      disabled={purge.isPending}
      onClick={() =>
        confirm === target ? purge.mutate(target) : setConfirm(target)
      }
    >
      {confirm === target ? confirmLabel : idle}
    </Button>
  );

  const showPlatforms = part !== "folders";
  const showFolders = part !== "platforms";
  // Na aba própria a lista de pastas aparece mesmo com uma só.
  const minSources = part === "folders" ? 0 : 1;

  return (
    <>
      {showFolders && (sources.data?.length ?? 0) > 0 && (
        <div className={s.refreshRow}>
          <Text className={s.hint}>{t("manage.refreshHint")}</Text>
          <Button
            appearance="secondary"
            className={s.refreshBtn}
            icon={<ArrowSyncRegular />}
            disabled={rescan.isPending}
            onClick={() => rescan.mutate(null)}
          >
            {t("library.refresh")}
          </Button>
        </div>
      )}

      {!showPlatforms ? null : cores.isLoading || sysCores.isLoading ? (
        <Spinner label={t("common.loading")} />
      ) : platforms.length === 0 ? (
        <Text>{t("manage.empty")}</Text>
      ) : (
        <>
          <div className={s.section}>{t("manage.platformsSection")}</div>
          {platforms.map(([sys, n]) => (
            <div key={sys} className={s.row}>
              <Text>{platformLabel(sys)}</Text>
              <span className={s.count}>
                {t("library.games", { count: n })}
              </span>
              <Select
                value={coreValue(sys)}
                onChange={(_, d) =>
                  setPending((p) => ({ ...p, [sys]: d.value }))
                }
              >
                <option value="">{t("manage.auto")}</option>
                {(cores.data ?? []).map((c) => (
                  <option key={c.coreId} value={c.coreId}>
                    {c.name}
                  </option>
                ))}
              </Select>
              {purgeBtn(`sys:${sys}`, t("common.remove"), t("manage.confirm"))}
            </div>
          ))}
        </>
      )}

      {showFolders && (sources.data?.length ?? 0) > minSources && (
        <>
          <div className={s.section}>{t("manage.sources")}</div>
          {sources.data!.map((src) => (
            <div key={src.path} className={s.srcRow}>
              <span className={s.path} title={src.path}>
                {src.path}
              </span>
              <span className={s.count}>{src.count}</span>
              {purgeBtn(src.path, t("common.remove"), t("manage.confirm"))}
            </div>
          ))}
        </>
      )}

      {showFolders && platforms.length > 0 && (
        <div className={s.srcRow} style={{ marginTop: 12 }}>
          <span className={s.path}>{t("manage.all")}</span>
          {purgeBtn(
            "__all__",
            t("manage.clearAll"),
            t("manage.confirmClearAll"),
          )}
        </div>
      )}
    </>
  );
}
