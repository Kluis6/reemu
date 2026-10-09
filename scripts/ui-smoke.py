#!/usr/bin/env python3
"""Teste visual rápido da interface, sem o app nem o backend.

Abre o frontend (Vite) num Chrome/Edge headless, com o backend do Tauri
trocado por respostas fixas (MOCKS abaixo), passa pelas telas nos tamanhos de
tela que o app usa e:
  - salva um PNG de cada tela/tamanho em --out;
  - confere os problemas que já apareceram (e foram corrigidos) na mão:
    página das Configurações que rola, linha da tela de Controles com
    elemento saindo pra fora, prévia do shader maior que o card.

Uso (Vite numa porta só pra isso — não a 1420 do `tauri dev`):
  pnpm --dir packages/app-desktop exec vite --port 1430 --strictPort &
  python scripts/ui-smoke.py --url http://localhost:1430 --out /tmp/ui-smoke

No Git Bash do Windows, `--routes /settings/...` vira caminho do Windows:
use `MSYS_NO_PATHCONV=1 python scripts/ui-smoke.py ...`.

Conferido que pega o problema: com o `KeyboardBindings.tsx` de antes da
correção de 2026-10-08 (colunas de 260 px), Controles falha em todas as
linhas e o script sai com 1.

Precisa de `pip install websocket-client` e de Chrome ou Edge. Sai com código
1 se alguma conferência falhar. `vh` já está em px da interface no app (zoom
nativo do webview, ver `uiScale.ts`); aqui o tamanho da janela é o tamanho
lógico, então 1051x591 = interface "Maior" num 1366x768.
"""

import argparse
import base64
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request

try:
    import websocket  # websocket-client
except ImportError:
    sys.exit("falta o pacote websocket-client: pip install websocket-client")

# Tamanhos lógicos: padrão (1366x768), interface Grande e Maior num 16:9,
# 4:3 e 1080p.
SIZES = [(1366, 768), (1188, 668), (1051, 591), (1024, 768), (1920, 1080)]

ROUTES = [
    "/settings",
    "/settings/sistema",
    "/settings/video/shaders",
    "/settings/controllers",
    "/settings/bios",
    "/settings/cores",
]

# Respostas do backend (`invoke`). Comando que não está aqui: lista vazia se
# o nome sugere lista, senão `null`.
MOCKS = r"""
{
  get_profile: {name: 'Teste', bio: null, avatar: 'preset:1', onboarded: true},
  get_theme_selection: '__THEME__',
  get_system_settings: {autostart: false, startFullscreen: true, minimizeToTray: false},
  get_hardware_info: {os: 'Windows 11', arch: 'x86_64', cpu: 'CPU de teste', cpuCores: 6,
    cpuThreads: 12, memoryBytes: 17179869184, gpu: 'GPU de teste', gpuBackend: 'Vulkan',
    gpuDriver: '1.0'},
  get_shader_info: {active: 'plain', available: ['plain', 'crt', 'lcd'], curated: [], gpu: true},
  list_roms: Array.from({length: 8}, (_, i) => ({id: 'r' + i, title: 'Jogo ' + i,
    systemId: 'snes', filePath: 'x', boxart: null, lastPlayedAt: null, addedAt: 0,
    isFavorite: false})),
  list_installed_cores: [{coreId: 'snes9x', name: 'Snes9x', version: '1.0', extensions: [],
    renderBackend: null, systems: ['snes']}],
  list_bios_status: [
    {systemId: 'psx', filename: 'scph5501.bin', required: false, note: 'NTSC-U',
     present: false, hashOk: null, anyOfFolder: null},
    {systemId: 'ps2', filename: '*', required: true, note: 'PS2', present: false,
     hashOk: null, anyOfFolder: 'pcsx2/bios'},
    {systemId: 'snes', filename: 'BS-X.bin', required: false, note: 'BS-X', present: false,
     hashOk: null, anyOfFolder: null},
  ],
}
"""

INIT = """
const MOCKS = %s;
window.__TAURI_INTERNALS__ = {
  transformCallback: () => 0,
  metadata: {currentWindow: {label: 'main'}, currentWebview: {label: 'main'}},
  invoke: async (cmd) => cmd in MOCKS ? MOCKS[cmd]
    : (/list|installed|sources|folders|cores|devices|recent/.test(cmd) ? [] : null),
};
"""

