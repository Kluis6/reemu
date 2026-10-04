import {
  Button,
  Tab,
  TabList,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { LoadingState } from "../../components/EmptyState";
import { ManageLibraryFields } from "../../components/ManageLibraryFields";
import { platformLabel } from "../../lib/platform";
import { listRoms } from "../../lib/tauri";
import { useManageLibrary } from "../../lib/useManageLibrary";
import { useTabStyles } from "../../styles/xbox";
import { useTranslation } from "react-i18next";

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalM,
  },
  saveRow: { display: "flex", justifyContent: "flex-end" },
  // 24 até o conteúdo, como nas outras abas (12 do `gap` + 12).
  tabs: { alignSelf: "flex-start", marginBottom: "12px" },
});

/**
 * Aba "Gerenciar biblioteca" em Configurações — mesma funcionalidade do
 * modal acionado pela Biblioteca (`ManageLibraryDialog`), só que como página
 * cheia em vez de modal, em duas abas (plataformas; pastas). Reusa
 * `useManageLibrary`/`ManageLibraryFields`.
 */
export function SettingsLibrary() {
  const { t } = useTranslation();
  const s = useStyles();
  const tb = useTabStyles();
  const [tab, setTab] = useState<"platforms" | "folders">("platforms");
  const roms = useQuery({
    queryKey: ["roms"],
    queryFn: listRoms,
    retry: false,
  });
  const platforms = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of roms.data ?? [])
      m.set(r.systemId, (m.get(r.systemId) ?? 0) + 1);
    return [...m.entries()].sort(([a], [b]) =>
      platformLabel(a).localeCompare(platformLabel(b)),
    );
  }, [roms.data]);

  const state = useManageLibrary(true);

  if (roms.isLoading) return <LoadingState label={t("library.loading")} />;

  return (
    <div className={s.root}>
      <TabList
        className={mergeClasses(tb.tabs, s.tabs)}
        selectedValue={tab}
        onTabSelect={(_, d) => setTab(d.value as "platforms" | "folders")}
      >
        <Tab value="platforms">{t("manage.tabPlatforms")}</Tab>
        <Tab value="folders">{t("manage.tabFolders")}</Tab>
      </TabList>
      <ManageLibraryFields state={state} platforms={platforms} part={tab} />
      {tab === "platforms" && (
        <div className={s.saveRow}>
          <Button
            appearance="primary"
            disabled={
              state.save.isPending || Object.keys(state.pending).length === 0
            }
            onClick={() => state.save.mutate()}
          >
            {t("common.save")}
          </Button>
        </div>
      )}
    </div>
  );
}
