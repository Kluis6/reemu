// Idiomas da interface: pt-BR (origem), en, es — i18next + react-i18next.
//
// Uso nos componentes: `const { t } = useTranslation()` e `t('nav.home')`.
// Chaves tipadas pelo pt-BR (ver `i18next.d.ts`): chave errada não compila.
// Texto novo: some a chave no pt-BR e nos outros dois (o `Messages` e o
// teste `locales.test.ts` cobram que ninguém fique pra trás).
//
// Escolha: preferência salva (`reemu.language`: um idioma ou `auto`) →
// `auto` segue o idioma do sistema (`navigator.languages`) → sem
// correspondência, pt-BR.

import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en'
import es from './locales/es'
import ptBR from './locales/pt-BR'
import { LANGUAGES, type Language } from './types'

export { LANGUAGES, type Language } from './types'

export type LanguagePreference = Language | 'auto'

const KEY = 'reemu.language'

/** `pt-PT`/`pt` → pt-BR, `es-MX` → es, `en-GB` → en; resto → `null`. */
export function matchLanguage(tag: string): Language | null {
  const base = tag.toLowerCase().split(/[-_]/)[0]
  if (base === 'pt') return 'pt-BR'
  if (base === 'es') return 'es'
  if (base === 'en') return 'en'
  return null
}

/** Idioma do sistema (o primeiro que suportamos da lista do navegador). */
export function systemLanguage(
  tags: readonly string[] = typeof navigator === 'undefined' ? [] : (navigator.languages ?? []),
): Language {
  for (const t of tags) {
    const m = matchLanguage(t)
    if (m) return m
  }
  return 'pt-BR'
}

export function getLanguagePreference(): LanguagePreference {
  try {
    const v = localStorage.getItem(KEY)
    if (v === 'auto' || (LANGUAGES as readonly string[]).includes(v ?? '')) {
      return v as LanguagePreference
    }
  } catch {
    // sem storage
  }
  return 'auto'
}

function resolve(pref: LanguagePreference): Language {
  return pref === 'auto' ? systemLanguage() : pref
}

/** Muda o idioma na hora e guarda a preferência. */
export async function setLanguagePreference(pref: LanguagePreference): Promise<void> {
  try {
    localStorage.setItem(KEY, pref)
  } catch {
    // sem storage: vale até fechar o app
  }
  await i18n.changeLanguage(resolve(pref))
}

/** `<html lang>` acompanha o idioma (leitor de tela, hifenização). */
function setDocumentLang(lng: string) {
  if (typeof document !== 'undefined') document.documentElement.lang = lng
}

i18n.on('languageChanged', setDocumentLang)

void i18n.use(initReactI18next).init({
  resources: {
    'pt-BR': { translation: ptBR },
    en: { translation: en },
    es: { translation: es },
  },
  lng: resolve(getLanguagePreference()),
  fallbackLng: 'pt-BR',
  supportedLngs: [...LANGUAGES],
  // o React já escapa o que renderiza
  interpolation: { escapeValue: false },
})
setDocumentLang(i18n.language)

export default i18n
