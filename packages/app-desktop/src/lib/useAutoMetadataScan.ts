import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useToastStore } from "../stores/useToastStore";
import { startMetadataScan } from "./tauri";
import { sysToast } from "./toast";

/**
 * Depois de uma varredura da biblioteca que trouxe jogos novos, começa a busca
 * de metadados sozinha (a busca só pega o que ainda não tem metadado, então
 * não refaz a biblioteca inteira). Toast de início igual ao do botão em
 * Configurações › Metadados, onde fica a barra de progresso. Se já houver uma
 * busca rodando (ou não houver banco), não avisa nada: o backend recusa e a
 * que está rodando segue.
 */
export function useAutoMetadataScan() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const push = useToastStore((s) => s.push);
  return (added: number) => {
    if (added <= 0) return;
    startMetadataScan()
      .then(() => {
        push(sysToast(t("metadata.started"), "Info"));
        qc.invalidateQueries({ queryKey: ["metadata-progress"] });
      })
      .catch(() => {
        /* já tem uma busca em andamento — ela segue */
      });
  };
}
