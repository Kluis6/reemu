#!/usr/bin/env bash
# Compila o core de teste Vulkan oficial do libretro
# (libretro-samples/video/vulkan/vk_rendering — triângulo girando) pra validar
# a negociação de HW render Vulkan da etapa 12.
#
# Uso:  scripts/build-vk-test-core.sh
# Saída: target/vk-test-core/testvulkan_libretro.so
#
# O teste `cargo test -p core-loader-desktop --test vk_hw_render -- --ignored`
# procura o .so nesse caminho (ou em $REEMU_VK_TEST_CORE).
#
# Dependências: curl, tar, make, cc. O SPIR-V dos shaders já vem pré-compilado
# no repo (.inc), então NÃO precisa de glslang.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$REPO_ROOT/target/vk-test-core"
SO_NAME="testvulkan_libretro.so"
TARBALL="https://codeload.github.com/libretro/libretro-samples/tar.gz/refs/heads/master"
SUBDIR="libretro-samples-master/video/vulkan/vk_rendering"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "==> baixando libretro-samples"
curl -fsSL "$TARBALL" -o "$work/samples.tar.gz"

echo "==> extraindo $SUBDIR"
tar xzf "$work/samples.tar.gz" -C "$work" --strip-components=3 "$SUBDIR"

echo "==> compilando"
make -C "$work/vk_rendering" -j"$(nproc)"

mkdir -p "$OUT_DIR"
cp "$work/vk_rendering/$SO_NAME" "$OUT_DIR/$SO_NAME"
echo "==> pronto: $OUT_DIR/$SO_NAME"
