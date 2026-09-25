import { describeError, type ErrorFix } from './errors'
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

/** Onde fica a solução de cada tipo de erro (rotas do HashRouter). */
const FIX_ROUTES: Record<ErrorFix, { label: string; hash: string }> = {
  cores: { label: 'Abrir Cores', hash: '#/settings/cores' },
  bios: { label: 'Abrir BIOS', hash: '#/settings/bios' },
  metadata: { label: 'Abrir Metadados', hash: '#/settings/metadata' },
  library: { label: 'Abrir biblioteca', hash: '#/library' },
}

export function fixRoute(fix: ErrorFix) {
  return FIX_ROUTES[fix]
}

/** Copia título + texto técnico — pra relatar o problema. */
export function copyErrorDetails(title: string, technical: string) {
  navigator.clipboard?.writeText(`${title}\n${technical}`).catch(() => {})
}

/**
 * Toast de erro legível: o que aconteceu (título), o que fazer (mensagem) e
 * o texto técnico em letra menor; botões pra ir à solução e pra copiar os
 * detalhes. `action` = o que o usuário tentou fazer, no infinitivo.
 */
export function errorToast(e: unknown, action: string): ToastItem {
  const d = describeError(e, action)
  const fix = d.fix ? FIX_ROUTES[d.fix] : null
  return {
    id: crypto.randomUUID(),
    title: d.title,
    message: d.hint ?? '',
    detail: d.technical,
    variant: 'Error',
    durationMs: 9000,
    source: 'System',
    action: fix ? { label: fix.label, onClick: () => (window.location.hash = fix.hash) } : undefined,
    moreActions: [
      { label: 'Copiar detalhes', onClick: () => copyErrorDetails(d.title, d.technical) },
    ],
  }
}

/** Mesmo conteúdo do `errorToast`, pra transformar um toast já aberto (de
 *  progresso: scan, download) no toast de erro, sem abrir outro. */
export function errorPatch(e: unknown, action: string): Omit<ToastItem, 'id'> {
  const { id: _id, ...rest } = errorToast(e, action)
  return { ...rest, progress: undefined }
}
