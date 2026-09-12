import { Button, mergeClasses } from "@fluentui/react-components";
import {
  AddRegular,
  GridRegular,
  SettingsRegular,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useRef, useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { AddRomsDialog } from "../components/AddRomsDialog";
import { GamepadArt } from "../components/EmptyArt";
import { EmptyState, LoadingState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { HeroCarousel } from "../components/HeroCarousel";
import { SectionHeader } from "../components/SectionHeader";
import { platformLabel } from "../lib/platform";
import { Shelf } from "../components/Shelf";
import {
  listRoms,
  scanLibrary,
  type RomEntry,
  type ScanProgress,
} from "../lib/tauri";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles, useMotionStyles } from "../styles/xbox";

/** Uma faixa curada da Início (cabeçalho + prateleira). */
function Row({
  title,
  subtitle,
  items,
  onMore,
  render,
  index = 0,
}: {
  title: string;
  subtitle?: string;
  items: readonly RomEntry[];
  onMore?: () => void;
  render: (r: RomEntry) => ReactNode;
  index?: number;
}) {
  const s = useBrowseStyles();
  const m = useMotionStyles();
  if (items.length === 0) return null;
  return (
    <section
      className={mergeClasses(s.section, m.riseIn)}
      style={{ animationDelay: `${40 + index * 40}ms` }}
    >
      <SectionHeader title={title} subtitle={subtitle} onSeeAll={onMore} />
      <Shelf>{items.map(render)}</Shelf>
    </section>
  );
}

/**
 * Tela inicial no estilo "modo Xbox": um hero + faixas curadas (Continuar
 * jogando, Adicionados recentemente…). A biblioteca completa ("Meus jogos")
 * fica em `/library`. Novas seções entram aqui.
 */
export function Home() {
  const s = useBrowseStyles();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const push = useToastStore((st) => st.push);
  const updateToast = useToastStore((st) => st.update);
  const roms = useQuery({
    queryKey: ["roms"],
    queryFn: listRoms,
    retry: false,
  });
  const all = useMemo(() => roms.data ?? [], [roms.data]);

  const [addOpen, setAddOpen] = useState(false);
  const scanId = useRef<string | null>(null);
  // Mesmo fluxo de scan da Library (modal → toast de progresso) — só
  // acionado daqui, pra não precisar navegar até "Meus jogos" primeiro.
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

  // Corta num teto generoso; a prateleira mostra só o que enche a linha (varia
  // com o tamanho da tela).
  const recent = useMemo(
    () =>
      all
        .filter((r) => r.lastPlayedAt != null)
        .sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
        .slice(0, 30),
    [all],
  );
  const added = useMemo(
    () => [...all].sort((a, b) => b.addedAt - a.addedAt).slice(0, 30),
    [all],
  );
  // Destaques do carrossel: jogados recentemente primeiro, depois os que têm
  // capa, sem repetir; teto de 6.
  const featured = useMemo(() => {
    const seen = new Set<string>();
    const out: RomEntry[] = [];
    for (const r of [
      ...recent,
      ...all.filter((r) => r.boxart),
      ...all,
    ]) {
      if (seen.has(r.id)) continue;
      seen.add(r.id);
      out.push(r);
      if (out.length >= 6) break;
    }
    return out;
  }, [recent, all]);

  const card = (r: RomEntry) => (
    <GameCard
      key={r.id}
      title={r.title}
      badge={platformLabel(r.systemId)}
      boxart={r.boxart}
      onClick={() => navigate(`/rom/${r.id}`)}
      menu={[
        {
          label: "Abrir",
          onClick: () => navigate(`/rom/${r.id}`),
        },
        {
          label: "Ver biblioteca",
          onClick: () => navigate("/library"),
        },
      ]}
    />
  );

  if (roms.isLoading) return <LoadingState />;

  if (!roms.isError && all.length === 0) {
    return (
      <>
        <EmptyState
          art={<GamepadArt />}
          title="Bem-vindo ao ReEmu"
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
          Adicione suas ROMs pra montar a biblioteca.
        </EmptyState>
        <AddRomsDialog open={addOpen} onOpenChange={setAddOpen} onScan={startScan} />
      </>
    );
  }

  return (
    <div>
      <HeroCarousel
        items={featured}
        onOpen={(id) => navigate(`/rom/${id}`)}
      />

      <div className={s.toolbar} style={{ marginTop: 18 }}>
        <Button
          shape="circular"
          appearance="subtle"
          icon={<GridRegular />}
          onClick={() => navigate("/library")}
        >
          Todos os jogos ({all.length})
        </Button>
        <Button
          shape="circular"
          appearance="subtle"
          icon={<AddRegular />}
          onClick={() => setAddOpen(true)}
        >
          Adicionar ROMs
        </Button>
        <Button
          shape="circular"
          appearance="subtle"
          icon={<SettingsRegular />}
          onClick={() => navigate("/settings")}
        >
          Configurações
        </Button>
      </div>

      <Row
        title="Continuar jogando"
        subtitle="De onde você parou"
        items={recent}
        onMore={() => navigate("/library")}
        render={card}
        index={0}
      />
      <Row
        title="Adicionados recentemente"
        subtitle="O que entrou por último na biblioteca"
        items={added}
        onMore={() => navigate("/library")}
        render={card}
        index={1}
      />

      <AddRomsDialog open={addOpen} onOpenChange={setAddOpen} onScan={startScan} />
    </div>
  );
}
