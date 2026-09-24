#!/usr/bin/env bash
# Notas da versão a partir dos commits desde a tag anterior — vira o corpo da
# Release (draft) no release.yml. Dá pra editar no GitHub antes de publicar:
# o `latest.json` do auto-update é gerado na publicação, com o texto final.
#
# Uso: scripts/release-notes.sh <tag>   (precisa do histórico: fetch-depth 0)
set -euo pipefail

tag="${1:?uso: release-notes.sh <tag>}"
prev="$(git describe --tags --abbrev=0 "${tag}^" 2>/dev/null || true)"
range="${prev:+${prev}..}${tag}"

# Só feat/fix/perf entram — docs/ci/chore/refactor não interessam a quem usa.
section() {
  local title="$1" pattern="$2" lines
  lines="$(git log --no-merges --pretty='%s' "$range" |
    grep -E "^${pattern}(\([^)]*\))?!?: " |
    sed -E "s/^${pattern}(\([^)]*\))?!?: /- /" || true)"
  if [[ -n "$lines" ]]; then
    printf '## %s\n\n%s\n\n' "$title" "$lines"
  fi
}

# `$(...)` corta as quebras de linha finais — junta as seções com uma linha
# em branco explícita.
out=""
for s in "$(section "Novidades" feat)" "$(section "Correções" fix)" "$(section "Desempenho" perf)"; do
  [[ -n "$s" ]] && out+="${out:+$'\n\n'}$s"
done
printf '%s\n' "${out:-Melhorias internas e correções menores.}"
if [[ -n "$prev" ]]; then
  printf '\nMudanças desde %s.\n' "$prev"
fi
