import {
  Button,
  makeStyles,
  Menu,
  MenuButton,
  MenuItem,
  MenuItemRadio,
  MenuList,
  MenuPopover,
  MenuTrigger,
  Spinner,
  Tab,
  TabList,
  Text,
} from "@fluentui/react-components";
import {
  AddRegular,
  DeleteRegular,
  FilterRegular,
  MoreHorizontalRegular,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AddRomsDialog } from "../components/AddRomsDialog";
import { GameCard } from "../components/GameCard";
import { platformLabel } from "../lib/platform";
import { sysToast } from "../lib/toast";
import {
  clearLibrary,
  listRomSources,
  listRoms,
  removeRom,
  removeRomSource,
  removeRomSystem,
  scanLibrary,
  setRomFavorite,
  type RomEntry,
  type ScanProgress,
} from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles } from "../styles/xbox";

const useLibStyles = makeStyles({
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

type LibTab = "mine" | "fav" | "recent";

/**
 * "Meus jogos" — a biblioteca no estilo modo Xbox: abas (todos / favoritos /
 * recém adicionados), filtro de plataforma, grade agrupada por plataforma. A
 * tela inicial (`/`) com hero e faixas curadas fica em `screens/Home`.
 */
export function Library() {
  const s = useBrowseStyles();
  const l = useLibStyles();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const push = useToastStore((s) => s.push);
  const updateToast = useToastStore((s) => s.update);
  const query = useSearchStore((s) => s.query)
    .trim()
    .toLowerCase();

  const [tab, setTab] = useState<LibTab>("mine");
  const [platform, setPlatform] = useState("all");
  const [addOpen, setAddOpen] = useState(false);
  const [showManage, setShowManage] = useState(false);
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

  const sources = useQuery({
    queryKey: ["romSources"],
    queryFn: listRomSources,
    enabled: showManage,
    retry: false,
  });
  const [confirmPurge, setConfirmPurge] = useState<string | null>(null);
  const purge = useMutation({
    mutationFn: (target: string) => {
      if (target === "__all__") return clearLibrary();
      if (target.startsWith("sys:")) return removeRomSystem(target.slice(4));
      return removeRomSource(target);
    },
    onSuccess: (n) => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      qc.invalidateQueries({ queryKey: ["romSources"] });
      setConfirmPurge(null);
      push(sysToast(`${n} jogo(s) removido(s) da biblioteca.`, "Success"));
    },
    onError: (e) => {
      setConfirmPurge(null);
      push(sysToast(`Falha: ${e}`, "Error"));
    },
  });
  const purgeBtn = (target: string, idle: string, confirm: string) => (
    <Button
      size="small"
      icon={<DeleteRegular />}
      appearance={confirmPurge === target ? "primary" : "secondary"}
      disabled={purge.isPending}
      onClick={() =>
        confirmPurge === target ? purge.mutate(target) : setConfirmPurge(target)
      }
    >
      {confirmPurge === target ? confirm : idle}
    </Button>
  );

  const all = useMemo(() => roms.data ?? [], [roms.data]);

  // plataformas presentes, ordenadas pelo rótulo
  const platforms = useMemo(() => {
    const ids = [...new Set(all.map((r) => r.systemId))];
    return ids.sort((a, b) => platformLabel(a).localeCompare(platformLabel(b)));
  }, [all]);

  // aplica busca + filtro de plataforma
  const filtered = useMemo(() => {
    let list = all;
    if (platform !== "all") list = list.filter((r) => r.systemId === platform);
    if (query)
      list = list.filter((r) => r.title.toLowerCase().includes(query));
    return list;
  }, [all, platform, query]);

  const view = useMemo(() => {
    if (tab === "fav") return filtered.filter((r) => r.isFavorite);
    if (tab === "recent")
      return [...filtered].sort((a, b) => b.addedAt - a.addedAt);
    return filtered;
  }, [filtered, tab]);

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
    <div className={s.grid}>{list.map(card)}</div>
  );

  const emptyState = (icon: string, title: string, sub?: string) => (
    <div className={s.empty}>
      <div className={s.emptyIcon}>{icon}</div>
      <h2>{title}</h2>
      {sub && <span>{sub}</span>}
    </div>
  );

  const body = () => {
    if (roms.isLoading)
      return <Spinner style={{ marginTop: 40 }} label="Carregando biblioteca…" />;
    if (roms.isError)
      return emptyState(
        "⚠",
        "Biblioteca indisponível",
        "O backend não conseguiu abrir o banco de dados.",
      );
    if (all.length === 0)
      return emptyState(
        "🕹",
        "Nenhuma ROM ainda",
        "Use o + pra apontar a pasta das suas ROMs.",
      );
    if (view.length === 0) {
      if (tab === "fav")
        return emptyState("★", "Sem favoritos", "Favorite um jogo pelo menu do cartão.");
      return emptyState("🔍", "Nada aqui", "Ajuste o filtro de plataforma ou a busca.");
    }
    // favoritos e recém adicionados: grade única; meus jogos: agrupada
    if (tab !== "mine") return grid(view);
    return byPlatform.map(([sys, list]) => (
      <section className={s.section} key={sys}>
        <div className={s.sectionHead}>
          <h2 className={s.sectionTitle}>{platformLabel(sys)}</h2>
          <span className={s.count}>
            {list.length} {list.length === 1 ? "jogo" : "jogos"}
          </span>
        </div>
        {grid(list)}
      </section>
    ));
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
          <Tab value="recent">Recém adicionados</Tab>
        </TabList>

        <div className={l.barRight}>
          <Text size={200} className={s.count}>
            {all.length} {all.length === 1 ? "jogo" : "jogos"}
          </Text>
          <Button
            appearance="primary"
            icon={<AddRegular />}
            aria-label="Adicionar ROMs"
            onClick={() => setAddOpen(true)}
          />
          <Menu>
            <MenuTrigger disableButtonEnhancement>
              <MenuButton
                appearance="subtle"
                icon={<MoreHorizontalRegular />}
                aria-label="Mais ações"
              />
            </MenuTrigger>
            <MenuPopover>
              <MenuList>
                <MenuItem
                  icon={<DeleteRegular />}
                  onClick={() => setShowManage((v) => !v)}
                >
                  Gerenciar biblioteca
                </MenuItem>
              </MenuList>
            </MenuPopover>
          </Menu>
        </div>
      </div>

      <div className={s.toolbar}>
        <Menu
          checkedValues={{ plat: [platform] }}
          onCheckedValueChange={(_, d) => setPlatform(d.checkedItems[0] ?? "all")}
        >
          <MenuTrigger disableButtonEnhancement>
            <MenuButton appearance="subtle" icon={<FilterRegular />}>
              {platform === "all" ? "Todas as plataformas" : platformLabel(platform)}
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
      </div>

      {showManage && (
        <div className={s.libManage}>
          <div className={s.sectionSub}>Por plataforma</div>
          {byPlatform.length === 0 && (
            <div className={s.count}>Biblioteca vazia.</div>
          )}
          {byPlatform.map(([sys, list]) => (
            <div key={sys} className={s.libRow}>
              <span className={s.libPath}>{platformLabel(sys)}</span>
              <span className={s.count}>{list.length}</span>
              {purgeBtn(`sys:${sys}`, "Remover", "Confirmar")}
            </div>
          ))}
          {(sources.data?.length ?? 0) > 1 && (
            <>
              <div className={s.sectionSub} style={{ marginTop: 12 }}>
                Por pasta de origem
              </div>
              {sources.data!.map((src) => (
                <div key={src.path} className={s.libRow}>
                  <span className={s.libPath} title={src.path}>
                    {src.path}
                  </span>
                  <span className={s.count}>{src.count}</span>
                  {purgeBtn(src.path, "Remover", "Confirmar")}
                </div>
              ))}
            </>
          )}
          <div className={s.libRow} style={{ marginTop: 12 }}>
            <span className={s.libPath}>
              Toda a biblioteca ({all.length} jogos)
            </span>
            {purgeBtn("__all__", "Limpar tudo", "Confirmar: apagar tudo")}
          </div>
        </div>
      )}

      {body()}

      <AddRomsDialog open={addOpen} onOpenChange={setAddOpen} onScan={startScan} />
    </div>
  );
}
