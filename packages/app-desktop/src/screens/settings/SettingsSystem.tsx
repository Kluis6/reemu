import {
  Caption1,
  makeStyles,
  tokens,
} from "@fluentui/react-components";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { AboutLicenses } from "../../components/AboutLicenses";
import { HdrSettings } from "../../components/HdrSettings";
import { LoadingState } from "../../components/EmptyState";
import {
  BlockGroup,
  SelectBlock,
  ToggleBlock,
} from "../../components/SettingBlock";
import {
  getLanguagePreference,
  LANGUAGES,
  setLanguagePreference,
  type LanguagePreference,
} from "../../i18n";
import {
  getHardwareInfo,
  getVideoConfig,
  updateVideoConfig,
  getSystemSettings,
  setAutostart,
  setMinimizeToTray,
  setStartFullscreen,
  type SystemSettings,
} from "../../lib/tauri";
import { errorToast } from "../../lib/toast";
import {
  OVERSCANS,
  UI_SCALES,
  getOverscan,
  getUiScale,
  setOverscan,
  setUiScale,
} from "../../lib/uiScale";
import { useToastStore } from "../../stores/useToastStore";
import {
  DesktopTowerRegular,
  InfoRegular,
  OptionsRegular,
} from "@fluentui/react-icons";
import { SettingsHubCards } from "./SettingsHome";

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXXL,
    maxWidth: "1120px",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalL,
  },
  // Opções: grupos de blocos, sobre o card geral (pedido do usuário).
  cards: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXXL,
  },
  hwCard: { maxWidth: "720px" },
  // rótulo à esquerda, valor à direita; em tela estreita um embaixo do outro
  hwList: {
    display: "grid",
    gridTemplateColumns: "minmax(140px, max-content) minmax(0, 1fr)",
    columnGap: tokens.spacingHorizontalXXL,
    rowGap: tokens.spacingVerticalM,
    margin: 0,
    "@media (max-width: 640px)": { gridTemplateColumns: "minmax(0, 1fr)" },
  },
  hwLabel: { color: tokens.colorNeutralForeground3 },
  hwValue: { margin: 0, overflowWrap: "anywhere" },
});

type Key = keyof SystemSettings;

// Cada interruptor e o comando que grava ele.
const SAVE: Record<Key, (enabled: boolean) => Promise<void>> = {
  autostart: setAutostart,
  startFullscreen: setStartFullscreen,
  minimizeToTray: setMinimizeToTray,
};

/** Configurações › Sistema: idioma, tamanho da interface, inicialização
 *  (iniciar com o sistema, tela cheia, bandeja — `src-tauri/src/system.rs`),
 *  o hardware em que o app roda (`commands/hardware.rs`) e as informações e
 *  licenças (`AboutLicenses` — o único lugar do app com créditos e
 *  licenças de terceiros). Abre nos cards das três partes, como Aparência e
 *  Decorações (pedido do usuário); cada uma tem a sua rota. */
