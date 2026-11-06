import {
  Button,
  Select,
  Spinner,
  Text,
  makeStyles,
  tokens,
  mergeClasses,
} from "@fluentui/react-components";
import {
  ArrowSyncRegular,
  FolderRegular,
  GamesRegular,
  LibraryRegular,
} from "@fluentui/react-icons";
import { platformLabel } from "../lib/platform";
import type { ManageLibraryState } from "../lib/useManageLibrary";
import { useModalButtonStyles } from "../styles/buttons";
import { useTranslation } from "react-i18next";

const useStyles = makeStyles({
  section: {
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
    marginTop: tokens.spacingVerticalM,
    marginBottom: tokens.spacingVerticalXS,
  },
  // 1º item da aba Plataformas: o espaço acima já vem das abas (24).
  sectionFirst: { marginTop: 0 },
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
  // os cards já têm 12 de espaço entre si (gap da página)
  cardTop: { marginTop: tokens.spacingVerticalM },
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
  const btn = useModalButtonStyles();
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
      className={s.action}
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
            // fora das faixas, direto no fundo do card: o preenchimento neutro
            // (o do `secondary` some no fundo claro)
            className={mergeClasses(s.refreshBtn, s.action, btn.secondaryFlat)}
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
          <div
            className={mergeClasses(
              s.section,
              part === "platforms" && s.sectionFirst,
            )}
          >
            {t("manage.platformsSection")}
          </div>
          {platforms.map(([sys, n]) => (
            <div key={sys} className={mergeClasses(s.card, s.row)}>
              <span className={s.icon} aria-hidden><GamesRegular /></span>
              <Text weight="semibold">{platformLabel(sys)}</Text>
              <span className={s.count}>
                {t("library.games", { count: n })}
              </span>
              <Select
                appearance="filled-darker"
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

      {showFolders && platforms.length > 0 && (
        <div className={mergeClasses(s.card, s.srcRow, s.cardTop)}>
          <span className={s.icon} aria-hidden><LibraryRegular /></span>
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
