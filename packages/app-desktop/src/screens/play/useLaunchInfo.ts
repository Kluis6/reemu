import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { useLocation, useSearchParams } from "react-router-dom";
import { listRoms } from "../../lib/tauri";

/** O que a tela de jogo precisa para abrir um jogo. */
export interface LaunchInfo {
  coreId: string;
  romPath: string;
  title: string;
  boxart?: string | null;
  system?: string;
}

/**
 * Core e arquivo do jogo a abrir: vêm do `navigate(state)` ou, num reload/URL
 * direta, da lista de ROMs + query param `?core=`. `loading` = ainda buscando
 * a lista para resolver.
 */
export function useLaunchInfo(romId: string): { launch: LaunchInfo | null; loading: boolean } {
  const [params] = useSearchParams();
  const locState = (useLocation().state ?? {}) as Partial<LaunchInfo>;
  const stateCore = locState.coreId;
  const statePath = locState.romPath;
  const stateTitle = locState.title;
  const stateBoxart = locState.boxart;
  const stateSystem = locState.system;

  const roms = useQuery({
    queryKey: ["roms"],
    queryFn: listRoms,
    retry: false,
    enabled: !statePath,
  });
  const rom = roms.data?.find((r) => r.id === romId);
  const coreParam = params.get("core");
  // Memoizado por primitivos — senão o objeto muda toda render e o efeito de
  // load/unload entra em loop.
  const launch: LaunchInfo | null = useMemo(() => {
    if (stateCore && statePath) {
      return {
        coreId: stateCore,
        romPath: statePath,
        title: stateTitle ?? romId,
        boxart: stateBoxart,
        system: stateSystem,
      };
    }
    if (rom && coreParam) {
      return {
        coreId: coreParam,
        romPath: rom.filePath,
        title: rom.title,
        boxart: rom.boxart,
        system: rom.systemId,
      };
    }
    return null;
  }, [stateCore, statePath, stateTitle, stateBoxart, stateSystem, rom, coreParam, romId]);

  return { launch, loading: roms.isLoading };
}
