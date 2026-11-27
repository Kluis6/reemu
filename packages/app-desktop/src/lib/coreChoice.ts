import type { InstalledCore } from './tauri'

// Cores validados com um arquivo de jogo e a BIOS na pasta do sistema.
// Alguns anunciam .bs/.st só para subsistemas ou omitem a extensão no probe.
const snesExpansionCores = new Map([
  ['bs', ['snes9x_libretro', 'mesen-s_libretro', 'mesen2_libretro']],
  ['st', ['snes9x_libretro', 'mesen2_libretro']],
])

export function requiresCompatibleCore(systemId: string | undefined, ext: string): boolean {
  return systemId === 'laserdisc' || systemId === 'zx81' || systemId === 'arduboy'
    || systemId === 'uzebox' || systemId === 'vaporspec' || systemId === 'gametank'
    || systemId === 'megaduck' || systemId === 'handheld_electronic'
    || systemId === 'arcadia' || systemId === 'vc4000' || systemId === 'elektor'
    || (systemId === 'zxspectrum' && ext === 'tzx')
}

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
  const available = systemId && requiresCompatibleCore(systemId, ext)
    ? cores.filter((core) => core.systems.includes(systemId) && core.extensions.includes(ext))
    : cores
  const preferred = systemId === 'snes' ? snesExpansionCores.get(ext) : undefined
  const score = (c: InstalledCore) => {
    if (preferred) {
      const order = preferred.indexOf(c.coreId)
      if (order >= 0) return order - preferred.length
    }
    return (systemId && c.systems.includes(systemId) ? 0 : 2) + (c.extensions.includes(ext) ? 0 : 1)
  }
  // Um ZIP de Arcade pode ser de outra placa. Em empate, o CPS-1 fica como
  // alternativa depois dos cores que a seleção automática já oferecia.
  const arcadePriority = (c: InstalledCore) =>
    c.coreId === 'fbalpha2012_cps1_libretro' ? 1 : 0
  return [...available].sort((a, b) =>
    score(a) - score(b) || arcadePriority(a) - arcadePriority(b) || a.name.localeCompare(b.name),
  )
}
