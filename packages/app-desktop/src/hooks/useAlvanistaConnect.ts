import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { sysToast } from '../lib/toast'
import { alvanistaCancel, alvanistaConnect, alvanistaDisconnect } from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { ALVANISTA_STATUS_KEY, DISCONNECTED } from './useAlvanistaStatusSync'
import { ALVANISTA_CACHED_KEY } from './useEffectiveProfile'

/** Códigos que o `alvanista_connect` devolve ao falhar. */
const OUTCOMES = ['denied', 'timeout', 'offline', 'keyring', 'failed'] as const
export type ConnectOutcome = (typeof OUTCOMES)[number]

function outcomeOf(err: unknown): ConnectOutcome | 'cancelled' {
  const code = typeof err === 'string' ? err : err instanceof Error ? err.message : ''
  if (code === 'cancelled') return 'cancelled'
  return (OUTCOMES as readonly string[]).includes(code) ? (code as ConnectOutcome) : 'failed'
}

/**
 * O fluxo "Entrar com Alvanista", dividido pelas telas: tentativa em curso,
 * resultado, cancelar. Nada roda antes de `connect` ser chamado; sair da tela
 * com uma conexão em andamento a cancela. `saves`: pede também o escopo do
 * cloud save no mesmo consentimento (a página Sincronização).
 */
export function useAlvanistaConnect({ saves = false }: { saves?: boolean } = {}) {
  const { i18n } = useTranslation()
  const qc = useQueryClient()
  const [connecting, setConnecting] = useState(false)
  const [outcome, setOutcome] = useState<ConnectOutcome | null>(null)
  // Só o resultado da tentativa mais recente conta (cancelar e tentar de
  // novo deixa a antiga rejeitando "cancelled" depois).
  const attempt = useRef(0)
  // Síncrono: um segundo clique antes do re-render não abre outra tentativa.
  const connectingRef = useRef(false)

  useEffect(
    () => () => {
      if (connectingRef.current) {
        attempt.current++
        void alvanistaCancel().catch(() => {})
      }
    },
    [],
  )

  const connect = useCallback(() => {
    if (connectingRef.current) return
    connectingRef.current = true
    const mine = ++attempt.current
    setOutcome(null)
    setConnecting(true)
    alvanistaConnect(i18n.language, saves)
      .then(async (next) => {
        if (mine !== attempt.current) return
        connectingRef.current = false
        // Uma leitura em voo ainda veria o estado antigo: descarta as duas.
        await Promise.all([
          qc.cancelQueries({ queryKey: ALVANISTA_STATUS_KEY }),
          qc.cancelQueries({ queryKey: ALVANISTA_CACHED_KEY }),
        ])
        qc.setQueryData(ALVANISTA_STATUS_KEY, next)
        // O topo e o corpo leem estados diferentes: os dois mudam juntos.
        qc.setQueryData(ALVANISTA_CACHED_KEY, next)
        void qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY })
        setConnecting(false)
      })
      .catch((err: unknown) => {
        if (mine !== attempt.current) return
        connectingRef.current = false
        const out = outcomeOf(err)
        setConnecting(false)
        setOutcome(out === 'cancelled' ? null : out)
      })
  }, [i18n.language, qc, saves])

  const cancel = useCallback(() => {
    void alvanistaCancel().catch(() => {})
  }, [])

  return { connect, cancel, connecting, outcome }
}

/** Desconectar: o status vira "desconectado" na hora (o cartão guardado some
 *  com ele) e o estado guardado é relido para o perfil local voltar. */
export function useAlvanistaDisconnect() {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const [disconnecting, setDisconnecting] = useState(false)
  const ref = useRef(false)

  const disconnect = useCallback(async () => {
    // Síncrono: um segundo clique não desconecta de novo (nem avisa de novo).
    if (ref.current) return
    ref.current = true
    setDisconnecting(true)
    try {
      await alvanistaDisconnect()
      // Um status em voo ainda veria a sessão antiga: descarta.
      await qc.cancelQueries({ queryKey: ALVANISTA_STATUS_KEY })
      qc.setQueryData(ALVANISTA_STATUS_KEY, DISCONNECTED)
      await qc.cancelQueries({ queryKey: ALVANISTA_CACHED_KEY })
      qc.setQueryData(ALVANISTA_CACHED_KEY, DISCONNECTED)
      void qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY })
      push(sysToast(t('alvanista.disconnected'), 'Success'))
    } catch {
      push(sysToast(t('alvanista.disconnectFailed'), 'Error'))
    } finally {
      ref.current = false
      setDisconnecting(false)
    }
  }, [push, qc, t])

  return { disconnect, disconnecting }
}
