import { expect, it } from 'vitest'
import { initials } from './initials'

it('pega até 2 iniciais maiúsculas', () => {
  expect(initials('super mario world')).toBe('SM')
  expect(initials('Tetris')).toBe('T')
  expect(initials('')).toBe('')
})
