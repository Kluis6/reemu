import {
  Button,
  Caption1,
  Card,
  ColorPicker,
  ColorSlider,
  Radio,
  RadioGroup,
  Switch,
  Text,
  makeStyles,
  tokens,
  type RadioGroupOnChangeData,
} from "@fluentui/react-components";
import {
  CheckmarkFilled,
  ImageAddRegular,
  WeatherMoonFilled,
  WeatherSunnyFilled,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getLanguagePreference,
  LANGUAGES,
  setLanguagePreference,
  type LanguagePreference,
} from "../../i18n";
import {
  clearWallpaper,
  pickImage,
  setWallpaperFile,
  wallpaperUrl,
} from "../../lib/tauri";
import { errorToast } from "../../lib/toast";
import { UI_SCALES, getUiScale, setUiScale } from "../../lib/uiScale";
import { useToastStore } from "../../stores/useToastStore";
import { useThemeStore } from "../../stores/useThemeStore";
import {
  familyOf,
  resolveTheme,
  THEME_FAMILIES,
  THEMES,
  type ReEmuTheme,
  type ThemeMode,
} from "../../styles/themes";

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    maxWidth: "1120px",
  },
  // temas à esquerda, papel de parede numa coluna à direita; em janela
  // estreita a coluna desce pra baixo dos temas
  // quebra pela largura DISPONÍVEL (flex-wrap), não pela da janela: a
  // coluna do papel de parede desce quando os temas não cabem em 2 colunas
  columns: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "flex-start",
    gap: tokens.spacingHorizontalXXL,
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    flex: "1 1 520px",
    minWidth: 0,
  },
  wallCard: {
    gap: tokens.spacingVerticalM,
    flex: "0 1 300px",
    minWidth: "240px",
  },
  grid: {
    display: "grid",
    // sempre duas colunas (uma só em janela bem estreita)
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    "@media (max-width: 640px)": { gridTemplateColumns: "minmax(0, 1fr)" },
    gap: tokens.spacingHorizontalM,
  },
  // Cor de fundo/borda/texto vêm inline do PRÓPRIO tema sendo mostrado (`t`),
  // não do tema ativo — senão o card do tema "Claro" ficaria escuro sempre
  // que o usuário já estivesse num tema escuro (e vice-versa), e a prévia
  // não serviria pra nada.
  // `Card` da Fluent; borda de seleção por cima da dela
  card: {
    position: "relative",
    gap: tokens.spacingVerticalS,
    padding: tokens.spacingHorizontalM,
    border: "2px solid transparent",
  },
  // nome à esquerda, Switch "Claro" à direita; altura fixa pra alinhar os
  // cards que não têm o Switch
  footer: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    flexWrap: "wrap",
    columnGap: tokens.spacingHorizontalS,
    minHeight: "32px",
  },
  swatch: {
    height: "56px",
    borderRadius: tokens.borderRadiusMedium,
    display: "flex",
    overflow: "hidden",
  },
  seg: { flexGrow: 1 },
  // bolinha do Switch (18 px, o tamanho do círculo original da Fluent)
  // com o ícone dentro
  thumb: {
    display: "inline-grid",
    placeItems: "center",
    width: "1em",
    height: "1em",
    padding: "2px",
    boxSizing: "border-box",
    borderRadius: "50%",
    backgroundClip: "content-box",
    fontSize: "inherit",
    "& > svg": { fontSize: "10px" },
  },
  name: {
    display: "inline-flex",
    alignItems: "center",
    gap: tokens.spacingHorizontalXS,
  },
  wallPreview: {
    width: "100%",
    aspectRatio: "16 / 9",
    borderRadius: tokens.borderRadiusLarge,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground3,
    backgroundSize: "cover",
    backgroundPosition: "center",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    color: tokens.colorNeutralForeground3,
    fontSize: "28px",
  },
  wallActions: {
    display: "flex",
    flexWrap: "wrap",
    gap: tokens.spacingHorizontalS,
  },
  customPanel: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalM,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  hueSlider: { width: "100%" },
});

/** Amostra do tema: as 4 cores do fundo (na ordem dos cantos, ver
 *  `BgPalette`) com a cor de marca no meio. */
function Swatch({ t }: { t: ReEmuTheme }) {
  const s = useStyles();
  return (
    <div className={s.swatch}>
      {[t.reemuBg1, t.reemuBg4, t.reemuBrandSolid, t.reemuBg2, t.reemuBg3].map(
        (c, i) => (
          <span key={i} className={s.seg} style={{ background: c }} />
        ),
      )}
    </div>
  );
}

/**
 * Um tema da grade. O `Card` (Fluent 2) escolhe o tema; o `Switch` "Claro"
 * troca entre as versões escura e clara (e já aplica). As cores do card vêm
 * do PRÓPRIO tema mostrado (`t`), não do ativo — senão a prévia do claro
 * ficaria escura quando o app está no escuro.
 */
