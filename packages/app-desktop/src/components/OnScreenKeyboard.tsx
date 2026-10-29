import { Button, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import {
  ArrowEnterLeftRegular,
  BackspaceRegular,
  DismissRegular,
  KeyboardShiftRegular,
  LineHorizontal3Regular,
} from '@fluentui/react-icons'
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactElement } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import { useSearchStore, type KeyboardField } from '../stores/useSearchStore'

/**
 * Teclado na tela pra digitar com o controle — na busca (Y) ou num campo de
 * texto confirmado com o A (`useMenuNav`), como o nome do perfil no
 * onboarding (o Fluent não tem
 * componente de teclado, e o teclado virtual do Windows é feito pra toque e
 * não existe no Linux). Na cara do teclado do Windows 11 com controle (print
 * do usuário): barra de título com alça e fechar, 4 linhas — letras com os
 * números de dica no canto, apagar, Enter, ⇧, &123 e espaço largos — e o
 * glifo do botão do controle no canto de cada tecla que tem atalho. A tecla
 * em foco usa o anel padrão do app.
 *
 * Botões (vêm do `useMenuNav` via `useSearchStore.keyboardNav`, enquanto o
 * teclado está aberto ele recebe os pulsos antes da navegação normal):
 * A digita a tecla em foco · B apaga (com o campo vazio, fecha) · X espaço ·
 * Y maiúsculas/minúsculas · ☰ (Menu) conclui e fecha. A biblioteca filtra
 * enquanto se digita (a busca é a mesma do teclado físico).
 */

type Action = 'shift' | 'space' | 'back' | 'done' | 'layer'
type KeyDef = { char: string; hint?: string; span?: number } | { action: Action; span: number }

/** 12 colunas por linha; tecla de letra = 1, especiais = `span`. */
const COLS = 12

const LETTERS: KeyDef[][] = [
  [
    ...'qwertyuiop'.split('').map((c, i) => ({ char: c, hint: String((i + 1) % 10) })),
    { action: 'back', span: 2 },
  ],
  [...'asdfghjklç'.split('').map((c) => ({ char: c })), { action: 'done', span: 2 }],
  [{ action: 'shift', span: 2 }, ...'zxcvbnm,.-'.split('').map((c) => ({ char: c }))],
  [{ action: 'layer', span: 2 }, { action: 'space', span: 10 }],
]

/** Camada "&123": números, acentos e símbolos. */
const SYMBOLS: KeyDef[][] = [
  [...'1234567890'.split('').map((c) => ({ char: c })), { action: 'back', span: 2 }],
  [...'áàãâéêíóõú'.split('').map((c) => ({ char: c })), { action: 'done', span: 2 }],
  [{ action: 'shift', span: 2 }, ..."':!?&()+/ü".split('').map((c) => ({ char: c }))],
  [{ action: 'layer', span: 2 }, { action: 'space', span: 10 }],
]

const span = (k: KeyDef) => ('action' in k ? k.span : (k.span ?? 1))

/**
 * Escreve num campo controlado pelo React: o setter nativo de `value` e um
 * evento `input` que borbulha — é o que o React escuta pra chamar o
 * `onChange` (atribuir `el.value` direto ele ignora). Corta no `maxLength`.
 */
function writeField(el: KeyboardField, value: string) {
  const v = el.maxLength > 0 ? Array.from(value).slice(0, el.maxLength).join('') : value
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype
  Object.getOwnPropertyDescriptor(proto, 'value')?.set?.call(el, v)
  el.dispatchEvent(new Event('input', { bubbles: true }))
}

/** Coluna (0–11) onde cada tecla começa, por linha. */
const starts = (rows: KeyDef[][]) =>
  rows.map((r) => {
    let c = 0
    return r.map((k) => {
      const at = c
      c += span(k)
      return at
    })
  })

/** Glifo do botão do controle que faz o mesmo que a tecla. */
const PAD_GLYPH: Partial<Record<Action, string>> = { back: 'B', space: 'X', shift: 'Y', done: 'MENU' }

