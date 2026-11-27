// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { GameLogPanel } from './GameLogPanel'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const mocks = vi.hoisted(() => ({
  alvanistaCachedStatus: vi.fn(),
  gamelogState: vi.fn(),
  gamelogUsage: vi.fn(),
  gamelogListItems: vi.fn(),
  gamelogCaptureScreenshot: vi.fn(),
  gamelogStartAudioNote: vi.fn(),
  gamelogStopAudioNote: vi.fn(),
  gamelogStartVideoRecording: vi.fn(),
  gamelogStopVideoRecording: vi.fn(),
  gamelogCreateNote: vi.fn(),
  gamelogSetPrefs: vi.fn(),
  listSystemHotkeys: vi.fn(),
  getRomMetadata: vi.fn(),
}))

vi.mock('../lib/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../lib/tauri')>()
  return {
    ...actual,
    alvanistaCachedStatus: mocks.alvanistaCachedStatus,
    gamelogState: mocks.gamelogState,
    gamelogUsage: mocks.gamelogUsage,
    gamelogListItems: mocks.gamelogListItems,
    gamelogCaptureScreenshot: mocks.gamelogCaptureScreenshot,
    gamelogStartAudioNote: mocks.gamelogStartAudioNote,
    gamelogStopAudioNote: mocks.gamelogStopAudioNote,
    gamelogStartVideoRecording: mocks.gamelogStartVideoRecording,
    gamelogStopVideoRecording: mocks.gamelogStopVideoRecording,
    gamelogCreateNote: mocks.gamelogCreateNote,
    gamelogSetPrefs: mocks.gamelogSetPrefs,
    listSystemHotkeys: mocks.listSystemHotkeys,
    getRomMetadata: mocks.getRomMetadata,
    onGamelogStateChanged: () => Promise.resolve(() => {}),
    onGamelogUpdated: () => Promise.resolve(() => {}),
  }
})

async function renderPanel(props: { romId: string; gameUuid?: string | null; inGame?: boolean }) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => {
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <QueryClientProvider client={qc}>
          <GameLogPanel {...props} />
        </QueryClientProvider>
      </FluentProvider>,
    )
  })
  await act(async () => {
    await new Promise((r) => setTimeout(r, 30))
  })
  return {
    host,
    unmount: async () => {
      await act(async () => root.unmount())
      host.remove()
    },
  }
}

describe('GameLogPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.alvanistaCachedStatus.mockResolvedValue({
      connected: true,
      ended: false,
      scopes: ['profile:read', 'gamelog'],
    })
    mocks.gamelogState.mockResolvedValue({
      audioRecording: false,
      audioElapsedMs: 0,
      videoRecording: false,
      videoElapsedMs: 0,
      ffmpegAvailable: true,
      activeRomId: 'rom-1',
      activeGameId: '11111111-2222-3333-4444-555555555555',
      activeSessionPostId: 'post_sess_1',
      activeRaGameId: 0,
      prefs: {
        autoSessionCheckin: true,
        autoAttachCaptures: true,
        visibility: 'friends',
      },
    })
    mocks.listSystemHotkeys.mockResolvedValue([
      { action: 'capture_screenshot', trigger: [{ Keyboard: { scancode: 0xd90c87bc } }] },
    ])
    mocks.gamelogUsage.mockResolvedValue({
      usedBytes: 50 * 1024 * 1024,
      quotaBytes: 500 * 1024 * 1024,
      itemCount: 2,
    })
    mocks.gamelogListItems.mockResolvedValue({
      items: [
        {
          id: 'item-1',
          gameId: '11111111-2222-3333-4444-555555555555',
          kind: 'screenshot',
          title: null,
          body: 'Boss derrotado!',
          platformId: 'ps2',
          visibility: 'friends',
          state: 'ready',
          takenAt: '2026-11-27T01:00:00Z',
          createdAt: '2026-11-27T01:00:00Z',
          updatedAt: '2026-11-27T01:00:00Z',
          media: [
            {
              key: 'k1',
              url: 'https://cdn.example.com/shot.png',
              mimeType: 'image/png',
              sizeBytes: 12000,
              width: 640,
              height: 480,
              durationMs: null,
              posterUrl: null,
            },
          ],
        },
      ],
      nextCursor: null,
    })
    mocks.gamelogCaptureScreenshot.mockResolvedValue({
      localPath: 'C:/tmp/shot.png',
      uploadedItem: { id: 'item-2' },
      attachedPostId: 'post_sess_1',
      warning: null,
    })
  })

  it('exibe controles em jogo, itens do Meu Log e dispara captura anexada na sessao', async () => {
    const view = await renderPanel({
      romId: 'rom-1',
      gameUuid: '11111111-2222-3333-4444-555555555555',
      inGame: true,
    })

    expect(view.host.textContent).toContain('Boss derrotado!')

    const buttons = Array.from(view.host.querySelectorAll('button'))
    const shotBtn = buttons.find((b) =>
      /Capturar tela|Capture screenshot/i.test(b.textContent ?? ''),
    )
    expect(shotBtn).toBeTruthy()
    await act(async () => {
      shotBtn!.click()
      await new Promise((r) => setTimeout(r, 20))
    })

    expect(mocks.gamelogCaptureScreenshot).toHaveBeenCalledTimes(1)
    await view.unmount()
  })

  it('pede autorizacao do escopo gamelog quando conectado sem o escopo', async () => {
    mocks.alvanistaCachedStatus.mockResolvedValue({
      connected: true,
      ended: false,
      scopes: ['profile:read'],
    })

    const view = await renderPanel({
      romId: 'rom-1',
      gameUuid: '11111111-2222-3333-4444-555555555555',
      inGame: false,
    })

    expect(view.host.querySelector('[data-testid="gamelog-consent-prompt"]')).toBeTruthy()
    await view.unmount()
  })
})
