import { Button, Card, mergeClasses, Text } from "@fluentui/react-components";
import {
  AddRegular,
  ChevronRightRegular,
  GridRegular,
} from "@fluentui/react-icons";
import { useQuery } from "@tanstack/react-query";
import { useMemo, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { EmptyState, LoadingState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { platformLabel } from "../lib/platform";
import { Shelf } from "../components/Shelf";
import { listRoms, type RomEntry } from "../lib/tauri";
import { useBrowseStyles, useHeroStyles, useMotionStyles } from "../styles/xbox";

/** Uma faixa curada da Início (título + "ver mais" opcional + prateleira). */
function Row({
  title,
  items,
  onMore,
  render,
  index = 0,
}: {
  title: string;
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
      style={{ animationDelay: `${120 + index * 70}ms` }}
    >
      <div className={s.sectionHead}>
        <Text as="h2" className={s.sectionTitle}>
          {title}
        </Text>
        {onMore && (
          <Button
            className={s.sectionChevron}
            onClick={onMore}
            tabIndex={-1}
            aria-hidden
            appearance="subtle"
            size="small"
            icon={<ChevronRightRegular />}
          />
        )}
      </div>
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
  const h = useHeroStyles();
  const navigate = useNavigate();
  const roms = useQuery({
    queryKey: ["roms"],
    queryFn: listRoms,
    retry: false,
  });
  const all = useMemo(() => roms.data ?? [], [roms.data]);

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
  const hero = recent[0] ?? all.find((r) => r.boxart) ?? all[0];

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
      <EmptyState
        icon="🕹"
        title="Bem-vindo ao ReEmu"
        action={
          <Button
            appearance="primary"
            icon={<AddRegular />}
            onClick={() => navigate("/library")}
          >
            Adicionar ROMs…
          </Button>
        }
      >
        Adicione suas ROMs pra montar a biblioteca.
      </EmptyState>
    );
  }

  return (
    <div>
      {hero && (
        <Card
          className={h.hero}
          onClick={() => navigate(`/rom/${hero.id}`)}
          aria-label={hero.title}
        >
          {hero.boxart && <img src={hero.boxart} alt="" />}
          <span className={h.body}>
            <span className={h.kicker}>
              {hero.lastPlayedAt ? "Continuar" : "Destaque"}
            </span>
            <span className={h.title}>{hero.title}</span>
            <span className={h.sub}>{platformLabel(hero.systemId)}</span>
          </span>
        </Card>
      )}

      <Row
        title="Continuar jogando"
        items={recent}
        onMore={() => navigate("/library")}
        render={card}
        index={0}
      />
      <Row
        title="Adicionados recentemente"
        items={added}
        onMore={() => navigate("/library")}
        render={card}
        index={1}
      />

      <div className={s.toolbar} style={{ marginTop: 28 }}>
        <Button
          shape="circular"
          appearance="subtle"
          icon={<GridRegular />}
          onClick={() => navigate("/library")}
        >
          Ver todos os jogos ({all.length})
        </Button>
      </div>
    </div>
  );
}
