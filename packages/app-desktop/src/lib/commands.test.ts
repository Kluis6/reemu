import { describe, expect, it } from 'vitest'
import handlers from '../../../../apps/desktop/src-tauri/src/lib.rs?raw'
import wrappers from './tauri.ts?raw'

/** O que o `generate_handler!` registra. */
function registered(rust: string): Set<string> {
  const block = rust.slice(rust.indexOf('generate_handler!['))
  const list = block.slice(0, block.indexOf(']'))
  return new Set([...list.matchAll(/([a-z0-9_]+),/g)].map((m) => m[1]))
}

/** Os comandos chamados em `tauri.ts`; os de plugin (`plugin:x|y`) ficam de fora. */
function invoked(ts: string): string[] {
  return [...ts.matchAll(/\binvoke(?:<[\s\S]*?>)?\(\s*'([a-z0-9_]+)'/g)].map((m) => m[1])
}

describe('comandos do backend', () => {
  it('todo comando chamado pelo frontend está registrado no Rust', () => {
    const known = registered(handlers)
    const calls = invoked(wrappers)
    expect(calls.length).toBeGreaterThan(50)
    expect(calls.filter((c) => !known.has(c))).toEqual([])
  })
})
