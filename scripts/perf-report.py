#!/usr/bin/env python3
"""Resume as linhas `perf core` e `perf vídeo` de uma sessão com REEMU_PERF=1.

Roteiro em docs/perf/README.md. Cada jogo aberto vira um trecho (separados por
10 s ou mais sem métricas), na ordem em que foram jogados.

  python scripts/perf-report.py SESSAO.log --labels software,opengl,vulkan,shader
  python scripts/perf-report.py SESSAO.log --labels ... --save docs/perf/baseline.json
  python scripts/perf-report.py SESSAO.log --labels ... --compare docs/perf/baseline.json

Com --compare, sai com código 1 se algum cenário passar dos limites do plano
(docs/ai-context/19-plano-unificado.md, "Portão de desempenho").
"""

import argparse
import json
import re
import statistics
import sys
from datetime import datetime

# Formatos do env_logger (core-host) e do tauri-plugin-log (app).
STAMP = re.compile(
    r"\[(\d{4}-\d{2}-\d{2})(?:T|\]\[)(\d{2}:\d{2}:\d{2})"
)
CORE = re.compile(
    r"perf core 1s: (?P<fps>[\d.]+) fps \(alvo (?P<alvo>[\d.]+)\) \| "
    r"intervalo méd (?P<int_med>[\d.]+) p99 (?P<int_p99>[\d.]+) máx (?P<int_max>[\d.]+) ms \| "
    r"retro_run méd (?P<run_med>[\d.]+) pior (?P<run_pior>[\d.]+) ms, (?P<acima>\d+) acima do budget.*"
    r"\((?P<cpu_spin>[\d.]+)% de 1 CPU\), (?P<atrasados>\d+) atrasados \| "
    r"áudio (?P<audio>\d+) amostras \(esperado ~(?P<audio_esp>\d+)\)"
)
VIDEO = re.compile(
    r"perf vídeo 1s: (?P<recebidos>\d+) recebidos do core, (?P<apresentados>\d+) apresentados, "
    r"(?P<perdidos>\d+) perdidos .*render méd (?P<render_med>[\d.]+) pior (?P<render_pior>[\d.]+) ms"
)

# Segundos descartados no começo (carregamento) e no fim (fechando) de cada trecho.
WARMUP = 5
COOLDOWN = 2
GAP = 10
# O que rodou em cada trecho, para conferir os nomes dados na linha de comando.
CORE_ID = re.compile(r"etapa 12: core (\S+) ")
VULKAN = re.compile(r"contexto Vulkan pronto|frame Vulkan in-process")
OPENGL = re.compile(r"WGL [\d.]+ .*HW render|EGL .*HW render|GLX .*HW render")
SHADER = re.compile(r"preset de shader → '([^']+)'")
STALE_HOST = "cargo build -p core-host-desktop falhou"
# Tempo menor é melhor; 2% é o limite do plano. Abaixo do piso, a diferença é ruído.
TIME_LIMIT = 0.02
TIME_FLOOR_MS = 0.2
TIME_METRICS = ("int_med", "int_p99", "run_med", "render_med")
RATE_METRICS = ("perdidos_s", "atrasados_s", "audio_baixo")
RATE_FLOOR = 0.05


def read_lines(path):
    raw = open(path, "rb").read()
    encoding = "utf-16" if raw[:2] in (b"\xff\xfe", b"\xfe\xff") else "utf-8"
    return raw.decode(encoding, errors="replace").splitlines()


def segments(lines):
    """Trechos de amostras, cada amostra = (segundo, tipo, campos). Os relógios só
    são comparados dentro do mesmo tipo: o core-host registra em UTC e o app em hora
    local."""
    found, current, last = [], [], {}
    for line in lines:
        core = CORE.search(line)
        video = None if core else VIDEO.search(line)
        if not core and not video:
            continue
        stamp = STAMP.search(line)
        if not stamp:
            sys.exit(f"linha de métrica sem horário: {line[:80]}")
        t = datetime.fromisoformat(f"{stamp[1]}T{stamp[2]}").timestamp()
        kind = "core" if core else "video"
        if kind in last and t - last[kind] >= GAP and current:
            found.append(current)
            current, last = [], {}
        last[kind] = t
        match = core or video
        current.append((t, kind, {k: float(v) for k, v in match.groupdict().items()}))
    if current:
        found.append(current)
    return found


def scene(lines):
    """O que rodou em cada trecho: uma lista paralela à de `segments`."""
    found, cur, last, seen = [], {}, {}, False
    shader = "?"
    for line in lines:
        m = SHADER.search(line)
        if m:
            shader = m[1]
            # Trocado com o jogo já rodando vale para o próximo trecho.
            if not seen:
                cur["shader"] = shader
        m = CORE_ID.search(line)
        if m:
            if seen:
                found.append(cur)
                cur, last, seen = {"shader": shader}, {}, False
            cur["core"] = m[1]
        if VULKAN.search(line):
            cur["render"] = "Vulkan"
        elif OPENGL.search(line):
            cur["render"] = "OpenGL"
        core = CORE.search(line)
        if core or VIDEO.search(line):
            stamp = STAMP.search(line)
            t = datetime.fromisoformat(f"{stamp[1]}T{stamp[2]}").timestamp()
            kind = "core" if core else "video"
            if kind in last and t - last[kind] >= GAP and seen:
                found.append(cur)
                cur, last = {"shader": shader, "core": cur.get("core"), "render": cur.get("render")}, {}
            last[kind] = t
            seen = True
    if seen:
        found.append(cur)
    return found


