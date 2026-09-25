// Lista as Releases publicadas do ReEmu direto da API do GitHub, no
// navegador: publicar uma Release já atualiza a página, sem rebuild.
// A API sem login aceita 60 consultas/hora por IP — a resposta fica em
// cache na sessão por 10 min pra não gastar isso à toa.
//
// Monta a interface com os componentes Fluent UI 2 (`fluent-*`, registrados
// em ui.js). Todo texto vindo da API entra como texto (nunca innerHTML).

const REPO = "Kluis6/reemu";
const API = `https://api.github.com/repos/${REPO}/releases?per_page=30`;
const RELEASES_PAGE = `https://github.com/${REPO}/releases`;
const CACHE_KEY = "reemu.releases.v1";
const CACHE_MS = 10 * 60 * 1000;

// Instaladores reconhecidos. `.sig` e `latest.json` são do auto-update.
const KINDS = [
  { re: /-setup\.exe$/i, os: "windows", label: "Instalador (.exe)", hint: "recomendado", main: true },
  { re: /\.msi$/i, os: "windows", label: "Pacote MSI (.msi)", hint: "instalação gerenciada" },
  { re: /\.AppImage$/i, os: "linux", label: "AppImage", hint: "qualquer distro · recomendado", main: true },
  { re: /\.deb$/i, os: "linux", label: "Pacote .deb", hint: "Debian, Ubuntu, Mint" },
  { re: /\.rpm$/i, os: "linux", label: "Pacote .rpm", hint: "Fedora, openSUSE" },
];
const OS_NAME = { windows: "Windows", linux: "Linux" };

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v == null) continue;
    if (k === "class") node.className = v;
    else node.setAttribute(k, v === true ? "" : v);
  }
  for (const c of children) {
    if (c == null || c === false) continue;
    node.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return node;
}

function visitorOs() {
  const p = (navigator.userAgentData?.platform || navigator.platform || navigator.userAgent || "").toLowerCase();
  if (p.includes("win")) return "windows";
  if (p.includes("android")) return null;
  if (p.includes("linux") || p.includes("x11")) return "linux";
  return null;
}

const fmtSize = (b) => `${(b / 1_048_576).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} MB`;
const fmtDate = (iso) =>
  new Date(iso).toLocaleDateString("pt-BR", { day: "2-digit", month: "long", year: "numeric" });

function installers(release) {
  const out = [];
  for (const a of release.assets || []) {
    const kind = KINDS.find((k) => k.re.test(a.name));
    if (kind) out.push({ ...kind, name: a.name, url: a.browser_download_url, size: a.size });
  }
  return out;
}

