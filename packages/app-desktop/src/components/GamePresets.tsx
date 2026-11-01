import { Button, Caption1, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { corePreset, detectLevel, presetChanges, presetHasOptions, type PresetLevel } from "../lib/corePresets";
import { getCoreOptions, setCoreOption } from "../lib/tauri";
import { errorToast } from "../lib/toast";
import { useToastStore } from "../stores/useToastStore";
import { useModalButtonStyles } from "../styles/buttons";
import { CoreOptions } from "./CoreOptions";
import { FullscreenDialog } from "./FullscreenDialog";

const LEVELS = ["performance", "normal", "quality"] as const;

const useStyles = makeStyles({
  root: { display: "flex", flexDirection: "column", rowGap: tokens.spacingVerticalM },
  row: {
    display: "grid",
    gridAutoFlow: "column",
    gridAutoColumns: "minmax(0, 1fr)",
    columnGap: tokens.spacingHorizontalM,
  },
  button: { maxWidth: "none", minHeight: "48px" },
  hint: { color: tokens.colorNeutralForeground3 },
});

/**
 * Predefinições do jogo na aba Emulador. Gravam nas opções do próprio jogo, e o botão
 * marcado vem desses valores (`detectLevel`). Personalizado abre as opções do core em
 * tela cheia. Core sem predefinição pronta muda só a resolução; sem essa opção, Desempenho
 * e Qualidade ficam desativados.
 */
export function GamePresets({ coreId, romId }: { coreId: string; romId: string }) {
  const { t } = useTranslation();
  const s = useStyles();
  const btn = useModalButtonStyles();
  const qc = useQueryClient();
  const push = useToastStore((st) => st.push);
  const [customOpen, setCustomOpen] = useState(false);
  // mesma chave de `CoreOptions`: as duas telas enxergam a mesma lista
  const key = ["core-options", coreId, romId];
  const options = useQuery({
    queryKey: key,
    queryFn: () => getCoreOptions(coreId, romId),
    enabled: !!coreId,
    retry: false,
    // relê ao voltar do jogo: é ao carregar que o core informa as opções
    staleTime: 0,
    refetchOnMount: "always",
  });
  const list = options.data ?? [];
  const preset = corePreset(coreId, list);
  const hasOptions = presetHasOptions(preset);
  const level: PresetLevel = hasOptions
    ? detectLevel(preset, list)
    : list.some((o) => o.romValue != null)
      ? "custom"
      : "normal";
  const apply = useMutation({
    mutationFn: async (next: (typeof LEVELS)[number]) => {
      for (const c of presetChanges(preset, next, list)) {
        await setCoreOption(coreId, c.key, c.value, romId);
      }
    },
    onError: (e) => push(errorToast(e, "applyCoreOption")),
    onSettled: () => qc.invalidateQueries({ queryKey: key }),
  });
  const needsRun = options.isSuccess && list.length === 0;
  const button = (p: PresetLevel, onClick: () => void, disabled = false) => (
    <Button
      key={p}
      className={mergeClasses(s.button, level !== p && btn.block)}
      role="radio"
      aria-checked={level === p}
      appearance={level === p ? "primary" : "secondary"}
      disabled={disabled}
      onClick={onClick}
    >
      {t(`game.preset.${p}`)}
    </Button>
  );

  return (
    <div className={s.root}>
      <div className={s.row} role="radiogroup" aria-label={t("game.preset.title")}>
        {LEVELS.map((p) =>
          button(p, () => apply.mutate(p), needsRun || apply.isPending || (p !== "normal" && !hasOptions)),
        )}
        {button("custom", () => setCustomOpen(true))}
      </div>
      <Caption1 className={s.hint}>
        {needsRun
          ? t("game.preset.needsRun")
          : !hasOptions
            ? t("game.preset.noOptions")
            : preset.resolutionOnly && level !== "normal" && level !== "custom"
              ? t(`game.preset.${level}ResolutionHint`)
              : t(`game.preset.${level}Hint`)}
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
