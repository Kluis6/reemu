import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { alvanistaCachedStatus, getProfile, onAlvanistaStatus } from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

/** Estado do Alvanista guardado no aparelho (sem rede). Quem muda a conexão
 *  (conectar, desconectar, um status que trouxe outra coisa) invalida esta
 *  chave, e todo lugar que mostra o perfil acompanha. */
export const ALVANISTA_CACHED_KEY = ['alvanista-cached'] as const

/** O perfil que o ReEmu mostra: a conta do Alvanista, se conectada, ou o
 *  perfil local. Serve ao `ProfileAvatar` como está. */
export interface EffectiveProfile {
  name: string
  /** `preset:N` ou `file`, como no perfil local. */
  avatar: string
  /** Imagem do avatar do Alvanista; sem ela, vale o `avatar`. */
  avatarUrl: string | null
  bio: string | null
  /** Capa do Alvanista, para o banner do `ProfileDrawer`; o perfil local não
   *  tem capa (o drawer usa as cores do tema). */
  bannerUrl: string | null
  source: 'alvanista' | 'local'
}

/**
 * O perfil em todo lugar que o ReEmu mostra o perfil (FR-11): conectado ao
 * Alvanista, o nome, o avatar, a bio e a capa de lá (sem avatar, o preset do
 * ReEmu; sem bio, nenhuma); fora disso, o perfil local. O perfil local nunca é gravado aqui e
 * volta sozinho ao desconectar. `undefined` até as duas leituras terminarem,
 * para não piscar o perfil local antes do Alvanista. Sem banco, o estado do
 * Alvanista falha e vale o perfil local.
 */
export function useEffectiveProfile(): EffectiveProfile | undefined {
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
  const alva = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })
  if (profile.isPending || alva.isPending) return undefined
  const st = alva.data
  // Conectado sem nome guardado ainda (o `/me` nunca respondeu): o perfil
  // local até o espelho chegar, em vez de "Conta do Alvanista".
  if (st?.connected && st.username) {
    return {
      name: st.username,
      avatar: 'preset:1',
      avatarUrl: st.avatar_url,
      bio: st.bio,
      bannerUrl: st.hero_url,
      source: 'alvanista',
    }
  }
  if (!profile.data) return undefined
  return {
    name: profile.data.name,
    avatar: profile.data.avatar,
    avatarUrl: null,
    bio: profile.data.bio,
    bannerUrl: null,
    source: 'local',
  }
}

/**
 * Recebe a renovação que o app faz sozinho ao abrir (`alvanista-status`):
 * relê o estado guardado, para o perfil mostrado acompanhar, e avisa quando
 * a conexão acabou no site (FR-12). Montado uma vez, no `AppShell`.
 */
export function useAlvanistaStatusEvents() {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  useEffect(() => {
    let disposed = false
    let dispose: (() => void) | undefined
    void onAlvanistaStatus((st) => {
      void qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY })
      if (st.ended) push(sysToast(t('alvanista.ended'), 'Warning'))
    }).then((d) => {
      if (disposed) d()
      else dispose = d
    })
    return () => {
      disposed = true
      dispose?.()
    }
  }, [qc, push, t])
}
