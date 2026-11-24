import { useEffect } from 'react'
import {
  SCREENSAVER_CONTROLLER_ACTIVITY,
  useScreensaverStore,
} from '../stores/useScreensaverStore'

const ACTIVITY_EVENTS = [
  'pointermove', 'pointerdown', 'wheel', SCREENSAVER_CONTROLLER_ACTIVITY,
] as const

/** Montado no AppShell, fora do emulador e do onboarding. */
export function useScreensaver(pathname: string): boolean {
  const minutes = useScreensaverStore((s) => s.minutes)
  const active = useScreensaverStore((s) => s.active)

  useEffect(() => {
    const store = useScreensaverStore
    const delay = minutes * 60_000
    let timer: number | undefined
    let lastActivity = performance.now()
    const focused = () => !document.hidden && document.hasFocus()
    const activity = () => { lastActivity = performance.now() }
    const stop = () => window.clearTimeout(timer)

    function check() {
      if (!focused() || store.getState().active) return
      const remaining = delay - (performance.now() - lastActivity)
      if (remaining > 0) {
        timer = window.setTimeout(check, remaining)
      } else if (document.querySelector('dialog[open], [aria-modal="true"], [role="menu"]')) {
        start()
      } else {
        store.getState().show()
      }
    }

    function start() {
      stop()
      activity()
      if (delay && focused() && !store.getState().active) timer = window.setTimeout(check, delay)
    }

    let wakeKey: string | undefined
    const consume = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopImmediatePropagation()
    }
    const onKeyDown = (event: KeyboardEvent) => {
      activity()
      const state = store.getState()
      const key = event.code || event.key
      if (state.active || (event.repeat && key === wakeKey)) {
        wakeKey = key
        consume(event)
        if (state.active) state.dismiss()
      } else {
        wakeKey = undefined
      }
    }
    const onKeyUp = (event: KeyboardEvent) => {
      if ((event.code || event.key) !== wakeKey) return
      wakeKey = undefined
      consume(event)
    }
    const reset = () => {
      wakeKey = undefined
      store.getState().dismiss()
      start()
    }
    const unsubscribe = store.subscribe((state, previous) => {
      if (state.active === previous.active) return
      if (state.active) stop()
      else start()
    })

    if (delay) {
      for (const event of ACTIVITY_EVENTS) {
        window.addEventListener(event, activity, { capture: true, passive: true })
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('keyup', onKeyUp, true)
    window.addEventListener('blur', reset)
    window.addEventListener('focus', reset)
    document.addEventListener('visibilitychange', reset)
    start()

    return () => {
      unsubscribe()
      stop()
      if (delay) {
        for (const event of ACTIVITY_EVENTS) window.removeEventListener(event, activity, true)
      }
      window.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('keyup', onKeyUp, true)
      window.removeEventListener('blur', reset)
      window.removeEventListener('focus', reset)
      document.removeEventListener('visibilitychange', reset)
      store.getState().dismiss()
    }
  }, [minutes, pathname])

  return active
}
