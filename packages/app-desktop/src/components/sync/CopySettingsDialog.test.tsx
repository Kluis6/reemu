// @vitest-environment happy-dom
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CopyReport, SettingsPreview, SyncMachine } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  syncSettingsPreview: vi.fn(),
  syncSettingsCopy: vi.fn(),
}))
vi.mock('../../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../../lib/tauri')>()), ...api }))
const appearance = vi.hoisted(() => ({ exportAppearance: vi.fn(), applyAppearance: vi.fn() }))
vi.mock('../../lib/appearanceGroup', () => appearance)
// O Dialog do Fluent não monta no happy-dom desta árvore ("presenceFn is not a
// function"): o diálogo de tela cheia vira uma camada simples.
vi.mock('../FullscreenDialog', () => ({
  FullscreenDialog: (p: {
    open: boolean
    onClose: () => void
    title: string
    children: React.ReactNode
    actions?: React.ReactNode
  }) =>
    p.open ? (
      <div className="fui-DialogSurface" role="dialog">
        <h2>{p.title}</h2>
        <button onClick={p.onClose}>fechar</button>
        {p.children}
        {p.actions}
      </div>
    ) : null,
}))

import '../../i18n'
import { APPEARANCE_UNDO_KEY } from '../../lib/settingsSync'
import { CopySettingsDialog } from './CopySettingsDialog'
import { button, mountWithQuery } from './syncTestSupport'

const HW = {
  os: 'Windows 11',
  arch: 'x86_64',
  cpu: 'Core i5',
  cpu_cores: 4,
  cpu_threads: 8,
  memory_mb: 16384,
  gpu: 'Iris Xe',
  gpu_backend: 'OpenGL',
  gpu_driver: '1',
  app_version: '0.9.0',
  captured_at: '2026-11-13T10:00:00Z',
}
const MACHINE: SyncMachine = {
  id: 'dev2',
  label: 'Notebook-Ed',
  clientKind: 'reemu',
  isThis: false,
  hardware: HW,
  lastUsedAt: null,
  revoked: false,
  paused: false,
}
const GROUPS = ['core-options', 'shaders-decorations', 'hotkeys-keyboard', 'audio', 'appearance', 'video'] as const
const PREVIEW: SettingsPreview = {
  machine: MACHINE,
  sentAt: 1_700_000_000,
  groups: GROUPS.map((group) => ({ group, present: true, error: null, missingFiles: [] })),
  videoDefaultOff: true,
  hereGpu: 'RTX 3060',
  hereBackend: 'Vulkan',
}
/** O instante da cópia no marcador do Rust. */
const COPIED_AT = 1_700_000_500
const REPORT = (over: Partial<CopyReport> = {}): CopyReport => ({
  perGroup: [],
  backupDir: 'sync-backups/x',
  appearanceDoc: null,
  copiedAt: COPIED_AT,
  ...over,
})
const APPLY = { applied: 1, skippedUnknownGames: 0, keptLocal: 0, missingFiles: [], coresNotInstalled: [] }

const onClose = vi.fn()
let ui: Awaited<ReturnType<typeof mountWithQuery>>
const mount = async () => {
  ui = await mountWithQuery(<CopySettingsDialog machine={MACHINE} onClose={onClose} />)
}
/** O checkbox da linha de um grupo, pelo nome dele. */
const box = (name: string) =>
  [...document.querySelectorAll('.fui-Checkbox')].find((c) => c.textContent?.includes(name))!.querySelector<HTMLInputElement>('input')!
const tick = async (name: string) => act(async () => box(name).click())

beforeEach(() => {
  api.syncSettingsPreview.mockReset().mockResolvedValue(PREVIEW)
  api.syncSettingsCopy.mockReset().mockResolvedValue(REPORT())
  appearance.exportAppearance.mockReset().mockResolvedValue({ v: 1, before: true })
  appearance.applyAppearance.mockReset().mockResolvedValue(undefined)
  onClose.mockReset()
  localStorage.clear()
})
afterEach(async () => {
  await ui.unmount()
})

