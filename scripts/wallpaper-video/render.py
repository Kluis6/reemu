#!/usr/bin/env python3
"""Gera os papéis de parede em vídeo do ReEmu.

Para cada cena de `scenes.js`, abre `render.html` num Chrome/Edge headless,
codifica o loop em VP9 pelo WebCodecs (`VideoEncoder`), grava o WebM com o
gravador de `webm.js` e salva, em `packages/app-desktop/public/wallpapers/`:
- `<id>.webm` — o vídeo em loop (1920×1080, 30 fps);
- `<id>.jpg`  — o primeiro quadro (pôster, miniatura e o fundo parado no
  Linux, onde o vídeo não toca; ver `AnimatedBackground`).

Uso:  python scripts/wallpaper-video/render.py [--only 1,6] [--seconds 12]
      [--bitrate 2500000]

Não precisa de ffmpeg: o encoder é o do próprio navegador. A página é servida
por um servidor HTTP local de curta duração (módulos ES não carregam de
`file://`).
"""

import argparse
import base64
import functools
import http.server
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

import websocket  # mesmo pacote do scripts/ui-smoke.py

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "packages", "app-desktop", "public", "wallpapers")

_spec = importlib.util.spec_from_file_location("ui_smoke", os.path.join(ROOT, "scripts", "ui-smoke.py"))
assert _spec and _spec.loader
smoke = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(smoke)


class _Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, format, *args):  # noqa: A002 (assinatura da stdlib)
        pass


def serve(directory):
    handler = functools.partial(_Quiet, directory=directory)
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only", help="ids separados por vírgula (padrão: 1 a 6)")
    ap.add_argument("--seconds", type=int, default=12)
    ap.add_argument("--bitrate", type=int, default=1_600_000)
    ap.add_argument("--codec", choices=["vp9", "av1"], default="vp9")
    ap.add_argument("--fps", type=int, default=24)
    ap.add_argument("--port", type=int, default=9388)
    ap.add_argument("--out", default=OUT)
    args = ap.parse_args()
    ids = args.only.split(",") if args.only else ["1", "2", "3", "4", "5", "6"]
    os.makedirs(args.out, exist_ok=True)

    httpd = serve(HERE)
    profile = tempfile.mkdtemp(prefix="reemu-wallvid-")
    browser = subprocess.Popen(
        [smoke.find_browser(), "--headless=new", f"--remote-debugging-port={args.port}",
         "--remote-allow-origins=*", f"--user-data-dir={profile}", "about:blank"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{args.port}/json"))
                break
            except OSError:
                time.sleep(0.2)
        else:
            sys.exit("o navegador não abriu a porta de depuração")
        ws = next(t for t in tabs if t["type"] == "page")["webSocketDebuggerUrl"]
        page = smoke.Page(websocket.create_connection(ws, suppress_origin=True, timeout=600))
        page.call("Page.enable")
        page.call("Page.navigate", url=f"http://127.0.0.1:{httpd.server_port}/render.html")
        for _ in range(100):
            if page.eval("window.ready === true"):
                break
            time.sleep(0.1)
        else:
            sys.exit("render.html não carregou")

        for wid in ids:
            started = time.time()
            poster = page.eval(f"window.frame({json.dumps(wid)}, 0)")
            with open(os.path.join(args.out, f"{wid}.jpg"), "wb") as f:
                f.write(base64.b64decode(poster.split(",", 1)[1]))
            size = page.eval(
                f"window.render({json.dumps(wid)}, {{ seconds: {args.seconds}, bitrate: {args.bitrate}, fps: {args.fps}, keyEvery: {args.fps * 4}, codec: '{args.codec}' }})"
            )
            if not size:
                sys.exit(f"cena {wid}: o encoder não devolveu nada")
            data = bytearray()
            for i in range((size + 1048575) // 1048576):
                data += base64.b64decode(page.eval(f"window.chunk({i})"))
            with open(os.path.join(args.out, f"{wid}.webm"), "wb") as f:
                f.write(data)
            print(f"{wid}: {len(data) / 1048576:.2f} MB em {time.time() - started:.0f} s")
    finally:
        browser.kill()
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    print(f"saída em {args.out}")


if __name__ == "__main__":
    main()
