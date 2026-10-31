export interface DiagnosticsData {
  appVersion: string
  language: string
  window: string
  hardware: unknown
  cores: unknown
  video: unknown
}

/** Relatório legível; o log entra depois, no backend, que também tira dados pessoais. */
export function formatDiagnostics(d: DiagnosticsData, generatedAt: Date): string {
  const section = (title: string, value: unknown) =>
    `## ${title}\n${JSON.stringify(value ?? null, null, 2)}`
  return [
    '# Diagnóstico do ReEmu',
    `Gerado em: ${generatedAt.toISOString()}`,
    `Versão: ${d.appVersion}`,
    `Idioma: ${d.language}`,
    `Janela: ${d.window}`,
    '',
    section('Hardware e GPU', d.hardware),
    '',
    section('Cores instalados', d.cores),
    '',
    section('Vídeo', d.video),
  ].join('\n')
}
