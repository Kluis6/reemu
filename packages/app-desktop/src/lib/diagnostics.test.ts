import { describe, expect, it } from 'vitest'
import { formatDiagnostics } from './diagnostics'

describe('diagnóstico', () => {
  it('traz versão, hardware, cores e vídeo', () => {
    const text = formatDiagnostics(
      {
        appVersion: '1.2.3',
        language: 'pt-BR',
        window: '1280x720',
        hardware: { gpu: 'Teste' },
        cores: [{ coreId: 'snes9x' }],
        video: { hdr: false },
      },
      new Date('2026-10-31T00:00:00Z'),
    )
    expect(text).toContain('Versão: 1.2.3')
    expect(text).toContain('"gpu": "Teste"')
    expect(text).toContain('"coreId": "snes9x"')
    expect(text).toContain('"hdr": false')
  })

  it('dado que falhou não derruba o relatório', () => {
    const text = formatDiagnostics(
      { appVersion: '', language: '', window: '', hardware: undefined, cores: undefined, video: undefined },
      new Date(0),
    )
    expect(text).toContain('## Vídeo\nnull')
  })
})