# Conferências rodadas na página. Cada uma devolve uma lista de problemas.
CHECKS = r"""
(() => {
  const out = [];
  const path = location.hash.slice(1);
  // 1. Configurações: a página não rola (quem rola é o card de dentro).
  if (path.startsWith('/settings')) {
    const page = document.scrollingElement;
    if (page.scrollHeight > page.clientHeight + 1) out.push('a janela rola');
    // a área que rola do shell = 1º ancestral rolável do `nav` da página
    // (grade do hub ou caminho "Configurações › …"); o card de conteúdo rolar
    // por dentro é esperado e não entra aqui.
    let el = document.querySelector('nav[aria-label]');
    while (el && !/auto|scroll/.test(getComputedStyle(el).overflowY)) el = el.parentElement;
    if (el && el.scrollHeight > el.clientHeight + 1) out.push('a área da página rola');
  }
  // 2. Hub: a grade de cards cabe sem rolar.
  if (path === '/settings') {
    const g = document.querySelector('nav[aria-label]');
    if (g && g.scrollHeight > g.clientHeight + 1) out.push('grade do hub rola');
  }
  // 3. Linhas com botões (Controles › Teclado, BIOS): nada passa da borda.
  for (const r of document.querySelectorAll('div')) {
    const st = getComputedStyle(r);
    if (st.display !== 'flex' || !r.querySelector(':scope > button')) continue;
    const box = r.getBoundingClientRect();
    if (box.width < 50) continue;
    for (const c of r.children) {
      const b = c.getBoundingClientRect();
      if (b.width && b.right > box.right + 1) {
        out.push('linha com elemento pra fora: ' + (r.textContent || '').slice(0, 40));
        break;
      }
    }
  }
  // 4. Prévia do shader: não passa do fundo da janela.
  const prev = [...document.querySelectorAll('div')].find((d) => getComputedStyle(d).aspectRatio === '4 / 3');
  if (prev && prev.getBoundingClientRect().bottom > innerHeight) out.push('prévia do shader passa da tela');
  return [...new Set(out)];
})()
"""


def find_browser():
    for p in [
        os.environ.get("CHROME", ""),
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        shutil.which("google-chrome") or "",
        shutil.which("chromium") or "",
        shutil.which("chromium-browser") or "",
        shutil.which("microsoft-edge") or "",
    ]:
        if p and os.path.exists(p):
            return p
    sys.exit("Chrome/Edge não encontrado (defina CHROME=<caminho>)")


class Page:
    def __init__(self, ws):
        self.ws, self.n = ws, 0

    def call(self, method, **params):
        self.n += 1
        self.ws.send(json.dumps({"id": self.n, "method": method, "params": params}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.n:
                return r

    def eval(self, expr):
        r = self.call("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
        return r.get("result", {}).get("result", {}).get("value")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--url", default="http://localhost:1430")
    ap.add_argument("--out", default=os.path.join(tempfile.gettempdir(), "reemu-ui-smoke"))
    ap.add_argument("--theme", default="reemu", help="id do tema (ex.: claro, alva-claro)")
    ap.add_argument("--port", type=int, default=9355, help="porta de depuração do navegador")
    ap.add_argument("--routes", help="rotas separadas por vírgula (padrão: todas)")
    ap.add_argument("--sizes", help="tamanhos LxA separados por vírgula (padrão: todos)")
    args = ap.parse_args()
    routes = args.routes.split(",") if args.routes else ROUTES
    sizes = [tuple(map(int, x.split("x"))) for x in args.sizes.split(",")] if args.sizes else SIZES
    os.makedirs(args.out, exist_ok=True)

    profile = tempfile.mkdtemp(prefix="reemu-ui-smoke-")
    browser = subprocess.Popen(
        [find_browser(), "--headless=new", "--disable-gpu", f"--remote-debugging-port={args.port}",
         "--remote-allow-origins=*", f"--user-data-dir={profile}", "about:blank"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    failures = []
    try:
        for _ in range(100):
            try:
                tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{args.port}/json"))
                break
            except OSError:
                time.sleep(0.2)
        else:
            sys.exit("o navegador não abriu a porta de depuração")
        url = next(t for t in tabs if t["type"] == "page")["webSocketDebuggerUrl"]
        page = Page(websocket.create_connection(url, suppress_origin=True))
        page.call("Page.enable")
        mocks = MOCKS.replace("__THEME__", args.theme)
        page.call("Page.addScriptToEvaluateOnNewDocument", source=INIT % mocks)

        for w, h in sizes:
            page.call("Emulation.setDeviceMetricsOverride", width=w, height=h,
                      deviceScaleFactor=1, mobile=False)
            for route in routes:
                page.call("Page.navigate", url=f"{args.url}/#{route}")
                time.sleep(2.0)  # rota + animação de entrada
                problems = page.eval(CHECKS) or []
                shot = page.call("Page.captureScreenshot", format="png")
                name = f"{w}x{h}{route.replace('/', '_')}.png"
                with open(os.path.join(args.out, name), "wb") as f:
                    f.write(base64.b64decode(shot["result"]["data"]))
                status = "ok" if not problems else "FALHOU: " + "; ".join(problems)
                print(f"{w}x{h} {route:28s} {status}")
                failures += [f"{w}x{h} {route}: {p}" for p in problems]
    finally:
        browser.kill()
        shutil.rmtree(profile, ignore_errors=True)

    print(f"\ncapturas em {args.out}")
    if failures:
        print(f"{len(failures)} problema(s)")
        sys.exit(1)


if __name__ == "__main__":
    main()
