#!/usr/bin/env python3
"""Gera as entradas novas do catálogo de cores (apps/desktop/src-tauri/src/core_catalog.rs).

Fontes oficiais, nada de memória:
  - libretro/libretro-core-info (.info de cada core: nome, sistema, licença,
    categoria, extensões, API gráfica exigida);
  - listagem do buildbot (nightly/<os>/x86_64/latest/) de Linux E Windows;
  - crates/library-scan/src/systems.rs (extensões que o scan reconhece).

Uso:
  git clone --depth 1 https://github.com/libretro/libretro-core-info /tmp/core-info
  scripts/gen_core_catalog.py /tmp/core-info > /tmp/novos.rs

Imprime em Rust só os cores que AINDA NÃO estão no catálogo, prontos pra colar
no fim de `CATALOG`, e um resumo dos descartados (com o motivo) no stderr.
"""

import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG_RS = ROOT / "apps/desktop/src-tauri/src/core_catalog.rs"
SYSTEMS_RS = ROOT / "crates/library-scan/src/systems.rs"
BUILDBOT = "https://buildbot.libretro.com/nightly/{}/x86_64/latest/"
# Cores que o `.info` descreve incompleto demais pra classificar sozinho.
EXCLUDE = {
    "play_libretro": "PS2 em OpenGL, mas o .info não declara hw_render nem API",
    "mesen2_libretro": "nome de sistema gigante; os sistemas dele já têm cores no catálogo",
}


def parse_info(path: Path) -> dict:
    info = {}
    for line in path.read_text(errors="replace").splitlines():
        m = re.match(r'\s*([a-z_0-9]+)\s*=\s*"(.*)"\s*$', line)
        if m:
            info[m.group(1)] = m.group(2)
    return info


def buildbot_cores(os_path: str, ext: str) -> set:
    html = urllib.request.urlopen(BUILDBOT.format(os_path), timeout=60).read().decode()
    return set(re.findall(rf"([a-z0-9_]+_libretro)\.{ext}\.zip", html))


def scan_systems() -> dict:
    """Nome de pasta/banco libretro (minúsculo) → `system_id` do scan.

    Mesma tabela que o `library-scan` usa pra reconhecer a pasta das ROMs
    (`system_from_folder_name`), que já traz os nomes dos bancos libretro
    (`nintendo - super nintendo entertainment system`…).
    """
    src = SYSTEMS_RS.read_text()
    fn = src[src.index("pub fn system_from_folder_name") : src.index("pub fn folder_only_exts")]
    table = {}
    for names, system_id in re.findall(r'((?:"[^"]+"\s*\|?\s*)+)=>\s*"([a-z0-9]+)"', fn):
        for name in re.findall(r'"([^"]+)"', names):
            table[name] = system_id
    return table


def render_class(info: dict):
    """`sw`/`gl`, ou `None` se o ReEmu não tem como rodar o core."""
    apis = [a.strip() for a in info.get("required_hw_api", "").split("|") if a.strip()]
    if any(re.match(r"OpenGL(?! ES)", a) for a in apis):
        return "gl"
    if apis or info.get("hw_render") == "true":
        # Só Vulkan/Direct3D/GLES, ou GPU sem dizer qual API — fora (doc 12).
        return None
    return "sw"


def scan_system_ids(info: dict, table: dict) -> set:
    dbs = info.get("database", "").split("|")
    return {table[d.strip().lower()] for d in dbs if d.strip().lower() in table}


def system_label(info: dict) -> str:
    """`Sega - Saturn (Yabause)` → `Sega Saturn`, igual ao padrão libretro."""
    label = info.get("display_name", "").split(" (", 1)[0].replace(" - ", " ").strip()
    return label or info.get("systemname", "?")


def is_arcade(info: dict) -> bool:
    db = info.get("database", "") + " " + info.get("systemname", "")
    return bool(re.search(r"\b(Arcade|MAME|FBNeo|FB Alpha)\b", db))


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    info_dir = Path(sys.argv[1])
    have = set(re.findall(r'\b(?:sw|gl|vk)\(\s*"([a-z0-9_]+)"', CATALOG_RS.read_text()))
    linux = buildbot_cores("linux", "so")
    windows = buildbot_cores("windows", "dll")
    table = scan_systems()

    skipped = {}
    out = []
    for path in sorted(info_dir.glob("*_libretro.info")):
        core_id = path.stem
        if core_id in have:
            continue
        info = parse_info(path)
        why = None
        if core_id in EXCLUDE:
            why = "exclusão manual"
        elif info.get("categories", "") not in ("Emulator", "Game"):
            # "Game" entra só por causa do filtro de sistema abaixo (na
            # prática: ScummVM); os demais "Game" não têm sistema no scan.
            why = "não é emulador"
        elif core_id not in linux or core_id not in windows:
            why = "fora do buildbot Linux+Windows"
        elif not scan_system_ids(info, table) and not is_arcade(info):
            why = "sistema que o scan não reconhece"
        elif render_class(info) is None:
            why = "API gráfica sem suporte"
        if why:
            skipped.setdefault(why.split(":")[0], []).append(core_id)
            continue
        kind = render_class(info)
        name = info.get("corename") or info.get("display_name") or core_id
        system = system_label(info)
        license_ = (info.get("license") or "?").split("|")[0].strip()[:40]
        out.append((system, name, core_id, kind, license_))

    out.sort(key=lambda e: (e[0].lower(), e[1].lower()))
    print("    // --- gerados por scripts/gen_core_catalog.py (libretro-core-info) ---")
    for system, name, core_id, kind, license_ in out:
        print(f"    {kind}({rust_str(core_id)}, {rust_str(name)}, {rust_str(system)}, {rust_str(license_)}),")

    print(f"\n{len(out)} cores novos; já no catálogo: {len(have)}", file=sys.stderr)
    for why, ids in sorted(skipped.items(), key=lambda kv: -len(kv[1])):
        print(f"  descartados ({why}): {len(ids)}", file=sys.stderr)
        if why in ("API gráfica sem suporte", "exclusão manual"):
            print("    " + " ".join(ids), file=sys.stderr)


if __name__ == "__main__":
    main()
