import { Body1, Button, Spinner, Title3, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useLocation, useNavigate, useParams, useSearchParams } from 'react-router-dom'
import { useFocusBridge } from '../hooks/useFocusBridge'
import { useFullscreen } from '../hooks/useFullscreen'
import { useKeyboardInput } from '../hooks/useKeyboardInput'
import { moveFocus } from '../lib/focusNav'
import { initials } from '../lib/initials'
import { sysToast } from '../lib/toast'
import {
  currentFocus,
  listRoms,
  listSaveStates,
  loadGame,
  loadSaveState,
  pollFrame,
  saveState,
  toggleFocus,
  unloadGame,
} from '../lib/tauri'
import { usePauseStyles } from '../styles/xbox'
import { useFocusStore } from '../stores/useFocusStore'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  // Opaco: o frame do core é desenhado no <canvas> (a webview não tem overlay
  // transparente nesse setup — ver src-tauri/src/main.rs).
  root: {
    position: 'fixed',
    inset: 0,
    background: '#000',
    display: 'grid',
    placeItems: 'center',
  },
  canvas: {
    // Preenche a altura da janela mantendo a proporção; encolhe se ficar
    // mais largo que a janela. `aspectRatio` vem do core (inline).
    height: '100%',
    width: 'auto',
    maxWidth: '100%',
    maxHeight: '100%',
    imageRendering: 'pixelated',
    background: '#000',
  },
  center: {
    position: 'fixed',
    inset: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    background: tokens.colorNeutralBackground1,
  },
  splash: {
    position: 'fixed',
    inset: 0,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: tokens.spacingVerticalL,
    // linear (não radial multicamada) — ver histórico do WebKitGTK em main.rs.
    background: 'linear-gradient(180deg, #161d2b 0%, #0b0b0d 70%)',
    color: '#fff',
  },
  splashArt: {
    width: '208px',
    height: '208px',
    borderRadius: '16px',
    objectFit: 'cover',
    display: 'grid',
    placeItems: 'center',
    fontSize: '52px',
    fontWeight: 700,
    color: 'rgba(255,255,255,0.4)',
    background: '#20263a',
    boxShadow: '0 22px 60px rgba(0,0,0,0.5), inset 0 0 0 1px rgba(255,255,255,0.06)',
  },
  splashTitle: {
    fontSize: '22px',
    fontWeight: 700,
    textAlign: 'center',
    maxWidth: '70%',
  },
  splashSub: {
    opacity: 0.55,
    fontSize: '13px',
    letterSpacing: '0.02em',
    textTransform: 'uppercase',
  },
})

interface LaunchInfo {
  coreId: string
  romPath: string
  title: string
  boxart?: string | null
  system?: string
}

