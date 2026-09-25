import { describe, expect, it } from 'vitest'
import i18n from '../i18n'
import { describeError, errorText } from './errors'

describe('describeError', () => {
  it.each([
    // mensagens reais do reqwest / do sistema / do Rust do ReEmu
    ['error sending request for url (https://buildbot.libretro.com/x.zip)', /Sem conexão/, undefined],
    ['HTTP 404 Not Found', /não existe mais no servidor/, undefined],
    ['HTTP 429 Too Many Requests', /limitou as consultas/, undefined],
    ['ScreenScraper: 401 Unauthorized', /recusou o login/, 'metadata'],
    ['core exige HW render ainda não suportado: flycast: contexto GL: x', /placa de vídeo/, 'cores'],
    ['core-host: canal fechado (broken pipe)', /fechou inesperadamente/, 'cores'],
    ['core-host não respondeu (timeout)', /travou ao abrir/, 'cores'],
    ['core-host: o core encerrou inesperadamente ao carregar o jogo (signal: 11 (SIGSEGV))', /fechou inesperadamente/, 'cores'],
    ['operation timed out', /Sem conexão/, undefined],
    ['Firmware scph5501.bin missing', /BIOS/, 'bios'],
    ['nenhum core instalado pra megadrive', /Não há um core/, 'cores'],
    ['No such file or directory (os error 2)', /não encontrado/, undefined],
    ['Permission denied (os error 13)', /Sem permissão/, undefined],
    ['Acesso negado. (os error 5)', /Sem permissão/, undefined],
    ['No space left on device (os error 28)', /disco está cheio/, undefined],
    ['save state incompatível: core diferente', /save state/, undefined],
    ['the signature verification failed', /verificação de segurança/, undefined],
  ])('%s', (raw, title, fix) => {
    const d = describeError(raw, 'openGame')
    expect(d.title).toMatch(title)
    expect(d.fix).toBe(fix)
    expect(d.technical).toBe(raw)
  })

  it('erro desconhecido diz o que falhou e mantém o texto técnico', () => {
    const d = describeError('algo estranho aconteceu', 'saveHotkey')
    expect(d.title).toBe('Não foi possível salvar o atalho')
    expect(d.technical).toBe('algo estranho aconteceu')
    expect(d.hint).toMatch(/relate/)
  })
})

describe('errorText', () => {
  it('aceita Error, string e objeto', () => {
    expect(errorText(new Error('x'))).toBe('x')
    expect(errorText('y')).toBe('y')
    expect(errorText({ message: 'z' })).toBe('z')
  })
})

describe('describeError em outro idioma', () => {
  it('título e dica seguem o idioma ativo; o texto técnico não muda', async () => {
    await i18n.changeLanguage('en')
    const d = describeError('HTTP 429 Too Many Requests', 'fetchMetadata')
    expect(d.title).toBe('The server is limiting requests for now')
    expect(d.technical).toBe('HTTP 429 Too Many Requests')
    await i18n.changeLanguage('es')
    expect(describeError('algo raro', 'saveHotkey').title).toBe('No se pudo guardar el atajo')
    await i18n.changeLanguage('pt-BR')
  })
})
