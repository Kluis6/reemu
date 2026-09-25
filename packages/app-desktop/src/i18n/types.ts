import type ptBR from './locales/pt-BR'

/** Mesmo formato do pt-BR, com qualquer texto no lugar. */
type Shape<T> = { [K in keyof T]: T[K] extends string ? string : Shape<T[K]> }

export type Messages = Shape<typeof ptBR>

/** Idiomas suportados. `pt-BR` é o de origem e o reserva. */
export const LANGUAGES = ['pt-BR', 'en', 'es'] as const
export type Language = (typeof LANGUAGES)[number]