export function PlayScreen() {
  const styles = useStyles()
  const pause = usePauseStyles()
  const navigate = useNavigate()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const { romId = '' } = useParams()
  const [params] = useSearchParams()
  const locState = (useLocation().state ?? {}) as Partial<LaunchInfo>
  const stateCore = locState.coreId
  const statePath = locState.romPath
  const stateTitle = locState.title
  const stateBoxart = locState.boxart
  const stateSystem = locState.system

  useKeyboardInput()
  useFocusBridge()
  const { on: fullscreen, toggle: toggleFullscreen } = useFullscreen()
  const focus = useFocusStore((s) => s.focus)
  const setFocus = useFocusStore((s) => s.setFocus)

  // Precisa de coreId + romPath: vêm do `navigate(state)` ou, num reload/URL
  // direta, da lista de ROMs + query param `?core=`.
  const roms = useQuery({
    queryKey: ['roms'],
    queryFn: listRoms,
    retry: false,
    enabled: !statePath,
  })
  const rom = roms.data?.find((r) => r.id === romId)
  const coreParam = params.get('core')
  // Memoizado por primitivos — senão o objeto muda toda render e o efeito de
  // load/unload entra em loop.
  const launch: LaunchInfo | null = useMemo(() => {
    if (stateCore && statePath) {
      return {
        coreId: stateCore,
        romPath: statePath,
        title: stateTitle ?? romId,
        boxart: stateBoxart,
        system: stateSystem,
      }
    }
    if (rom && coreParam) {
      return {
        coreId: coreParam,
        romPath: rom.filePath,
        title: rom.title,
        boxart: rom.boxart,
        system: rom.systemId,
      }
    }
    return null
  }, [stateCore, statePath, stateTitle, stateBoxart, stateSystem, rom, coreParam, romId])

  const [status, setStatus] = useState<'loading' | 'ready' | { error: string }>('loading')
  const [artBroken, setArtBroken] = useState(false)
  const [aspect, setAspect] = useState(4 / 3)
  const loadedRef = useRef(false)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  // O loop de fetch olha o foco por ref (não re-monta o efeito a cada pausa).
  const pausedRef = useRef(false)
  useEffect(() => {
    pausedRef.current = focus === 'MenuFocused'
  }, [focus])

  // Vídeo: dois loops desacoplados. O de **fetch** puxa o frame RGBA do core
  // por IPC e guarda o mais recente numa ref; o de **paint** (rAF, sincronizado
  // com o vblank) pinta o último frame no canvas. Assim a latência do IPC não
  // trava a pintura, e ao pausar o canvas segura o último frame (freeze) — o
  // fetch cai pra 4Hz. É o caminho de vídeo do desktop (a surface nativa fora
  // da webview não funciona no WebKitGTK+NVIDIA desse setup — ver
  // docs/ai-context/03; `REEMU_X11_VIDEO=1` tenta o esquema antigo).
  useEffect(() => {
    if (status !== 'ready') return
    let alive = true
    let raf = 0
    const latest = { img: null as ImageData | null, w: 0, h: 0 }

    void (async () => {
      while (alive) {
        try {
          const buf = await pollFrame()
          if (buf.byteLength > 8) {
            const dv = new DataView(buf)
            const w = dv.getUint32(0, true)
            const h = dv.getUint32(4, true)
            const need = w * h * 4
            if (w > 0 && h > 0 && buf.byteLength >= 8 + need) {
              latest.img = new ImageData(new Uint8ClampedArray(buf.slice(8, 8 + need)), w, h)
              latest.w = w
              latest.h = h
            }
          }
        } catch {
          /* frame perdido, tenta no próximo */
        }
        // pausado: ~8Hz (segura o freeze, retoma rápido); rodando: o mais
        // rápido possível — o `poll_frame` já devolve vazio na hora se não há
        // frame novo, então isso não vira busy-loop.
        await new Promise((r) => setTimeout(r, pausedRef.current ? 120 : 3))
      }
    })()

    const paint = () => {
      const c = canvasRef.current
      if (c && latest.img) {
        if (c.width !== latest.w || c.height !== latest.h) {
          c.width = latest.w
          c.height = latest.h
        }
        c.getContext('2d')?.putImageData(latest.img, 0, 0)
      }
      if (alive) raf = requestAnimationFrame(paint)
    }
    raf = requestAnimationFrame(paint)

    return () => {
      alive = false
      cancelAnimationFrame(raf)
    }
  }, [status])

  useEffect(() => {
    if (!launch || loadedRef.current) return
    loadedRef.current = true
    let cancelled = false
    void (async () => {
      try {
        const g = await loadGame(launch.coreId, launch.romPath, romId || undefined)
        if (cancelled) return
        setAspect(g.aspectRatio > 0 ? g.aspectRatio : g.baseWidth / g.baseHeight || 4 / 3)
        // Garante que o jogo começa com foco (não no menu de pausa).
        const f = await currentFocus().catch(() => 'GameFocused' as const)
        if (f === 'MenuFocused') await toggleFocus().catch(() => {})
        setFocus('GameFocused')
        setStatus('ready')
      } catch (e) {
        if (!cancelled) setStatus({ error: String(e) })
      }
    })()
    return () => {
      cancelled = true
      void unloadGame().catch(() => {})
      loadedRef.current = false
    }
  }, [launch, romId, setFocus])

  const quickSave = useMutation({
    mutationFn: () => saveState(romId, 0),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['save-states', romId] })
      push(sysToast('QuickSave gravado.', 'Success'))
    },
    onError: (e) => push(sysToast(`Falha no save: ${e}`, 'Error')),
  })

  const quickLoad = useMutation({
    mutationFn: async () => {
      const list = await listSaveStates(romId)
      const quick = list.find((s) => s.slot === 0)
      if (!quick) throw new Error('nenhum QuickSave pra este jogo')
      await loadSaveState(quick.id)
    },
    onSuccess: () => push(sysToast('QuickLoad aplicado.', 'Success')),
    onError: (e) => push(sysToast(`Falha no load: ${e}`, 'Error')),
  })

  const resume = () => {
    void toggleFocus().then(setFocus).catch(() => setFocus('GameFocused'))
  }
  const quit = () => {
    void unloadGame().catch(() => {})
    navigate('/')
  }

  // A navegação do menu de pausa pelo gamepad é global (`useMenuNav` no
  // RootLayout). Aqui só focamos o 1º item ao abrir, pra `confirm` ter alvo.
  useEffect(() => {
    if (focus !== 'MenuFocused') return
    const raf = requestAnimationFrame(() => moveFocus('down'))
    return () => cancelAnimationFrame(raf)
  }, [focus])

  if (!launch && !roms.isLoading) {
    return (
      <div className={styles.center}>
        <div style={{ textAlign: 'center', display: 'flex', flexDirection: 'column', gap: 12 }}>
          <Body1>Não foi possível determinar o core/ROM.</Body1>
          <Button onClick={() => navigate(`/rom/${romId}`)}>Voltar ao detalhe</Button>
        </div>
      </div>
    )
  }

  if (status === 'loading') {
    const showArt = launch?.boxart && !artBroken
    return (
      <div className={styles.splash}>
        {showArt ? (
          <img
            className={styles.splashArt}
            src={launch.boxart ?? ''}
            alt={launch?.title ?? ''}
            onError={() => setArtBroken(true)}
          />
        ) : (
          <div className={styles.splashArt}>{initials(launch?.title ?? '?')}</div>
        )}
        <div className={styles.splashTitle}>{launch?.title ?? 'Carregando…'}</div>
        {launch?.system && <div className={styles.splashSub}>{launch.system}</div>}
        <Spinner size="small" label="Carregando…" />
      </div>
    )
  }

  if (typeof status === 'object') {
    return (
      <div className={styles.center}>
        <div style={{ textAlign: 'center', display: 'flex', flexDirection: 'column', gap: 12, maxWidth: 420 }}>
          <Title3>Falha ao carregar</Title3>
          <Body1>{status.error}</Body1>
          <Button appearance="primary" onClick={() => navigate(`/rom/${romId}`)}>
            Voltar
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className={styles.root}>
      <canvas
        ref={canvasRef}
        className={styles.canvas}
        width={256}
        height={240}
        style={{ aspectRatio: String(aspect) }}
      />

      {focus === 'MenuFocused' && (
        <div className={pause.scrim}>
          <div className={pause.panel}>
            <h2 className={pause.title}>Pausado</h2>
            <Button appearance="primary" onClick={resume}>
              Continuar
            </Button>
            <Button disabled={quickSave.isPending} onClick={() => quickSave.mutate()}>
              QuickSave
            </Button>
            <Button disabled={quickLoad.isPending} onClick={() => quickLoad.mutate()}>
              QuickLoad
            </Button>
            <Button onClick={() => navigate('/settings')}>Configurações</Button>
            <Button appearance="subtle" onClick={() => void toggleFullscreen()}>
              {fullscreen ? 'Sair da tela cheia' : 'Tela cheia'}
            </Button>
            <Button appearance="subtle" onClick={quit}>
              Sair do jogo
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
