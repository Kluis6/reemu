/** Fileiras a mais renderizadas acima e abaixo da tela: o controle precisa do vizinho
 *  no DOM para achar o próximo foco, e o LT/RT pula cerca de uma tela. */
export const OVERSCAN_ROWS = 6

/** Abaixo disso a grade renderiza tudo: não há ganho e o layout fica mais simples. */
export const VIRTUAL_MIN_ITEMS = 120

export interface GridWindow {
  /** Índice do primeiro item renderizado. */
  start: number
  /** Índice depois do último item renderizado. */
  end: number
  /** Altura vazia reservada acima e abaixo do que foi renderizado. */
  padTop: number
  padBottom: number
}

/** O trecho da grade a renderizar. `top` é o quanto da grade já passou do topo da tela. */
export function gridWindow(
  count: number,
  cols: number,
  rowHeight: number,
  top: number,
  viewHeight: number,
): GridWindow {
  const rows = Math.ceil(count / cols)
  const first = Math.max(0, Math.floor(top / rowHeight) - OVERSCAN_ROWS)
  const last = Math.min(rows, Math.ceil((top + viewHeight) / rowHeight) + OVERSCAN_ROWS)
  const startRow = Math.min(first, Math.max(0, rows - 1))
  const endRow = Math.max(last, startRow + 1)
  return {
    start: startRow * cols,
    end: Math.min(count, endRow * cols),
    padTop: startRow * rowHeight,
    padBottom: Math.max(0, rows - endRow) * rowHeight,
  }
}

/** Quantas colunas o `repeat(auto-fill, minmax(min, 1fr))` da grade forma. */
export function gridColumns(width: number, minCell: number, gap: number): number {
  return Math.max(1, Math.floor((width + gap) / (minCell + gap)))
}

/** Letra de um título, sem acento; o que não for letra vira `#`. */
export function titleLetter(title: string): string {
  const c = title.trim().normalize('NFD').charAt(0).toUpperCase()
  return c >= 'A' && c <= 'Z' ? c : '#'
}

/** Letras presentes numa lista ordenada, com o índice do primeiro título de cada uma. */
export function letterIndex(titles: readonly string[]): { letter: string; index: number }[] {
  const seen = new Set<string>()
  const out: { letter: string; index: number }[] = []
  titles.forEach((title, index) => {
    const letter = titleLetter(title)
    if (seen.has(letter)) return
    seen.add(letter)
    out.push({ letter, index })
  })
  return out
}
