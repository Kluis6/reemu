#!/usr/bin/env python3
"""Gera crates/domain/src/bios_generated.rs: os BIOS/firmwares de cada sistema
do ReEmu que a tabela escrita à mão (crates/domain/src/bios.rs) ainda não tem.

Fontes oficiais, nada de memória:
  - libretro/libretro-database `dat/System.dat`: a lista de arquivos de
    sistema que o RetroArch usa, com caminho dentro de `system/` e MD5;
  - libretro/libretro-core-info: `firmwareN_path/_desc/_opt` de cada core —
    diz se o arquivo é obrigatório e dá a descrição;
  - o catálogo de cores do app (`core_catalog::system_ids`), exportado num
    arquivo `MAP <core> <sistema,sistema>` por linha (ver o --help).

Obrigatório (`required`) segue a regra da tabela à mão: só quando TODOS os
cores do catálogo pra aquele sistema marcam o arquivo com `opt = false`.

Pasta com muitos dumps equivalentes (o PS2: qualquer BIOS em `pcsx2/bios/`
serve) vira UMA entrada "qualquer um destes" (`any_of`), em vez de dezenas
de linhas.

Uso:
  git clone --depth 1 https://github.com/libretro/libretro-core-info /tmp/core-info
  curl -L -o /tmp/System.dat \\
    https://raw.githubusercontent.com/libretro/libretro-database/master/dat/System.dat
  # mapa core → sistemas do app (teste temporário imprimindo system_ids)
  scripts/gen_bios_table.py /tmp/System.dat /tmp/core-info /tmp/coremap.txt \\
    > crates/domain/src/bios_generated.rs
"""

import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HANDWRITTEN = ROOT / "crates/domain/src/bios.rs"

# Sistema do app → seções (`comment`) do System.dat. Nomes do System.dat nem
# sempre batem com os do thumbnails ("Gameboy", "Mega CD", "NeoGeo CD").
SECTIONS = {
    "nes": ["Nintendo - Nintendo Entertainment System", "Nintendo - Famicom Disk System"],
    "snes": [
        "Nintendo - Super Nintendo Entertainment System",
        "Nintendo - Satellaview",
        "Nintendo - SuFami Turbo",
        "Nintendo - Super Game Boy",
    ],
    "gb": ["Nintendo - Gameboy"],
    "gbc": ["Nintendo - Gameboy Color"],
    "gba": ["Nintendo - Game Boy Advance"],
    "n64": ["Nintendo - Nintendo 64DD"],
    "nds": ["Nintendo - Nintendo DS"],
    "pokemini": ["Nintendo - Pokemon Mini"],
    "megadrive": ["Sega - Mega Drive - Genesis"],
    "segacd": ["Sega - Mega CD - Sega CD"],
    "mastersystem": ["Sega - Master System - Mark III"],
    "gamegear": ["Sega - Game Gear"],
    "saturn": ["Sega - Saturn"],
    "dreamcast": ["Sega - Dreamcast"],
    "naomi": ["Sega - Dreamcast-based Arcade"],
    "atomiswave": ["Sega - Dreamcast-based Arcade"],
    "neogeocd": ["SNK - NeoGeo CD"],
    "cdi": ["Philips - CD-i"],
    "pcenginecd": ["NEC - PC Engine - TurboGrafx 16 - SuperGrafx"],
    "pcfx": ["NEC - PC-FX"],
    "atari5200": ["Atari - 5200"],
    "atari7800": ["Atari - 7800"],
    "atari8bit": ["Atari - 400-800"],
    "lynx": ["Atari - Lynx"],
    "coleco": ["Coleco - ColecoVision"],
    "intellivision": ["Mattel - Intellivision"],
    "3do": ["3DO Company, The - 3DO"],
    "psx": ["Sony - PlayStation"],
    "ps2": ["Sony - PlayStation 2"],
    "msx": ["Microsoft - MSX"],
    "odyssey2": ["Magnavox - Odyssey2", "Phillips - Videopac+"],
    "amiga": ["Commodore - Amiga"],
    "zxspectrum": ["Sinclair - ZX Spectrum"],
    "amstradcpc": ["Amstrad - CPC"],
    "dos": ["DOS"],
    "arcade": ["Arcade"],
}
# A seção do System.dat junta NAOMI e Atomiswave; separa pelo nome.
SPLIT = {
    "atomiswave": lambda p: "awbios" in p.lower(),
    "naomi": lambda p: "awbios" not in p.lower(),
}
# Não é BIOS: bancos de dados de jogos, fontes, cartuchos de trapaça/add-on.
NOT_FIRMWARE_EXT = (".xml", ".yaml", ".dat", ".db", ".sha", ".fnt", ".dll", ".so", ".dylib")
# O `.info` marca obrigatórios arquivos que são ALTERNATIVAS por modelo. O2EM:
# "Currently the libretro core only works with the o2rom.bin" (os outros são
# do hardware escolhido nas opções) — docs.libretro.com/library/o2em/.
REQUIRED_OVERRIDE = {
    ("odyssey2", "c52.bin"): False,
    ("odyssey2", "g7400.bin"): False,
    ("odyssey2", "jopac.bin"): False,
}
NOT_FIRMWARE_DESC = ("database", "game genie", "action replay", "sonic & knuckles",
                     "high score", "folder")
