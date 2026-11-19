import {
  Field,
  Input,
  Select,
  Textarea,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { useTranslation } from "react-i18next";
import type { GameMetadata, GameMetadataEdits } from "../lib/tauri";
import { formatReleaseDate } from "../lib/metadataFormat";

const textFields = [
  ["releaseDate", "game.release"],
  ["genre", "game.genres"],
  ["developer", "game.developer"],
  ["publisher", "game.publisher"],
  ["players", "game.players"],
  ["rating", "game.catalog.rating"],
  ["ageRating", "game.ageRating"],
  ["modes", "game.modes"],
] as const;

const artFields = [
  ["coverUrl", "game.catalog.cover", "covers"],
  ["fanartUrl", "game.media.hero", "heroes"],
  ["screenshotUrl", "game.media.screenshot", "screenshots"],
  ["logoUrl", "game.media.logo", null],
] as const;

export function gameMetadataDraft(meta: GameMetadata | null | undefined): GameMetadataEdits {
  const draft: GameMetadataEdits = { description: meta?.description ?? "" };
  for (const [key] of [...textFields, ...artFields]) {
    draft[key] = String(meta?.[key] ?? "");
  }
  return draft;
}

function imageUrl(value: string) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.username && !url.password ? url.href : undefined;
  } catch {
    return undefined;
  }
}

export function validMetadataDraft(draft: GameMetadataEdits) {
  const rating = draft.rating?.trim() ?? "";
  return (!rating || (/^\d{1,3}$/.test(rating) && Number(rating) <= 100))
    && artFields.every(([key]) => !draft[key]?.trim() || imageUrl(draft[key]!));
}

const useStyles = makeStyles({
  fields: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 190px), 1fr))",
    gap: tokens.spacingHorizontalL,
  },
  art: { display: "flex", flexDirection: "column", gap: tokens.spacingVerticalS, minWidth: 0 },
  preview: {
    width: "100%",
    height: "112px",
    objectFit: "contain",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  facts: { display: "grid", gap: tokens.spacingVerticalS, margin: 0 },
  fact: {
    display: "grid",
    gridTemplateColumns: "minmax(110px, 34%) minmax(0, 1fr)",
    gap: tokens.spacingHorizontalM,
  },
  label: { color: tokens.colorNeutralForeground3 },
  value: { margin: 0, overflowWrap: "anywhere" },
});

export function GameCatalogFacts({ catalog }: { catalog: GameMetadata["gamekave"] | undefined }) {
  const { t, i18n } = useTranslation();
  const s = useStyles();
  if (!catalog) return null;
  const language = i18n.resolvedLanguage ?? i18n.language;
  const regions = new Intl.DisplayNames([language], { type: "region" });
  const dates = Object.entries(catalog.releaseDates).map(([region, date]) => {
    const label = /^[A-Z]{2}$/.test(region) ? regions.of(region) ?? region : region;
    return `${label} · ${formatReleaseDate(date) ?? date}`;
  });
  const durations = [
    t("game.catalog.fast"),
    t("game.catalog.normal"),
    t("game.catalog.complete"),
  ];
  const number = new Intl.NumberFormat(language);
  const times = catalog.timeToBeatMinutes.flatMap((minutes, index) => {
    if (minutes == null) return [];
    const hours = Math.floor(minutes / 60);
    const rest = minutes % 60;
    const duration = hours ? `${number.format(hours)} h ${number.format(rest)} min` : `${number.format(rest)} min`;
    return [`${durations[index]} · ${duration}`];
  });
  const rows = [
    [t("game.catalog.regionalDates"), dates],
    [t("game.catalog.timeToBeat"), times],
    [t("game.tabs.achievements"), catalog.achievementCount ? [number.format(catalog.achievementCount)] : []],
    [t("game.catalog.soundtrack"), catalog.soundtrackCount ? [number.format(catalog.soundtrackCount)] : []],
  ] as const;
  if (!rows.some(([, values]) => values.length)) return null;
  return (
    <dl className={s.facts}>
      {rows.map(([label, values]) => values.length > 0 && (
        <div key={label} className={s.fact}>
          <dt className={s.label}>{label}</dt>
          <dd className={s.value}>{values.map((value) => <div key={value}>{value}</div>)}</dd>
        </div>
      ))}
    </dl>
  );
}

export function GameMetadataFields({
  draft, catalog, onChange,
}: {
  draft: GameMetadataEdits;
  catalog: GameMetadata["gamekave"] | undefined;
  onChange: (key: keyof GameMetadataEdits, value: string) => void;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  return (
    <>
      <Field label={t("game.description")}>
        <Textarea rows={4} value={draft.description ?? ""} onChange={(_, data) => onChange("description", data.value)} />
      </Field>
      <div className={s.fields}>
        {textFields.map(([key, label]) => (
          <Field key={key} label={t(label)}>
            <Input
              value={draft[key] ?? ""}
              inputMode={key === "rating" ? "numeric" : undefined}
              onChange={(_, data) => onChange(key, data.value)}
            />
          </Field>
        ))}
      </div>
      <GameCatalogFacts catalog={catalog} />
    </>
  );
}

export function GameArtworkFields({
  draft, catalog, onChange,
}: {
  draft: GameMetadataEdits;
  catalog: GameMetadata["gamekave"] | undefined;
  onChange: (key: keyof GameMetadataEdits, value: string) => void;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  return (
    <div className={s.fields}>
      {artFields.map(([key, label, source]) => {
        const value = draft[key] ?? "";
        const options = source ? catalog?.[source] ?? [] : [];
        const preview = imageUrl(value);
        return (
          <div key={key} className={s.art}>
            {preview && <img key={preview} className={s.preview} src={preview} alt={t(label)} />}
            {options.length > 0 && (
              <Field label={t("game.catalog.chooseArt", { art: t(label) })}>
                <Select value={options.includes(value) ? value : ""} onChange={(_, data) => {
                  if (data.value) onChange(key, data.value);
                }}>
                  <option value="" disabled>{t("game.catalog.chooseImage")}</option>
                  {options.map((url, index) => (
                    <option key={url} value={url}>
                      {t("game.catalog.imageNumber", { number: index + 1 })}
                    </option>
                  ))}
                </Select>
              </Field>
            )}
            <Field
              label={t("game.catalog.artUrl", { art: t(label) })}
              validationState={value.trim() && !preview ? "error" : "none"}
              validationMessage={value.trim() && !preview ? t("game.catalog.invalidUrl") : undefined}
            >
              <Input type="url" value={value} onChange={(_, data) => onChange(key, data.value)} />
            </Field>
          </div>
        );
      })}
    </div>
  );
}
