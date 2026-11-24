import {
  Button,
  Caption1,
  Field,
  Input,
  Select,
  Textarea,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { ArrowUploadRegular } from "@fluentui/react-icons";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { readGameArtworkFile, type GameMetadata, type GameMetadataEdits } from "../lib/tauri";
import { formatReleaseDate } from "../lib/metadataFormat";
import { inNavMode } from "../lib/focusNav";

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

export type GameArtworkKey = typeof artFields[number][0];
const artworkTypes = ["image/png", "image/jpeg", "image/webp", "image/gif"];
const maxArtworkBytes = 4 * 1024 * 1024;

export function isLocalArtwork(value: string) {
  return artworkTypes.some((type) => value.startsWith(`data:${type};base64,`));
}

async function readArtworkInput(file: File): Promise<string> {
  if (file.size === 0 || file.size > maxArtworkBytes || !artworkTypes.includes(file.type)) {
    throw new Error("invalid artwork file");
  }
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

export async function readGameArtwork(file: File | string): Promise<string> {
  const url = await (typeof file === "string" ? readGameArtworkFile(file) : readArtworkInput(file));
  const image = new Image();
  image.src = url;
  await image.decode();
  return url;
}

function ArtworkUpload({ label, disabled, onFile }: {
  label: string;
  disabled: boolean;
  onFile: (file?: File) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
      <Button appearance="secondary" icon={<ArrowUploadRegular />} disabled={disabled} onClick={() => {
        if (inNavMode()) onFile();
        else input.current?.click();
      }}>
        {label}
      </Button>
      <input
        ref={input}
        type="file"
        hidden
        accept={artworkTypes.join(",")}
        onChange={(event) => {
          const file = event.currentTarget.files?.[0];
          event.currentTarget.value = "";
          if (file) onFile(file);
        }}
      />
    </>
  );
}

export const gameMetadataLabels = [
  ["description", "game.description"],
  ...textFields,
  ...artFields,
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
    && artFields.every(([key]) => {
      const value = draft[key]?.trim() ?? "";
      return !value || isLocalArtwork(value) || !!imageUrl(value);
    });
}

const useStyles = makeStyles({
  fields: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 190px), 1fr))",
    gap: tokens.spacingHorizontalL,
  },
  uploadGrid: {
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    "@media (max-width: 1050px)": { gridTemplateColumns: "minmax(0, 1fr)" },
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
  draft, catalog, onChange, onUpload, disabled = false,
}: {
  draft: GameMetadataEdits;
  catalog: GameMetadata["gamekave"] | undefined;
  onChange: (key: keyof GameMetadataEdits, value: string) => void;
  onUpload?: (key: GameArtworkKey, file?: File) => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  const s = useStyles();
  return (
    <div className={mergeClasses(s.fields, !!onUpload && s.uploadGrid)}>
      {artFields.map(([key, label, source]) => {
        const value = draft[key] ?? "";
        const options = source ? catalog?.[source] ?? [] : [];
        const local = isLocalArtwork(value);
        const preview = local ? value : imageUrl(value);
        return (
          <div key={key} className={s.art}>
            {preview && <img key={preview} className={s.preview} src={preview} alt={t(label)} />}
            {options.length > 0 && (
              <Field label={t("game.catalog.chooseArt", { art: t(label) })}>
                <Select disabled={disabled} value={options.includes(value) ? value : ""} onChange={(_, data) => {
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
              <Input disabled={disabled} type="url" value={local ? "" : value} onChange={(_, data) => onChange(key, data.value)} />
            </Field>
            {onUpload && (
              <ArtworkUpload
                label={t("game.catalog.uploadArt", { art: t(label) })}
                disabled={disabled}
                onFile={(file) => onUpload(key, file)}
              />
            )}
            {local && (
              <>
                <Caption1>{t("game.catalog.localArt")}</Caption1>
                <Button appearance="subtle" disabled={disabled} onClick={() => onChange(key, "")}>
                  {t("game.catalog.removeLocalArt", { art: t(label) })}
                </Button>
              </>
            )}
          </div>
        );
      })}
    </div>
  );
}
