import type { TFunction } from 'i18next'
import i18n from '../i18n'

// Textos que o backend manda em pt-BR (shaders prontos, notas de BIOS)
// traduzidos pelo id; sem tradução, fica o texto do Rust.

const keyPart = (s: string) => s.replace(/[^A-Za-z0-9]/g, '_')

function lookup(t: TFunction, key: string, fallback: string): string {
  // chave montada em runtime: fora da união tipada das chaves
  return i18n.exists(key) ? (t as unknown as (k: string) => string)(key) : fallback
}

/** Nome/descrição de um shader pronto (`id` = `curated:<id>`). */
export function curatedText(
  t: TFunction,
  id: string,
  field: 'label' | 'desc',
  fallback: string,
): string {
  return lookup(t, `video.curated.${keyPart(id.replace(/^curated:/, ''))}.${field}`, fallback)
}

/** Nota de um arquivo de BIOS (`crates/domain/src/bios.rs`), pelo nome do arquivo. */
export function biosNote(t: TFunction, filename: string, fallback: string): string {
  return lookup(t, `bios.notes.${keyPart(filename)}`, fallback)
}

/** Nome de um preset embutido (`plain`/`crt`/`lcd`); outros ficam como vieram. */
export function presetTitle(t: TFunction, name: string): string {
  return lookup(t, `video.presets.${keyPart(name)}.title`, name)
}
