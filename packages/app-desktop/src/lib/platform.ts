/**
 * `system_id` do backend → nome curto de plataforma pra UI (chips, abas,
 * filtro). O backend usa ids técnicos (`snes`, `n64`); aqui viram rótulos
 * legíveis. Ids desconhecidos caem no próprio id em maiúsculas.
 */
const LABELS: Record<string, string> = {
  nes: "Nintendo (NES)",
  snes: "Super Nintendo",
  n64: "Nintendo 64",
  gb: "Game Boy",
  gbc: "Game Boy Color",
  gba: "Game Boy Advance",
  vb: "Virtual Boy",
  megadrive: "Mega Drive",
  mastersystem: "Master System",
  gamegear: "Game Gear",
  sega32x: "Sega 32X",
  segacd: "Sega CD",
  saturn: "Sega Saturn",
  dreamcast: "Dreamcast",
  naomi: "NAOMI",
  atomiswave: "Atomiswave",
  neogeocd: "Neo Geo CD",
  cdi: "CD-i",
  psx: "PlayStation",
  ps2: "PlayStation 2",
  psp: "PlayStation Portable",
  pcengine: "PC Engine",
  pcenginecd: "PC Engine CD",
  pcfx: "PC-FX",
  atari2600: "Atari 2600",
  atari7800: "Atari 7800",
  lynx: "Atari Lynx",
  wonderswan: "WonderSwan",
  ngp: "Neo Geo Pocket",
  coleco: "ColecoVision",
  intellivision: "Intellivision",
  "3do": "3DO",
  arcade: "Arcade",
  disc: "Disco",
};

export function platformLabel(systemId: string): string {
  return LABELS[systemId] ?? systemId.toUpperCase();
}
