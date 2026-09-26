import {
  Badge,
  Button,
  Caption1,
  Subtitle2,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ArrowResetRegular } from '@fluentui/react-icons'
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

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  head: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: tokens.spacingHorizontalM },
  groups: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
    gap: tokens.spacingHorizontalXL,
  },
  group: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  row: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    padding: `${tokens.spacingVerticalXXS} ${tokens.spacingHorizontalS}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  name: { minWidth: '88px' },
  key: { flex: 1, minWidth: 0 },
  none: { color: tokens.colorNeutralForeground3 },
})

// Alvos na ordem do backend (`input_desktop::keymap::TARGETS`), por grupo.
const GROUPS = [
  { id: 'dpad', targets: ['Up', 'Down', 'Left', 'Right'] },
  {
    id: 'buttons',
    targets: ['B', 'A', 'Y', 'X', 'L1', 'R1', 'L2', 'R2', 'L3', 'R3', 'Start', 'Select'],
  },
  { id: 'lstick', targets: ['LStickUp', 'LStickDown', 'LStickLeft', 'LStickRight'] },
  { id: 'rstick', targets: ['RStickUp', 'RStickDown', 'RStickLeft', 'RStickRight'] },
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
        <Subtitle2>{t('keyboard.title')}</Subtitle2>
        <Button
          size="small"
          appearance="subtle"
          icon={<ArrowResetRegular />}
          disabled={reset.isPending}
          onClick={() => reset.mutate()}
        >
          {t('keyboard.reset')}
        </Button>
      </div>
      <Caption1>{t('keyboard.intro')}</Caption1>
      <div className={s.groups}>
        {GROUPS.map((g) => (
          <div key={g.id} className={s.group}>
            <Text weight="semibold">{t(`keyboard.groups.${g.id}`)}</Text>
            {g.targets.map((target) => {
              const b = byTarget.get(target)
              const code = b?.code ?? ''
              return (
                <div key={target} className={s.row}>
                  <Text className={s.name}>{targetLabel(target, t)}</Text>
                  <span className={s.key}>
                    {capturing === target ? (
                      <Caption1>{t('keyboard.press')}</Caption1>
                    ) : code ? (
                      <Text weight="semibold">{keyLabel(code, t)}</Text>
                    ) : (
                      <Caption1 className={s.none}>{t('keyboard.none')}</Caption1>
                    )}
                    {b && !b.isDefault && capturing !== target && (
                      <Badge appearance="tint" size="small" style={{ marginLeft: 8 }}>
                        {t('keyboard.custom')}
                      </Badge>
                    )}
                  </span>
                  <Button
                    size="small"
                    disabled={set.isPending}
                    onClick={() => setCapturing(capturing === target ? null : target)}
                  >
                    {t('keyboard.change')}
                  </Button>
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={set.isPending || !code}
                    onClick={() => set.mutate({ target, code: '' })}
                  >
                    {t('keyboard.clear')}
                  </Button>
                </div>
              )
            })}
          </div>
        ))}
      </div>
    </div>
  )
}
