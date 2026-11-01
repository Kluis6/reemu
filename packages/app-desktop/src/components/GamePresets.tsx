import { Button, Caption1, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { getCoreOptions } from "../lib/tauri";
import { useModalButtonStyles } from "../styles/buttons";
import { CoreOptions } from "./CoreOptions";
import { FullscreenDialog } from "./FullscreenDialog";

const PRESETS = ["performance", "normal", "quality", "custom"] as const;
type Preset = (typeof PRESETS)[number];

const storageKey = (romId: string) => `reemu.gamePreset.${romId}`;

function savedPreset(romId: string): Preset | null {
  try {
    const value = localStorage.getItem(storageKey(romId));
    return PRESETS.find((p) => p === value) ?? null;
  } catch {
    return null;
  }
}

const useStyles = makeStyles({
  root: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalM },
  row: {
    display: "grid",
    gridTemplateColumns: `repeat(${PRESETS.length}, minmax(0, 1fr))`,
    columnGap: tokens.spacingHorizontalM,
  },
  button: { maxWidth: "none", minHeight: "48px" },
  hint: { color: tokens.colorNeutralForeground3 },
});

/**
 * Presets do jogo na aba Emulador. Por enquanto só a escolha: Desempenho, Normal e
 * Qualidade ainda não mudam o core. Personalizado abre as opções do core em tela
 * cheia; um jogo que já tem ajustes próprios já vem com ele marcado.
 */
export function GamePresets({ coreId, romId }: { coreId: string; romId: string }) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const [chosen, setChosen] = useState<Preset | null>(() => savedPreset(romId));
  const [customOpen, setCustomOpen] = useState(false);
  // mesma chave de `CoreOptions`: as duas telas enxergam a mesma lista
  const options = useQuery({
    queryKey: ["core-options", coreId, romId],
    queryFn: () => getCoreOptions(coreId, romId),
    enabled: !!coreId,
    retry: false,
  });
  const hasOverrides = options.data?.some((o) => o.romValue != null) ?? false;
  const preset = chosen ?? (hasOverrides ? "custom" : "normal");

  const choose = (next: Preset) => {
    setChosen(next);
    if (next === "custom") setCustomOpen(true);
    try {
      localStorage.setItem(storageKey(romId), next);
    } catch {
      // sem armazenamento, a escolha vale até sair da tela
    }
  };

  return (
    <div className={s.root}>
      <div className={s.row} role="radiogroup" aria-label={t("game.preset.title")}>
        {PRESETS.map((p) => (
          <Button
            key={p}
            className={mergeClasses(s.button, preset !== p && btn.block)}
            role="radio"
            aria-checked={preset === p}
            appearance={preset === p ? "primary" : "secondary"}
            onClick={() => choose(p)}
          >
            {t(`game.preset.${p}`)}
          </Button>
        ))}
      </div>
      <Caption1 className={s.hint}>
        {t(preset === "custom" ? "game.preset.customHint" : "game.preset.pending")}
      </Caption1>
      <FullscreenDialog
        open={customOpen}
        onClose={() => setCustomOpen(false)}
        title={t("game.preset.customTitle")}
      >
        <CoreOptions coreId={coreId} romId={romId} />
      </FullscreenDialog>
    </div>
  );
}
