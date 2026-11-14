import {
  Button,
  Spinner,
  Tab,
  TabList,
  Text,
  makeStyles,
  tokens,
  mergeClasses,
} from "@fluentui/react-components";
import { FolderRegular, GamesRegular } from "@fluentui/react-icons";
import { platformLabel } from "../lib/platform";
import type { ManageLibraryState } from "../lib/useManageLibrary";
import { useModalButtonStyles } from "../styles/buttons";
import { SelectBlock } from "./SettingBlock";
import { useTabStyles } from "../styles/xbox";
import { useTranslation } from "react-i18next";

const useStyles = makeStyles({
  section: {
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
    marginTop: tokens.spacingVerticalM,
    marginBottom: tokens.spacingVerticalXS,
  },
  // Cada plataforma/pasta numa faixa igual à da lista de Cores (pedido do
  // usuário, 2026-11-06): o mesmo padding nos 4 lados, o ícone num quadrado
  // de 48 na cor do tema. ícone | nome | … | ações.
  card: {
    padding: tokens.spacingHorizontalM,
    boxSizing: "border-box",
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  row: {
    display: "grid",
    // contagem numa coluna fixa (à direita): "1 jogo" e "12 jogos" não
    // empurram o seletor de core, que fica alinhado em todos os cards
    gridTemplateColumns: "auto minmax(0, 1fr) 6em minmax(240px, 1fr) auto",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalL,
    "& > :nth-child(3)": { textAlign: "right", whiteSpace: "nowrap" },
  },
  srcRow: {
    display: "flex",
    alignItems: "center",
    columnGap: tokens.spacingHorizontalL,
  },
  // o quadrado do ícone da lista de Cores (`SettingsCores`, `tile`)
  icon: {
    width: "48px",
    height: "48px",
    flexShrink: 0,
    display: "grid",
    placeItems: "center",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "var(--reemuBrandSolid)",
    color: "var(--reemuOnBrand)",
    fontSize: "28px",
  },
  // Botões com o fundo do `secondary` e sem borda, como os de Cores: o fundo
  // claro do botão aparece sobre a faixa em todos os temas (o `Background3`
  // some em alguns). Borda de 1 px transparente: o tamanho não muda no foco.
  action: {
    flexShrink: 0,
    minWidth: "120px",
    border: "1px solid transparent !important",
  },
  // "Remover" da linha na altura do seletor de core compacto e do ícone
  // (48 px; pedido do usuário, 2026-11-14 — era a altura padrão, 32)
  rowAction: { height: "48px" },
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
  // abas à esquerda; "Atualizar biblioteca" e "Limpar tudo" à direita, na
  // mesma linha (pedido do usuário, 2026-11-10)
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    columnGap: tokens.spacingHorizontalM,
    marginBottom: "12px",
  },
  // o traço da aba ativa começa na borda das faixas (ver SettingsLibrary)
  tabs: { marginLeft: "-3px" },
  headerActions: { display: "flex", columnGap: tokens.spacingHorizontalS },
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
  const btn = useModalButtonStyles();
  const {
    cores,
    sysCores,
    sources,
    setPending,
    confirm,
    setConfirm,
    purge,
    coreValue,
  } = state;

  const purgeBtn = (target: string, idle: string, confirmLabel: string) => (
    <Button
      appearance={confirm === target ? "primary" : "secondary"}
      // com fundo e sem borda, como o "Atualizar biblioteca"; na confirmação
      // fica o primary
      className={mergeClasses(s.action, s.rowAction, confirm !== target && btn.secondaryFlat)}
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
      {!showPlatforms ? null : cores.isLoading || sysCores.isLoading ? (
        <Spinner label={t("common.loading")} />
      ) : platforms.length === 0 ? (
        <Text>{t("manage.empty")}</Text>
      ) : (
        <>
          {platforms.map(([sys, n]) => (
            <div key={sys} className={mergeClasses(s.card, s.row)}>
              <span className={s.icon} aria-hidden><GamesRegular /></span>
              <Text weight="semibold">{platformLabel(sys)}</Text>
              <span className={s.count}>
                {t("library.games", { count: n })}
              </span>
              <SelectBlock
                compact
                raised
                label={t("manage.coreLabel")}
                value={coreValue(sys)}
                options={[
                  { value: "", label: t("manage.auto") },
                  ...(cores.data ?? []).map((c) => ({ value: c.coreId, label: c.name })),
                ]}
                onChange={(v) => setPending((p) => ({ ...p, [sys]: v }))}
              />
              {purgeBtn(`sys:${sys}`, t("common.remove"), t("manage.confirm"))}
            </div>
          ))}
        </>
      )}

      {showFolders && (sources.data?.length ?? 0) > minSources && (
        <>
          <div className={s.section}>{t("manage.sources")}</div>
          {sources.data!.map((src) => (
            <div key={src.path} className={mergeClasses(s.card, s.srcRow)}>
              <span className={s.icon} aria-hidden><FolderRegular /></span>
              <span className={s.path} title={src.path}>
                {src.path}
              </span>
              <span className={s.count}>{src.count}</span>
              {purgeBtn(src.path, t("common.remove"), t("manage.confirm"))}
            </div>
          ))}
        </>
      )}

    </>
  );
}

export type ManageTab = "platforms" | "folders";

/**
 * Linha das abas de "Gerenciar biblioteca": Plataformas e Pastas à esquerda;
 * "Atualizar biblioteca" (só texto) e "Limpar tudo" à direita. "Limpar tudo"
 * pede confirmação no segundo clique, como os "Remover".
 */
export function ManageLibraryHeader({
  state,
  tab,
  onTab,
  hasGames,
}: {
  state: ManageLibraryState;
  tab: ManageTab;
  onTab: (tab: ManageTab) => void;
  hasGames: boolean;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const tb = useTabStyles();
  const btn = useModalButtonStyles();
  const { confirm, setConfirm, purge, rescan, sources } = state;
  const confirming = confirm === "__all__";
  return (
    <div className={s.header}>
      <TabList
        className={mergeClasses(tb.tabs, s.tabs)}
        selectedValue={tab}
        onTabSelect={(_, d) => onTab(d.value as ManageTab)}
      >
        <Tab value="platforms">{t("manage.tabPlatforms")}</Tab>
        <Tab value="folders">{t("manage.tabFolders")}</Tab>
      </TabList>
      <div className={s.headerActions}>
        {(sources.data?.length ?? 0) > 0 && (
          <Button
            appearance="secondary"
            className={mergeClasses(s.action, btn.secondaryFlat)}
            disabled={rescan.isPending}
            onClick={() => rescan.mutate(null)}
          >
            {t("library.refresh")}
          </Button>
        )}
        {hasGames && (
          <Button
            appearance={confirming ? "primary" : "secondary"}
            className={mergeClasses(s.action, !confirming && btn.secondaryFlat)}
            disabled={purge.isPending}
            onClick={() => (confirming ? purge.mutate("__all__") : setConfirm("__all__"))}
          >
            {confirming ? t("manage.confirmClearAll") : t("manage.clearAll")}
          </Button>
        )}
      </div>
    </div>
  );
}
