// Registra os componentes Fluent UI 2 (`fluent-*`), aplica o tema e cuida do
// movimento da página: entrada dos blocos ao rolar, contadores, topo com
// fundo depois de rolar, doações e o ampliador das capturas. Sem JS (ou com
// "reduzir movimento") a página aparece inteira e parada.
import { setTheme } from "./vendor/fluent-web-components-3.1.3.min.js";
import theme from "./vendor/reemu-theme.js";
import { lang, t } from "./i18n.js";

setTheme(theme);

const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

// Topo: transparente no hero, com fundo desfocado depois de rolar.
const header = document.querySelector(".top");
const onScroll = () => header.classList.toggle("scrolled", scrollY > 12);
addEventListener("scroll", onScroll, { passive: true });
onScroll();

// Contador de 0 até `data-to` (com `data-decimals` casas, no idioma ativo).
function countUp(el) {
  const to = Number(el.dataset.to);
  const decimals = Number(el.dataset.decimals || 0);
  const fmt = (v) =>
    v.toLocaleString(lang(), { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  if (reduceMotion) {
    el.textContent = fmt(to);
    return;
  }
  const start = performance.now();
  const dur = 1400;
  const tick = (now) => {
    const t = Math.min(1, (now - start) / dur);
    el.textContent = fmt(to * (1 - Math.pow(1 - t, 3)));
    if (t < 1) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

// Entrada ao rolar. Irmãos entram em cascata (60 ms entre um e outro).
const reveals = document.querySelectorAll(".reveal");
if ("IntersectionObserver" in window && !reduceMotion) {
  const siblingsIndex = new Map();
  for (const el of reveals) {
    const parent = el.parentElement;
    const i = siblingsIndex.get(parent) ?? 0;
    siblingsIndex.set(parent, i + 1);
    el.style.setProperty("--d", `${Math.min(i, 8) * 60}ms`);
  }
  let observed = false;
  const io = new IntersectionObserver(
    (entries) => {
      observed = true;
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        e.target.classList.add("in");
        e.target.querySelectorAll(".count").forEach(countUp);
        io.unobserve(e.target);
      }
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.12 },
  );
  // O hero já está na tela ao abrir: entra em cascata no próximo quadro.
  requestAnimationFrame(() => {
    document.querySelectorAll(".hero .reveal").forEach((el) => {
      el.classList.add("in");
      el.querySelectorAll(".count").forEach(countUp);
    });
  });
  reveals.forEach((el) => {
    if (!el.closest(".hero")) io.observe(el);
  });
  // Rede de segurança: se o observer nunca responder, nada fica invisível.
  setTimeout(() => {
    if (observed) return;
    io.disconnect();
    reveals.forEach((el) => el.classList.add("in"));
    document.querySelectorAll(".count").forEach(countUp);
  }, 2500);
} else {
  reveals.forEach((el) => el.classList.add("in"));
}

// Ampliador das capturas (só as que existem — quadros "em breve" ignoram).
const lightbox = document.getElementById("lightbox");
const big = lightbox?.querySelector(".lightbox-img");
document.querySelectorAll(".shot-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    if (btn.closest("figure").classList.contains("missing") || !big) return;
    const img = btn.querySelector("img");
    big.src = img.currentSrc || img.src;
    big.alt = img.alt;
    lightbox.show();
  });
});

// Doação: mostra só as formas com link configurado no HTML (`data-link`,
// `data-pix`). Sem nenhuma, esconde a grade e mostra o aviso.
{
  const grid = document.getElementById("donate-methods");
  let shown = 0;
  grid?.querySelectorAll("[data-link]").forEach((card) => {
    const url = card.dataset.link.trim();
    if (/^https:\/\//.test(url)) {
      card.querySelector("fluent-anchor-button")?.setAttribute("href", url);
      shown++;
    } else {
      card.remove();
    }
  });
  grid?.querySelectorAll("[data-pix]").forEach((card) => {
    const key = card.dataset.pix.trim();
    if (!key) {
      card.remove();
      return;
    }
    shown++;
    card.querySelector(".pix-key").textContent = key;
    const btn = card.querySelector(".pix-copy");
    btn.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(key);
        btn.textContent = t("pix.copied");
      } catch {
        btn.textContent = t("pix.manual");
      }
      setTimeout(() => (btn.textContent = t("pix.copy")), 2500);
    });
  });
  if (grid && shown === 0) {
    grid.hidden = true;
    document.getElementById("donate-soon").hidden = false;
  }
}

// "Divulgue": compartilhamento nativo quando existe, senão copia o link.
document.querySelectorAll(".share-btn").forEach((btn) => {
  btn.addEventListener("click", async () => {
    const data = {
      title: "ReEmu",
      text: t("share.text"),
      url: "https://kluis6.github.io/reemu/",
    };
    try {
      if (navigator.share) await navigator.share(data);
      else {
        await navigator.clipboard.writeText(data.url);
        btn.textContent = t("share.copied");
      }
    } catch {
      // compartilhamento cancelado — nada a fazer
    }
  });
});