def trimmed(samples, kind):
    """Campos de um tipo, sem o começo e o fim do trecho. Trecho curto demais fica inteiro."""
    rows = [s for s in samples if s[1] == kind]
    if not rows:
        return [], 0
    start, end = rows[0][0], rows[-1][0]
    kept = [s[2] for s in rows if start + WARMUP <= s[0] <= end - COOLDOWN]
    return kept or [s[2] for s in rows], end - start


def summarize(samples):
    core, core_s = trimmed(samples, "core")
    video, video_s = trimmed(samples, "video")
    out = {"segundos": round(max(core_s, video_s))}

    def med(rows, key):
        return round(statistics.median(r[key] for r in rows), 3) if rows else None

    if core:
        out |= {k: med(core, k) for k in ("fps", "alvo", "int_med", "int_p99", "run_med", "cpu_spin")}
        out["int_max"] = max(r["int_max"] for r in core)
        out["run_pior"] = max(r["run_pior"] for r in core)
        out["atrasados_s"] = round(sum(r["atrasados"] for r in core) / len(core), 3)
        # A fração de segundos com pouca amostra é o que o log diz sobre falha
        # de áudio; a falta real na saída de som não é medida.
        low = [r for r in core if r["audio"] < 0.95 * r["audio_esp"]]
        out["audio_baixo"] = round(len(low) / len(core), 3)
    if video:
        out["render_med"] = med(video, "render_med")
        out["render_pior"] = max(r["render_pior"] for r in video)
        out["perdidos_s"] = round(sum(r["perdidos"] for r in video) / len(video), 3)
    return out


def compare(now, base):
    """(linhas da tabela, quantos passaram do limite)."""
    rows, bad = [], 0
    for label, cur in now.items():
        ref = base.get(label)
        if not ref:
            rows.append(f"| {label} | — | — | — | sem referência |")
            continue
        for key in TIME_METRICS + RATE_METRICS:
            if cur.get(key) is None or ref.get(key) is None:
                continue
            a, b = ref[key], cur[key]
            if key in TIME_METRICS:
                over = b > a * (1 + TIME_LIMIT) and b - a > TIME_FLOOR_MS
            else:
                over = b - a > RATE_FLOOR
            bad += over
            rows.append(f"| {label} | {key} | {a} | {b} | {'**PIOROU**' if over else 'ok'} |")
    return rows, bad


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("log")
    ap.add_argument("--labels", required=True, help="nome de cada trecho, na ordem jogada, separados por vírgula")
    ap.add_argument("--save", help="grava o resultado neste JSON")
    ap.add_argument("--compare", help="compara com este JSON")
    args = ap.parse_args()

    lines = read_lines(args.log)
    found = segments(lines)
    scenes = scene(lines)
    stale = any(STALE_HOST in line for line in lines)
    if stale:
        print("ATENÇÃO: o core-host não compilou nesta sessão; os jogos rodaram com um binário antigo.")
        if args.save or args.compare:
            sys.exit("medida inválida: resolva a compilação e repita (docs/perf/README.md)")
    labels = [x.strip() for x in args.labels.split(",") if x.strip()]
    print(f"{len(found)} trecho(s) no log, {len(labels)} nome(s).")
    if not found:
        sys.exit("nenhuma métrica neste log: o app não chegou a abrir com REEMU_PERF ou nenhum jogo rodou "
                 "(veja o fim do arquivo)")
    if len(found) != len(labels):
        for i, seg in enumerate(found):
            print(f"  trecho {i + 1}: {summarize(seg)['segundos']} s")
        sys.exit("a quantidade de nomes precisa ser igual à de trechos")

    result = {label: summarize(seg) for label, seg in zip(labels, found)}
    for label, info in zip(labels, scenes):
        print(f"  {label} -> core {info.get('core') or '?'}, render {info.get('render') or 'software'}, "
              f"shader {info.get('shader', '?')}")
    for label, data in result.items():
        print(f"\n{label}: {json.dumps(data, ensure_ascii=False)}")
        if data["segundos"] < 30:
            print("  aviso: menos de 30 s; a medida vale pouco")
    if args.save:
        with open(args.save, "w", encoding="utf-8", newline="\n") as f:
            json.dump(result, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"\ngravado em {args.save}")
    if args.compare:
        rows, bad = compare(result, json.load(open(args.compare, encoding="utf-8")))
        print("\n| cenário | medida | referência | agora | situação |\n|---|---|---|---|---|")
        print("\n".join(rows))
        print(f"\n{bad} medida(s) passaram do limite.")
        sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
