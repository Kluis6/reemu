import {
  Button,
  Caption1,
  Field,
  Input,
  Switch,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { AddRegular, DeleteRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { addCheat, deleteCheat, listCheats, updateCheat, type Cheat } from '../lib/tauri'
import { errorToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalM,
    width: 'min(520px, 80vw)',
  },
  hint: { color: tokens.colorNeutralForeground3 },
  list: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalXS,
    maxHeight: '40vh',
    overflowY: 'auto',
    // espaço pro anel de foco não ser cortado pela rolagem
    padding: '6px',
    margin: '-6px',
  },
  row: {
    display: 'flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalS,
  },
  // o texto do cheat ocupa o que sobra; o código vem embaixo, menor
  rowText: { flexGrow: 1, minWidth: 0, display: 'flex', flexDirection: 'column' },
  code: {
    color: tokens.colorNeutralForeground3,
    fontFamily: tokens.fontFamilyMonospace,
    overflowWrap: 'anywhere',
  },
  form: {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr auto',
    columnGap: tokens.spacingHorizontalS,
    alignItems: 'end',
  },
})

/**
 * Cheats por código do jogo aberto, no menu de pausa (docs/ai-context/
 * 17-cheats-rewind.md, fase 1). Ligar, desligar, apagar e acrescentar valem na
 * hora: o app manda a lista pro core (`retro_cheat_set`). Quem entende o
 * código é o core — o aviso de cima diz isso.
 */
export function CheatsPanel({ romId, onBack }: { romId: string; onBack: () => void }) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [description, setDescription] = useState('')
  const [code, setCode] = useState('')
  const key = ['cheats', romId]
  const list = useQuery({ queryKey: key, queryFn: () => listCheats(romId), retry: false })
  const refresh = () => qc.invalidateQueries({ queryKey: key })

  const toggle = useMutation({
    mutationFn: (c: Cheat) => updateCheat(c.id, c.description, c.code, !c.enabled),
    onSuccess: refresh,
    onError: (e) => push(errorToast(e, 'saveCheat')),
  })
  const remove = useMutation({
    mutationFn: (id: number) => deleteCheat(id),
    onSuccess: refresh,
    onError: (e) => push(errorToast(e, 'deleteCheat')),
  })
  const add = useMutation({
    mutationFn: () => addCheat(romId, description, code),
    onSuccess: () => {
      setDescription('')
      setCode('')
      void refresh()
    },
    onError: (e) => push(errorToast(e, 'saveCheat')),
  })

  const cheats = list.data ?? []
  return (
    <div className={s.root}>
      <Caption1 className={s.hint}>{t('cheats.hint')}</Caption1>

      {cheats.length === 0 ? (
        <Text>{t('cheats.empty')}</Text>
      ) : (
        <div className={s.list} role="list" aria-label={t('cheats.title')}>
          {cheats.map((c) => (
            <div key={c.id} className={s.row} role="listitem">
              <Switch
                checked={c.enabled}
                disabled={toggle.isPending}
                aria-label={c.description}
                onChange={() => toggle.mutate(c)}
              />
              <span className={s.rowText}>
                <Text>{c.description}</Text>
                {c.description !== c.code && <Caption1 className={s.code}>{c.code}</Caption1>}
              </span>
              <Button
                appearance="subtle"
                icon={<DeleteRegular />}
                aria-label={t('cheats.remove', { name: c.description })}
                disabled={remove.isPending}
                onClick={() => remove.mutate(c.id)}
              />
            </div>
          ))}
        </div>
      )}

      <div className={s.form}>
        <Field label={t('cheats.description')}>
          <Input
            appearance="filled-darker"
            value={description}
            maxLength={80}
            placeholder={t('cheats.descriptionPlaceholder')}
            onChange={(_, d) => setDescription(d.value)}
          />
        </Field>
        <Field label={t('cheats.code')}>
          <Input
            appearance="filled-darker"
            value={code}
            maxLength={400}
            placeholder={t('cheats.codePlaceholder')}
            onChange={(_, d) => setCode(d.value)}
          />
        </Field>
        <Button
          appearance="primary"
          icon={<AddRegular />}
          disabled={!code.trim() || add.isPending}
          onClick={() => add.mutate()}
        >
          {t('cheats.add')}
        </Button>
      </div>

      <Button appearance="subtle" onClick={onBack}>
        {t('common.back')}
      </Button>
    </div>
  )
}
