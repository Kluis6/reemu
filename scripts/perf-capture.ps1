# Abre o app em modo dev com REEMU_PERF=1 e grava toda a saída num arquivo. O
# terminal é o único lugar com as duas métricas: o `perf core` sai no stderr do
# processo do core e não entra no log do app. Roteiro em docs/perf/README.md.
#
# Uso (PowerShell, na raiz do repo, com o `tauri dev` parado):
#   .\scripts\perf-capture.ps1
#   .\scripts\perf-capture.ps1 -Saida target\perf\depois.log
param([string]$Saida = "target\perf\sessao.log")

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

# Sem encerrar nada: a porta pode ser de um `tauri dev` em uso.
$porta = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue
if ($porta) {
    $donos = ($porta.OwningProcess | Sort-Object -Unique) -join ", "
    Write-Host "A porta 1420 esta ocupada (PID $donos). Pare o tauri dev; se nao houver nenhum, rode:"
    Write-Host "  Stop-Process -Id $donos"
    exit 1
}

New-Item -ItemType Directory -Force (Split-Path $Saida) | Out-Null
$env:REEMU_PERF = "1"
# Compilação incremental desligada: já quebrou o link duas vezes nesta pasta e deixou
# o jogo rodando com um core-host antigo. A primeira execução recompila o projeto.
$env:CARGO_INCREMENTAL = "0"
if (-not $env:RUST_LOG) { $env:RUST_LOG = "info" }

Write-Host "Gravando em $Saida. O terminal fica em silencio ate fechar o app."
# cmd para o arquivo receber os bytes como saem, sem a conversão do PowerShell 5.1.
cmd /c "cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json > `"$Saida`" 2>&1"
Write-Host "Pronto: $Saida"
exit $LASTEXITCODE
