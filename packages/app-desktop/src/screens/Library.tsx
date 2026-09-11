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
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AddRomsDialog } from "../components/AddRomsDialog";
import { GamepadArt, SearchArt, StarArt } from "../components/EmptyArt";
import { EmptyState, LoadingState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { SectionHeader } from "../components/SectionHeader";
import { PlatformTile } from "../components/PlatformTile";
import { Shelf } from "../components/Shelf";
import { ManageLibraryDialog } from "../components/ManageLibraryDialog";
import { platformLabel } from "../lib/platform";
import { sysToast } from "../lib/toast";
import {
  listRoms,
  removeRom,
  scanLibrary,
  setRomFavorite,
  type RomEntry,
  type ScanProgress,
} from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles, useMotionStyles } from "../styles/xbox";

const useLibStyles = makeStyles({
  surface: {
    backgroundColor: "var(--reemuSurfaceSoft)",
    color: tokens.colorNeutralForeground1,
    ":hover": {
      backgroundColor: "var(--reemuSurfaceSoft)",
      color: tokens.colorNeutralForeground1,
    },
  },
  bar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    columnGap: "12px",
    flexWrap: "wrap",
    marginBottom: "10px",
  },
  barRight: { display: "flex", alignItems: "center", columnGap: "10px" },
});

type LibTab = "mine" | "fav";
type LibSort = "name" | "added" | "played";

const SORT_LABEL: Record<LibSort, string> = {
  name: "Nome (A–Z)",
  added: "Adicionado recentemente",
  played: "Jogado por último",
};

/**
 * "Meus jogos" — a biblioteca no estilo modo Xbox: abas (todos / favoritos /
 * recém adicionados), filtro de plataforma, grade agrupada por plataforma. A
 * tela inicial (`/`) com hero e faixas curadas fica em `screens/Home`.
 */
