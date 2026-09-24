#!/usr/bin/env bash
# `cargo check` do workspace inteiro pra Windows, rodando no Linux — pega
# erro de compilação específico de Windows (código atrás de `cfg(unix)`
# faltando, import só-Linux…) em minutos, sem esperar o release no CI.
#
# Só CHECA o Rust (não linka, não gera .exe). Os build scripts que compilam
# C/C++ (glslang, ring, sqlite, testcore…) recebem um compilador FALSO que só
# cria os arquivos de saída vazios — o que importa aqui é o código Rust.
#
# Uso:  scripts/check-windows.sh            (1ª vez instala o target via rustup)
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=x86_64-pc-windows-gnu
rustup target list --installed | grep -qx "$TARGET" || rustup target add "$TARGET"

# Fora de /tmp: build scripts rodam de dentro do target dir, e /tmp costuma
# ser `noexec`.
export CARGO_TARGET_DIR=target/win-check
FAKE="$CARGO_TARGET_DIR/fake-cc"
mkdir -p "$FAKE"
cat > "$FAKE/fake" <<'EOF'
#!/usr/bin/env bash
# Compilador/windres falso: cria o arquivo de saída pedido e sai com sucesso.
# Aceita `-o X`, `-oX`, `--output X`, `--output=X` e o `-FoX`/`/FoX` que o
# crate `cc` usa quando acha que o compilador é estilo MSVC.
out=""; prev=""
for a in "$@"; do
  case "$prev" in -o|--output|-O) out="$a";; esac
  case "$a" in
    -o|--output) ;;
    --output=*) out="${a#--output=}";;
    -o*) out="${a#-o}";;
    -Fo*) out="${a#-Fo}";;
    /Fo*) out="${a#/Fo}";;
  esac
  prev="$a"
done
case " $* " in *" --version "*|*" -dumpversion "*) echo "10.0"; exit 0;; esac
[ -n "$out" ] && : > "$out"
exit 0
EOF
chmod +x "$FAKE/fake"
for tool in gcc g++ windres; do
  ln -sf fake "$FAKE/x86_64-w64-mingw32-$tool"
done

PATH="$PWD/$FAKE:$PATH" \
  CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
  CXX_x86_64_pc_windows_gnu=x86_64-w64-mingw32-g++ \
  AR_x86_64_pc_windows_gnu=ar \
  cargo check --target "$TARGET" --workspace "$@"
