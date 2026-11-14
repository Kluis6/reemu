import { useEffect, useMemo } from 'react'

/** Revogações adiadas, por URL (ver `useBlobUrl`). */
const pendingRevoke = new Map<string, number>()

/** `ArrayBuffer` de PNG → URL de blob, revogada quando troca. Sem buffer
 *  (a prévia falhou) volta a `null`: a URL anterior já foi revogada e, se
 *  ficasse, a tela mostrava o ícone de imagem quebrada.
 *
 *  A revogação é adiada e cancelada se a mesma URL voltar a montar: no
 *  `StrictMode` (dev) o React desmonta e remonta o efeito com o mesmo `useMemo`,
 *  e revogar na hora deixava a prévia com a imagem quebrada quando ela abria
 *  com o PNG já no cache (trocar de aba; relato do usuário, 2026-11-14). */
export function useBlobUrl(buf: ArrayBuffer | undefined) {
  // derivada do buffer, sem estado (um `setState` no efeito re-renderizava
  // duas vezes a cada prévia)
  const url = useMemo(
    () => (buf ? URL.createObjectURL(new Blob([buf], { type: 'image/png' })) : null),
    [buf],
  )
  useEffect(() => {
    if (!url) return
    const pending = pendingRevoke.get(url)
    if (pending !== undefined) {
      window.clearTimeout(pending)
      pendingRevoke.delete(url)
    }
    return () => {
      pendingRevoke.set(
        url,
        window.setTimeout(() => {
          pendingRevoke.delete(url)
          URL.revokeObjectURL(url)
        }, 1000),
      )
    }
  }, [url])
  return url
}
