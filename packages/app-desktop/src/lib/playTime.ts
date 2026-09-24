/** Segundos de jogo → texto curto: `"45 min"`, `"3 h 12 min"`, `"2 h"`. */
export function formatPlayTime(secs: number): string {
  if (secs < 60) return 'menos de 1 min'
  const totalMin = Math.floor(secs / 60)
  const h = Math.floor(totalMin / 60)
  const m = totalMin % 60
  if (h === 0) return `${m} min`
  return m === 0 ? `${h} h` : `${h} h ${m} min`
}
