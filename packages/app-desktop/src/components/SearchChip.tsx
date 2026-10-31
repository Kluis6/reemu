import { Button } from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { useSearchStore } from "../stores/useSearchStore";

/** Mostra que a lista está filtrada pela busca, como o filtro ativo com X do app Xbox. */
export function SearchChip({ className }: { className?: string }) {
  const { t } = useTranslation();
  const query = useSearchStore((s) => s.query).trim();
  const setQuery = useSearchStore((s) => s.setQuery);
  if (!query) return null;
  return (
    <Button
      appearance="secondary"
      shape="circular"
      className={className}
      icon={<DismissRegular />}
      iconPosition="after"
      aria-label={t("library.clearSearch", { query })}
      onClick={() => setQuery("")}
    >
      {t("library.searchActive", { query })}
    </Button>
  );
}
