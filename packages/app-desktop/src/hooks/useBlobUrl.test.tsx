// @vitest-environment happy-dom
import { StrictMode, act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useBlobUrl } from '../hooks/useBlobUrl'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

let seen: (string | null)[] = []
function Probe({ buf }: { buf: ArrayBuffer | undefined }) {
  seen.push(useBlobUrl(buf))
  return null
}

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  seen = []
})

describe('useBlobUrl', () => {
  // Relato de 2026-11-14: no StrictMode (dev) a prévia abria com o PNG já no
  // cache e mostrava a imagem quebrada, porque a remontagem simulada revogava
  // a URL que continuava na tela.
  it('no StrictMode a URL montada não é revogada', async () => {
    vi.useFakeTimers()
    let n = 0
    vi.spyOn(URL, 'createObjectURL').mockImplementation(() => `blob:t/${++n}`)
    const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
    const host = document.createElement('div')
    const root = createRoot(host)
    const buf = new ArrayBuffer(8)
    await act(async () =>
      root.render(
        <StrictMode>
          <Probe buf={buf} />
        </StrictMode>,
      ),
    )
    const url = seen[seen.length - 1]
    expect(url).toMatch(/^blob:/)
    vi.advanceTimersByTime(5000)
    expect(revoke).not.toHaveBeenCalledWith(url)

    // ao sair, a URL é revogada (depois do adiamento)
    await act(async () => root.unmount())
    vi.advanceTimersByTime(5000)
    expect(revoke).toHaveBeenCalledWith(url)
  })

  it('troca de imagem revoga a URL anterior', async () => {
    vi.useFakeTimers()
    let n = 0
    vi.spyOn(URL, 'createObjectURL').mockImplementation(() => `blob:t/${++n}`)
    const revoke = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {})
    const host = document.createElement('div')
    const root = createRoot(host)
    await act(async () => root.render(<Probe buf={new ArrayBuffer(8)} />))
    const first = seen[seen.length - 1]
    await act(async () => root.render(<Probe buf={new ArrayBuffer(8)} />))
    const second = seen[seen.length - 1]
    expect(second).not.toBe(first)
    vi.advanceTimersByTime(5000)
    expect(revoke).toHaveBeenCalledWith(first)
    expect(revoke).not.toHaveBeenCalledWith(second)
    await act(async () => root.unmount())
  })
})
