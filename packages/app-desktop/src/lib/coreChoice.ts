import type { InstalledCore } from './tauri'

/**
 * Ordem dos cores pra um jogo: primeiro os que atendem o SISTEMA do jogo,
 * depois os que aceitam a extensão do arquivo, e por nome. Só a extensão
 * não basta: `.bin` é aceito por dezenas de cores, e a ordem alfabética
 * mandava jogo de Mega Drive (ou de 2600) pro a5200.
 */
export function rankCores(
  cores: readonly InstalledCore[],
  systemId: string | undefined,
  ext: string,
): InstalledCore[] {
  const score = (c: InstalledCore) =>
    (systemId && c.systems.includes(systemId) ? 0 : 2) + (c.extensions.includes(ext) ? 0 : 1)
  return [...cores].sort((a, b) => score(a) - score(b) || a.name.localeCompare(b.name))
}
