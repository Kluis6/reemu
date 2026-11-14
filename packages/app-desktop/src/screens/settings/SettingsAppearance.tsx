import {
  Button,
  Caption1,
  Card,
  ColorPicker,
  ColorSlider,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Switch,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { BlockGroup, SelectBlock } from "../../components/SettingBlock";
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
  setWallpaperPreset,
  wallpaperPresetId,
  wallpaperUrl,
} from "../../lib/tauri";
import { useWallpaperStore, type WallpaperScope } from "../../stores/useWallpaperStore";
import { REEMU_ACTION } from "../../hooks/useMenuNav";
import { errorToast } from "../../lib/toast";
import { WALLPAPER_IDS, wallpaperDataUri, wallpaperSvg, type WallpaperId } from "../../lib/wallpapers";
import { DIALOG_FADE_ONLY } from "../../lib/motion";
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
  // Conteúdo do plano de fundo direto no card geral das Configurações (sem
  // um card interno — pedido do usuário).
  wallCard: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalM,
    maxWidth: "720px",
  },
  // Papéis de parede embutidos (pedido do usuário, 2026-11-14): miniaturas
  // 16:9 em grade, o nome embaixo e o check no escolhido, como os temas.
  presetGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(128px, 1fr))",
    gap: tokens.spacingHorizontalM,
  },
  preset: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalXS,
    padding: 0,
    border: "none",
    background: "none",
    color: tokens.colorNeutralForeground1,
    cursor: "pointer",
    textAlign: "left",
    borderRadius: tokens.borderRadiusMedium,
    fontFamily: "inherit",
  },
  presetImg: {
    display: "block",
    width: "100%",
    aspectRatio: "16 / 9",
    objectFit: "cover",
    borderRadius: tokens.borderRadiusMedium,
  },
  presetName: {
    display: "inline-flex",
    alignItems: "center",
    gap: tokens.spacingHorizontalXS,
    fontSize: tokens.fontSizeBase200,
    lineHeight: tokens.lineHeightBase200,
  },
  presetCheck: { fontSize: "16px", color: tokens.colorBrandForeground1 },
  // Temas na altura útil do card das Configurações (pedido do usuário,
  // 2026-11-14): a raiz e a seção esticam, e as linhas da grade dividem a
  // altura (mínimo de 150 px por card; abaixo disso a página rola).
  rootFill: { height: "100%" },
  sectionFill: { flexGrow: 1, minHeight: 0 },
  gridFill: { flexGrow: 1, gridAutoRows: "minmax(150px, 1fr)" },
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
    // A mesma borda em todos os cards, do tema ATIVO (pedido do usuário,
    // 2026-11-14): com a do próprio tema, os escuros tinham borda escura e os
    // claros, clara. O fundo continua o do tema mostrado (a prévia).
    border: `2px solid ${tokens.colorNeutralStroke2}`,
    // Sem a sombra do <Card> do Fluent (`shadow4`, e a do hover) — pedido do
    // usuário; a separação fica com a borda e o fundo do próprio tema.
    boxShadow: "none !important",
    // Anel de foco padrão do app (o mesmo dos cards de jogo e da rail:
    // 3 px na cor de destaque, afastado 3 px, com o brilho) no lugar do anel do <Card> do
    // Fluent, que desenha o próprio via `::after` com borda. Num card
    // selecionável o foco fica no checkbox escondido do Fluent
    // (`.fui-Card__checkbox`), então o anel vai no card quando ELE tem foco
    // (o Switch "Claro" de dentro tem o anel dele).
    // Sem o traço do `::after` do Fluent em nenhum estado: no card
    // selecionado ele punha uma borda de 1 px por cima (pedido do usuário: a
    // borda do tema ativo é a mesma dos outros); o foco é o anel abaixo.
    "::after": { border: "none !important" },
    // E no foco marcado pelo Fluent (teclado/controle no app): a regra 1 do
    // index.css (`:not(.fui-Tab)[data-fui-focus-within]::after`, anel de 3 px
    // a −6 px) tem especificidade maior que o `::after` acima e ganhava. Como
    // o `::after` é posicionado dentro da borda de 2 px do card, esse anel
    // caía 2 px mais pra dentro que o `outline` e os dois somavam: 5 px de
    // anel e 1 px de vão no controle, 3 + 3 no mouse (print do usuário).
    // Mesma especificidade da regra 1, injetada depois: ganha.
    "&[data-fui-focus-visible]::after, &[data-fui-focus-within]::after": {
      border: "none !important",
      boxShadow: "none !important",
    },
    // Igual ao card de jogo (pedido do usuário): anel e brilho no foco E no
    // hover. O fundo NÃO muda (pedido do usuário, 2026-11-14): é a prévia do
    // tema, e a camada cinza que clareava/escurecia o card mudava as cores.
    "&:hover, &:has(> .fui-Card__checkbox:focus)": {
      outlineWidth: "3px",
      outlineStyle: "solid",
      outlineColor: tokens.colorBrandStroke1,
      outlineOffset: "3px",
      // o brilho vem do `::before` (o do index.css, regra 4, no foco; o de
      // baixo no hover), por fora do anel; o card deixa ele aparecer
      overflow: "visible !important",
      // A borda de 2 px do card (cor do tema, inline) ficava logo dentro do
      // anel e somava com ele — parecia um anel mais grosso que o padrão. No
      // foco/hover ela some, como no card de jogo (que não tem borda).
      // (as 4 propriedades: o Griffel não aceita o atalho `borderColor`)
      borderTopColor: "transparent !important",
      borderRightColor: "transparent !important",
      borderBottomColor: "transparent !important",
      borderLeftColor: "transparent !important",
    },
    "&:hover::before": {
      content: '""',
      position: "absolute",
      inset: "-4px",
      borderRadius: `calc(${tokens.borderRadiusMedium} + 4px)`,
      boxShadow: `0 0 12px color-mix(in srgb, ${tokens.colorBrandStroke1} 45%, transparent)`,
      pointerEvents: "none",
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
  // Cor primária em destaque numa linha inteira e as outras quatro numa
  // linha abaixo (pedido do usuário, 2026-11-14); a altura total segue 56 px.
  // Cresce com o card (a grade estica até a altura útil): as duas linhas
  // ficam sempre com a mesma altura (pedido do usuário, 2026-11-14).
  swatch: {
    flexGrow: 1,
    minHeight: "56px",
    borderRadius: tokens.borderRadiusMedium,
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
  },
  primary: { flexGrow: 1, flexBasis: 0 },
  others: { flexGrow: 1, flexBasis: 0, display: "flex" },
  seg: { flexGrow: 1 },
  // check do tema ativo, depois do nome
  check: { fontSize: "20px", flexShrink: 0 },
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
    // a prévia fica no tamanho de antes; a grade dos embutidos usa os 720
    maxWidth: "480px",
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
  // Diálogo do tema personalizado (pedido do usuário, 2026-11-14): abre ao
  // escolher o card "Personalizado", com a prévia e a barra de matiz.
  customSurface: { maxWidth: "560px" },
  // no diálogo a faixa é maior: duas linhas de 60 px (pedido do usuário)
  customSwatch: { minHeight: "120px" },
  // botão no tamanho grande do Fluent (40 px) e largo, como os dos diálogos
  // do dashboard do Xbox Series X (controles de ~50 px na largura de 2000;
  // doc 18, "Escala igual à do app Xbox")
  customAction: { minWidth: "160px" },
  customContent: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
  },
  hueSlider: { width: "100%" },
});