# Pastas onde qualquer dump conhecido serve → uma entrada `any_of`.
SLOTS = {
    "ps2": ("pcsx2/bios", "BIOS do PlayStation 2 (qualquer região/revisão)"),
}


def parse_system_dat(path):
    sections = defaultdict(list)
    current = None
    for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
        m = re.match(r'\s*comment "(.*)"', line)
        if m:
            current = m.group(1)
            continue
        # o nome vem entre aspas só quando tem espaço ("BS-X (En).bin")
        m = re.match(r'\s*rom \( name (?:"([^"]+)"|(\S+)) .*?md5 ([0-9a-fA-F]{32})', line)
        if m and current:
            sections[current].append((m.group(1) or m.group(2), m.group(3).lower()))
    return sections


def parse_info(path):
    kv = {}
    for k, v in re.findall(r"^\s*(\w+)\s*=\s*(.*?)\s*$", path.read_text(errors="replace"), re.M):
        kv[k] = v.strip().strip('"')
    out = {}
    for i in range(int(kv.get("firmware_count", "0") or 0)):
        p = kv.get(f"firmware{i}_path", "")
        if p:
            out[p] = (kv.get(f"firmware{i}_opt", "true") == "true", kv.get(f"firmware{i}_desc", ""))
    return out


def handwritten_files():
    """{sistema: {caminho}} do que bios.rs já tem (não repete)."""
    src = HANDWRITTEN.read_text(encoding="utf-8")
    # cada `const X: &[BiosFile]` vai até o começo do próximo (há consts de
    # uma linha só, `&[BiosFile { .. }];`, e de várias)
    starts = [(m.start(), m.group(1)) for m in re.finditer(r"const (\w+): &\[BiosFile\]", src)]
    consts = {}
    for i, (pos, name) in enumerate(starts):
        end = starts[i + 1][0] if i + 1 < len(starts) else src.index("pub fn bios_files_for_system")
        body = src[pos:end]
        files = set()
        for b in re.finditer(r'filename: "([^"]+)",\s*subfolder: (None|Some\("([^"]+)"\))', body):
            files.add((b.group(3) + "/" if b.group(3) else "") + b.group(1))
        consts[name] = files
    out = {}
    for m in re.finditer(r'"(\w+)" => (\w+),', src):
        out[m.group(1)] = consts.get(m.group(2), set())
    return out


