import i18n from '../i18n'
import { describeError, type ActionKey, type ErrorFix } from './errors'
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
const FIX_ROUTES: Record<ErrorFix, string> = {
  cores: '#/settings/cores',
  bios: '#/settings/bios',
  metadata: '#/settings/metadata',
  library: '#/library',
}

/** Rótulo (no idioma ativo) e rota da tela que resolve o erro. */
export function fixRoute(fix: ErrorFix): { label: string; hash: string } {
  return { label: i18n.t(`errors.fix.${fix}`), hash: FIX_ROUTES[fix] }
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
export function errorToast(e: unknown, action: ActionKey): ToastItem {
  const d = describeError(e, action)
  const fix = d.fix ? fixRoute(d.fix) : null
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
      { label: i18n.t('errors.copyDetails'), onClick: () => copyErrorDetails(d.title, d.technical) },
    ],
  }
}

/** Mesmo conteúdo do `errorToast`, pra transformar um toast já aberto (de
 *  progresso: scan, download) no toast de erro, sem abrir outro. */
export function errorPatch(e: unknown, action: ActionKey): Omit<ToastItem, 'id'> {
  const { id: _id, ...rest } = errorToast(e, action)
  return { ...rest, progress: undefined }
}