describe('CopySettingsDialog', () => {
  it('video is unticked when the preview says so and ticks by hand', async () => {
    await mount()
    const text = document.body.textContent ?? ''
    expect(text).toContain('Copiar de Notebook-Ed para esta máquina')
    expect(text).toContain('As suas atuais ficam guardadas para desfazer.')
    expect(text).toContain('hardware diferente')
    expect(text).toContain('Iris Xe em OpenGL → RTX 3060 em Vulkan. Desmarcado por padrão.')
    expect(text).toContain('Pastas da biblioteca e controles físicos')
    expect(text).toContain('Nunca viajam: são desta máquina.')
    expect(box('Vídeo').checked).toBe(false)
    for (const name of ['Opções dos cores', 'Shaders e decorações', 'Atalhos e teclado', 'Áudio', 'Aparência']) {
      expect(box(name).checked).toBe(true)
    }
    expect(button('Copiar 5 grupos')).toBeTruthy()
    await tick('Vídeo')
    expect(box('Vídeo').checked).toBe(true)
    expect(button('Copiar 6 grupos')).toBeTruthy()
    // a linha das pastas e controles físicos não se marca
    expect(box('Pastas da biblioteca').disabled).toBe(true)
  })

  it('video starts ticked when the hardware matches', async () => {
    api.syncSettingsPreview.mockResolvedValue({ ...PREVIEW, videoDefaultOff: false })
    await mount()
    expect(box('Vídeo').checked).toBe(true)
    expect(document.body.textContent).not.toContain('hardware diferente')
  })

  it('a group the other machine did not send or sent bad cannot be ticked', async () => {
    api.syncSettingsPreview.mockResolvedValue({
      ...PREVIEW,
      groups: PREVIEW.groups.map((g) =>
        g.group === 'audio'
          ? { ...g, present: false }
          : g.group === 'hotkeys-keyboard'
            ? { ...g, error: 'settings_version' }
            : g,
      ),
    })
    await mount()
    expect(box('Áudio').disabled).toBe(true)
    expect(box('Áudio').checked).toBe(false)
    expect(box('Atalhos e teclado').disabled).toBe(true)
    expect(document.body.textContent).toContain('A outra máquina ainda não enviou este grupo.')
    expect(document.body.textContent).toContain('vêm de um ReEmu mais novo')
    expect(button('Copiar 3 grupos')).toBeTruthy()
  })

  it('copy sends the ticked groups and shows what did not come', async () => {
    api.syncSettingsCopy.mockResolvedValue(
      REPORT({
        perGroup: [
          {
            group: 'shaders-decorations',
            report: {
              ...APPLY,
              keptLocal: 2,
              skippedUnknownGames: 1,
              missingFiles: [
                { kind: 'imported_elsewhere', item: 'shader', name: 'crt-do-ed', scope: 'default', systemId: null, crc32: null },
                { kind: 'not_downloaded_here', item: 'decoration', name: 'moldura-gba', scope: 'default', systemId: null, crc32: null },
              ],
            },
          },
        ],
      }),
    )
    await mount()
    await tick('Atalhos e teclado')
    await act(async () => button('Copiar 4 grupos')!.click())
    expect(api.syncSettingsCopy).toHaveBeenCalledWith('dev2', ['core-options', 'shaders-decorations', 'audio'], true)
    const text = document.body.textContent ?? ''
    expect(text).toContain('Shader crt-do-ed: não veio: arquivo local em Notebook-Ed')
    expect(text).toContain('Moldura moldura-gba: não veio: não está baixado aqui')
    expect(text).toContain('2 lugares ficaram com a escolha desta máquina.')
    expect(text).toContain('1 jogo não está nesta biblioteca e ficou de fora.')
    expect(button('Fechar')).toBeTruthy()
  })

  it('a copy with appearance applies the returned doc and keeps the previous one', async () => {
    api.syncSettingsCopy.mockResolvedValue(REPORT({ appearanceDoc: { v: 1, theirs: true } }))
    await mount()
    await act(async () => button('Copiar 5 grupos')!.click())
    expect(appearance.applyAppearance).toHaveBeenCalledWith({ v: 1, theirs: true })
    // guardada com o instante desta cópia: só o Desfazer dela a traz
    expect(JSON.parse(localStorage.getItem(APPEARANCE_UNDO_KEY)!)).toEqual({ copiedAt: COPIED_AT, doc: { v: 1, before: true } })
  })

  it('a copy without appearance clears the appearance undo', async () => {
    localStorage.setItem(APPEARANCE_UNDO_KEY, JSON.stringify({ v: 1, older: true }))
    await mount()
    await tick('Aparência')
    await act(async () => button('Copiar 4 grupos')!.click())
    expect(api.syncSettingsCopy).toHaveBeenCalledWith('dev2', ['core-options', 'shaders-decorations', 'hotkeys-keyboard', 'audio'], false)
    expect(appearance.applyAppearance).not.toHaveBeenCalled()
    expect(localStorage.getItem(APPEARANCE_UNDO_KEY)).toBeNull()
  })

  it('a copy refused while a game is loaded shows the error and stays open', async () => {
    api.syncSettingsCopy.mockRejectedValue('game_open')
    await mount()
    await act(async () => button('Copiar 5 grupos')!.click())
    expect(document.body.textContent).toContain('Feche o jogo para copiar ou desfazer configurações.')
    // continua na lista, pronto para tentar de novo
    expect(box('Opções dos cores')).toBeTruthy()
    expect(button('Copiar 5 grupos')).toBeTruthy()
    expect(appearance.applyAppearance).not.toHaveBeenCalled()
  })

  it('puts the controller focus on the first checkbox', async () => {
    await mount()
    await act(async () => {
      await new Promise((r) => setTimeout(r, 5))
    })
    expect(document.activeElement).toBe(box('Opções dos cores'))
  })

  it('a copy whose appearance export failed clears the saved appearance undo', async () => {
    localStorage.setItem(APPEARANCE_UNDO_KEY, JSON.stringify({ v: 1, older: true }))
    appearance.exportAppearance.mockRejectedValue(new Error('x'))
    api.syncSettingsCopy.mockResolvedValue(REPORT({ appearanceDoc: { v: 1, theirs: true } }))
    await mount()
    await act(async () => button('Copiar 5 grupos')!.click())
    expect(appearance.applyAppearance).toHaveBeenCalledWith({ v: 1, theirs: true })
    expect(localStorage.getItem(APPEARANCE_UNDO_KEY)).toBeNull()
  })

  it('an appearance that fails to apply keeps the previous one saved and warns', async () => {
    api.syncSettingsCopy.mockResolvedValue(REPORT({ appearanceDoc: { v: 1, theirs: true } }))
    appearance.applyAppearance.mockRejectedValue(new Error('settings_invalid'))
    await mount()
    await act(async () => button('Copiar 5 grupos')!.click())
    expect(JSON.parse(localStorage.getItem(APPEARANCE_UNDO_KEY)!)).toEqual({ copiedAt: COPIED_AT, doc: { v: 1, before: true } })
    expect(document.body.textContent).toContain('A aparência não pôde ser aplicada. O resto foi copiado.')
    expect(document.body.textContent).toContain('Copiado.')
  })

  it('a preview that fails to load offers Tentar de novo, which reads it again', async () => {
    api.syncSettingsPreview.mockRejectedValueOnce('offline').mockResolvedValue(PREVIEW)
    await mount()
    expect(document.body.textContent).toContain('Não foi possível falar com o Alvanista')
    expect(api.syncSettingsPreview).toHaveBeenCalledTimes(1)
    await act(async () => button('Tentar de novo')!.click())
    await act(async () => {
      await new Promise((r) => setTimeout(r, 5))
    })
    expect(api.syncSettingsPreview).toHaveBeenCalledTimes(2)
    expect(button('Tentar de novo')).toBeUndefined()
    expect(box('Opções dos cores').checked).toBe(true)
  })

  it('a copy invalidates every query, so no settings screen writes its old config over it', async () => {
    await mount()
    // Áudio e Sistema leram a configuração antes da cópia
    ui.qc.setQueryData(['audio-config'], { rateControlEnabled: false })
    ui.qc.setQueryData(['video-config'], { integerScaling: false, hdr: false })
    ui.qc.setQueryData(['core-options', 'snes9x'], [])
    await act(async () => button('Copiar 5 grupos')!.click())
    for (const key of [['audio-config'], ['video-config'], ['core-options', 'snes9x']]) {
      expect(ui.qc.getQueryState(key)?.isInvalidated, key.join('/')).toBe(true)
    }
  })

  it('a refused copy leaves the cached settings alone', async () => {
    api.syncSettingsCopy.mockRejectedValue('game_open')
    await mount()
    ui.qc.setQueryData(['audio-config'], { rateControlEnabled: false })
    await act(async () => button('Copiar 5 grupos')!.click())
    expect(ui.qc.getQueryState(['audio-config'])?.isInvalidated).toBe(false)
  })

  it('puts the focus on Fechar when the result shows', async () => {
    await mount()
    await act(async () => button('Copiar 5 grupos')!.click())
    await act(async () => {
      await new Promise((r) => setTimeout(r, 5))
    })
    expect(document.activeElement).toBe(button('Fechar')!.closest('button'))
    expect(document.activeElement?.textContent).toBe('Fechar')
  })
})
