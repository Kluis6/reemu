import { Button, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import {
  ArrowUpRegular,
  BackspaceRegular,
  CheckmarkRegular,
  DismissRegular,
} from '@fluentui/react-icons'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useSearchStore } from '../stores/useSearchStore'

/**
 * Teclado na tela pra digitar na busca com o controle (o Fluent não tem
 * componente de teclado, e o teclado virtual do Windows é feito pra toque e
 * não existe no Linux). No estilo do teclado do Xbox: grade QWERTY navegada
 * pelo direcional, com a tecla em foco no anel padrão do app.
 *
 * Botões (vêm do `useMenuNav` via `useSearchStore.keyboardNav`, enquanto o
 * teclado está aberto ele recebe os pulsos antes da navegação normal):
 * A digita a tecla em foco · B apaga (com o campo vazio, fecha) · X espaço ·
 * Y maiúsculas/minúsculas · ☰ (Menu) confirma e fecha. A biblioteca filtra
 * enquanto se digita (a busca é a mesma do teclado físico).
 */

type KeyDef =
  | { kind: 'char'; label: string }
  | { kind: 'shift' | 'space' | 'back' | 'done' | 'close'; span: number }

const ROWS_LOWER: string[][] = [
  ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
  ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
  ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'ç'],
  ['z', 'x', 'c', 'v', 'b', 'n', 'm', '-', ':', "'"],
  ['á', 'à', 'ã', 'â', 'é', 'ê', 'í', 'ó', 'õ', 'ú'],
]

const ROWS_UPPER = ROWS_LOWER.map((r) => r.map((c) => c.toLocaleUpperCase('pt-BR')))

/** Linha de ações: ⇧ (2) · espaço (4) · apagar (2) · OK (1) · fechar (1). */
const ACTIONS: KeyDef[] = [
  { kind: 'shift', span: 2 },
  { kind: 'space', span: 4 },
  { kind: 'back', span: 2 },
  { kind: 'done', span: 1 },
  { kind: 'close', span: 1 },
]

const COLS = 10

/** Coluna inicial (0–9) de cada tecla de ação, pra navegar na vertical. */
const ACTION_COL = (() => {
  let c = 0
  return ACTIONS.map((a) => {
    const at = c
    c += a.kind === 'char' ? 1 : a.span
    return at
  })
})()

