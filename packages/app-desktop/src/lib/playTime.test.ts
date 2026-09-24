import { expect, it } from 'vitest'
import { formatPlayTime } from './playTime'

it('formata o tempo de jogo', () => {
  expect(formatPlayTime(0)).toBe('menos de 1 min')
  expect(formatPlayTime(59)).toBe('menos de 1 min')
  expect(formatPlayTime(60)).toBe('1 min')
  expect(formatPlayTime(45 * 60 + 30)).toBe('45 min')
  expect(formatPlayTime(2 * 3600)).toBe('2 h')
  expect(formatPlayTime(3 * 3600 + 12 * 60 + 5)).toBe('3 h 12 min')
})
