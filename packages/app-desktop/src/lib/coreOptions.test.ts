import { describe, expect, it } from 'vitest'
import { toggleValues } from './coreOptions'

describe('opção liga/desliga', () => {
  it('reconhece o par e mantém a grafia do core', () => {
    expect(toggleValues(['disabled', 'enabled'])).toEqual({ on: 'enabled', off: 'disabled' })
    expect(toggleValues(['ON', 'OFF'])).toEqual({ on: 'ON', off: 'OFF' })
  })

  it('o resto continua lista', () => {
    expect(toggleValues(['VGA', 'TV'])).toBeNull()
    expect(toggleValues(['off', 'on', 'auto'])).toBeNull()
  })
})