export function Library() {
  const s = useBrowseStyles();
  const m = useMotionStyles();
  const l = useLibStyles();
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
          message: `Escaneando ${p.current}${p.total ? `/${p.total}` : ""}…`,
          progress: p.total ? p.current / p.total : null,
        });
      }),
    onMutate: () => {
      const id = crypto.randomUUID();
      scanId.current = id;
      push({
        id,
        message: "Escaneando…",
        variant: "Info",
        durationMs: 0,
        source: "System",
        progress: null,
      });
    },
    onSuccess: (r) => {
      if (scanId.current) {
        updateToast(scanId.current, {
          message: `${r.added} adicionada(s) · ${r.skippedKnown} já na biblioteca · ${r.skippedUnrecognized} ignorada(s)`,
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
        updateToast(scanId.current, {
          message: `Scan falhou: ${e}`,
          variant: "Error",
          durationMs: 6000,
          progress: undefined,
        });
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
      push(sysToast("Removida da biblioteca.", "Success"));
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, "Error")),
  });

  const fav = useMutation({
    mutationFn: ({ id, on }: { id: string; on: boolean }) =>
      setRomFavorite(id, on),
    onMutate: async ({ id, on }) => {
      await qc.cancelQueries({ queryKey: ["roms"] });
      const prev = qc.getQueryData<RomEntry[]>(["roms"]);
      qc.setQueryData<RomEntry[]>(["roms"], (old) =>
        (old ?? []).map((r) => (r.id === id ? { ...r, isFavorite: on } : r)),
      );
      return { prev };
    },
    onError: (e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(["roms"], ctx.prev);
      push(sysToast(`Falha ao favoritar: ${e}`, "Error"));
    },
    onSettled: () => qc.invalidateQueries({ queryKey: ["roms"] }),
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
      { label: "Abrir", onClick: () => navigate(`/rom/${r.id}`) },
      {
        label: r.isFavorite ? "Desfavoritar" : "Favoritar",
        onClick: () => fav.mutate({ id: r.id, on: !r.isFavorite }),
      },
      { label: "Remover da biblioteca", onClick: () => del.mutate(r.id) },
    ] as const;

  const card = (r: RomEntry) => (
    <GameCard
      key={r.id}
      title={r.title}
      badge={platformLabel(r.systemId)}
      boxart={r.boxart}
      favorite={r.isFavorite}
      onToggleFavorite={() => fav.mutate({ id: r.id, on: !r.isFavorite })}
      onClick={() => navigate(`/rom/${r.id}`)}
      menu={cardMenu(r)}
    />
  );

  const grid = (list: readonly RomEntry[]) => (
    <div className={mergeClasses(s.grid, m.fadeIn)}>{list.map(card)}</div>
  );

  const body = () => {
    if (roms.isLoading) return <LoadingState label="Carregando biblioteca…" />;
    if (roms.isError)
      return (
        <EmptyState icon="⚠" title="Biblioteca indisponível">
          O backend não conseguiu abrir o banco de dados.
        </EmptyState>
      );
    if (all.length === 0)
      return (
        <EmptyState
          art={<GamepadArt />}
          title="Nenhuma ROM ainda"
          action={
            <Button
              appearance="primary"
              icon={<AddRegular />}
              onClick={() => setAddOpen(true)}
            >
              Adicionar ROMs…
            </Button>
          }
        >
          Aponte a pasta das suas ROMs pra montar a biblioteca.
        </EmptyState>
      );
    if (view.length === 0) {
      if (tab === "fav")
        return (
          <EmptyState art={<StarArt />} title="Sem favoritos">
            Favorite um jogo pelo menu do cartão.
          </EmptyState>
        );
      return (
        <EmptyState art={<SearchArt />} title="Nada aqui">
          Ajuste o filtro de plataforma ou a busca.
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
          style={{ animationDelay: `${Math.min(i, 8) * 55}ms` }}
          key={sys}
        >
          <SectionHeader
            title={platformLabel(sys)}
            onSeeAll={goAll}
            right={
              <span className={s.count}>
                {plist.length} {plist.length === 1 ? "jogo" : "jogos"}
              </span>
            }
          />
          <Shelf
            more={
              <PlatformTile
                key="more"
                ariaLabel={`Ver todos os ${plist.length} de ${platformLabel(sys)}`}
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
          <Tab value="mine">Meus jogos</Tab>
          <Tab value="fav">Favoritos</Tab>
        </TabList>

        <div className={l.barRight}>
          <Text size={200} className={s.count}>
            {all.length} {all.length === 1 ? "jogo" : "jogos"}
          </Text>
          <Tooltip content="Adicionar ROM" relationship="label">
            <Button
              appearance="subtle"
              className={l.surface}
              icon={<AddRegular />}
              aria-label="Adicionar ROM"
              onClick={() => setAddOpen(true)}
            />
          </Tooltip>
          <Tooltip content="Gerenciar biblioteca" relationship="label">
            <Button
              appearance="subtle"
              icon={<MoreHorizontalRegular />}
              aria-label="Gerenciar biblioteca"
              onClick={() => setManageOpen(true)}
            />
          </Tooltip>
        </div>
      </div>

      <div className={s.toolbar}>
        <Tooltip content="Limpar filtros" relationship="label">
          <Button
            appearance="subtle"
            className={l.surface}
            icon={<FilterRegular />}
            aria-label="Limpar filtros"
            disabled={platform === "all"}
            onClick={() => setPlatform("all")}
          />
        </Tooltip>

        <Menu
          checkedValues={{ plat: [platform] }}
          onCheckedValueChange={(_, d) => setPlatform(d.checkedItems[0] ?? "all")}
        >
          <MenuTrigger disableButtonEnhancement>
            <MenuButton appearance="subtle">
              {platform === "all" ? "Plataforma" : platformLabel(platform)}
            </MenuButton>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              <MenuItemRadio name="plat" value="all">
                Todas as plataformas
              </MenuItemRadio>
              {platforms.map((p) => (
                <MenuItemRadio key={p} name="plat" value={p}>
                  {platformLabel(p)}
                </MenuItemRadio>
              ))}
            </MenuList>
          </MenuPopover>
        </Menu>

        <Menu
          checkedValues={{ sort: [sort] }}
          onCheckedValueChange={(_, d) =>
            setSort((d.checkedItems[0] as LibSort) ?? "name")
          }
        >
          <MenuTrigger disableButtonEnhancement>
            <MenuButton appearance="subtle" icon={<ArrowSortRegular />}>
              {SORT_LABEL[sort]}
            </MenuButton>
          </MenuTrigger>
          <MenuPopover>
            <MenuList>
              {(Object.keys(SORT_LABEL) as LibSort[]).map((k) => (
                <MenuItemRadio key={k} name="sort" value={k}>
                  {SORT_LABEL[k]}
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
