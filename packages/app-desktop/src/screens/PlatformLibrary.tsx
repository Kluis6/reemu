import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { CardGridSkeleton } from "../components/CardGridSkeleton";
import { SearchArt } from "../components/EmptyArt";
import { EmptyState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { SectionHeader } from "../components/SectionHeader";
import { platformLabel } from "../lib/platform";
import { errorToast, sysToast } from "../lib/toast";
import { listRoms, removeRom } from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles } from "../styles/xbox";
import { useTranslation } from "react-i18next";

/**
 * `/library/:platform` — grade completa de uma plataforma só. A tela
 * "Meus jogos" mostra só uma prévia de cada plataforma e manda pra cá.
 */
export function PlatformLibrary() {
  const { t } = useTranslation();
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

  const del = useMutation({
    mutationFn: (id: string) => removeRom(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      push(sysToast(t("library.removed"), "Success"));
    },
    onError: (e) => push(errorToast(e, "removeGame")),
  });

  return (
    <div>
      <SectionHeader
        title={platformLabel(platform)}
        right={
          <span className={s.count}>
            {t("library.games", { count: list.length })}
          </span>
        }
      />

      {roms.isLoading ? (
        <CardGridSkeleton />
      ) : list.length === 0 ? (
        <EmptyState art={<SearchArt />} title={t("home.emptyPlatform")}>
          {t("home.emptyPlatformHint")}
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
              onClick={() => navigate(`/rom/${r.id}`)}
              menu={[
                { label: t("library.open"), onClick: () => navigate(`/rom/${r.id}`) },
                {
                  label: t("library.remove"),
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
