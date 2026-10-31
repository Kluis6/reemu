const PAIRS = [
  ['enabled', 'disabled'],
  ['on', 'off'],
  ['true', 'false'],
  ['yes', 'no'],
] as const

/** Valores de uma opção de core que é só liga/desliga; `null` nas demais. Os cores
 *  declaram tudo como lista de textos, então o par é reconhecido pelos nomes. */
export function toggleValues(choices: readonly string[]): { on: string; off: string } | null {
  if (choices.length !== 2) return null
  const low = choices.map((c) => c.toLowerCase())
  for (const [on, off] of PAIRS) {
    const i = low.indexOf(on)
    const j = low.indexOf(off)
    if (i >= 0 && j >= 0) return { on: choices[i], off: choices[j] }
  }
  return null
}