const useStyles = makeStyles({
  // Painel na superfície do tema (`Background1`), sem borda nem sombra, como
  // o resto do app; preso embaixo, no meio da área de conteúdo.
  panel: {
    position: 'fixed',
    left: '50%',
    bottom: '24px',
    transform: 'translateX(-50%)',
    zIndex: 60,
    width: 'min(760px, calc(100vw - 120px))',
    padding: tokens.spacingHorizontalL,
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground1,
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalS,
    animationName: { from: { opacity: 0, transform: 'translate(-50%, 12px)' }, to: { opacity: 1, transform: 'translate(-50%, 0)' } },
    animationDuration: tokens.durationNormal,
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'backwards',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  // o que está sendo digitado (o campo de busca também mostra, lá em cima)
  preview: {
    minHeight: '32px',
    display: 'flex',
    alignItems: 'center',
    paddingLeft: tokens.spacingHorizontalM,
    paddingRight: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground1,
    fontSize: tokens.fontSizeBase400,
    overflow: 'hidden',
    whiteSpace: 'nowrap',
  },
  placeholder: { color: tokens.colorNeutralForeground3 },
  caret: {
    display: 'inline-block',
    width: '2px',
    height: '1.1em',
    marginLeft: '1px',
    backgroundColor: tokens.colorNeutralForeground1,
    verticalAlign: 'middle',
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: `repeat(${COLS}, minmax(0, 1fr))`,
    gap: tokens.spacingHorizontalXS,
  },
  key: {
    minWidth: '0 !important',
    // o Fluent trava botão só-ícone em 32 px (`max-width`); aqui a tecla
    // ocupa a(s) coluna(s) da grade
    maxWidth: 'none !important',
    width: '100%',
    height: '40px',
    padding: '0 !important',
    fontSize: tokens.fontSizeBase400,
    fontWeight: `${tokens.fontWeightRegular} !important`,
    backgroundColor: `${tokens.colorNeutralBackground3} !important`,
    border: 'none !important',
    ':hover': { backgroundColor: `${tokens.colorNeutralBackground3Hover} !important` },
    ':active': { backgroundColor: `${tokens.colorNeutralBackground3Pressed} !important` },
  },
  action: { backgroundColor: `${tokens.colorNeutralBackground4} !important` },
  on: {
    backgroundColor: 'var(--reemuBrandSolid) !important',
    color: 'var(--reemuOnBrand) !important',
  },
  hints: {
    display: 'flex',
    flexWrap: 'wrap',
    justifyContent: 'center',
    columnGap: tokens.spacingHorizontalL,
    color: tokens.colorNeutralForeground3,
    fontSize: tokens.fontSizeBase200,
  },
})

export function OnScreenKeyboard() {
  const { t } = useTranslation()
  const s = useStyles()
  const open = useSearchStore((st) => st.keyboard)
  const query = useSearchStore((st) => st.query)
  const setQuery = useSearchStore((st) => st.setQuery)
  const setKeyboard = useSearchStore((st) => st.setKeyboard)
  const setKeyboardNav = useSearchStore((st) => st.setKeyboardNav)
  const [upper, setUpper] = useState(false)
  // posição em foco: linha 0–4 = letras, 5 = ações; coluna 0–9 (letras) ou
  // índice da tecla de ação
  const [pos, setPos] = useState({ row: 1, col: 0 })
  const keyRefs = useRef(new Map<string, HTMLButtonElement>())
  const posRef = useRef(pos)
  useEffect(() => {
    posRef.current = pos
  }, [pos])

  // memo: um array novo a cada render recriava `press`/o handler do
  // controle, que re-registrava na store e re-renderizava sem parar
  const rows = useMemo(
    () => (upper ? ROWS_UPPER : ROWS_LOWER),
    [upper],
  )

  const type = useCallback((txt: string) => setQuery(useSearchStore.getState().query + txt), [setQuery])
  const backspace = useCallback(() => {
    const q = useSearchStore.getState().query
    setQuery(Array.from(q).slice(0, -1).join(''))
  }, [setQuery])
  // Fecha e devolve o foco pro primeiro resultado (ou pra busca), pra o
  // controle continuar navegando a partir dali.
  const close = useCallback(() => {
    setKeyboard(false)
    requestAnimationFrame(() => {
      const first =
        document.querySelector<HTMLElement>('[data-art]')?.closest<HTMLElement>('[tabindex]') ??
        document.querySelector<HTMLElement>('.fui-SearchBox input')
      first?.focus()
    })
  }, [setKeyboard])

  const press = useCallback(
    (row: number, col: number) => {
      if (row < rows.length) {
        type(rows[row][col])
        return
      }
      const a = ACTIONS[col]
      if (a.kind === 'shift') setUpper((u) => !u)
      else if (a.kind === 'space') type(' ')
      else if (a.kind === 'back') backspace()
      else close()
    },
    [rows, type, backspace, close],
  )

  // Direcional: anda na grade (sem sair do teclado). Da linha de letras pra
  // de ações e de volta, mantém a coluna mais próxima.
  const move = useCallback((dir: string) => {
    setPos(({ row, col }) => {
      const last = ROWS_LOWER.length // linha de ações
      if (dir === 'left' || dir === 'right') {
        const n = row === last ? ACTIONS.length : COLS
        const next = (col + (dir === 'right' ? 1 : -1) + n) % n
        return { row, col: next }
      }
      if (dir === 'up') {
        if (row === 0) return { row, col }
        if (row === last) return { row: row - 1, col: ACTION_COL[col] }
        return { row: row - 1, col }
      }
      // down
      if (row === last) return { row, col }
      if (row === last - 1) {
        let idx = 0
        ACTION_COL.forEach((start, i) => {
          if (col >= start) idx = i
        })
        return { row: last, col: idx }
      }
      return { row: row + 1, col }
    })
  }, [])

  // Recebe os pulsos do controle enquanto aberto (ver `useMenuNav`).
  useEffect(() => {
    if (!open) return
    const handler = (nav: string): boolean => {
      switch (nav) {
        case 'up':
        case 'down':
        case 'left':
        case 'right':
          move(nav)
          return true
        case 'confirm': {
          const { row, col } = posRef.current
          press(row, col)
          return true
        }
        case 'back':
          if (useSearchStore.getState().query) backspace()
          else close()
          return true
        case 'action':
          type(' ')
          return true
        case 'search':
          setUpper((u) => !u)
          return true
        case 'context':
          close()
          return true
        default:
          return false
      }
    }
    setKeyboardNav(handler)
    return () => setKeyboardNav(null)
  }, [open, move, press, backspace, close, type, setKeyboardNav])

  // A tecla em foco recebe o foco do DOM (anel padrão do app).
  useLayoutEffect(() => {
    if (!open) return
    keyRefs.current.get(`${pos.row}:${pos.col}`)?.focus({ preventScroll: true })
  }, [open, pos])

  if (!open) return null

  const keyRef = (row: number, col: number) => (el: HTMLButtonElement | null) => {
    if (el) keyRefs.current.set(`${row}:${col}`, el)
    else keyRefs.current.delete(`${row}:${col}`)
  }

  const actionLabel = (a: KeyDef) =>
    a.kind === 'shift'
      ? t('osk.shift')
      : a.kind === 'space'
        ? t('osk.space')
        : a.kind === 'back'
          ? t('osk.backspace')
          : a.kind === 'done'
            ? t('osk.done')
            : t('osk.close')
  const actionIcon = (a: KeyDef) =>
    a.kind === 'shift' ? (
      <ArrowUpRegular />
    ) : a.kind === 'back' ? (
      <BackspaceRegular />
    ) : a.kind === 'done' ? (
      <CheckmarkRegular />
    ) : a.kind === 'close' ? (
      <DismissRegular />
    ) : undefined

  return (
    <div className={s.panel} role="dialog" aria-label={t('osk.title')} data-nav-skip>
      <div className={s.preview} aria-live="polite">
        {query ? query : <span className={s.placeholder}>{t('shell.searchPlaceholder')}</span>}
        <span className={s.caret} aria-hidden />
      </div>
      <div className={s.grid} role="group">
        {rows.map((r, ri) =>
          r.map((c, ci) => (
            <Button
              key={`${ri}:${ci}`}
              ref={keyRef(ri, ci)}
              className={s.key}
              tabIndex={-1}
              onClick={() => {
                setPos({ row: ri, col: ci })
                press(ri, ci)
              }}
            >
              {c}
            </Button>
          )),
        )}
        {ACTIONS.map((a, i) => (
          <Button
            key={a.kind}
            ref={keyRef(ROWS_LOWER.length, i)}
            className={mergeClasses(s.key, s.action, a.kind === 'shift' && upper && s.on)}
            style={{ gridColumn: `span ${a.kind === 'char' ? 1 : a.span}` }}
            tabIndex={-1}
            aria-label={actionLabel(a)}
            aria-pressed={a.kind === 'shift' ? upper : undefined}
            icon={actionIcon(a)}
            onClick={() => {
              setPos({ row: ROWS_LOWER.length, col: i })
              press(ROWS_LOWER.length, i)
            }}
          >
            {a.kind === 'space' ? t('osk.space') : undefined}
          </Button>
        ))}
      </div>
      <div className={s.hints} aria-hidden>
        <span>{t('osk.hintA')}</span>
        <span>{t('osk.hintB')}</span>
        <span>{t('osk.hintX')}</span>
        <span>{t('osk.hintY')}</span>
        <span>{t('osk.hintMenu')}</span>
      </div>
    </div>
  )
}