export function SettingsSystem({
  section,
}: {
  /** `undefined` = entrada do Sistema (cards das opções). */
  section?: "opcoes" | "hardware" | "sobre";
}) {
  const { t, i18n } = useTranslation();
  const s = useStyles();
  const qc = useQueryClient();
  const push = useToastStore((st) => st.push);
  const [langPref, setLangPref] = useState<LanguagePreference>(
    getLanguagePreference,
  );
  const [uiScale, setUiScaleState] = useState(getUiScale);
  const [overscan, setOverscanState] = useState(getOverscan);

  const settings = useQuery({
    queryKey: ["system-settings"],
    queryFn: getSystemSettings,
    retry: false,
  });
  const hardware = useQuery({
    queryKey: ["hardware-info"],
    queryFn: getHardwareInfo,
    retry: false,
    staleTime: Infinity,
  });
  // Integer scaling (veio de Vídeo): `video-config`, o mesmo do jogo.
  const videoCfg = useQuery({
    queryKey: ['video-config'],
    queryFn: getVideoConfig,
    retry: false,
  })
  const setIntegerScaling = useMutation({
    // A config inteira: mandar só este campo apagaria a do HDR.
    mutationFn: (integerScaling: boolean) =>
      videoCfg.data
        ? updateVideoConfig({ ...videoCfg.data, integerScaling })
        : Promise.resolve(),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['video-config'] }),
    onError: (e) => push(errorToast(e, 'saveVideoConfig')),
  })
  const save = useMutation({
    mutationFn: ({ key, value }: { key: Key; value: boolean }) =>
      SAVE[key](value),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["system-settings"] }),
    onError: (e) => push(errorToast(e, "saveSystemSettings")),
  });

  if (!section)
    return (
      <SettingsHubCards
        label={t("settings.tabs.system")}
        items={[
          {
            to: "opcoes",
            icon: <OptionsRegular />,
            title: t("system.options"),
            description: t("system.optionsDesc"),
          },
          {
            to: "hardware",
            icon: <DesktopTowerRegular />,
            title: t("system.hardware.title"),
            description: t("system.hardware.description"),
          },
          {
            to: "sobre",
            icon: <InfoRegular />,
            title: t("about.tab"),
            description: t("about.cardDesc"),
          },
        ]}
      />
    );

  if (settings.isLoading) return <LoadingState />;

  const toggle = (key: Key, label: string, hint: string) => (
    <ToggleBlock
      label={label}
      description={hint}
      checked={settings.data?.[key] ?? false}
      disabled={!settings.data || save.isPending}
      onChange={(value) => save.mutate({ key, value })}
    />
  );

  const hw = hardware.data;
  const unknown = t("system.hardware.unknown");
  const gb = (bytes: number) =>
    `${new Intl.NumberFormat(i18n.language, {
      maximumFractionDigits: 1,
    }).format(bytes / 1024 ** 3)} GB`;
  const rows: [string, ReactNode][] = hw
    ? [
        [t("system.hardware.os"), `${hw.os ?? unknown} (${hw.arch})`],
        [
          t("system.hardware.cpu"),
          <>
            {hw.cpu ?? unknown}
            <Caption1 block>
              {hw.cpuCores
                ? t("system.hardware.cores", {
                    cores: hw.cpuCores,
                    threads: hw.cpuThreads,
                  })
                : t("system.hardware.threads", { threads: hw.cpuThreads })}
            </Caption1>
          </>,
        ],
        [
          t("system.hardware.memory"),
          hw.memoryBytes > 0 ? gb(hw.memoryBytes) : unknown,
        ],
        [t("system.hardware.gpu"), hw.gpu ?? unknown],
        [t("system.hardware.backend"), hw.gpuBackend ?? unknown],
        [t("system.hardware.driver"), hw.gpuDriver ?? unknown],
      ]
    : [];

  return (
    <div className={s.root}>
      {section === "opcoes" && (
        <div className={s.cards}>
          <BlockGroup>
            <SelectBlock
              label={t("language.title")}
              description={t("language.description")}
              value={langPref}
              options={[
                { value: "auto", label: t("language.auto") },
                // cada idioma no próprio nome (quem não lê o atual acha o seu)
                ...LANGUAGES.map((l) => ({ value: l, label: t(`language.${l}`) })),
              ]}
              onChange={(value) => {
                const v = value as LanguagePreference;
                setLangPref(v);
                void setLanguagePreference(v);
              }}
            />
            <SelectBlock
              label={t("appearance.uiScale.title")}
              description={t("appearance.uiScale.description")}
              value={String(uiScale)}
              options={UI_SCALES.map((o) => ({
                value: String(o.value),
                label: `${t(o.label)} (${Math.round(o.value * 100)}%)`,
              }))}
              onChange={(value) => {
                const v = Number(value);
                setUiScaleState(v);
                setUiScale(v).catch((e) => push(errorToast(e, "changeUiScale")));
              }}
            />
            <SelectBlock
              label={t("appearance.overscan.title")}
              description={t("appearance.overscan.description")}
              value={String(overscan)}
              options={OVERSCANS.map((o) => ({ value: String(o.value), label: t(o.label) }))}
              onChange={(value) => {
                const v = Number(value);
                setOverscanState(v);
                setOverscan(v);
              }}
            />
          </BlockGroup>

          <BlockGroup title={t("system.general")}>
            {toggle("autostart", t("system.autostart"), t("system.autostartHint"))}
            {toggle("startFullscreen", t("system.startFullscreen"), t("system.startFullscreenHint"))}
            {toggle("minimizeToTray", t("system.minimizeToTray"), t("system.minimizeToTrayHint"))}
          </BlockGroup>

          <BlockGroup title={t("settings.tabs.video")}>
            <ToggleBlock
              label={t("video.integerScaling")}
              description={t("video.integerScalingHint")}
              checked={videoCfg.data?.integerScaling ?? false}
              disabled={videoCfg.isLoading || setIntegerScaling.isPending}
              onChange={(checked) => setIntegerScaling.mutate(checked)}
            />
            <HdrSettings />
          </BlockGroup>
        </div>
      )}

      {section === "hardware" && (
        <section className={s.section}>
          <Caption1 as="p" block style={{ margin: 0 }}>
            {t("system.hardware.description")}
          </Caption1>
          {hardware.isLoading ? (
            <LoadingState />
          ) : (
            // direto no card geral, sem card interno (pedido do usuário)
            <div className={s.hwCard}>
              <dl className={s.hwList}>
                {(hw
                  ? rows
                  : [
                      [t("system.hardware.title"), unknown] as [
                        string,
                        ReactNode,
                      ],
                    ]
                ).map(([label, value]) => (
                  <div key={label} style={{ display: "contents" }}>
                    <dt className={s.hwLabel}>{label}</dt>
                    <dd className={s.hwValue}>{value}</dd>
                  </div>
                ))}
              </dl>
            </div>
          )}
        </section>
      )}

      {section === "sobre" && <AboutLicenses />}
    </div>
  );
}
