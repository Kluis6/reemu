/** Até 2 iniciais maiúsculas de um título — fallback quando não há capa. */
export function initials(s: string): string {
  return s
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? '')
    .join('')
}
