import {
  Badge,
  Caption1,
  Subtitle2,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ArrowResetRegular, DeleteRegular, DismissRegular, EditRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { errorToast, sysToast } from '../lib/toast'
import {
  listKeyboardBindings,
  resetKeyboardBindings,
  setKeyboardBinding,
  type KeyboardBinding,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { ListIconAction } from './ListIconAction'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  head: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: tokens.spacingHorizontalM },
  heading: { margin: 0 },
  groups: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(min(100%, 300px), 1fr))',
    gap: tokens.spacingHorizontalXL,
  },
  column: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, minWidth: 0 },
  group: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  row: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalL,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  binding: { display: 'grid', gap: tokens.spacingVerticalXXS, minWidth: 0 },
  key: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    overflowWrap: 'anywhere',
    color: tokens.colorNeutralForeground2,
  },
  actions: { display: 'flex', gap: tokens.spacingHorizontalS, flexShrink: 0 },
})

const GROUP_COLUMNS = [
  [
    { id: 'dpad', targets: ['Up', 'Down', 'Left', 'Right'] },
    { id: 'lstick', targets: ['LStickUp', 'LStickDown', 'LStickLeft', 'LStickRight'] },
    { id: 'rstick', targets: ['RStickUp', 'RStickDown', 'RStickLeft', 'RStickRight'] },
  ],
  [{
    id: 'buttons',
    targets: ['B', 'A', 'Y', 'X', 'L1', 'R1', 'L2', 'R2', 'L3', 'R3', 'Start', 'Select'],
  }],
] as const

const UI_KEYS = new Set(['Tab', 'F5', 'F12'])

const DIRS = { Up: 'up', Down: 'down', Left: 'left', Right: 'right' } as const

/** Nome do alvo na tela: direções traduzidas; botões pelo nome do RetroPad. */
function targetLabel(target: string, t: TFunction): string {
  const dir = target.replace(/^[LR]Stick/, '') as keyof typeof DIRS
  return dir in DIRS ? t(`keyboard.dirs.${DIRS[dir]}`) : target
}

const NAMED_KEYS = {
  Space: 'space',
  ShiftLeft: 'shiftLeft',
  ShiftRight: 'shiftRight',
  ControlLeft: 'ctrlLeft',
  ControlRight: 'ctrlRight',
  AltLeft: 'altLeft',
  AltRight: 'altRight',
} as const
const ARROWS: Record<string, string> = { ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→' }

/** `KeyboardEvent.code` → rótulo curto (a posição física, não o caractere). */
function keyLabel(code: string, t: TFunction): string {
  if (code in NAMED_KEYS) return t(`keyboard.keys.${NAMED_KEYS[code as keyof typeof NAMED_KEYS]}`)
  if (code in ARROWS) return ARROWS[code]
  const m = code.match(/^(?:Key|Digit)(.+)$/) ?? code.match(/^Numpad(.+)$/)
  if (m) return code.startsWith('Numpad') ? `Num ${m[1]}` : m[1]
  return code
}

/**
 * Configurações › Controles › Teclado: troca a tecla de cada alvo (botão do
 * RetroPad ou direção de analógico). A captura escuta o `keydown` na fase de
 * captura e para a propagação — a tecla não chega ao jogo nem à navegação.
 */
export function KeyboardBindings() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [capturing, setCapturing] = useState<string | null>(null)
  const list = useQuery({ queryKey: ['keyboard-bindings'], queryFn: listKeyboardBindings })
  const refresh = () => qc.invalidateQueries({ queryKey: ['keyboard-bindings'] })

  const set = useMutation({
    mutationFn: ({ target, code }: { target: string; code: string }) =>
      setKeyboardBinding(target, code),
    onSuccess: refresh,
    onError: (e) => push(errorToast(e, 'saveKeyboard')),
  })
  const reset = useMutation({
    mutationFn: resetKeyboardBindings,
    onSuccess: () => {
      refresh()
      push(sysToast(t('keyboard.resetDone'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'resetKeyboard')),
  })

  useEffect(() => {
    if (!capturing) return
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      // Tab/F5/F12 são da interface e nunca chegam ao jogo (`useKeyboardInput`)
      if (UI_KEYS.has(e.code)) return
      if (e.code !== 'Escape' && e.code) set.mutate({ target: capturing, code: e.code })
      setCapturing(null)
    }
    window.addEventListener('keydown', onKey, { capture: true })
    return () => window.removeEventListener('keydown', onKey, { capture: true })
    // `set` muda de identidade a cada render; só a captura importa aqui
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [capturing])

  const byTarget = new Map<string, KeyboardBinding>((list.data ?? []).map((b) => [b.target, b]))

  return (
    <div className={s.root}>
      <div className={s.head}>
        <Subtitle2 className={s.heading} as="h2">{t('keyboard.title')}</Subtitle2>
        <ListIconAction
          label={t('keyboard.reset')}
          icon={<ArrowResetRegular />}
          disabled={reset.isPending}
          onClick={() => reset.mutate()}
        />
      </div>
      <Caption1>{t('keyboard.intro')}</Caption1>
      <div className={s.groups}>
        {GROUP_COLUMNS.map((column) => (
          <div key={column[0].id} className={s.column}>
            {column.map((group) => (
              <section key={group.id} className={s.group} aria-labelledby={`keyboard-group-${group.id}`}>
                <Subtitle2 className={s.heading} id={`keyboard-group-${group.id}`} as="h3">
                  {t(`keyboard.groups.${group.id}`)}
                </Subtitle2>
                {group.targets.map((target) => {
                  const binding = byTarget.get(target)
                  const code = binding?.code ?? ''
                  const isCapturing = capturing === target
                  const label = targetLabel(target, t)
                  return (
                    <div key={target} className={s.row} role="group" aria-label={label}>
                      <div className={s.binding}>
                        <Text weight="semibold">{label}</Text>
                        <span className={s.key}>
                          {isCapturing ? (
                            <Caption1 role="status">{t('keyboard.press')}</Caption1>
                          ) : (
                            <Caption1>{code ? keyLabel(code, t) : t('keyboard.none')}</Caption1>
                          )}
                          {binding && !binding.isDefault && !isCapturing && (
                            <Badge appearance="tint" size="small">{t('keyboard.custom')}</Badge>
                          )}
                        </span>
                      </div>
                      <div className={s.actions}>
                        <ListIconAction
                          primary={isCapturing}
                          label={t(isCapturing ? 'common.cancel' : 'keyboard.change')}
                          icon={isCapturing ? <DismissRegular /> : <EditRegular />}
                          disabled={set.isPending}
                          onClick={() => setCapturing(isCapturing ? null : target)}
                        />
                        <ListIconAction
                          label={t('keyboard.clear')}
                          icon={<DeleteRegular />}
                          disabled={set.isPending || !code}
                          onClick={() => set.mutate({ target, code: '' })}
                        />
                      </div>
                    </div>
                  )
                })}
              </section>
            ))}
          </div>
        ))}
      </div>
    </div>
  )
}
