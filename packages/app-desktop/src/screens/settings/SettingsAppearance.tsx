import {
  Button,
  Caption1,
  Card,
  ColorPicker,
  ColorSlider,
  Switch,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import {
  CheckmarkCircleFilled,
  ColorRegular,
  ImageAddRegular,
  ImageRegular,
  WeatherMoonFilled,
  WeatherSunnyFilled,
} from "@fluentui/react-icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  clearWallpaper,
  pickImage,
  setWallpaperFile,
  wallpaperUrl,
} from "../../lib/tauri";
import { REEMU_ACTION } from "../../hooks/useMenuNav";
import { errorToast } from "../../lib/toast";
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
import { SettingsHubCards } from "./SettingsHome";

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    // Sem teto de largura: a grade de temas ocupa a largura útil do card
    // das Configurações (o card do papel de parede tem o próprio limite).
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    minWidth: 0,
  },
  wallCard: {
    gap: tokens.spacingVerticalM,
    maxWidth: "480px",
  },
  grid: {
    display: "grid",
    // Até 4 colunas que esticam pra encher a largura do card (pedido do
    // usuário). O mínimo de cada coluna é o maior entre 200 px e 1/4 da
    // linha: com espaço, sempre 4; quando 1/4 ficaria menor que 200 (tela
    // estreita / interface Maior), o `auto-fill` cai pra 3, 2 ou 1 — sem
    // card espremido. 12 px = `spacingHorizontalM`, o `gap` abaixo.
    gridTemplateColumns:
      "repeat(auto-fill, minmax(max(200px, calc((100% - 3 * 12px) / 4)), 1fr))",
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
    // Sem a sombra do <Card> do Fluent (`shadow4`, e a do hover) — pedido do
    // usuário; a separação fica com a borda e o fundo do próprio tema.
    boxShadow: "none !important",
    // Anel de foco padrão do app (o mesmo dos cards de jogo e da rail:
    // 3 px na cor de destaque, afastado 3 px, com o brilho) no lugar do anel do <Card> do
    // Fluent, que desenha o próprio via `::after` com borda. Num card
    // selecionável o foco fica no checkbox escondido do Fluent
    // (`.fui-Card__checkbox`), então o anel vai no card quando ELE tem foco
    // (o Switch "Claro" de dentro tem o anel dele).
    "&[data-fui-focus-visible]::after, &[data-fui-focus-within]::after": {
      border: "none !important",
    },
    "&:has(> .fui-Card__checkbox:focus)": {
      outlineWidth: "3px",
      outlineStyle: "solid",
      outlineColor: tokens.colorBrandStroke1,
      outlineOffset: "3px",
      // o brilho vem do `::before` global (index.css, regra 4), por fora do
      // anel; o card precisa deixar o `::before` aparecer fora dele
      overflow: "visible !important",
    },
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
  // Selo do tema ativo: check cheio de 28 no canto de cima da amostra, num
  // círculo da cor do card (destaca sobre qualquer cor da amostra).
  check: {
    position: "absolute",
    top: tokens.spacingVerticalS,
    right: tokens.spacingHorizontalS,
    width: "32px",
    height: "32px",
    borderRadius: tokens.borderRadiusCircular,
    display: "grid",
    placeItems: "center",
    fontSize: "28px",
    lineHeight: 0,
    pointerEvents: "none",
  },
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
  /** Ausente = tema de um modo só: sem Switch (o rodapé mantém a altura). */
  onModeChange?: (m: ThemeMode) => void;
}) {
  const { t: tr } = useTranslation();
  const s = useStyles();
  // X do controle (`REEMU_ACTION`) com o card focado: troca claro/escuro.
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || !onModeChange) return;
    const toggle = () => onModeChange(mode === "light" ? "dark" : "light");
    el.addEventListener(REEMU_ACTION, toggle);
    return () => el.removeEventListener(REEMU_ACTION, toggle);
  }, [mode, onModeChange]);
  return (
    <Card
      ref={ref}
      className={s.card}
      selected={selected}
      onSelectionChange={onSelect}
      aria-label={tr("appearance.theme.card", { name: label })}
      style={{
        backgroundColor: t.colorNeutralBackground2,
        // a mesma borda em todos: o ativo é marcado só pelo selo de check
        // (pedido do usuário), não pela borda na cor do tema
        borderColor: t.colorNeutralStroke2,
      }}
    >
      <Swatch t={t} />
      {selected && (
        <span
          className={s.check}
          style={{ backgroundColor: t.colorNeutralBackground2, color: t.reemuBrandText }}
          aria-hidden
        >
          <CheckmarkCircleFilled />
        </span>
      )}
      <div className={s.footer}>
        <Text
          className={s.name}
          weight={selected ? "semibold" : "regular"}
          style={{ color: t.colorNeutralForeground1 }}
        >
          {label}
        </Text>
        {onModeChange ? (
          // o clique/tecla no Switch não pode chegar no Card — ele
          // re-escolheria a versão que estava na tela. `data-nav-skip`: o
          // controle não para no Switch (pedido do usuário) — com ele, claro/
          // escuro troca pelo atalho X; mouse e Tab do teclado continuam.
          <span
            data-nav-skip
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

/** Configurações › Aparência — tema de cor e papel de parede da tela
 *  inicial, em abas. Idioma e tamanho da interface ficam em Sistema. */
export function SettingsAppearance({
  section,
}: {
  /** `undefined` = entrada da Aparência (cards das opções). */
  section?: "temas" | "papel-de-parede";
}) {
  const { t } = useTranslation();
  const s = useStyles();
  const tab = section === "papel-de-parede" ? "wallpaper" : "theme";
  const {
    selection,
    customDraft,
    setPreset,
    activateCustom,
    setCustomHue,
    setCustomMode,
  } = useThemeStore();
  const isCustom = selection.kind === "custom";
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
    onError: (e) => push(errorToast(e, "loadImage")),
  });
  const remove = useMutation({
    mutationFn: clearWallpaper,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["wallpaper"] }),
    onError: (e) => push(errorToast(e, "removeWallpaper")),
  });

  if (!section)
    return (
      <SettingsHubCards
        label={t("settings.tabs.appearance")}
        items={[
          {
            to: "temas",
            icon: <ColorRegular />,
            title: t("appearance.theme.title"),
            description: t("appearance.theme.cardDesc"),
          },
          {
            to: "papel-de-parede",
            icon: <ImageRegular />,
            title: t("appearance.wallpaper.title"),
            description: t("appearance.wallpaper.description"),
          },
        ]}
      />
    );

  return (
    <div className={s.root}>

      {tab === "theme" && (
        <div className={s.section}>
          <div className={s.grid} aria-label={t("appearance.theme.title")}>
            {THEME_FAMILIES.map((f) => {
              const on = active?.family === f;
              const mode: ThemeMode = on
                ? active.mode
                : f.light
                  ? previewMode(f.dark, "dark")
                  : f.lightOnly
                    ? "light"
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
      )}

      {tab === "wallpaper" && (
        <Card className={s.wallCard} appearance="filled-alternative">
          <Caption1 as="p" block style={{ margin: 0 }}>
              {t("appearance.wallpaper.description")}
            </Caption1>

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
      )}
    </div>
  );
}