// Markdown mínimo (o que scripts/release-notes.sh gera): `## seção`, `- item`
// e parágrafos. Mesmo recorte do parser do app (lib/releaseNotes.ts).
function renderNotes(md) {
  const box = el("div", { class: "notes" });
  let list = null;
  for (const raw of (md || "").split(/\r?\n/)) {
    const line = raw.trim().replace(/\*\*|__|`/g, "");
    if (!line) continue;
    const h = line.match(/^#{1,6}\s+(.*)$/);
    const li = line.match(/^[-*+]\s+(.*)$/);
    if (h) {
      list = null;
      box.append(el("h4", {}, h[1]));
    } else if (li) {
      if (!list) box.append((list = el("ul")));
      list.append(el("li", {}, li[1]));
    } else {
      list = null;
      box.append(el("p", {}, line));
    }
  }
  return box.childElementCount ? box : null;
}

// Botão de download grande: título + linha de detalhe (versão, tamanho).
function downloadButton(f, version, primary) {
  return el(
    "fluent-anchor-button",
    { class: "dl-btn", appearance: primary ? "primary" : "outline", size: "large", href: f.url },
    el("span", { class: "dl-stack" },
      el("span", { class: "dl-title" }, `Baixar para ${OS_NAME[f.os]} — ${f.label}`),
      el("span", { class: "dl-sub" }, `${version} · ${fmtSize(f.size)} · ${f.hint}`)),
  );
}

function renderPrimary(latest) {
  const box = document.getElementById("primary");
  box.replaceChildren();
  if (!latest) {
    box.append(
      el("fluent-message-bar", { intent: "info", layout: "multiline" },
        "Ainda não há versão publicada. Acompanhe em ",
        el("fluent-link", { inline: true, href: RELEASES_PAGE }, "GitHub Releases"), "."),
    );
    return;
  }
  const files = installers(latest);
  const os = visitorOs();
  // No sistema do visitante: o recomendado em destaque + os outros dele.
  // Fora de Windows/Linux (Mac, celular): o recomendado de cada sistema.
  const order = os
    ? files.filter((f) => f.os === os).sort((a, b) => Number(!!b.main) - Number(!!a.main))
    : files.filter((f) => f.main);
  order.forEach((f, i) => box.append(downloadButton(f, latest.tag_name, i === 0 && !!os)));
  if (!os) box.append(el("fluent-text", { size: "200", block: true }, "O ReEmu roda em Windows e Linux."));
  if (order.length === 0) {
    box.append(
      el("fluent-anchor-button", { appearance: "primary", size: "large", href: latest.html_url },
        `Ver a versão ${latest.tag_name} no GitHub`),
    );
  }
}

function renderDownloads(files) {
  const grid = el("div", { class: "downloads" });
  for (const os of ["windows", "linux"]) {
    const mine = files.filter((f) => f.os === os);
    if (!mine.length) continue;
    grid.append(
      el("div", { class: "dl-group" },
        el("fluent-text", { size: "300", weight: "semibold", block: true }, OS_NAME[os]),
        ...mine.map((f) =>
          el("div", { class: "dl-row" },
            el("fluent-anchor-button", { appearance: "outline", size: "small", href: f.url }, f.label),
            el("fluent-text", { size: "200", class: "muted" }, `${fmtSize(f.size)} · ${f.hint}`)),
        )),
    );
  }
  return grid;
}

function renderRelease(r, isLatest) {
  const card = el("article", { class: "card release", id: r.tag_name });
  card.append(
    el("div", { class: "release-head" },
      el("h3", {}, r.name || r.tag_name),
      isLatest && el("fluent-badge", { appearance: "filled", color: "brand" }, "Mais recente"),
      r.prerelease && el("fluent-badge", { appearance: "tint", color: "warning" }, "Pré-lançamento"),
      el("fluent-text", { size: "200", class: "muted" }, fmtDate(r.published_at))),
  );

  const notes = renderNotes(r.body);
  if (notes) {
    // Só a mais recente abre as notas; as antigas ficam recolhidas.
    if (isLatest) card.append(notes);
    else {
      card.append(
        el("fluent-accordion", { class: "notes-more" },
          el("fluent-accordion-item", {},
            el("span", { slot: "heading" }, "O que mudou"),
            notes)),
      );
    }
  }

  const files = installers(r);
  if (files.length) {
    card.append(el("fluent-divider", { class: "release-divider" }), renderDownloads(files));
  }
  card.append(
    el("p", { class: "release-foot" }, el("fluent-link", { href: r.html_url }, "Ver no GitHub")),
  );
  return card;
}

function renderList(releases) {
  const box = document.getElementById("releases");
  box.replaceChildren();
  if (!releases.length) {
    box.append(el("fluent-message-bar", { intent: "info" }, "Nenhuma versão publicada ainda."));
    return;
  }
  const latest = releases.find((r) => !r.prerelease) || releases[0];
  for (const r of releases) box.append(renderRelease(r, r === latest));
}

function renderError(message) {
  document.getElementById("primary").replaceChildren(
    el("fluent-anchor-button", { class: "dl-btn", appearance: "primary", size: "large", href: `${RELEASES_PAGE}/latest` },
      el("span", { class: "dl-stack" },
        el("span", { class: "dl-title" }, "Baixar no GitHub Releases"),
        el("span", { class: "dl-sub" }, "abre a versão mais recente"))),
  );
  document.getElementById("releases").replaceChildren(
    el("fluent-message-bar", { intent: "warning", layout: "multiline" },
      message, " ", el("fluent-link", { inline: true, href: RELEASES_PAGE }, "Ver todas no GitHub"), "."),
  );
}

async function loadReleases() {
  try {
    const hit = JSON.parse(sessionStorage.getItem(CACHE_KEY) || "null");
    if (hit && Date.now() - hit.at < CACHE_MS) return hit.data;
  } catch {
    // sem storage (aba privada, bloqueado): só não usa cache
  }
  const res = await fetch(API, { headers: { Accept: "application/vnd.github+json" } });
  if (res.status === 403 || res.status === 429) throw new Error("rate");
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  // A API sem login já não devolve drafts; o filtro é só garantia.
  const data = (await res.json()).filter((r) => !r.draft);
  try {
    sessionStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), data }));
  } catch {
    // idem
  }
  return data;
}

loadReleases()
  .then((releases) => {
    renderPrimary(releases.find((r) => !r.prerelease) || releases[0] || null);
    renderList(releases);
  })
  .catch((e) => {
    console.warn("releases:", e);
    renderError(
      e.message === "rate"
        ? "O GitHub limitou as consultas deste endereço por agora. Tente de novo em alguns minutos."
        : "Não foi possível carregar a lista de versões.",
    );
  });
