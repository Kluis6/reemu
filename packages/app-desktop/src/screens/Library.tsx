import {
  Button,
  makeStyles,
  mergeClasses,
  Menu,
  MenuButton,
  MenuItemRadio,
  MenuList,
  MenuPopover,
  MenuTrigger,
  Tab,
  TabList,
  Text,
  tokens,
  Tooltip,
} from "@fluentui/react-components";
import {
  AddRegular,
  ArrowSortRegular,
  FilterRegular,
  MoreHorizontalRegular,
  WarningRegular,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AddRomsDialog } from "../components/AddRomsDialog";
import { CardGridSkeleton } from "../components/CardGridSkeleton";
import { GamepadArt, SearchArt, StarArt } from "../components/EmptyArt";
import { EmptyState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { SectionHeader } from "../components/SectionHeader";
import { PlatformTile } from "../components/PlatformTile";
import { Shelf } from "../components/Shelf";
import { ManageLibraryDialog } from "../components/ManageLibraryDialog";
import { platformLabel } from "../lib/platform";
import { errorPatch, errorToast, sysToast } from "../lib/toast";
import {
  listRoms,
  removeRom,
  scanLibrary,
  type RomEntry,
  type ScanProgress,
} from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles, useMotionStyles, useShellStyles } from "../styles/xbox";
import { useTranslation } from "react-i18next";

const useLibStyles = makeStyles({
  surface: {
    backgroundColor: "var(--reemuSurfaceSoft)",
    color: tokens.colorNeutralForeground1,
    ":hover": {
      backgroundColor: "var(--reemuSurfaceSoft)",
      color: tokens.colorNeutralForeground1,
    },
  },
  // "Adicionar ROM": mesmo tom de fundo da sidebar (`rail`,
  // `colorNeutralBackground2`) no fundo E na borda, igual aos botões
  // Voltar/Fullscreen da topbar (`layouts/AppShell.tsx`).
  navBtn: {
    backgroundColor: `${tokens.colorNeutralBackground2} !important`,
    border: `1px solid ${tokens.colorNeutralBackground2} !important`,
    ":hover": {
      backgroundColor: `${tokens.colorNeutralBackground2Hover} !important`,
      border: `1px solid ${tokens.colorNeutralBackground2Hover} !important`,
    },
  },
  bar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    columnGap: "12px",
    flexWrap: "wrap",
    // Respiro entre as tabs (Meus jogos/Favoritos) e a linha de filtros
    // abaixo, igual ao modo Xbox de verdade (tabs bem separadas do filtro).
    marginBottom: "24px",
  },
  barRight: { display: "flex", alignItems: "center", columnGap: "10px" },
  // `MenuItemRadio` do Fluent renderiza [checkmark, content] nessa ordem de
  // DOM (sem prop pra inverter) — reordena visualmente via flex `order`: o
  // `content` (`flexGrow: 1` já de fábrica) ocupa a esquerda e empurra o
  // checkmark pro fim da linha. Usado nos dois dropdowns (Plataforma/Ordenar).
  radioMenuList: {
    "& .fui-MenuItemRadio__content": {
      order: 1,
      textAlign: "left",
    },
    "& .fui-MenuItemRadio__checkmark": {
      order: 2,
    },
  },
});

type LibTab = "mine" | "fav";
type LibSort = "name" | "added" | "played";

// chave de tradução de cada ordenação
const SORT_LABEL = {
  name: "library.sort.name",
  added: "library.sort.added",
  played: "library.sort.played",
} as const satisfies Record<LibSort, string>;

/**
 * "Meus jogos" — a biblioteca no estilo modo Xbox: abas (todos / favoritos /
 * recém adicionados), filtro de plataforma, grade agrupada por plataforma. A
 * tela inicial (`/`) com hero e faixas curadas fica em `screens/Home`.
 */
