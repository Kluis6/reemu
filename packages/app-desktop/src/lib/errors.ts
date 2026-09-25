// Traduz erros crus (texto do Rust, do sistema operacional, do reqwest…) numa
// mensagem que diz O QUE aconteceu e O QUE FAZER — heurística 9 de Nielsen
// (ajudar a reconhecer, diagnosticar e se recuperar de erros). O texto
// técnico original não some: vai em `technical`, pra "Copiar detalhes" e
// relato de problema.

import i18n from '../i18n'
import type { Messages } from '../i18n/types'

/** Lugar do app que resolve o problema — vira um botão na tela de erro. */
export type ErrorFix = 'cores' | 'bios' | 'metadata' | 'library'

export interface FriendlyError {
  /** Uma linha: o que deu errado, em português. */
  title: string
  /** O que fazer agora (pode faltar quando não há o que sugerir). */
  hint?: string
  /** Tela que resolve (ex.: instalar um core). */
  fix?: ErrorFix
  /** Mensagem original, limpa — pros detalhes técnicos. */
  technical: string
}

/** Texto do erro, seja `Error`, string do Rust (`invoke` rejeita com string)
 *  ou objeto. */
export function errorText(e: unknown): string {
  if (e instanceof Error) return e.message
  if (typeof e === 'string') return e
  if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message)
  try {
    return JSON.stringify(e)
  } catch {
    return String(e)
  }
}

/** Chave da regra em `errors.rules.<id>` (título + dica, i18n). */
type RuleId = keyof Messages['errors']['rules']

interface Rule {
  test: RegExp
  id: RuleId
  fix?: ErrorFix
}

// Ordem importa: a primeira regra que casa vence (as mais específicas antes).
const RULES: Rule[] = [
  {
    test: /signature|assinatura|minisign|pubkey/i,
    id: 'badSignature',
  },
  {
    // antes da regra de rede: "timeout" aqui é o core, não a internet
    test: /core-host não respondeu|core-host.*timeout/i,
    id: 'coreHung',
    fix: 'cores',
  },
  {
    test: /fora do Tauri/i,
    id: 'outsideApp',
  },
  {
    test: /\b429\b|too many requests|limite de (consultas|requisi)/i,
    id: 'rateLimited',
  },
  {
    test: /\b401\b|\b403\b|unauthori[sz]ed|forbidden|senha|credencia|login|chave (da api|inválida)/i,
    id: 'authFailed',
    fix: 'metadata',
  },
  {
    test: /error sending request|dns|failed to lookup|tcp connect|connection (refused|reset|closed)|(operation|request|connection|connect) timed? ?out|network|sem conex|HTTP 5\d\d|\b50[0-4]\b/i,
    id: 'offline',
  },
  {
    test: /\b404\b|not found.*http|http.*not found/i,
    id: 'notFoundRemote',
  },
  {
    test: /bios|firmware/i,
    id: 'missingBios',
    fix: 'bios',
  },
  {
    test: /contexto GL|HW render|hardware render|OpenGL|Vulkan|wglCreateContext|pixel format|driver de vídeo/i,
    id: 'gpuRejected',
    fix: 'cores',
  },
  {
    test: /core-host|encerrou inesperadamente|processo filho|broken pipe|pipe|canal .*fechad|encerrou inesperad|child/i,
    id: 'coreCrashed',
    fix: 'cores',
  },
  {
    test: /nenhum core|core não (instalado|encontrado)|sem core|core.*(not found|ausente)|carregar core: .*(no such file|não encontrad)/i,
    id: 'noCore',
    fix: 'cores',
  },
  {
    test: /incompatível com a plataforma|incompatible platform|arquitetura|wrong ELF|%1 is not a valid Win32|bad exe format/i,
    id: 'coreIncompatible',
    fix: 'cores',
  },
  {
    test: /save state|estado salvo|serializ|unserializ/i,
    id: 'saveStateIncompatible',
  },
  {
    test: /retro_load_game|não carregou o jogo|rom (inválida|não reconhecida)|nenhuma rom reconhecida|formato não suportado/i,
    id: 'romRejected',
    fix: 'library',
  },
  {
    test: /no space left|os error 28|os error 112|disco cheio|espaço insuficiente/i,
    id: 'diskFull',
  },
  {
    test: /permission denied|access is denied|acesso negado|os error 13|os error 5\b|sem permiss/i,
    id: 'noPermission',
  },
  {
    test: /no such file|cannot find the (file|path)|os error [23]\b|não encontrad|not found/i,
    id: 'notFound',
  },
  {
    test: /invalid (zip|archive)|corrupt|checksum|unexpected end of file|zip/i,
    id: 'corrupt',
  },
]

/** O que o usuário tentou fazer — chave em `actions.*` (i18n), usada em
 *  "Não foi possível {ação}" quando nenhuma regra reconhece o erro. */
export type ActionKey = keyof Messages['actions']

/** Traduz no idioma ativo na hora da chamada. Os padrões (`test`) casam o
 *  texto técnico, que vem do Rust/SO — independem do idioma da interface. */
export function describeError(e: unknown, action: ActionKey): FriendlyError {
  const technical = errorText(e)
    .replace(/^Error:\s*/i, '')
    .trim()
  const rule = RULES.find((r) => r.test.test(technical))
  if (rule) {
    const hint = i18n.t(`errors.rules.${rule.id}.hint`)
    return {
      title: i18n.t(`errors.rules.${rule.id}.title`),
      hint: hint || undefined,
      fix: rule.fix,
      technical,
    }
  }
  return {
    title: i18n.t('errors.cannot', { action: i18n.t(`actions.${action}`) }),
    hint: i18n.t('errors.genericHint'),
    technical,
  }
}
