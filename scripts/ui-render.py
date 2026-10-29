#!/usr/bin/env python3
"""Roteiro de teste visual: abre uma tela do frontend com o backend do Tauri
trocado por respostas fixas e executa passos em ordem (esperar, mandar
evento, navegar com o controle, focar, passar o mouse, clicar, capturar,
ler valores do DOM, auditar o foco).

Reaproveita de `ui-smoke.py` os mocks, o navegador headless e o cliente CDP;
acrescenta os eventos do Tauri (`listen`/`emit`), mocks extras por passo de
linha de comando e os passos. É a ferramenta da skill `teste-visual`
(`.claude/skills/teste-visual/SKILL.md`).

Vite numa porta só pra isso — NUNCA a 1420 (é a do `tauri dev`):
  pnpm --dir packages/app-desktop exec vite --port 1430 --strictPort

Exemplos (no Git Bash: `MSYS_NO_PATHCONV=1` antes, senão `/rom/r1` vira
caminho do Windows):
  python scripts/ui-render.py --route /rom/r1 --theme claro \\
      --step wait:6 --step shot:pagina
  python scripts/ui-render.py --route "/play/r1?core=snes9x" \\
      --mock 'list_cheats=[{"id":1,"romId":"r1","description":"Vidas","code":"7E0DBF09","enabled":true}]' \\
      --step wait:5 --step 'emit:focus-changed={"focus":"MenuFocused"}' \\
      --step wait:1 --step nav:tabNext --step shot:menu
  python scripts/ui-render.py --route /settings/video/shaders --step wait:6 --step audit-focus

Passos (`--step`, na ordem dada):
  wait:<s>                  espera
  emit:<evento>=<json>      dispara um evento do Tauri pros `listen` da tela
  nav:<pulso>               pulso do controle (`menu-nav`): up, down, left,
                            right, confirm, back, tabPrev, tabNext...
  focus:<seletor CSS>       foca como o controle (marca `data-nav-focus`)
  hover:<seletor CSS>       move o mouse pro centro do elemento
  click:<seletor CSS>       clica no elemento (`.click()`)
  text:<texto>              clica no 1º botão/aba/link cujo texto COMEÇA com
                            <texto> (o Fluent repete o rótulo da aba)
  eval:<js>                 roda JS e imprime o resultado
  shot:<nome>               captura <out>/<nome>.png
  audit-focus               foca cada elemento focável e lista quem fica sem
                            anel ou sem brilho (o foco do card de jogo)

Precisa de `pip install websocket-client` e de Chrome ou Edge.
"""

import argparse
import base64
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import time
import urllib.request

_spec = importlib.util.spec_from_file_location(
    "ui_smoke", os.path.join(os.path.dirname(os.path.abspath(__file__)), "ui-smoke.py")
)
smoke = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(smoke)

# Backend falso com eventos: guarda os callbacks do `listen` pra `window.__emit`
# disparar à mão (focus-changed, menu-nav, shader-failed...).
INIT = """
const MOCKS = %s;
%s
try { localStorage.setItem('reemu.theme', %s) } catch (e) {}
window.__cbs = {}; window.__listeners = {}; let __n = 1;
window.__TAURI_INTERNALS__ = {
  transformCallback: (cb) => { const id = __n++; window.__cbs[id] = cb; return id },
  metadata: {currentWindow: {label: 'main'}, currentWebview: {label: 'main'}},
  invoke: async (cmd, args) => {
    if (cmd === 'plugin:event|listen') { (window.__listeners[args.event] ||= []).push(args.handler); return args.handler }
    if (cmd === 'plugin:event|unlisten') { const l = window.__listeners[args.event] || []; window.__listeners[args.event] = l.filter(h => h !== args.eventId); return null }
    if (cmd.startsWith('plugin:event|')) return null
    return cmd in MOCKS ? MOCKS[cmd] : (/list|installed|sources|folders|cores|devices|recent/.test(cmd) ? [] : null)
  },
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (event, id) => { window.__listeners[event] = (window.__listeners[event] || []).filter(h => h !== id) } };
window.__emit = (ev, payload) => (window.__listeners[ev] || []).forEach(h => window.__cbs[h]({event: ev, id: 1, payload}));
"""