export function Library() {
  const { t } = useTranslation();
  const s = useBrowseStyles();
  const m = useMotionStyles();
  const l = useLibStyles();
  const shell = useShellStyles();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const push = useToastStore((s) => s.push);
  const updateToast = useToastStore((s) => s.update);
  const query = useSearchStore((s) => s.query)
    .trim()
    .toLowerCase();

  const [tab, setTab] = useState<LibTab>("mine");
  const [sort, setSort] = useState<LibSort>("name");
  const [platform, setPlatform] = useState("all");
  const [addOpen, setAddOpen] = useState(false);
  const [manageOpen, setManageOpen] = useState(false);
  const scanId = useRef<string | null>(null);

  const roms = useQuery({ queryKey: ["roms"], queryFn: listRoms, retry: false });

  const scan = useMutation({
    mutationFn: (path: string) =>
      scanLibrary(path, (p: ScanProgress) => {
        if (!scanId.current) return;
        updateToast(scanId.current, {
          message: t("scan.progress", { current: p.current, total: p.total ? `/${p.total}` : "" }),
          progress: p.total ? p.current / p.total : null,
        });
      }),
    onMutate: () => {
      const id = crypto.randomUUID();
      scanId.current = id;
      push({
        id,
        message: t("scan.start"),
        variant: "Info",
        durationMs: 0,
        source: "System",
        progress: null,
      });
    },
    onSuccess: (r) => {
      if (scanId.current) {
        updateToast(scanId.current, {
          message: t("scan.result", { added: r.added, known: r.skippedKnown, skipped: r.skippedUnrecognized }),
          variant: r.errors > 0 ? "Warning" : "Success",
          durationMs: 5000,
          progress: undefined,
        });
      }
      qc.invalidateQueries({ queryKey: ["roms"] });
      qc.invalidateQueries({ queryKey: ["romSources"] });
    },
    onError: (e) => {
      if (scanId.current) {
        updateToast(scanId.current, errorPatch(e, "scanFolder"));
      }
    },
    onSettled: () => {
      scanId.current = null;
    },
  });

  const startScan = (dir: string) => {
    setAddOpen(false);
    scan.mutate(dir);
  };

  const del = useMutation({
    mutationFn: (id: string) => removeRom(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      push(sysToast(t("library.removed"), "Success"));
    },
    onError: (e) => push(errorToast(e, "removeGame")),
  });

  const all = useMemo(() => roms.data ?? [], [roms.data]);

  // plataformas presentes → `[systemId, quantidade]`, ordenado pelo rótulo
  const platformCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const r of all) m.set(r.systemId, (m.get(r.systemId) ?? 0) + 1);
    return [...m.entries()].sort(([a], [b]) =>
      platformLabel(a).localeCompare(platformLabel(b)),
    );
  }, [all]);
  const platforms = useMemo(
    () => platformCounts.map(([id]) => id),
    [platformCounts],
  );

  // aplica busca + filtro de plataforma
  const filtered = useMemo(() => {
    let list = all;
    if (platform !== "all") list = list.filter((r) => r.systemId === platform);
    if (query)
      list = list.filter((r) => r.title.toLowerCase().includes(query));
    return list;
  }, [all, platform, query]);

  const view = useMemo(() => {
    const base = tab === "fav" ? filtered.filter((r) => r.isFavorite) : filtered;
    const cmp: Record<LibSort, (a: RomEntry, b: RomEntry) => number> = {
      name: (a, b) => a.title.localeCompare(b.title),
      added: (a, b) => b.addedAt - a.addedAt,
      played: (a, b) =>
        (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0) ||
        a.title.localeCompare(b.title),
    };
    return [...base].sort(cmp[sort]);
  }, [filtered, tab, sort]);

  const byPlatform = useMemo(() => {
    const groups = new Map<string, RomEntry[]>();
    for (const r of view) {
      const list = groups.get(r.systemId) ?? [];
      list.push(r);
      groups.set(r.systemId, list);
    }
    return [...groups.entries()].sort(([a], [b]) =>
      platformLabel(a).localeCompare(platformLabel(b)),
    );
  }, [view]);

  const cardMenu = (r: RomEntry) =>
    [
      { label: t("library.open"), onClick: () => navigate(`/rom/${r.id}`) },
      { label: t("library.remove"), onClick: () => del.mutate(r.id) },
    ] as const;

  const card = (r: RomEntry) => (
    <GameCard
      key={r.id}
      title={r.title}
      badge={platformLabel(r.systemId)}
      boxart={r.boxart}
      favorite={r.isFavorite}
      onClick={() => navigate(`/rom/${r.id}`)}
      menu={cardMenu(r)}
    />
  );

  const grid = (list: readonly RomEntry[]) => (
    <div className={mergeClasses(s.grid, m.fadeIn)}>{list.map(card)}</div>
  );

  const body = () => {
    if (roms.isLoading) return <CardGridSkeleton />;
    if (roms.isError)
      return (
        <EmptyState icon={<WarningRegular />} title={t("library.unavailable")}>
          {t("library.unavailableHint")}
        </EmptyState>
      );
    if (all.length === 0)
      return (
        <EmptyState
          art={<GamepadArt />}
          title={t("library.empty")}
          action={
            <Button
              appearance="primary"
              icon={<AddRegular />}
              onClick={() => setAddOpen(true)}
            >
              {t("library.addRoms")}
            </Button>
          }
        >
          {t("library.emptyHint")}
        </EmptyState>
      );
    if (view.length === 0) {
      if (tab === "fav")
        return (
          <EmptyState art={<StarArt />} title={t("library.noFavorites")}>
            {t("library.noFavoritesHint")}
          </EmptyState>
        );
      return (
        <EmptyState art={<SearchArt />} title={t("library.nothing")}>
          {t("library.nothingHint")}
        </EmptyState>
      );
    }
    // favoritos: grade única
    if (tab !== "mine") return grid(view);
    // meus jogos: prateleira por plataforma. A Shelf mede a largura e mostra só
    // o que enche a linha; se sobrar jogo, o card 2×2 "ver todos" fecha a linha.
    return byPlatform.map(([sys, plist], i) => {
      const goAll = () => navigate(`/library/${sys}`);
      // teto generoso — a Shelf corta no que couber; não renderiza milhares.
      const capped = plist.slice(0, 40);
      return (
        <section
          className={mergeClasses(s.section, m.riseIn)}
          style={{ animationDelay: `${Math.min(i, 6) * 32}ms` }}
          key={sys}
        >
          <SectionHeader
            title={platformLabel(sys)}
            onSeeAll={goAll}
            right={
              <span className={s.count}>
                {t("library.games", { count: plist.length })}
              </span>
            }
          />
          <Shelf
            more={
              <PlatformTile
                key="more"
                ariaLabel={t("library.seeAll", { count: plist.length, platform: platformLabel(sys) })}
                sample={plist.slice(-8)}
                onClick={goAll}
              />
            }
          >
            {capped.map(card)}
          </Shelf>
        </section>
      );
    });
  };

  return (
    <div>
      <div className={l.bar}>
        <TabList
          selectedValue={tab}
          onTabSelect={(_, d) => setTab(d.value as LibTab)}
        >
          <Tab value="mine">{t("library.tabMine")}</Tab>
          <Tab value="fav">{t("library.tabFav")}</Tab>
        </TabList>

        <div className={l.barRight}>
          <Text size={200} className={s.count}>
            {t("library.games", { count: all.length })}
          </Text>
          <Tooltip content={t("library.addRom")} relationship="label">
            <Button
              appearance="secondary"
              className={mergeClasses(l.navBtn, shell.navIconBtn)}
              icon={<AddRegular />}
              aria-label={t("library.addRom")}
              onClick={() => setAddOpen(true)}
            />
          </Tooltip>
          <Tooltip content={t("library.manage")} relationship="label">
            <Button
              appearance="subtle"
              className={shell.navIconBtn}
              icon={<MoreHorizontalRegular />}
              aria-label={t("library.manage")}
              onClick={() => setManageOpen(true)}
            />
          </Tooltip>
        </div>
      </div>

      <div className={s.toolbar}>
        <Menu
          checkedValues={{ plat: [platform] }}
          onCheckedValueChange={(_, d) => setPlatform(d.checkedItems[0] ?? "all")}
        >
          <MenuTrigger disableButtonEnhancement>
            <MenuButton appearance="subtle" className={l.surface}>
              {platform === "all" ? t("common.platform") : platformLabel(platform)}
            </MenuButton>
          </MenuTrigger>
          <MenuPopover>
            <MenuList className={l.radioMenuList}>
              <MenuItemRadio name="plat" value="all">
                {t("library.allPlatforms")}
              </MenuItemRadio>
              {platforms.map((p) => (
                <MenuItemRadio key={p} name="plat" value={p}>
                  {platformLabel(p)}
                </MenuItemRadio>
              ))}
            </MenuList>
          </MenuPopover>
        </Menu>

        {/* Só com filtro ativo, com texto: desabilitado e só com ícone ele
            virava um quadrado cinza sem significado. */}
        {platform !== "all" && (
          <Button
            appearance="subtle"
            className={l.surface}
            icon={<FilterRegular />}
            onClick={() => setPlatform("all")}
          >
            {t("library.clearFilter")}
          </Button>
        )}

        <Menu
          checkedValues={{ sort: [sort] }}
          onCheckedValueChange={(_, d) =>
            setSort((d.checkedItems[0] as LibSort) ?? "name")
          }
        >
          <MenuTrigger disableButtonEnhancement>
            <MenuButton appearance="subtle" className={l.surface} icon={<ArrowSortRegular />}>
              {t(SORT_LABEL[sort])}
            </MenuButton>
          </MenuTrigger>
          <MenuPopover>
            <MenuList className={l.radioMenuList}>
              {(Object.keys(SORT_LABEL) as LibSort[]).map((k) => (
                <MenuItemRadio key={k} name="sort" value={k}>
                  {t(SORT_LABEL[k])}
                </MenuItemRadio>
              ))}
            </MenuList>
          </MenuPopover>
        </Menu>
      </div>

      {body()}

      <AddRomsDialog open={addOpen} onOpenChange={setAddOpen} onScan={startScan} />
      <ManageLibraryDialog
        open={manageOpen}
        onOpenChange={setManageOpen}
        platforms={platformCounts}
      />
    </div>
  );
}
