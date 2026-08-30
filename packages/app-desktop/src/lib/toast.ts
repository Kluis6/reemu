import type { ToastItem, ToastVariant } from '../stores/useToastStore'

/** Cria um `ToastItem` de sistema com duração padrão por variante. */
export function sysToast(message: string, variant: ToastVariant = 'Info'): ToastItem {
  return {
    id: crypto.randomUUID(),
    message,
    variant,
    durationMs: variant === 'Error' ? 4500 : 2500,
    source: 'System',
  }
}
