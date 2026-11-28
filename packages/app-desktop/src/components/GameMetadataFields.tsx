import {
  Button,
  Card,
  Caption1,
  Field,
  Input,
  Subtitle2,
  Textarea,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { ArrowUploadRegular, ImageRegular } from "@fluentui/react-icons";
import { useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { readGameArtworkFile, type GameMetadata, type GameMetadataEdits } from "../lib/tauri";
import { formatReleaseDate } from "../lib/metadataFormat";
import { inNavMode } from "../lib/focusNav";
import { SelectBlock } from "./SettingBlock";

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
  fieldsContainer: { containerType: "inline-size", containerName: "game-metadata-fields" },
  fields: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr)",
    gap: tokens.spacingHorizontalL,
    "@container game-metadata-fields (min-width: 390px)": {
      gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    },
    "@container game-metadata-fields (min-width: 840px)": {
      gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
    },
  },
  artGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, max(310px, 40%)), 1fr))",
    gap: tokens.spacingHorizontalL,
  },
  art: {
    minWidth: 0,
    gap: tokens.spacingVerticalM,
    padding: tokens.spacingHorizontalL,
    backgroundColor: "var(--reemuControlFill)",
    boxShadow: "none",
  },
  artTitle: { margin: 0 },
  artBody: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(min(100%, 180px), 1fr))",
    alignItems: "start",
    gap: tokens.spacingHorizontalM,
  },
  artControls: { display: "grid", gap: tokens.spacingVerticalM, minWidth: 0 },
  artActions: { display: "flex", flexWrap: "wrap", gap: tokens.spacingHorizontalS },
  preview: {
    position: "relative",
    display: "grid",
    placeContent: "center",
    justifyItems: "center",
    gap: tokens.spacingVerticalXS,
    height: "132px",
    overflow: "hidden",
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "var(--reemuSurface1)",
    color: tokens.colorNeutralForeground3,
  },
  previewImage: { position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "contain" },
  previewIcon: { fontSize: "24px" },
  facts: { display: "grid", gap: tokens.spacingVerticalS, margin: 0 },
  fact: {
    display: "grid",
    gridTemplateColumns: "minmax(110px, 34%) minmax(0, 1fr)",
    gap: tokens.spacingHorizontalM,
  },
  label: { color: tokens.colorNeutralForeground3 },
  value: { margin: 0, overflowWrap: "anywhere" },
});

function ArtworkPreview({ url, label }: { url: string | undefined; label: string }) {
  const { t } = useTranslation();
  const s = useStyles();
  const [failed, setFailed] = useState(false);
  return (
    <div className={s.preview}>
      {url && !failed ? (
        <img className={s.previewImage} src={url} alt={label} onError={() => setFailed(true)} />
      ) : (
        <>
          <ImageRegular className={s.previewIcon} aria-hidden />
          <Caption1>{t("game.catalog.noImage")}</Caption1>
        </>
      )}
    </div>
  );
}

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
      <div className={s.fieldsContainer}>
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
  const id = useId();
  return (
    <div className={s.artGrid}>
      {artFields.map(([key, label, source]) => {
        const value = draft[key] ?? "";
        const options = source ? catalog?.[source] ?? [] : [];
        const local = isLocalArtwork(value);
        const preview = local ? value : imageUrl(value);
        const invalid = !!value.trim() && !preview;
        const title = t(label);
        const titleId = `${id}-${key}`;
        return (
          <Card key={key} appearance="filled-alternative" className={s.art} aria-labelledby={titleId}>
            <Subtitle2 as="h3" id={titleId} className={s.artTitle}>{title}</Subtitle2>
            <div className={s.artBody}>
              <ArtworkPreview key={preview} url={preview} label={title} />
              <div className={s.artControls}>
                {options.length > 0 && (
                  <SelectBlock
                    compact
                    label={t("game.catalog.chooseArt", { art: title })}
                    placeholder={t("game.catalog.chooseImage")}
                    disabled={disabled}
                    value={value}
                    options={options.map((url, index) => ({
                      value: url,
                      label: t("game.catalog.imageNumber", { number: index + 1 }),
                    }))}
                    onChange={(url) => onChange(key, url)}
                  />
                )}
                <Field
                  label={t("game.catalog.artUrl", { art: title })}
                  validationState={invalid ? "error" : "none"}
                  validationMessage={invalid ? t("game.catalog.invalidUrl") : undefined}
                >
                  <Input
                    appearance="filled-darker"
                    disabled={disabled}
                    type="url"
                    value={local ? "" : value}
                    onChange={(_, data) => onChange(key, data.value)}
                  />
                </Field>
                {local && <Caption1>{t("game.catalog.localArt")}</Caption1>}
                {(onUpload || local) && (
                  <div className={s.artActions}>
                    {onUpload && (
                      <ArtworkUpload
                        label={t("game.catalog.uploadArt", { art: title })}
                        disabled={disabled}
                        onFile={(file) => onUpload(key, file)}
                      />
                    )}
                    {local && (
                      <Button appearance="secondary" disabled={disabled} onClick={() => onChange(key, "")}>
                        {t("game.catalog.removeLocalArt", { art: title })}
                      </Button>
                    )}
                  </div>
                )}
              </div>
            </div>
          </Card>
        );
      })}
    </div>
  );
}
