// @vitest-environment happy-dom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { SCREENSAVER_CONTROLLER_ACTIVITY, useScreensaverStore } from '../stores/useScreensaverStore'
import { useScreensaver } from './useScreensaver'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const interval = 60_000
let focused: boolean
let root: ReturnType<typeof createRoot>

function Launcher({ pathname }: { pathname: string }) {
  useScreensaver(pathname)
  return null
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] })
  focused = true
  vi.spyOn(document, 'hasFocus').mockImplementation(() => focused)
  useScreensaverStore.setState({ active: false, minutes: 1 })
  root = createRoot(document.createElement('div'))
})

afterEach(() => {
  act(() => root.unmount())
  vi.useRealTimers()
  vi.restoreAllMocks()
})

function advance(ms: number) {
  act(() => vi.advanceTimersByTime(ms))
}

function active() {
  return useScreensaverStore.getState().active
}

it('extends the idle deadline after controller activity and re-arms after waking', () => {
  act(() => root.render(<Launcher pathname="/" />))
  advance(interval - 1000)
  act(() => window.dispatchEvent(new Event(SCREENSAVER_CONTROLLER_ACTIVITY)))
  advance(interval - 1000)
  expect(active()).toBe(false)
  advance(1000)
  expect(active()).toBe(true)
  act(() => useScreensaverStore.getState().dismiss())
  advance(interval)
  expect(active()).toBe(true)
})

it('suspends the timer while unfocused and cancels it when leaving the launcher', () => {
  act(() => root.render(<Launcher pathname="/library" />))
  advance(interval - 1000)
  focused = false
  act(() => window.dispatchEvent(new Event('blur')))
  advance(interval)
  expect(active()).toBe(false)
  focused = true
  act(() => window.dispatchEvent(new Event('focus')))
  advance(interval - 1000)
  expect(active()).toBe(false)
  act(() => root.render(null))
  advance(interval)
  expect(active()).toBe(false)
})

it('permits a manual preview without arming the automatic timer when disabled', () => {
  useScreensaverStore.setState({ minutes: 0 })
  act(() => root.render(<Launcher pathname="/settings/aparencia/protecao-de-tela" />))
  advance(2 * interval)
  expect(active()).toBe(false)
  act(() => useScreensaverStore.getState().show())
  expect(active()).toBe(true)
  focused = false
  act(() => window.dispatchEvent(new Event('blur')))
  expect(active()).toBe(false)
})

it('consumes the key that dismisses the saver until the key is released', () => {
  act(() => root.render(<Launcher pathname="/" />))
  act(() => useScreensaverStore.getState().show())

  function key(type: 'keydown' | 'keyup', repeat = false) {
    const event = new KeyboardEvent(type, { code: 'Enter', bubbles: true, cancelable: true, repeat })
    act(() => window.dispatchEvent(event))
    return event.defaultPrevented
  }

  expect(key('keydown')).toBe(true)
  expect(active()).toBe(false)
  expect(key('keydown', true)).toBe(true)
  expect(key('keyup')).toBe(true)
  expect(key('keydown')).toBe(false)
})
