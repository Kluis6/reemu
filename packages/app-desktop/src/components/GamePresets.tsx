import { Button, Caption1, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { getCoreOptions } from "../lib/tauri";
import { useModalButtonStyles } from "../styles/buttons";
import { CoreOptions } from "./CoreOptions";

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
 * Qualidade ainda não mudam o core. As opções do core aparecem em Personalizado; um
 * jogo que já tem ajustes próprios abre nele, para eles não ficarem escondidos.
 */
export function GamePresets({ coreId, romId }: { coreId: string; romId: string }) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const [chosen, setChosen] = useState<Preset | null>(() => savedPreset(romId));
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
            className={mergeClasses(s.button, preset !== p && btn.secondary)}
            role="radio"
            aria-checked={preset === p}
            appearance={preset === p ? "primary" : "secondary"}
            onClick={() => choose(p)}
          >
            {t(`game.preset.${p}`)}
          </Button>
        ))}
      </div>
      {preset === "custom" ? (
        <CoreOptions coreId={coreId} romId={romId} />
      ) : (
        <Caption1 className={s.hint}>{t("game.preset.pending")}</Caption1>
      )}
    </div>
  );
}
