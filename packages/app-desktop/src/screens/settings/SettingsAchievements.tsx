import {
  Badge,
  Body1,
  Button,
  Caption1,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { SignOutRegular, TrophyRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { LoadingState } from '../../components/EmptyState'
import { BlockGroup, FieldBlock, ToggleBlock } from '../../components/SettingBlock'
import { RA_STATUS_KEY } from '../../hooks/useRetroAchievements'
import { raLogin, raLogout, raSetPreferences, raStatus } from '../../lib/tauri'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
  },
  dim: {
    color: tokens.colorNeutralForeground3,
  },
  accountCard: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
    padding: tokens.spacingHorizontalL,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
  },
  accountIdentity: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    minWidth: 0,
  },
  avatar: {
    width: '56px',
    height: '56px',
    borderRadius: tokens.borderRadiusCircular,
    objectFit: 'cover',
    flexShrink: 0,
    backgroundColor: tokens.colorNeutralBackground4,
  },
  avatarFallback: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '56px',
    height: '56px',
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: tokens.colorNeutralBackground4,
    color: 'var(--reemuBrandAccent)',
    fontSize: '24px',
    flexShrink: 0,
  },
  accountMeta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
    minWidth: 0,
  },
  scores: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  form: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
  },
  submit: {
    alignSelf: 'flex-start',
  },
})

export function SettingsAchievements() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const status = useQuery({
    queryKey: RA_STATUS_KEY,
    queryFn: raStatus,
    retry: false,
  })

  const [username, setUsername] = useState<string | null>(null)
  const [password, setPassword] = useState('')

  const effectiveUsername =
    username ?? status.data?.username ?? status.data?.linkedAlvanistaUsername ?? ''

  const login = useMutation({
    mutationFn: ({ u, p }: { u: string; p: string }) => raLogin(u, p),
    onSuccess: (next) => {
      setPassword('')
      qc.setQueryData(RA_STATUS_KEY, next)
      void qc.invalidateQueries({ queryKey: ['ra-game'] })
      push(
        sysToast(
          t('achievements.signedInToast', {
            user: next.displayName ?? next.username ?? effectiveUsername,
          }),
          'Success',
        ),
      )
    },
    onError: (e) => {
      setPassword('')
      push(errorToast(e, 'raLogin'))
    },
  })

  const logout = useMutation({
    mutationFn: () => raLogout(),
    onSuccess: (next) => {
      qc.setQueryData(RA_STATUS_KEY, next)
      void qc.invalidateQueries({ queryKey: ['ra-game'] })
      push(sysToast(t('achievements.signedOutToast'), 'Info'))
    },
    onError: (e) => push(errorToast(e, 'raLogout')),
  })

  const prefs = useMutation({
    mutationFn: (patch: Parameters<typeof raSetPreferences>[0]) => raSetPreferences(patch),
    onSuccess: (next) => {
      qc.setQueryData(RA_STATUS_KEY, next)
    },
    onError: (e) => push(errorToast(e, 'raSavePrefs')),
  })

  if (status.isLoading) return <LoadingState />
  if (status.isError || !status.data) {
    return <Body1>{t('audio.unavailable')}</Body1>
  }

  const st = status.data

  const onSubmit = (e: FormEvent) => {
    e.preventDefault()
    const u = effectiveUsername.trim()
    if (!u || !password || login.isPending) return
    login.mutate({ u, p: password })
  }

  return (
    <div className={s.root}>
      <Caption1 className={s.dim}>{t('achievements.intro')}</Caption1>

      {!st.connected ? (
        <form className={s.form} onSubmit={onSubmit}>
          {st.linkedAlvanistaUsername && (
            <MessageBar intent="info">
              <MessageBarBody>
                {t('achievements.suggestedFromAlvanista', {
                  user: st.linkedAlvanistaUsername,
                })}
              </MessageBarBody>
            </MessageBar>
          )}

          <BlockGroup>
            <FieldBlock>
              <Field label={t('achievements.username')}>
                <Input
                  appearance="filled-darker"
                  value={effectiveUsername}
                  autoComplete="username"
                  onChange={(_, d) => setUsername(d.value)}
                />
              </Field>
            </FieldBlock>
            <FieldBlock>
              <Field label={t('achievements.password')} hint={t('achievements.passwordHint')}>
                <Input
                  appearance="filled-darker"
                  type="password"
                  value={password}
                  autoComplete="current-password"
                  onChange={(_, d) => setPassword(d.value)}
                />
              </Field>
            </FieldBlock>
          </BlockGroup>

          <Button
            type="submit"
            appearance="primary"
            className={s.submit}
            disabled={login.isPending || !effectiveUsername.trim() || !password}
          >
            {login.isPending ? t('achievements.signingIn') : t('achievements.signIn')}
          </Button>
        </form>
      ) : (
        <>
          <div className={s.accountCard}>
            <div className={s.accountIdentity}>
              {st.avatarUrl ? (
                <img className={s.avatar} src={st.avatarUrl} alt="" />
              ) : (
                <span className={s.avatarFallback} aria-hidden>
                  <TrophyRegular />
                </span>
              )}
              <div className={s.accountMeta}>
                <Text weight="semibold" size={400}>
                  {t('achievements.connectedAs', {
                    user: st.displayName ?? st.username ?? '',
                  })}
                </Text>
                <div className={s.scores}>
                  <Badge appearance="tint" color="warning">
                    {t('achievements.scoreHardcore', { points: st.score })}
                  </Badge>
                  <Badge appearance="tint" color="informative">
                    {t('achievements.scoreSoftcore', { points: st.scoreSoftcore })}
                  </Badge>
                </div>
              </div>
            </div>
            <Button
              appearance="secondary"
              icon={<SignOutRegular />}
              disabled={logout.isPending}
              onClick={() => logout.mutate()}
            >
              {t('achievements.signOut')}
            </Button>
          </div>

          <BlockGroup title={t('achievements.prefsTitle')}>
            <ToggleBlock
              label={t('achievements.enabled')}
              description={t('achievements.enabledHint')}
              checked={st.enabled}
              busy={prefs.isPending}
              onChange={(checked) => prefs.mutate({ enabled: checked })}
            />
            <ToggleBlock
              label={t('achievements.hardcore')}
              description={t('achievements.hardcoreHint')}
              checked={st.hardcore}
              disabled={!st.enabled}
              busy={prefs.isPending}
              onChange={(checked) => prefs.mutate({ hardcore: checked })}
            />
            <ToggleBlock
              label={t('achievements.unofficial')}
              description={t('achievements.unofficialHint')}
              checked={st.unofficial}
              disabled={!st.enabled}
              busy={prefs.isPending}
              onChange={(checked) => prefs.mutate({ unofficial: checked })}
            />
            <ToggleBlock
              label={t('achievements.encore')}
              description={t('achievements.encoreHint')}
              checked={st.encore}
              disabled={!st.enabled}
              busy={prefs.isPending}
              onChange={(checked) => prefs.mutate({ encore: checked })}
            />
          </BlockGroup>

          {st.enabled && st.hardcore && (
            <MessageBar intent="info">
              <MessageBarBody>{t('achievements.hardcorePendingApproval')}</MessageBarBody>
            </MessageBar>
          )}

          <Caption1 className={s.dim}>{t('achievements.nextSessionNote')}</Caption1>
        </>
      )}
    </div>
  )
}
