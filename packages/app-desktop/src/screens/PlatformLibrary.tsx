import { DeleteRegular, OpenRegular } from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { useMemo, useRef } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { CardGridSkeleton } from "../components/CardGridSkeleton";
import { SearchArt } from "../components/EmptyArt";
import { EmptyState } from "../components/EmptyState";
import { GameCard } from "../components/GameCard";
import { SearchChip } from "../components/SearchChip";
import { SectionHeader } from "../components/SectionHeader";
import { VirtualGrid, type VirtualGridHandle } from "../components/VirtualGrid";
import { platformLabel } from "../lib/platform";
import {
  formatCardGroupBadge,
  groupRoms,
  romVariantLabel,
  type GroupedRomEntry,
} from "../lib/romGrouping";
import { letterIndex } from "../lib/virtualGrid";
import { errorToast, sysToast } from "../lib/toast";
import { listRoms, removeRom } from "../lib/tauri";
import { useSearchStore } from "../stores/useSearchStore";
import { useToastStore } from "../stores/useToastStore";
import { useBrowseStyles } from "../styles/xbox";
import { useTranslation } from "react-i18next";

// Abaixo disso a lista cabe em poucas telas e pular por letra não ajuda.
const LETTERS_MIN_ITEMS = 30;

const useStyles = makeStyles({
  tools: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: tokens.spacingHorizontalXS,
    marginBottom: tokens.spacingVerticalL,
  },
  letter: {
    minWidth: "36px",
    maxWidth: "36px",
    paddingLeft: 0,
    paddingRight: 0,
    color: `var(--reemuWallpaperText, ${tokens.colorNeutralForeground1})`,
  },
});

/**
 * `/library/:platform` — grade completa de uma plataforma só. A tela
 * "Meus jogos" mostra só uma prévia de cada plataforma e manda pra cá.
 */
export function PlatformLibrary() {
  const { t, i18n } = useTranslation();
  const s = useBrowseStyles();
  const l = useStyles();
  const grid = useRef<VirtualGridHandle>(null);
  const qc = useQueryClient();
  const navigate = useNavigate();
  const push = useToastStore((t) => t.push);
  const { platform = "" } = useParams();
  const query = useSearchStore((q) => q.query).trim().toLowerCase();

  const roms = useQuery({ queryKey: ["roms"], queryFn: listRoms, retry: false });
  const uiLang = i18n.resolvedLanguage ?? i18n.language;

  const list = useMemo(() => {
    let items = groupRoms(
      (roms.data ?? []).filter((r) => r.systemId === platform),
      uiLang,
    );
    if (query) {
      items = items.filter(
        (r) =>
          r.title.toLowerCase().includes(query) ||
          r.variants.some((v) => v.title.toLowerCase().includes(query)),
      );
    }
    return items.sort((a, b) => a.title.localeCompare(b.title));
  }, [roms.data, platform, query, uiLang]);

  const del = useMutation({
    mutationFn: async (ids: string[]) => {
      await Promise.all(ids.map((id) => removeRom(id)));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["roms"] });
      push(sysToast(t("library.removed"), "Success"));
    },
    onError: (e) => push(errorToast(e, "removeGame")),
  });

  const letters = useMemo(
    () => (list.length >= LETTERS_MIN_ITEMS ? letterIndex(list.map((r) => r.title)) : []),
    [list],
  );

  const card = (r: GroupedRomEntry) => (
    <GameCard
      key={r.id}
      title={r.title}
      badge={platformLabel(r.systemId)}
      regionBadge={formatCardGroupBadge(r, t)}
      boxart={r.boxart}
      favorite={r.isFavorite}
      onClick={() => navigate(`/rom/${r.id}`)}
      menu={[
        ...(r.variants.length > 1
          ? r.variants.map((v) => ({
              label: t("library.openVersion", {
                version: romVariantLabel(v, r.variants, t),
              }),
              icon: <OpenRegular />,
              onClick: () => navigate(`/rom/${v.id}`),
            }))
          : [
              {
                label: t("library.open"),
                icon: <OpenRegular />,
                onClick: () => navigate(`/rom/${r.id}`),
              },
            ]),
        {
          label: t("library.remove"),
          icon: <DeleteRegular />,
          onClick: () => del.mutate(r.variants.map((v) => v.id)),
        },
      ]}
    />
  );

  return (
    <div>
      <SectionHeader
        title={platformLabel(platform)}
        right={
          <span className={s.shelfCount}>
            {t("library.games", { count: list.length })}
          </span>
        }
      />

      <div className={mergeClasses(l.tools, "reemu-on-wallpaper")}>
        <SearchChip />
        {letters.length > 1 &&
          letters.map(({ letter, index }) => (
            <Button
              key={letter}
              className={l.letter}
              appearance="subtle"
              aria-label={t("library.jumpToLetter", { letter })}
              onClick={() => grid.current?.focusIndex(index)}
            >
              {letter}
            </Button>
          ))}
      </div>

      {roms.isLoading ? (
        <CardGridSkeleton />
      ) : list.length === 0 ? (
        <EmptyState art={<SearchArt />} title={t("home.emptyPlatform")}>
          {t("home.emptyPlatformHint")}
        </EmptyState>
      ) : (
        <VirtualGrid className={s.grid} items={list} render={card} handle={grid} />
      )}
    </div>
  );
}
