const MINUTE = 60
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR
/** Mês e ano aproximados (30 e 365 dias): a idade é uma ordem de grandeza. */
const MONTH = 30 * DAY
const YEAR = 365 * DAY

/** "há 5 minutos", "agora", "há 2 meses", "há 1 ano": o instante (segundos
 *  Unix) contra `nowMs`. Mês e ano sempre com o número ("há 1 mês", nunca "mês
 *  passado"): a conta é por dias corridos, não pelo calendário. */
export function relativeAge(secs: number, nowMs: number, lang: string): string {
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' })
  const diff = Math.max(0, Math.round(nowMs / 1000 - secs))
  if (diff < MINUTE) return rtf.format(0, 'second')
  if (diff < HOUR) return rtf.format(-Math.floor(diff / MINUTE), 'minute')
  if (diff < DAY) return rtf.format(-Math.floor(diff / HOUR), 'hour')
  if (diff < MONTH) return rtf.format(-Math.floor(diff / DAY), 'day')
  const always = new Intl.RelativeTimeFormat(lang, { numeric: 'always' })
  if (diff < YEAR) return always.format(-Math.floor(diff / MONTH), 'month')
  return always.format(-Math.floor(diff / YEAR), 'year')
}
