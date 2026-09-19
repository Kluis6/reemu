# Equivalente Windows do scripts/dev.sh — sobe o app desktop em modo dev
# (Vite + binário Rust juntos, via `cargo tauri dev`).
#
# `reemu-core-host` (processo filho que carrega o core) é um [[bin]] de
# `core-host-desktop`, crate de que o `reemu-desktop` NÃO depende — o
# `cargo tauri dev` sozinho nunca o recompila, e um core-host velho roda
# código velho sem erro nenhum (ver comentário do dev.sh). Por isso o build
# explícito antes.
#
# Uso (PowerShell, na raiz do repo):
#   .\scripts\dev.ps1
#   $env:RUST_LOG="debug"; .\scripts\dev.ps1
#   $env:REEMU_NATIVE_VIDEO="0"; .\scripts\dev.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not $env:RUST_LOG) { $env:RUST_LOG = "info" }

cargo build -p core-host-desktop
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json @args
exit $LASTEXITCODE
