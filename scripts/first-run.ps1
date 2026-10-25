# Abre o ReEmu como na primeira vez (onboarding), sem tocar nos dados reais.
#
# - REEMU_DATA_DIR: pasta de dados nova (banco, perfil, app-settings.json,
#   cores, saves…) — ver `data_dir` em apps/desktop/src-tauri/src/lib.rs.
# - WEBVIEW2_USER_DATA_FOLDER: localStorage novo (tema, idioma, tamanho da
#   interface…). O WebView2 usa a variável no lugar da pasta que o Tauri
#   passa: "use the browserExecutableFolder and userDataFolder values as
#   replacements" (Microsoft Learn, CreateCoreWebView2EnvironmentWithOptions).
#
# Uso (com o `pnpm tauri:desktop` de sempre fechado — os dois usam a porta 1420):
#   scripts/first-run.ps1          recomeça do zero a cada execução
#   scripts/first-run.ps1 -Keep    continua de onde parou (ex.: depois do onboarding)
param([switch]$Keep)

$root = Join-Path $env:TEMP 'reemu-primeira-abertura'
if (-not $Keep -and (Test-Path $root)) {
    Remove-Item -Recurse -Force $root
}
New-Item -ItemType Directory -Force (Join-Path $root 'dados'), (Join-Path $root 'webview') | Out-Null

$env:REEMU_DATA_DIR = Join-Path $root 'dados'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $root 'webview'
Write-Host "ReEmu de primeira abertura: $root"

Set-Location (Join-Path $PSScriptRoot '..')
pnpm tauri:desktop
