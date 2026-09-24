import { describe, expect, it } from 'vitest'
import systemsRs from '../../../../crates/library-scan/src/systems.rs?raw'
import { knownPlatforms, platformLabel } from './platform'

describe('platformLabel', () => {
  it('traduz id conhecido', () => {
    expect(platformLabel('snes')).toBe('Super Nintendo')
  })

  it('id desconhecido cai no próprio id em maiúsculas', () => {
    expect(platformLabel('foo')).toBe('FOO')
  })

  // `library-scan/src/systems.rs` é a tabela canônica de `system_id`. Sistema
  // novo lá sem rótulo aqui aparece na UI como "ATARI5200" — já aconteceu com
  // os 15 sistemas que entraram em 2026-09-19.
  it('todo system_id do scan (Rust) tem rótulo', () => {
    const ids = new Set([...systemsRs.matchAll(/=>\s*"([a-z0-9]+)"/g)].map((m) => m[1]))
    expect(ids.size).toBeGreaterThan(30) // o regex ainda acha a tabela
    const semRotulo = [...ids].filter((id) => !knownPlatforms().some(([k]) => k === id))
    expect(semRotulo).toEqual([])
  })
})

describe('knownPlatforms', () => {
  it('vem ordenado pelo rótulo', () => {
    const labels = knownPlatforms().map(([, l]) => l)
    expect(labels).toEqual([...labels].sort((a, b) => a.localeCompare(b)))
  })
})
