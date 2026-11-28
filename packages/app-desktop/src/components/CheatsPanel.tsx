import {
  Button,
  Caption1,
  Field,
  Input,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  MessageBarTitle,
  Switch,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  AddRegular,
  ArrowDownloadRegular,
  CheckmarkRegular,
  DeleteRegular,
  DismissRegular,
  EditRegular,
  FolderOpenRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { RA_STATUS_KEY } from '../hooks/useRetroAchievements'
import {
  addCheat,
  deleteCheat,
  downloadLibretroCheats,
  importCheatsCht,
  listCheats,
  pickChtFile,
  raSetPreferences,
  raStatus,
  updateCheat,
  type Cheat,
} from '../lib/tauri'
import { errorToast, sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalM,
    width: '100%',
  },
  hint: { color: tokens.colorNeutralForeground3 },
  toolbar: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
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
  editRow: {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr',
    columnGap: tokens.spacingHorizontalS,
    rowGap: tokens.spacingVerticalXS,
    paddingTop: tokens.spacingVerticalXS,
    paddingBottom: tokens.spacingVerticalXS,
  },
  editActions: {
    gridColumn: '1 / -1',
    display: 'flex',
    columnGap: tokens.spacingHorizontalS,
  },
  form: {
    display: 'grid',
    gridTemplateColumns: '1fr 1fr',
    columnGap: tokens.spacingHorizontalS,
    rowGap: tokens.spacingVerticalS,
    alignItems: 'end',
  },
  addBtn: { gridColumn: '1 / -1', justifySelf: 'start' },
})

/**
 * Cheats por código do jogo aberto, no menu de pausa (docs/ai-context/
 * 17-cheats-rewind.md, fase 1). Ligar, desligar, apagar e acrescentar valem na
 * hora: o app manda a lista pro core (`retro_cheat_set`). Quem entende o
 * código é o core — o aviso de cima diz isso.
 */