/** Amostra do tema: as 4 cores do fundo (na ordem dos cantos, ver
 *  `BgPalette`) com a cor de marca no meio. */
function Swatch({ t, className }: { t: ReEmuTheme; className?: string }) {
  const s = useStyles();
  return (
    <div className={mergeClasses(s.swatch, className)}>
      <span className={s.primary} style={{ background: t.reemuBrandSolid }} />
      <span className={s.others}>
        {[t.reemuBg1, t.reemuBg4, t.reemuBg2, t.reemuBg3].map((c, i) => (
          <span key={i} className={s.seg} style={{ background: c }} />
        ))}
      </span>
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
      data-nav-action={onModeChange ? "" : undefined}
      className={s.card}
      selected={selected}
      onSelectionChange={onSelect}
      aria-label={tr("appearance.theme.card", { name: label })}
      style={{
        backgroundColor: t.colorNeutralBackground2,
      }}
    >
      <Swatch t={t} />
      <div className={s.footer}>
        <Text
          className={s.name}
          weight={selected ? "semibold" : "regular"}
          style={{ color: t.colorNeutralForeground1 }}
        >
          {label}
          {/* tema ativo: check cheio depois do nome (pedido do usuário), na
              cor de destaque do próprio tema */}
          {selected && (
            <CheckmarkCircleFilled
              className={s.check}
              style={{ color: t.reemuBrandText }}
              aria-hidden
            />
          )}
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
  const wallScope = useWallpaperStore((st) => st.scope);
  const setWallScope = useWallpaperStore((st) => st.setScope);
  const {
    selection,
    customDraft,
    setPreset,
    activateCustom,
    setCustomHue,
    setCustomMode,
  } = useThemeStore();
  const isCustom = selection.kind === "custom";
  // diálogo da cor do tema personalizado
  const [customOpen, setCustomOpen] = useState(false);
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
      qc.invalidateQueries({ queryKey: ["wallpaper-preset"] });
    },
    onError: (e) => push(errorToast(e, "loadImage")),
  });
  const wallPreset = useQuery({
    queryKey: ["wallpaper-preset"],
    queryFn: wallpaperPresetId,
  });
  const applyPreset = useMutation({
    mutationFn: (id: WallpaperId) => setWallpaperPreset(wallpaperSvg(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["wallpaper"] });
      qc.invalidateQueries({ queryKey: ["wallpaper-preset"] });
    },
    onError: (e) => push(errorToast(e, "loadImage")),
  });
  const remove = useMutation({
    mutationFn: clearWallpaper,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["wallpaper"] });
      qc.invalidateQueries({ queryKey: ["wallpaper-preset"] });
    },
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
    <div className={mergeClasses(s.root, tab === "theme" && s.rootFill)}>

      {tab === "theme" && (
        <div className={mergeClasses(s.section, s.sectionFill)}>
          <div
            className={mergeClasses(s.grid, s.gridFill)}
            aria-label={t("appearance.theme.title")}
          >
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
              onSelect={() => {
                if (customMode === customDraft.mode) activateCustom();
                else setCustomMode(customMode);
                setCustomOpen(true);
              }}
              mode={customMode}
              onModeChange={(m) => {
                setPreviewMode("custom", m);
                if (isCustom) setCustomMode(m);
              }}
            />
          </div>

          <Dialog
            open={customOpen}
            onOpenChange={(_, d) => setCustomOpen(d.open)}
            surfaceMotion={DIALOG_FADE_ONLY}
          >
            <DialogSurface className={s.customSurface}>
              <DialogBody>
                <DialogTitle>{t("appearance.theme.customTitle")}</DialogTitle>
                <DialogContent className={s.customContent}>
                  <Caption1>{t("appearance.theme.customHint")}</Caption1>
                  {/* prévia com a cor escolhida, como a faixa dos cards */}
                  <Swatch t={customPreview} className={s.customSwatch} />
                  <ColorPicker
                    color={{ h: customDraft.hue, s: 1, v: 1 }}
                    onColorChange={(_, data) => setCustomHue(data.color.h)}
                  >
                    <ColorSlider
                      className={s.hueSlider}
                      aria-label={t("appearance.theme.hue")}
                    />
                  </ColorPicker>
                </DialogContent>
                <DialogActions>
                  <Button
                    appearance="primary"
                    size="large"
                    className={s.customAction}
                    onClick={() => setCustomOpen(false)}
                  >
                    {t("common.close")}
                  </Button>
                </DialogActions>
              </DialogBody>
            </DialogSurface>
          </Dialog>
        </div>
      )}

      {tab === "wallpaper" && (
        <div className={s.wallCard}>
          <Caption1 as="p" block style={{ margin: 0 }}>
              {t("appearance.wallpaper.description")}
            </Caption1>

          <Caption1 as="p" block style={{ margin: 0 }}>
            {t("appearance.wallpaper.presetsTitle")}
          </Caption1>
          <div className={s.presetGrid}>
            {WALLPAPER_IDS.map((id) => {
              const name = t(`appearance.wallpaper.presets.${id}`);
              const on = wallPreset.data === id;
              return (
                <button
                  key={id}
                  type="button"
                  className={s.preset}
                  aria-pressed={on}
                  aria-label={t("appearance.wallpaper.presetUse", { name })}
                  disabled={applyPreset.isPending}
                  onClick={() => applyPreset.mutate(id)}
                >
                  <img className={s.presetImg} src={wallpaperDataUri(id)} alt="" draggable={false} />
                  <span className={s.presetName}>
                    {name}
                    {on && <CheckmarkCircleFilled className={s.presetCheck} aria-hidden />}
                  </span>
                </button>
              );
            })}
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
          {/* onde aparece; a página do jogo fica sempre sem (arte do jogo) */}
          <BlockGroup>
            <SelectBlock
              label={t("appearance.wallpaper.scope")}
              value={wallScope}
              options={[
                { value: "home", label: t("appearance.wallpaper.scopeHome") },
                { value: "all", label: t("appearance.wallpaper.scopeAll") },
              ]}
              onChange={(v) => setWallScope(v as WallpaperScope)}
            />
          </BlockGroup>
        </div>
      )}
    </div>
  );
}
