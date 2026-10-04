import {
  Caption1,
  Card,
  Field,
  Radio,
  RadioGroup,
  Switch,
  Tab,
  TabList,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
  type RadioGroupOnChangeData,
} from "@fluentui/react-components";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { LoadingState } from "../../components/EmptyState";
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
import { UI_SCALES, getUiScale, setUiScale } from "../../lib/uiScale";
import { useToastStore } from "../../stores/useToastStore";
import { useTabStyles } from "../../styles/xbox";

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
  // Opções e hardware, cada um na sua aba (24 até o conteúdo: o `rowGap`).
  tabs: { alignSelf: "flex-start" },
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

function Heading({
  title,
  description,
}: {
  title: string;
  description?: string;
}) {
  return (
    <div>
      <Text as="strong" weight="semibold">
        {title}
      </Text>
      {description && (
        <Caption1 as="p" block style={{ margin: "2px 0 0" }}>
          {description}
        </Caption1>
      )}
    </div>
  );
}

/** Configurações › Sistema: idioma, tamanho da interface, inicialização
 *  (iniciar com o sistema, tela cheia, bandeja — `src-tauri/src/system.rs`)
 *  e o hardware em que o app roda (`commands/hardware.rs`). */
export function SettingsSystem() {
  const { t, i18n } = useTranslation();
  const s = useStyles();
  const tb = useTabStyles();
  const [tab, setTab] = useState<"options" | "hardware">("options");
  const qc = useQueryClient();
  const push = useToastStore((st) => st.push);
  const [langPref, setLangPref] = useState<LanguagePreference>(
    getLanguagePreference,
  );
  const [uiScale, setUiScaleState] = useState(getUiScale);

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
    mutationFn: (integerScaling: boolean) => updateVideoConfig({ integerScaling }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['video-config'] }),
    onError: (e) => push(errorToast(e, 'saveVideoConfig')),
  })
  const save = useMutation({
    mutationFn: ({ key, value }: { key: Key; value: boolean }) =>
      SAVE[key](value),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["system-settings"] }),
    onError: (e) => push(errorToast(e, "saveSystemSettings")),
  });

  if (settings.isLoading) return <LoadingState />;

  const toggle = (key: Key, label: string, hint: string) => (
    <Field label={label} hint={hint}>
      <Switch
        checked={settings.data?.[key] ?? false}
        disabled={!settings.data || save.isPending}
        onChange={(_, d) => save.mutate({ key, value: d.checked })}
      />
    </Field>
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
      <TabList
        className={mergeClasses(tb.tabs, s.tabs)}
        selectedValue={tab}
        onTabSelect={(_, d) => setTab(d.value as "options" | "hardware")}
      >
        <Tab value="options">{t("system.options")}</Tab>
        <Tab value="hardware">{t("system.hardware.title")}</Tab>
      </TabList>

      {tab === "options" && (
        <>
          <section className={s.section}>
            <Heading
              title={t("language.title")}
              description={t("language.description")}
            />
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
          </section>

          <section className={s.section}>
            <Heading
              title={t("appearance.uiScale.title")}
              description={t("appearance.uiScale.description")}
            />
            <RadioGroup
              layout="horizontal"
              aria-label={t("appearance.uiScale.title")}
              value={String(uiScale)}
              onChange={(_, data: RadioGroupOnChangeData) => {
                const v = Number(data.value);
                setUiScaleState(v);
                setUiScale(v).catch((e) =>
                  push(errorToast(e, "changeUiScale")),
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
          </section>

          <section className={s.section}>
            <Heading title={t("system.general")} />
            {toggle(
              "autostart",
              t("system.autostart"),
              t("system.autostartHint"),
            )}
            {toggle(
              "startFullscreen",
              t("system.startFullscreen"),
              t("system.startFullscreenHint"),
            )}
            {toggle(
              "minimizeToTray",
              t("system.minimizeToTray"),
              t("system.minimizeToTrayHint"),
            )}
          </section>

          <section className={s.section}>
            <Heading title={t("settings.tabs.video")} />
            <Field
              label={t("video.integerScaling")}
              hint={t("video.integerScalingHint")}
            >
              <Switch
                checked={videoCfg.data?.integerScaling ?? false}
                disabled={videoCfg.isLoading || setIntegerScaling.isPending}
                onChange={(_, d) => setIntegerScaling.mutate(d.checked)}
              />
            </Field>
          </section>
        </>
      )}

      {tab === "hardware" && (
        <section className={s.section}>
          <Caption1 as="p" block style={{ margin: 0 }}>
            {t("system.hardware.description")}
          </Caption1>
          {hardware.isLoading ? (
            <LoadingState />
          ) : (
            <Card className={s.hwCard} appearance="filled-alternative">
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
            </Card>
          )}
        </section>
      )}
    </div>
  );
}
