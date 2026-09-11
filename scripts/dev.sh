#!/usr/bin/env bash
# Sobe o app desktop em modo dev — Vite + binário Rust JUNTOS.
#
# `cargo run -p reemu-desktop` sozinho NÃO serve: a webview aponta pro Vite
# (127.0.0.1:1420) e dá "Connection refused". Este script usa `cargo tauri dev`,
# que sobe o Vite (beforeDevCommand) e espera ele responder antes de rodar o
# Rust.
#
# Uso:
#   scripts/dev.sh                       # log em info
#   RUST_LOG=debug scripts/dev.sh
#   scripts/dev.sh --vk-validation       # + sync validation da camada Khronos
#   scripts/dev.sh 2>&1 | grep VkFormat  # filtrar a saída
#
#   REEMU_WEBKIT_COMPOSITING=1 scripts/dev.sh   # força GPU no WebKitGTK
#     Auto-ligado em AMD/Intel; desligado em NVIDIA proprietário (tela branca).
#     Teste com =1 se o driver NVIDIA for novo; =0 pra forçar software.
set -euo pipefail
cd "$(dirname "$0")/.."

export RUST_LOG="${RUST_LOG:-info}"

if [[ "${1:-}" == "--vk-validation" ]]; then
  shift
  export VK_LAYER_SETTINGS_PATH="$PWD"
  export VK_INSTANCE_LAYERS="VK_LAYER_KHRONOS_validation"
  echo "dev.sh: sync validation Vulkan LIGADA (vk_layer_settings.txt)" >&2
fi

exec cargo tauri dev --config apps/desktop/src-tauri/tauri.conf.json "$@"
