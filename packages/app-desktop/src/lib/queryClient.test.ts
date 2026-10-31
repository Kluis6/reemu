import { MutationObserver, onlineManager } from '@tanstack/react-query'
import { afterEach, describe, expect, it } from 'vitest'
import { queryClient } from './queryClient'

describe('queryClient sem rede', () => {
  afterEach(() => onlineManager.setOnline(true))

  it('roda o comando e devolve o erro, em vez de ficar pausado', async () => {
    onlineManager.setOnline(false)
    const observer = new MutationObserver(queryClient, {
      mutationFn: () => Promise.reject(new Error('sem rede')),
    })
    await expect(observer.mutate()).rejects.toThrow('sem rede')
    expect(observer.getCurrentResult().isError).toBe(true)
  })
})
