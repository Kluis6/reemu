#!/usr/bin/env -S python3 -u
"""Mostra L2/R2 do DualSense direto do evdev (sem gilrs), por 20 s.

Uso: python3 scripts/dualsense-triggers.py [/dev/input/eventN]

Imprime cada mudança de ABS_Z (L2), ABS_RZ (R2) e dos botões digitais
BTN_TL2/BTN_TR2, com o valor cru (0-255). Diagnóstico de gatilho com
"zero" fora de 0 (ver docs/historico.md, 2026-09-26).
"""
import glob
import os
import struct
import sys
import time

NAMES = {(3, 2): "L2 eixo (ABS_Z)", (3, 5): "R2 eixo (ABS_RZ)",
         (1, 0x138): "L2 botão (BTN_TL2)", (1, 0x139): "R2 botão (BTN_TR2)"}


def find_device():
    for d in sorted(glob.glob("/sys/class/input/event*")):
        try:
            name = open(f"{d}/device/name").read().strip()
        except OSError:
            continue
        if name == "DualSense Wireless Controller":
            return "/dev/input/" + os.path.basename(d)
    sys.exit("DualSense não encontrado")


path = sys.argv[1] if len(sys.argv) > 1 else find_device()
fmt = "llHHi"  # struct input_event (64 bits): timeval, type, code, value
size = struct.calcsize(fmt)
print(f"{path}: aperte o R2 devagar até o fundo e solte; depois o L2. 20 s…")
fd = os.open(path, os.O_RDONLY)
end = time.time() + 20
last = {}
while time.time() < end:
    sec, usec, typ, code, value = struct.unpack(fmt, os.read(fd, size))
    key = (typ, code)
    if key in NAMES and last.get(key) != value:
        last[key] = value
        print(f"{sec % 1000}.{usec // 1000:03d}  {NAMES[key]:20s} {value}")
