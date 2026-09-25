// Traduz erros crus (texto do Rust, do sistema operacional, do reqwest…) numa
// mensagem que diz O QUE aconteceu e O QUE FAZER — heurística 9 de Nielsen
// (ajudar a reconhecer, diagnosticar e se recuperar de erros). O texto
// técnico original não some: vai em `technical`, pra "Copiar detalhes" e
// relato de problema.

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

interface Rule {
  test: RegExp
  title: string
  hint?: string
  fix?: ErrorFix
}

// Ordem importa: a primeira regra que casa vence (as mais específicas antes).
const RULES: Rule[] = [
  {
    test: /signature|assinatura|minisign|pubkey/i,
    title: 'A atualização baixada não passou na verificação de segurança',
    hint: 'Nada foi instalado. Tente de novo mais tarde.',
  },
  {
    // antes da regra de rede: "timeout" aqui é o core, não a internet
    test: /core-host não respondeu|core-host.*timeout/i,
    title: 'O emulador travou ao abrir o jogo',
    hint: 'O core levou mais de 30 segundos para carregar e foi encerrado. Tente de novo ou escolha outro core para este sistema.',
    fix: 'cores',
  },
  {
    test: /fora do Tauri/i,
    title: 'Esta função só funciona dentro do app ReEmu',
  },
  {
    test: /\b429\b|too many requests|limite de (consultas|requisi)/i,
    title: 'O servidor limitou as consultas por agora',
    hint: 'Espere alguns minutos e tente de novo.',
  },
  {
    test: /\b401\b|\b403\b|unauthori[sz]ed|forbidden|senha|credencia|login|chave (da api|inválida)/i,
    title: 'O servidor recusou o login ou a chave de acesso',
    hint: 'Confira usuário, senha ou chave em Configurações › Metadados.',
    fix: 'metadata',
  },
  {
    test: /error sending request|dns|failed to lookup|tcp connect|connection (refused|reset|closed)|(operation|request|connection|connect) timed? ?out|network|sem conex|HTTP 5\d\d|\b50[0-4]\b/i,
    title: 'Sem conexão com a internet, ou o servidor não respondeu',
    hint: 'Confira a conexão e tente de novo em instantes.',
  },
  {
    test: /\b404\b|not found.*http|http.*not found/i,
    title: 'O arquivo não existe mais no servidor',
    hint: 'Pode ter sido removido ou renomeado lá. Tente de novo mais tarde.',
  },
  {
    test: /bios|firmware/i,
    title: 'Falta um arquivo de BIOS que este sistema precisa',
    hint: 'Veja em Configurações › BIOS quais arquivos faltam e importe-os.',
    fix: 'bios',
  },
  {
    test: /contexto GL|HW render|hardware render|OpenGL|Vulkan|wglCreateContext|pixel format|driver de vídeo/i,
    title: 'A placa de vídeo não aceitou o modo de desenho deste core',
    hint: 'Atualize o driver de vídeo ou escolha outro core para este sistema.',
    fix: 'cores',
  },
  {
    test: /core-host|processo filho|broken pipe|pipe|canal .*fechad|encerrou inesperad|child/i,
    title: 'O emulador fechou inesperadamente',
    hint: 'Tente abrir de novo ou escolha outro core para este jogo. Se repetir, relate o problema.',
    fix: 'cores',
  },
  {
    test: /nenhum core|core não (instalado|encontrado)|sem core|core.*(not found|ausente)|carregar core: .*(no such file|não encontrad)/i,
    title: 'Não há um core instalado para rodar este jogo',
    hint: 'Instale um core para este sistema em Configurações › Cores.',
    fix: 'cores',
  },
  {
    test: /incompatível com a plataforma|incompatible platform|arquitetura|wrong ELF|%1 is not a valid Win32|bad exe format/i,
    title: 'Este core não é compatível com este computador',
    hint: 'Baixe o core de novo em Configurações › Cores.',
    fix: 'cores',
  },
  {
    test: /save state|estado salvo|serializ|unserializ/i,
    title: 'O save state não é compatível com este core ou versão',
    hint: 'Save states só abrem no mesmo core em que foram criados.',
  },
  {
    test: /retro_load_game|não carregou o jogo|rom (inválida|não reconhecida)|nenhuma rom reconhecida|formato não suportado/i,
    title: 'O core não aceitou este arquivo de jogo',
    hint: 'Confira se o arquivo é do sistema certo e não está corrompido.',
    fix: 'library',
  },
  {
    test: /no space left|os error 28|os error 112|disco cheio|espaço insuficiente/i,
    title: 'O disco está cheio',
    hint: 'Libere espaço e tente de novo.',
  },
  {
    test: /permission denied|access is denied|acesso negado|os error 13|os error 5\b|sem permiss/i,
    title: 'Sem permissão para acessar o arquivo ou a pasta',
    hint: 'Feche programas que estejam usando o arquivo ou escolha outra pasta.',
  },
  {
    test: /no such file|cannot find the (file|path)|os error [23]\b|não encontrad|not found/i,
    title: 'Arquivo ou pasta não encontrado',
    hint: 'Ele pode ter sido movido, renomeado ou apagado.',
  },
  {
    test: /invalid (zip|archive)|corrupt|checksum|unexpected end of file|zip/i,
    title: 'O arquivo está corrompido ou incompleto',
    hint: 'Baixe ou copie o arquivo de novo.',
  },
]

/** `action` = o que o usuário tentou fazer, no infinitivo ("baixar o core"). */
export function describeError(e: unknown, action: string): FriendlyError {
  const technical = errorText(e)
    .replace(/^Error:\s*/i, '')
    .trim()
  const rule = RULES.find((r) => r.test.test(technical))
  if (rule) return { title: rule.title, hint: rule.hint, fix: rule.fix, technical }
  return {
    title: `Não foi possível ${action}`,
    hint: 'Tente de novo. Se repetir, copie os detalhes e relate o problema.',
    technical,
  }
}
