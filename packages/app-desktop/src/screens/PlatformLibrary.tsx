import { Spinner, Text } from "@fluentui/react-components";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { GameCard } from "../components/GameCard";
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
      <div className={s.sectionHead}>
        <h2 className={s.sectionTitle}>{platformLabel(platform)}</h2>
        <span className={s.count}>
          {list.length} {list.length === 1 ? "jogo" : "jogos"}
        </span>
      </div>

      {roms.isLoading ? (
        <Spinner style={{ marginTop: 40 }} label="Carregando…" />
      ) : list.length === 0 ? (
        <div className={s.empty}>
          <div className={s.emptyIcon}>🕹</div>
          <h2>Nada nessa plataforma</h2>
          <Text>Ajuste a busca ou volte pra biblioteca.</Text>
        </div>
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
