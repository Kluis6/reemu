// Chaves de tradução tipadas pelo pt-BR: `t('nav.hom')` não compila.
import 'i18next'
import type ptBR from './locales/pt-BR'

declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation'
    resources: { translation: typeof ptBR }
  }
}
