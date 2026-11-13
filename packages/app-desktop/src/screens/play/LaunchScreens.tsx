import {
  Accordion,
  AccordionHeader,
  AccordionItem,
  AccordionPanel,
  Body1,
  Button,
  Caption1,
  Spinner,
  Title3,
} from "@fluentui/react-components";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { describeError } from "../../lib/errors";
import { initials } from "../../lib/initials";
import { platformLabel } from "../../lib/platform";
import { copyErrorDetails, fixRoute } from "../../lib/toast";
import { usePlayStyles } from "./styles";
import type { LaunchInfo } from "./useLaunchInfo";

/** Sem jogo para abrir (URL sem `?core=` e sem `state` do `navigate`). */
export function NoTarget({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const styles = usePlayStyles();
  return (
    <div className={styles.center}>
      <div style={{ textAlign: "center", display: "flex", flexDirection: "column", gap: 12 }}>
        <Body1>{t("play.noTarget")}</Body1>
        <Button onClick={onBack}>{t("common.back")}</Button>
      </div>
    </div>
  );
}

/** Abertura enquanto o core carrega: capa (ou iniciais), nome e sistema. */
export function LaunchSplash({ launch }: { launch: LaunchInfo | null }) {
  const { t } = useTranslation();
  const styles = usePlayStyles();
  const [artBroken, setArtBroken] = useState(false);
  const showArt = launch?.boxart && !artBroken;
  return (
    <div className={styles.splash}>
      {showArt ? (
        <img
          className={styles.splashArt}
          src={launch.boxart ?? ""}
          alt={launch?.title ?? ""}
          onError={() => setArtBroken(true)}
        />
      ) : (
        <div className={styles.splashArt}>{initials(launch?.title ?? "?")}</div>
      )}
      <div className={styles.splashTitle}>{launch?.title ?? t("common.loading")}</div>
      {launch?.system && <div className={styles.splashSub}>{platformLabel(launch.system)}</div>}
      <Spinner size="small" label={t("common.loading")} />
    </div>
  );
}

/**
 * O jogo não abriu: o que aconteceu e o que fazer (`lib/errors.ts`), com o
 * botão pra solução quando existe e o texto técnico recolhido pra relatar.
 */
export function LaunchError({
  error,
  launch,
  onBack,
}: {
  error: unknown;
  launch: LaunchInfo | null;
  onBack: () => void;
}) {
  const { t } = useTranslation();
  const styles = usePlayStyles();
  const err = describeError(error, "openGame");
  const fix = err.fix ? fixRoute(err.fix) : null;
  return (
    <div className={styles.center}>
      <div className={styles.errorBox} role="alert">
        <Title3>{err.title}</Title3>
        {launch?.title && (
          <Caption1 className={styles.errorGame}>
            {launch.title}
            {launch.system ? ` · ${platformLabel(launch.system)}` : ""}
          </Caption1>
        )}
        {err.hint && <Body1>{err.hint}</Body1>}
        <div className={styles.errorActions}>
          {fix && (
            <Button appearance="primary" onClick={() => (window.location.hash = fix.hash)}>
              {fix.label}
            </Button>
          )}
          <Button appearance={fix ? "secondary" : "primary"} onClick={onBack}>
            {t("common.back")}
          </Button>
          <Button appearance="subtle" onClick={() => copyErrorDetails(err.title, err.technical)}>
            {t("errors.copyDetails")}
          </Button>
        </div>
        <Accordion collapsible className={styles.errorDetails}>
          <AccordionItem value="technical">
            <AccordionHeader size="small">{t("errors.technicalDetails")}</AccordionHeader>
            <AccordionPanel>
              <code>{err.technical}</code>
            </AccordionPanel>
          </AccordionItem>
        </Accordion>
      </div>
    </div>
  );
}