const useStyles = makeStyles({
  // Painel flutuante preso embaixo, no meio da área de conteúdo, como o
  // teclado do Windows: superfície do tema com borda fina e cantos de 8,
  // sombra da escala do Fluent 2 › Elevation pra "bottom sheets" (`shadow28`).
  panel: {
    position: 'fixed',
    left: '50%',
    bottom: '24px',
    transform: 'translateX(-50%)',
    zIndex: 60,
    width: 'min(800px, calc(100vw - 120px))',
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusXLarge,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground1,
    boxShadow: tokens.shadow28,
    display: 'flex',
    flexDirection: 'column',
    animationName: { from: { opacity: 0, transform: 'translate(-50%, 12px)' }, to: { opacity: 1, transform: 'translate(-50%, 0)' } },
    animationDuration: tokens.durationNormal,
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'backwards',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  // Barra de título: alça no meio (decorativa) e fechar à direita, com uma
  // linha embaixo — a do teclado do Windows.
  titleBar: {
    position: 'relative',
    height: '32px',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'flex-end',
    paddingRight: tokens.spacingHorizontalXS,
    borderBottom: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  grip: {
    position: 'absolute',
    left: '50%',
    top: '50%',
    transform: 'translate(-50%, -50%)',
    width: '44px',
    height: '3px',
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: tokens.colorNeutralForeground3,
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: `repeat(${COLS}, minmax(0, 1fr))`,
    // 8 px entre as teclas: o anel de foco vai 6 px pra fora (3 de vão + 3
    // de traço) e com menos a tecla vizinha cobria o anel
    gap: tokens.spacingHorizontalS,
    padding: tokens.spacingHorizontalS,
  },
  key: {
    position: 'relative',
    minWidth: '0 !important',
    // o Fluent trava botão só-ícone em 32 px (`max-width`); aqui a tecla
    // ocupa a(s) coluna(s) da grade
    maxWidth: 'none !important',
    width: '100%',
    height: '52px',
    padding: '0 !important',
    fontSize: tokens.fontSizeBase500,
    fontWeight: `${tokens.fontWeightRegular} !important`,
    backgroundColor: `${tokens.colorNeutralBackground3} !important`,
    border: 'none !important',
    ':hover': { backgroundColor: `${tokens.colorNeutralBackground3Hover} !important` },
    ':active': { backgroundColor: `${tokens.colorNeutralBackground3Pressed} !important` },
    // a tecla em foco por cima das vizinhas (o brilho do anel não some atrás)
    // e clara, como o card de jogo
    ':focus': {
      zIndex: 1,
      backgroundColor: `${tokens.colorNeutralBackground3Hover} !important`,
    },
    '& .fui-Button__icon': { fontSize: '20px', width: '20px', height: '20px' },
  },
  // tecla especial (apagar, Enter, ⇧, &123): rótulo menor
  special: { fontSize: tokens.fontSizeBase300 },
  on: {
    backgroundColor: 'var(--reemuBrandSolid) !important',
    color: 'var(--reemuOnBrand) !important',
  },
  // número de dica no canto de cima à esquerda (o "1" do Q no Windows)
  hint: {
    position: 'absolute',
    top: '4px',
    left: '7px',
    fontSize: tokens.fontSizeBase100,
    lineHeight: 1,
    color: tokens.colorNeutralForeground3,
  },
  // glifo do botão do controle no canto de cima à direita
  pad: {
    position: 'absolute',
    top: '5px',
    right: '6px',
    minWidth: '14px',
    height: '14px',
    paddingLeft: '2px',
    paddingRight: '2px',
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusCircular,
    display: 'grid',
    placeItems: 'center',
    fontSize: '9px',
    fontWeight: tokens.fontWeightBold,
    lineHeight: 1,
    backgroundColor: tokens.colorNeutralForeground2,
    color: tokens.colorNeutralBackground1,
    '& svg': { width: '10px', height: '10px' },
  },
})

export function OnScreenKeyboard() {
  const { t } = useTranslation()
  const s = useStyles()
  const open = useSearchStore((st) => st.keyboard)
  const setQuery = useSearchStore((st) => st.setQuery)
  const setKeyboard = useSearchStore((st) => st.setKeyboard)
  const setKeyboardNav = useSearchStore((st) => st.setKeyboardNav)
  const [upper, setUpper] = useState(false)
  const [symbols, setSymbols] = useState(false)
  // tecla em foco: linha 0–3 e índice dela na linha
  const [pos, setPos] = useState({ row: 0, idx: 0 })
  const keyRefs = useRef(new Map<string, HTMLButtonElement>())
  const posRef = useRef(pos)
  useEffect(() => {
    posRef.current = pos
  }, [pos])

  // as duas camadas têm a mesma geometria: a posição vale nas duas
  const rows = symbols ? SYMBOLS : LETTERS
  const rowsRef = useRef(rows)
  useEffect(() => {
    rowsRef.current = rows
  }, [rows])

  // Texto editado: o do campo escolhido, ou a busca.
  const read = useCallback(() => {
    const st = useSearchStore.getState()
    return st.keyboardTarget ? st.keyboardTarget.value : st.query
  }, [])
  const write = useCallback(
    (v: string) => {
      const field = useSearchStore.getState().keyboardTarget
      if (field) writeField(field, v)
      else setQuery(v)
    },
    [setQuery],
  )
  const type = useCallback((txt: string) => write(read() + txt), [read, write])
  const backspace = useCallback(() => write(Array.from(read()).slice(0, -1).join('')), [read, write])
  // Fecha e devolve o foco: ao campo editado, ou (busca) ao primeiro
  // resultado / à busca, pra o controle continuar navegando dali.
  const close = useCallback(() => {
    const field = useSearchStore.getState().keyboardTarget
    setKeyboard(false)
    requestAnimationFrame(() => {
      const next =
        field ??
        document.querySelector<HTMLElement>('[data-art]')?.closest<HTMLElement>('[tabindex]') ??
        document.querySelector<HTMLElement>('.fui-SearchBox input')
      next?.focus()
    })
  }, [setKeyboard])

  const press = useCallback(
    (row: number, idx: number) => {
      const k = rowsRef.current[row][idx]
      if ('char' in k) {
        type(upper ? k.char.toLocaleUpperCase('pt-BR') : k.char)
        return
      }
      if (k.action === 'shift') setUpper((u) => !u)
      else if (k.action === 'layer') setSymbols((v) => !v)
      else if (k.action === 'space') type(' ')
      else if (k.action === 'back') backspace()
      else close()
    },
    [upper, type, backspace, close],
  )

  // Direcional: anda na grade (sem sair do teclado). Na vertical vai pra
  // tecla da linha vizinha que cobre o meio da tecla atual.
  const move = useCallback((dir: string) => {
    setPos(({ row, idx }) => {
      const r = rowsRef.current
      if (dir === 'left' || dir === 'right') {
        const n = r[row].length
        return { row, idx: (idx + (dir === 'right' ? 1 : -1) + n) % n }
      }
      const next = row + (dir === 'down' ? 1 : -1)
      if (next < 0 || next >= r.length) return { row, idx }
      const st = starts(r)
      const mid = st[row][idx] + span(r[row][idx]) / 2
      const target = st[next].findIndex((c, i) => mid >= c && mid < c + span(r[next][i]))
      return { row: next, idx: target < 0 ? r[next].length - 1 : target }
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
          const { row, idx } = posRef.current
          press(row, idx)
          return true
        }
        case 'back':
          if (read()) backspace()
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
  }, [open, move, press, backspace, close, type, read, setKeyboardNav])

  // A tecla em foco recebe o foco do DOM (anel padrão do app).
  useLayoutEffect(() => {
    if (!open) return
    keyRefs.current.get(`${pos.row}:${pos.idx}`)?.focus({ preventScroll: true })
  }, [open, pos, symbols])

  if (!open) return null

  const keyRef = (row: number, idx: number) => (el: HTMLButtonElement | null) => {
    if (el) keyRefs.current.set(`${row}:${idx}`, el)
    else keyRefs.current.delete(`${row}:${idx}`)
  }

  const label = (a: Action) =>
    a === 'shift'
      ? t('osk.shift')
      : a === 'space'
        ? t('osk.space')
        : a === 'back'
          ? t('osk.backspace')
          : a === 'done'
            ? t('osk.done')
            : symbols
              ? t('osk.letters')
              : t('osk.symbols')
  const icon = (a: Action): ReactElement | undefined =>
    a === 'shift' ? (
      <KeyboardShiftRegular />
    ) : a === 'back' ? (
      <BackspaceRegular />
    ) : a === 'done' ? (
      <ArrowEnterLeftRegular />
    ) : undefined
  const padGlyph = (a: Action) => {
    const g = PAD_GLYPH[a]
    if (!g) return null
    return (
      <span className={s.pad} aria-hidden>
        {g === 'MENU' ? <LineHorizontal3Regular /> : g}
      </span>
    )
  }

  const panel = (
    <div className={s.panel} role="dialog" aria-label={t('osk.title')} data-nav-skip>
      <div className={s.titleBar}>
        <span className={s.grip} aria-hidden />
        <Button
          appearance="subtle"
          size="small"
          icon={<DismissRegular />}
          aria-label={t('osk.close')}
          tabIndex={-1}
          onClick={close}
        />
      </div>
      <div className={s.grid} role="group">
        {rows.map((r, ri) =>
          r.map((k, ki) => {
            const onClick = () => {
              setPos({ row: ri, idx: ki })
              press(ri, ki)
            }
            if ('char' in k)
              return (
                <Button key={`${ri}:${ki}`} ref={keyRef(ri, ki)} className={s.key} tabIndex={-1} onClick={onClick}>
                  {k.hint && !symbols && (
                    <span className={s.hint} aria-hidden>
                      {k.hint}
                    </span>
                  )}
                  {/* rótulo em maiúscula, como no teclado do Windows; o que
                      sai depende do ⇧ */}
                  {k.char.toLocaleUpperCase('pt-BR')}
                </Button>
              )
            return (
              <Button
                key={`${ri}:${k.action}`}
                ref={keyRef(ri, ki)}
                className={mergeClasses(s.key, s.special, k.action === 'shift' && upper && s.on)}
                style={{ gridColumn: `span ${k.span}` }}
                tabIndex={-1}
                aria-label={label(k.action)}
                aria-pressed={k.action === 'shift' ? upper : undefined}
                icon={icon(k.action)}
                onClick={onClick}
              >
                {padGlyph(k.action)}
                {k.action === 'layer' ? (symbols ? 'abc' : '&123') : undefined}
              </Button>
            )
          }),
        )}
      </div>
    </div>
  )
  // Com um modal aberto (ex.: o onboarding), o teclado vai DENTRO dele: o
  // Dialog do Fluent fica num portal por cima do app e prende o foco no
  // próprio conteúdo — fora dele as teclas ficariam atrás e sem foco.
  // O OverlayDrawer (menu de pausa) também é modal e prende o foco.
  const surface = document.querySelector<HTMLElement>('.fui-DialogSurface, .fui-OverlayDrawer')
  return surface ? createPortal(panel, surface) : panel
}