def rs(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def main():
    # o console do Windows escreveria em cp1252 (acentos quebrados no .rs)
    sys.stdout.reconfigure(encoding="utf-8", newline="")
    dat, info_dir, coremap = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
    sections = parse_system_dat(dat)
    cores_of = defaultdict(list)
    for line in Path(coremap).read_text().splitlines():
        parts = line.split()
        if len(parts) >= 3:
            for s in parts[2].split(","):
                cores_of[s].append(parts[1])
    fw = {c: parse_info(info_dir / f"{c}.info") for cs in cores_of.values() for c in cs
          if (info_dir / f"{c}.info").exists()}
    have = handwritten_files()

    def declared(system, path):
        """Algum core DO CATÁLOGO pra esse sistema usa o arquivo? (O
        System.dat lista o de todos os cores da libretro.)"""
        return any(path in fw.get(c, {}) for c in cores_of[system])

    def skip(path, system):
        desc = " ".join(fw.get(c, {}).get(path, (True, ""))[1] for c in cores_of[system]).lower()
        name = path.lower()
        return name.endswith(NOT_FIRMWARE_EXT) or any(k in desc for k in NOT_FIRMWARE_DESC)

    def required_and_desc(system, path):
        cores = [c for c in cores_of[system] if c in fw]
        hits = [fw[c][path] for c in cores if path in fw[c]]
        desc = next((d for _, d in hits if d), "")
        req = bool(cores) and len(hits) == len(cores) and all(not opt for opt, _ in hits)
        return req, desc

    out = ["// GERADO por scripts/gen_bios_table.py — não editar à mão.",
           "// Fontes: libretro-database dat/System.dat + libretro-core-info.",
           "",
           "use crate::bios::BiosFile;",
           "",
           "pub(crate) const GENERATED: &[(&str, BiosFile)] = &["]
    systems = []
    for system, secs in SECTIONS.items():
        if system not in cores_of:
            continue  # sistema sem core no catálogo
        roms = [(p, h) for s in secs for (p, h) in sections.get(s, [])]
        if system in SPLIT:
            roms = [(p, h) for p, h in roms if SPLIT[system](p)]
        known = {f.split("/")[-1] for f in have.get(system, set())}
        entries = []
        if system in SLOTS:
            folder, note = SLOTS[system]
            dumps = [(p.split("/")[-1], h) for p, h in roms if p.startswith(folder + "/")]
            roms = [(p, h) for p, h in roms if not p.startswith(folder + "/")]
            if dumps:
                req, _ = required_and_desc(system, folder)
                entries.append((folder, None, [], req, note, dumps))
        by_path = defaultdict(list)
        for p, h in roms:
            if p.split("/")[-1] not in known:
                by_path[p].append(h)
        for p, hashes in by_path.items():
            sub, _, name = p.rpartition("/")
            if not declared(system, p) or skip(p, system):
                continue
            req, desc = required_and_desc(system, p)
            req = REQUIRED_OVERRIDE.get((system, p), req)
            entries.append((name, sub or None, sorted(set(hashes)), req, desc, []))
        if not entries:
            continue
        systems.append(system)
        for name, sub, hashes, req, note, dumps in entries:
            if dumps:
                sub, name = name, "*"
            out.append(f"    ({rs(system)}, BiosFile {{")
            out.append(f"        filename: {rs(name)},")
            out.append(f"        subfolder: {('Some(' + rs(sub) + ')') if sub else 'None'},")
            out.append(f"        md5: &[{', '.join(rs(h) for h in hashes)}],")
            out.append(f"        required: {'true' if req else 'false'},")
            out.append(f"        note: {rs(note)},")
            if dumps:
                out.append("        any_of: &[")
                for n, h in dumps:
                    out.append(f"            ({rs(n)}, {rs(h)}),")
                out.append("        ],")
            else:
                out.append("        any_of: &[],")
            out.append("    }),")
    out.append("];")
    out.append("")
    out.append("/// Sistemas que só aparecem na parte gerada (fora da tabela à mão).")
    out.append("pub(crate) const GENERATED_SYSTEMS: &[&str] = &[")
    for s in systems:
        if s not in have:
            out.append(f"    {rs(s)},")
    out.append("];")
    print("\n".join(out))


if __name__ == "__main__":
    main()