function ThemeCard({
  t,
  label,
  selected,
  onSelect,
  mode,
  onModeChange,
}: {
  t: ReEmuTheme;
  label: string;
  selected: boolean;
  onSelect: () => void;
  mode: ThemeMode;
  /** Ausente = tema só escuro: sem Switch (o rodapé mantém a altura). */
  onModeChange?: (m: ThemeMode) => void;
}) {
  const { t: tr } = useTranslation();
  const s = useStyles();
  return (
    <Card
      className={s.card}
      selected={selected}
      onSelectionChange={onSelect}
      aria-label={tr("appearance.theme.card", { name: label })}
      style={{
        backgroundColor: t.colorNeutralBackground2,
        borderColor: selected ? t.colorBrandStroke1 : t.colorNeutralStroke2,
      }}
    >
      <Swatch t={t} />
      <div className={s.footer}>
        <Text
          className={s.name}
          weight={selected ? "semibold" : "regular"}
          style={{ color: t.colorNeutralForeground1 }}
        >
          {selected && (
            <CheckmarkFilled
              style={{ color: t.reemuBrandText, flexShrink: 0 }}
            />
          )}
          {label}
        </Text>
        {onModeChange ? (
          // o clique/tecla no Switch não pode chegar no Card — ele
          // re-escolheria a versão que estava na tela
          <span
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <Switch
              aria-label={tr("appearance.theme.lightMode", { name: label })}
              // bolinha normal do Switch (o filho do `indicator` é o que
              // desliza), com a lua/o sol pequeno dentro dela
              indicator={{
                children: (
                  <span
                    className={s.thumb}
                    style={{
                      // círculo: branco no trilho de marca / cinza do card
                      // desligado; ícone na cor do trilho por baixo
                      backgroundColor:
                        mode === "light"
                          ? t.colorNeutralForegroundOnBrand
                          : t.colorNeutralForeground2,
                      color:
                        mode === "light"
                          ? t.colorBrandBackground
                          : t.colorNeutralBackground2,
                    }}
                  >
                    {mode === "light" ? (
                      <WeatherSunnyFilled />
                    ) : (
                      <WeatherMoonFilled />
                    )}
                  </span>
                ),
                style:
                  mode === "light"
                    ? undefined
                    : {
                        color: t.colorNeutralForeground2,
                        borderColor: t.colorNeutralStrokeAccessible,
                      },
              }}
              checked={mode === "light"}
              onChange={(_, d) => onModeChange(d.checked ? "light" : "dark")}
            />
          </span>
        ) : null}
      </div>
    </Card>
  );
}

