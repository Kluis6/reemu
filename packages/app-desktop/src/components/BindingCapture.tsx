import {
  Badge,
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { describeRawInput, onRawInputCaptured, saveBinding } from '../lib/tauri'
import { useBindingCaptureStore } from '../stores/useBindingCaptureStore'
import { useToastStore } from '../stores/useToastStore'

const SETTLE_MS = 300

const useStyles = makeStyles({
  chips: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
    minHeight: '32px',
    alignItems: 'center',
  },
  hint: { color: tokens.colorNeutralForeground3 },
})

/**
 * Diálogo único de captura de binding, compartilhado por hotkey de sistema e
 * mapeamento de controle (`docs/ai-context/05`). Os dois `target` aceitam
 * combinação hold+press: cada evento reinicia a janela de ~300ms; quando ela
 * assenta, grava via `save_binding`.
 */
export function BindingCapture() {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const active = useBindingCaptureStore((s) => s.active)
  const events = useBindingCaptureStore((s) => s.events)
  const addEvent = useBindingCaptureStore((s) => s.addEvent)
  const reset = useBindingCaptureStore((s) => s.reset)

  // Escuta os eventos brutos enquanto a captura está ativa.
  useEffect(() => {
    if (!active) return
    let unlisten = () => {}
    let alive = true
    void onRawInputCaptured((ev) => addEvent(ev)).then((fn) => {
      if (alive) unlisten = fn
      else fn()
    })
    return () => {
      alive = false
      unlisten()
    }
  }, [active, addEvent])

  // Janela de assentamento: 300ms sem evento novo → grava.
  useEffect(() => {
    if (!active || events.length === 0) return
    const timer = window.setTimeout(() => {
      const { target, targetKey, label } = active
      void saveBinding(target, targetKey, events)
        .then(() => {
          push({
            id: crypto.randomUUID(),
            message: `Binding salvo: ${label}`,
            variant: 'Success',
            durationMs: 2500,
            source: 'System',
          })
          qc.invalidateQueries({ queryKey: ['system-hotkeys'] })
          qc.invalidateQueries({ queryKey: ['controller-mappings'] })
        })
        .catch((e) => {
          push({
            id: crypto.randomUUID(),
            message: `Falha ao salvar binding: ${e}`,
            variant: 'Error',
            durationMs: 4000,
            source: 'System',
          })
        })
        .finally(() => reset())
    }, SETTLE_MS)
    return () => window.clearTimeout(timer)
  }, [active, events, push, qc, reset])

  return (
    <Dialog open={active !== null} onOpenChange={(_, d) => !d.open && reset()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>Capturar atalho{active ? ` — ${active.label}` : ''}</DialogTitle>
          <DialogContent>
            <Text as="p" className={styles.hint}>
              Pressione a tecla ou o botão do controle. Segure a primeira e aperte outra
              para uma combinação. Grava sozinho após um instante.
            </Text>
            <div className={styles.chips}>
              {events.length === 0 ? (
                <Text className={styles.hint}>aguardando input…</Text>
              ) : (
                events.map((ev, i) => (
                  <Badge key={i} appearance="tint" size="large">
                    {describeRawInput(ev)}
                  </Badge>
                ))
              )}
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => reset()}>
              Cancelar
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  )
}
