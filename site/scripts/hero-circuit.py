"""Gera o circuito do hero do site e grava em site/index.html (lado esquerdo;
o direito é o espelho). Rodar da raiz do repositório:
`python3 site/scripts/hero-circuit.py`, depois `pnpm --filter reemu-site build`.

Mesma regra das trilhas originais: cada trilha sai da borda na horizontal,
dobra a 45° rumo à linha do centro e termina na horizontal com um terminal.
Espaçamento de 41 unidades (o do desenho original) por toda a altura: o
SVG cobre o hero inteiro com `slice`, então a escala vem da largura (40% da
tela) e o vão fica igual ao do desenho original (~34 px numa tela de 1280).
Nas trilhas de fora a diagonal é limitada a 200 unidades, pra caber.
"""
import random, re, sys

W, H, GAP = 600, 1240, 41
C = H / 2
K = int((C - 30) // GAP)
rnd = random.Random(7)

traces, pads, chips, branches = [], [], [], []
END_X = [492, 506, 520, 534, 548]
for i, k in enumerate(range(-K, K + 1)):
    y = C + k * GAP
    d = abs(y - C)
    x0 = rnd.choice(range(24, 120, 7))
    xt = 230 + 0.39 * min(d, 164)          # início da dobra
    dy = min(0.7 * d, 200)                  # diagonal a 45°, limitada
    ye = y - dy if y > C else y + dy        # rumo ao centro
    xe = END_X[i % 5]
    traces.append(f"M{x0} {y:.1f} H{xt:.0f} L{xt + dy:.1f} {ye:.1f} H{xe}")
    pads += [(x0, y), (xe, ye)]
    # ramo curto (a cada 4 trilhas, alternando acima/abaixo): sobe/desce meio
    # vão a partir da parte horizontal e corre um pouco pra borda
    if i % 4 == 1:
        bx = rnd.choice(range(140, int(xt) - 40, 10))
        s = -1 if (i // 4) % 2 else 1
        by = y + s * GAP / 2
        bx2 = bx - rnd.choice((30, 40, 50))
        branches.append(f"M{bx} {y:.1f} V{by:.1f} H{bx2}")
        pads.append((bx2, by))
    # fileira de chips no vão abaixo (a cada 3 trilhas, sem ramo)
    elif i % 3 == 0 and k != K:
        cx = rnd.choice(range(150, 230, 13))
        cy = y + GAP / 2 - 4
        n = rnd.choice((2, 3, 4))
        chips += [(cx + j * 13, cy) for j in range(n)]

def g_traces():
    return ('<g fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" class="*:opacity-40">'
            + "".join(f'<path d="{p}"/>' for p in traces + branches) + "</g>")

def g_pads():
    return ('<g stroke="currentColor" stroke-width="1.6" class="fill-bg *:opacity-65">'
            + "".join(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="4.5"/>' for x, y in pads) + "</g>")

def g_chips():
    return ('<g class="fill-current *:opacity-32">'
            + "".join(f'<rect x="{x}" y="{y:.1f}" width="8" height="8" rx="1"/>' for x, y in chips) + "</g>")

timing = [(2.6 + rnd.random() * 2.1, rnd.random() * 2.8) for _ in traces]

def g_pulses(glow):
    attrs = ('stroke-width="8" stroke-linecap="round" stroke-dasharray="0.06 1.4" stroke-dashoffset="1.46" class="*:opacity-22 motion-reduce:hidden"'
             if glow else
             'stroke-width="2.2" stroke-linecap="round" stroke-dasharray="0.06 1.4" stroke-dashoffset="1.46" class="motion-reduce:hidden"')
    return (f'<g data-pulses fill="none" stroke="currentColor" {attrs}>'
            + "".join(f'<path pathLength="1" d="{p}" data-dur="{du:.2f}" data-delay="{de:.2f}"/>'
                      for p, (du, de) in zip(traces, timing)) + "</g>")

inner = g_traces() + g_pads() + g_chips() + g_pulses(True) + g_pulses(False)
MASK_L = "mask-[linear-gradient(90deg,transparent_2%,#000_38%,#000_60%,transparent_90%)]"
MASK_R = "mask-[linear-gradient(270deg,transparent_2%,#000_38%,#000_60%,transparent_90%)]"
svg_l = (f'<svg class="absolute inset-y-0 left-0 h-full w-[40%] fill-none text-green {MASK_L} max-[760px]:hidden" '
         f'viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMid slice">\n            {inner}\n          </svg>')
svg_r = (f'<svg class="absolute inset-y-0 right-0 h-full w-[40%] fill-none text-blue {MASK_R} max-[760px]:hidden" '
         f'viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMid slice">\n            '
         f'<g transform="translate({W} 0) scale(-1 1)">{inner}</g>\n          </svg>')

p = "site/index.html"
s = open(p).read()
pat = re.compile(r'<svg class="absolute \S+ left-0 h-\S+ w-\[40%\].*?</svg>\s*<svg class="absolute \S+ right-0 h-\S+ w-\[40%\].*?</svg>', re.S)
assert len(pat.findall(s)) == 1
s = pat.sub(lambda m: svg_l + "\n          " + svg_r, s)
open(p, "w").write(s)
print(len(traces), "trilhas por lado,", len(branches), "ramos,", len(chips), "chips")
