# Equivalente Windows do scripts/dev.sh — sobe o app desktop em modo dev
# (Vite + binário Rust juntos, via `cargo tauri dev`).
#
# `reemu-core-host` (processo filho que carrega o core) é recompilado pelo
# `beforeDevCommand` do `tauri.conf.json` (ver comentário do dev.sh).
#
# Uso (PowerShell, na raiz do repo):
#   .\scripts\dev.ps1
#   $env:RUST_LOG="debug"; .\scripts\dev.ps1
#   $env:REEMU_NATIVE_VIDEO="0"; .\scripts\dev.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not $env:RUST_LOG) { $env:RUST_LOG = "info" }

cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json @args
exit $LASTEXITCODE