export function CheatsPanel({ romId }: { romId: string }) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [description, setDescription] = useState('')
  const [code, setCode] = useState('')
  const [editingId, setEditingId] = useState<number | null>(null)
  const [editDesc, setEditDesc] = useState('')
  const [editCode, setEditCode] = useState('')
  const key = ['cheats', romId]
  const list = useQuery({ queryKey: key, queryFn: () => listCheats(romId), retry: false })
  const ra = useQuery({ queryKey: RA_STATUS_KEY, queryFn: raStatus, retry: false })
  const hardcoreActive = Boolean(ra.data?.connected && ra.data?.enabled && ra.data?.hardcore)
  const refresh = () => qc.invalidateQueries({ queryKey: key })

  const exitHardcore = useMutation({
    mutationFn: () => raSetPreferences({ hardcore: false }),
    onSuccess: (st) => {
      qc.setQueryData(RA_STATUS_KEY, st)
      void refresh()
    },
    onError: (e) => push(errorToast(e, 'raSavePrefs')),
  })
  const toggle = useMutation({
    mutationFn: (c: Cheat) => updateCheat(c.id, c.description, c.code, !c.enabled),
    onSuccess: refresh,
    onError: (e) => push(errorToast(e, 'saveCheat')),
  })
  const saveEdit = useMutation({
    mutationFn: (c: Cheat) =>
      updateCheat(c.id, editDesc, editCode, hardcoreActive ? false : c.enabled),
    onSuccess: () => {
      setEditingId(null)
      void refresh()
    },
    onError: (e) => push(errorToast(e, 'saveCheat')),
  })
  const remove = useMutation({
    mutationFn: (id: number) => deleteCheat(id),
    onSuccess: (_, id) => {
      if (editingId === id) setEditingId(null)
      void refresh()
    },
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
  const download = useMutation({
    mutationFn: () => downloadLibretroCheats(romId),
    onSuccess: (count) => {
      if (count > 0) {
        push(sysToast(t('cheats.importedCount', { count }), 'Success'))
        void refresh()
      } else {
        const msg =
          (list.data?.length ?? 0) === 0 ? t('cheats.noneFoundRemote') : t('cheats.noNewRemote')
        push(sysToast(msg, 'Info'))
      }
    },
    onError: (e) => push(errorToast(e, 'downloadCheats')),
  })
  const importFile = useMutation({
    mutationFn: async () => {
      const path = await pickChtFile()
      if (!path) return null
      return importCheatsCht(romId, path)
    },
    onSuccess: (count) => {
      if (count === null) return
      if (count > 0) {
        push(sysToast(t('cheats.importedCount', { count }), 'Success'))
        void refresh()
      } else {
        push(sysToast(t('cheats.noNewFile'), 'Info'))
      }
    },
    onError: (e) => push(errorToast(e, 'importCheats')),
  })

  const cheats = list.data ?? []
  return (
    <div className={s.root}>
      {hardcoreActive && (
        <MessageBar intent="warning">
          <MessageBarBody>
            <MessageBarTitle>{t('cheats.hardcoreTitle')}</MessageBarTitle>
            {t('cheats.hardcoreNotice')}
          </MessageBarBody>
          <MessageBarActions>
            <Button disabled={exitHardcore.isPending} onClick={() => exitHardcore.mutate()}>
              {t('cheats.disableHardcore')}
            </Button>
          </MessageBarActions>
        </MessageBar>
      )}

      <Caption1 className={s.hint}>{t('cheats.hint')}</Caption1>

      <div className={s.toolbar}>
        <Button
          appearance="secondary"
          icon={<ArrowDownloadRegular />}
          disabled={download.isPending}
          onClick={() => download.mutate()}
        >
          {download.isPending ? t('cheats.downloadingLibretro') : t('cheats.downloadLibretro')}
        </Button>
        <Button
          appearance="secondary"
          icon={<FolderOpenRegular />}
          disabled={importFile.isPending}
          onClick={() => importFile.mutate()}
        >
          {t('cheats.importCht')}
        </Button>
      </div>

      {cheats.length === 0 ? (
        <Text>{t('cheats.empty')}</Text>
      ) : (
        <div className={s.list} role="list" aria-label={t('cheats.title')}>
          {cheats.map((c) =>
            editingId === c.id ? (
              <div key={c.id} className={s.editRow} role="listitem">
                <Field label={t('cheats.description')}>
                  <Input
                    appearance="filled-darker"
                    value={editDesc}
                    maxLength={80}
                    placeholder={t('cheats.descriptionPlaceholder')}
                    onChange={(_, d) => setEditDesc(d.value)}
                  />
                </Field>
                <Field label={t('cheats.code')}>
                  <Input
                    appearance="filled-darker"
                    value={editCode}
                    maxLength={400}
                    placeholder={t('cheats.codePlaceholder')}
                    onChange={(_, d) => setEditCode(d.value)}
                  />
                </Field>
                <div className={s.editActions}>
                  <Button
                    appearance="primary"
                    icon={<CheckmarkRegular />}
                    disabled={!editCode.trim() || saveEdit.isPending}
                    onClick={() => saveEdit.mutate(c)}
                  >
                    {t('cheats.saveEdit')}
                  </Button>
                  <Button
                    appearance="secondary"
                    icon={<DismissRegular />}
                    disabled={saveEdit.isPending}
                    onClick={() => setEditingId(null)}
                  >
                    {t('cheats.cancelEdit')}
                  </Button>
                </div>
              </div>
            ) : (
              <div key={c.id} className={s.row} role="listitem">
                <Switch
                  checked={!hardcoreActive && c.enabled}
                  disabled={hardcoreActive || toggle.isPending}
                  aria-label={c.description}
                  onChange={() => toggle.mutate(c)}
                />
                <span className={s.rowText}>
                  <Text>{c.description}</Text>
                  {c.description !== c.code && <Caption1 className={s.code}>{c.code}</Caption1>}
                </span>
                <Button
                  appearance="subtle"
                  icon={<EditRegular />}
                  aria-label={t('cheats.edit', { name: c.description })}
                  onClick={() => {
                    setEditingId(c.id)
                    setEditDesc(c.description)
                    setEditCode(c.code)
                  }}
                />
                <Button
                  appearance="subtle"
                  icon={<DeleteRegular />}
                  aria-label={t('cheats.remove', { name: c.description })}
                  disabled={remove.isPending}
                  onClick={() => remove.mutate(c.id)}
                />
              </div>
            ),
          )}
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
          className={s.addBtn}
          icon={<AddRegular />}
          disabled={hardcoreActive || !code.trim() || add.isPending}
          onClick={() => add.mutate()}
        >
          {t('cheats.add')}
        </Button>
      </div>

      <Caption1 className={s.hint}>{t('cheats.attribution')}</Caption1>
    </div>
  )
}
