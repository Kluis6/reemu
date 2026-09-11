import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { CardGridSkeleton } from "../components/CardGridSkeleton";
import { SearchArt } from "../components/EmptyArt";
import { EmptyState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { SectionHeader } from "../components/SectionHeader";
import { platformLabel } from "../lib/platform";
import { sysToast } from "../lib/toast";
import {
  listRoms,
  removeRom,
  setRomFavorite,
  type RomEntry,
} from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles } from "../styles/xbox";

/**
 * `/library/:platform` — grade completa de uma plataforma só. A tela
 * "Meus jogos" mostra só uma prévia de cada plataforma e manda pra cá.
 */
export function PlatformLibrary() {
  const s = useBrowseStyles();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const push = useToastStore((t) => t.push);
  const { platform = "" } = useParams();
  const query = useSearchStore((q) => q.query).trim().toLowerCase();

  const roms = useQuery({ queryKey: ["roms"], queryFn: listRoms, retry: false });

  const list = useMemo(() => {
    let l = (roms.data ?? []).filter((r) => r.systemId === platform);
    if (query) l = l.filter((r) => r.title.toLowerCase().includes(query));
    return l.sort((a, b) => a.title.localeCompare(b.title));
  }, [roms.data, platform, query]);

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

  const del = useMutation({
    mutationFn: (id: string) => removeRom(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      push(sysToast("Removida da biblioteca.", "Success"));
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, "Error")),
  });

  return (
    <div>
      <SectionHeader
        title={platformLabel(platform)}
        right={
          <span className={s.count}>
            {list.length} {list.length === 1 ? "jogo" : "jogos"}
          </span>
        }
      />

      {roms.isLoading ? (
        <CardGridSkeleton />
      ) : list.length === 0 ? (
        <EmptyState art={<SearchArt />} title="Nada nessa plataforma">
          Ajuste a busca ou volte pra biblioteca.
        </EmptyState>
      ) : (
        <div className={s.grid}>
          {list.map((r) => (
            <GameCard
              key={r.id}
              title={r.title}
              badge={platformLabel(r.systemId)}
              boxart={r.boxart}
              favorite={r.isFavorite}
              onToggleFavorite={() =>
                fav.mutate({ id: r.id, on: !r.isFavorite })
              }
              onClick={() => navigate(`/rom/${r.id}`)}
              menu={[
                { label: "Abrir", onClick: () => navigate(`/rom/${r.id}`) },
                {
                  label: r.isFavorite ? "Desfavoritar" : "Favoritar",
                  onClick: () => fav.mutate({ id: r.id, on: !r.isFavorite }),
                },
                {
                  label: "Remover da biblioteca",
                  onClick: () => del.mutate(r.id),
                },
              ]}
            />
          ))}
        </div>
      )}
    </div>
  );
}