# Para cada elemento focável visível: foca como o controle e confere o anel
# (outline >= 2 px no elemento ou no componente que o contém) e o brilho
# (`::before` com box-shadow e display != none nesse mesmo host).
AUDIT = r"""(() => {
  document.documentElement.setAttribute('data-nav-focus','');
  const sel = 'a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select, textarea, [tabindex]:not([tabindex="-1"])';
  const els = [...document.querySelectorAll(sel)].filter(e => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && !e.closest('[aria-hidden=true]'); });
  const hosts = '.fui-Card, .fui-Input, .fui-Textarea, .fui-SearchBox, .fui-Select, .fui-Dropdown, .fui-Combobox, .fui-SpinButton, .fui-Switch, .fui-Radio, .fui-Checkbox, .fui-Slider, .fui-Carousel';
  const out = {};
  for (const e of els) {
    e.focus({preventScroll: true});
    const cands = [e, e.closest(hosts)].filter(Boolean);
    let ring = false, glow = false;
    for (const h of cands) {
      const c = getComputedStyle(h);
      if (c.outlineStyle !== 'none' && parseFloat(c.outlineWidth) >= 2) ring = true;
      const b = getComputedStyle(h, '::before');
      if (b.content !== 'none' && b.display !== 'none' && b.boxShadow !== 'none') glow = true;
      const a = getComputedStyle(h, '::after');
      if (a.content !== 'none' && a.display !== 'none' && a.boxShadow !== 'none' && parseFloat(a.borderTopWidth) >= 2) { ring = true; glow = true; }
    }
    const host = e.closest(hosts);
    const kind = (host ? [...host.classList].find(c => c.startsWith('fui-')) : [...e.classList].find(c => c.startsWith('fui-'))) || (e.getAttribute('role') ? 'role=' + e.getAttribute('role') : e.tagName.toLowerCase());
    const label = (e.getAttribute('aria-label') || e.textContent || e.placeholder || '').trim().slice(0, 28);
    const k = kind + (ring ? '' : ' SEM-ANEL') + (glow ? '' : ' SEM-BRILHO');
    (out[k] ||= []).push(label);
  }
  return JSON.stringify(Object.entries(out).map(([k, v]) => [k, v.length, v.slice(0, 3)]));
})()"""


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--url", default="http://localhost:1430")
    ap.add_argument("--route", default="/", help="rota do HashRouter (ex.: /rom/r1)")
    ap.add_argument("--theme", default="reemu", help="id do tema (styles/themes.ts: claro, ps1-claro...)")
    ap.add_argument("--size", default="1366x768", help="tamanho lógico LxA")
    ap.add_argument("--out", default=os.path.join(tempfile.gettempdir(), "reemu-ui-render"))
    ap.add_argument("--port", type=int, default=9399, help="porta de depuração do navegador")
    ap.add_argument("--mock", action="append", default=[], metavar="CMD=JSON",
                    help="resposta de um comando do backend (substitui a de ui-smoke.py)")
    ap.add_argument("--step", action="append", default=[], help="passo (ver acima)")
    args = ap.parse_args()
    if args.port == 1420 or ":1420" in args.url:
        raise SystemExit("a 1420 é do `tauri dev` — use a 1430")
    w, h = map(int, args.size.split("x"))
    os.makedirs(args.out, exist_ok=True)

    extra = "\n".join(
        f"MOCKS[{json.dumps(k)}] = {v};" for k, v in (m.split("=", 1) for m in args.mock)
    )
    init = INIT % (smoke.MOCKS.replace("__THEME__", args.theme), extra, json.dumps(args.theme))

    profile = tempfile.mkdtemp(prefix="reemu-ui-render-")
    browser = subprocess.Popen(
        [smoke.find_browser(), "--headless=new", "--disable-gpu", f"--remote-debugging-port={args.port}",
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
            raise SystemExit("o navegador não abriu a porta de depuração")
        page = smoke.Page(smoke.websocket.create_connection(
            next(t for t in tabs if t["type"] == "page")["webSocketDebuggerUrl"], suppress_origin=True))
        page.call("Page.enable")
        page.call("Emulation.setFocusEmulationEnabled", enabled=True)
        page.call("Page.addScriptToEvaluateOnNewDocument", source=init)
        page.call("Emulation.setDeviceMetricsOverride", width=w, height=h, deviceScaleFactor=1, mobile=False)
        page.call("Page.navigate", url=f"{args.url}/#{args.route}")

        def center(selector):
            r = page.eval(f"(() => {{ const e = document.querySelector({json.dumps(selector)}); if (!e) return null;"
                          " const b = e.getBoundingClientRect(); return [b.x + b.width / 2, b.y + b.height / 2] }})()")
            if not r:
                print(f"  (não achei {selector})")
            return r

        for step in args.step:
            kind, _, arg = step.partition(":")
            if kind == "wait":
                time.sleep(float(arg))
            elif kind == "emit":
                ev, _, payload = arg.partition("=")
                page.eval(f"window.__emit({json.dumps(ev)}, {payload or 'null'})")
                time.sleep(0.3)
            elif kind == "nav":
                page.eval(f"window.__emit('menu-nav', {json.dumps(arg)})")
                time.sleep(0.4)
            elif kind == "focus":
                page.eval("document.documentElement.setAttribute('data-nav-focus','');"
                          f"document.querySelector({json.dumps(arg)})?.focus()")
                time.sleep(0.5)
            elif kind == "hover":
                c = center(arg)
                if c:
                    page.call("Input.dispatchMouseEvent", type="mouseMoved", x=c[0], y=c[1])
                    time.sleep(0.6)
            elif kind == "click":
                page.eval(f"document.querySelector({json.dumps(arg)})?.click()")
                time.sleep(0.8)
            elif kind == "text":
                ok = page.eval("(() => { const e = [...document.querySelectorAll('button, a, [role=tab]')]"
                               f".find(b => b.textContent.trim().startsWith({json.dumps(arg)})); e?.click(); return !!e }})()")
                print(f"  text:{arg} -> {'clicado' if ok else 'não achei'}")
                time.sleep(0.8)
            elif kind == "eval":
                print(f"  {page.eval(arg)}")
            elif kind == "shot":
                data = page.call("Page.captureScreenshot", format="png")["result"]["data"]
                path = os.path.join(args.out, f"{arg}.png")
                with open(path, "wb") as f:
                    f.write(base64.b64decode(data))
                print(f"  captura: {path}")
            elif kind == "audit-focus":
                for k, n, ex in json.loads(page.eval(AUDIT) or "[]"):
                    mark = "  <<" if "SEM-" in k else ""
                    print(f"  {k:45} {n:3}  {ex}{mark}")
            else:
                raise SystemExit(f"passo desconhecido: {step}")
    finally:
        browser.kill()
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    main()