/** Configurações › Aparência — tema de cor + papel de parede da tela inicial. */
export function SettingsAppearance() {
  const { t } = useTranslation();
  const [langPref, setLangPref] = useState<LanguagePreference>(
    getLanguagePreference,
  );
  const s = useStyles();
  const {
    selection,
    customDraft,
    setPreset,
    activateCustom,
    setCustomHue,
    setCustomMode,
  } = useThemeStore();
  const isCustom = selection.kind === "custom";
  const [uiScale, setUiScaleState] = useState(getUiScale);
  // Cada card guarda o próprio modo. No card selecionado o Switch aplica na
  // hora; nos outros só troca a prévia daquele card — clicar no card aplica
  // o tema no modo que está na tela. Um card nunca muda por causa de outro.
  const active = selection.kind === "preset" ? familyOf(selection.id) : null;
  const [previewModes, setPreviewModes] = useState<Record<string, ThemeMode>>(
    {},
  );
  const previewMode = (key: string, fallback: ThemeMode) =>
    previewModes[key] ?? fallback;
  const setPreviewMode = (key: string, m: ThemeMode) =>
    setPreviewModes((p) => ({ ...p, [key]: m }));
  const customMode = isCustom
    ? customDraft.mode
    : previewMode("custom", customDraft.mode);
  const customPreview = resolveTheme({
    kind: "custom",
    hue: customDraft.hue,
    mode: customMode,
  });
  const qc = useQueryClient();
  const push = useToastStore((t) => t.push);
  const wallpaper = useQuery({
    queryKey: ["wallpaper"],
    queryFn: wallpaperUrl,
  });

  const upload = useMutation({
    mutationFn: async () => {
      const path = await pickImage(t("appearance.wallpaper.pickTitle"));
      if (!path) return false;
      await setWallpaperFile(path);
      return true;
    },
    onSuccess: (ok) => {
      if (!ok) return;
      qc.invalidateQueries({ queryKey: ["wallpaper"] });
    },
    onError: (e) => push(errorToast(e, "carregar a imagem")),
  });
  const remove = useMutation({
    mutationFn: clearWallpaper,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["wallpaper"] }),
    onError: (e) => push(errorToast(e, "remover o papel de parede")),
  });

  return (
    <div className={s.root}>
      <div>
        <Text as="strong" weight="semibold">
          {t("language.title")}
        </Text>
        <Caption1 as="p" block style={{ margin: "2px 0 0" }}>
          {t("language.description")}
        </Caption1>
      </div>
      <RadioGroup
        layout="horizontal"
        aria-label={t("language.title")}
        value={langPref}
        onChange={(_, data: RadioGroupOnChangeData) => {
          const v = data.value as LanguagePreference;
          setLangPref(v);
          void setLanguagePreference(v);
        }}
      >
        <Radio value="auto" label={t("language.auto")} />
        {LANGUAGES.map((l) => (
          // cada idioma no próprio nome (quem não lê o atual acha o seu)
          <Radio key={l} value={l} label={t(`language.${l}`)} />
        ))}
      </RadioGroup>

      <div>
        <Text as="strong" weight="semibold">
          {t("appearance.uiScale.title")}
        </Text>
        <Caption1 as="p" block style={{ margin: "2px 0 0" }}>
          {t("appearance.uiScale.description")}
        </Caption1>
      </div>
      <RadioGroup
        layout="horizontal"
        aria-label={t("appearance.uiScale.title")}
        value={String(uiScale)}
        onChange={(_, data: RadioGroupOnChangeData) => {
          const v = Number(data.value);
          setUiScaleState(v);
          setUiScale(v).catch((e) =>
            push(errorToast(e, "mudar o tamanho da interface")),
          );
        }}
      >
        {UI_SCALES.map((o) => (
          <Radio
            key={o.value}
            value={String(o.value)}
            label={`${t(o.label)} (${Math.round(o.value * 100)}%)`}
          />
        ))}
      </RadioGroup>

      <div className={s.columns}>
        <div className={s.section}>
          <div>
            <Text as="strong" weight="semibold">
              {t("appearance.theme.title")}
            </Text>
            <Caption1 as="p" block style={{ margin: "2px 0 0" }}>
              {t("appearance.theme.description")}
            </Caption1>
          </div>

          <div className={s.grid} aria-label={t("appearance.theme.title")}>
            {THEME_FAMILIES.map((f) => {
              const on = active?.family === f;
              const mode: ThemeMode = on
                ? active.mode
                : f.light
                  ? previewMode(f.dark, "dark")
                  : "dark";
              const id = mode === "light" && f.light ? f.light : f.dark;
              const light = f.light;
              const changeMode = (m: ThemeMode) => {
                setPreviewMode(f.dark, m);
                if (on && light) setPreset(m === "light" ? light : f.dark);
              };
              return (
                <ThemeCard
                  key={f.dark}
                  t={THEMES[id].theme}
                  label={t(`appearance.theme.names.${f.nameKey}`)}
                  selected={on}
                  onSelect={() => setPreset(id)}
                  mode={mode}
                  onModeChange={light ? changeMode : undefined}
                />
              );
            })}
            {/* "Personalizado": como o fundo do Xbox Series S/X — escuro/claro
            + um matiz (painel abaixo); o resto da rampa é gerado. */}
            <ThemeCard
              t={customPreview}
              label={t("appearance.theme.names.custom")}
              selected={isCustom}
              onSelect={() =>
                customMode === customDraft.mode
                  ? activateCustom()
                  : setCustomMode(customMode)
              }
              mode={customMode}
              onModeChange={(m) => {
                setPreviewMode("custom", m);
                if (isCustom) setCustomMode(m);
              }}
            />
          </div>

          {isCustom && (
            <div className={s.customPanel}>
              <ColorPicker
                color={{ h: customDraft.hue, s: 1, v: 1 }}
                onColorChange={(_, data) => setCustomHue(data.color.h)}
              >
                <ColorSlider
                  className={s.hueSlider}
                  aria-label={t("appearance.theme.hue")}
                />
              </ColorPicker>
            </div>
          )}
        </div>

        <Card className={s.wallCard} appearance="filled-alternative">
          <div>
            <Text as="strong" weight="semibold">
              {t("appearance.wallpaper.title")}
            </Text>
            <Caption1 as="p" block style={{ margin: "2px 0 0" }}>
              {t("appearance.wallpaper.description")}
            </Caption1>
          </div>

          <div
            className={s.wallPreview}
            style={
              wallpaper.data
                ? { backgroundImage: `url(${wallpaper.data})` }
                : undefined
            }
          >
            {!wallpaper.data && <ImageAddRegular />}
          </div>
          <div className={s.wallActions}>
            <Button
              appearance="secondary"
              disabled={upload.isPending}
              onClick={() => upload.mutate()}
            >
              {wallpaper.data
                ? t("appearance.wallpaper.change")
                : t("appearance.wallpaper.choose")}
            </Button>
            {wallpaper.data && (
              <Button
                appearance="subtle"
                disabled={remove.isPending}
                onClick={() => remove.mutate()}
              >
                {t("appearance.wallpaper.remove")}
              </Button>
            )}
          </div>
        </Card>
      </div>
    </div>
  );
}
