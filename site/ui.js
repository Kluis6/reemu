// Registra os componentes Fluent UI 2 (`fluent-*`), aplica o tema e cuida do
// movimento da página com o Motion (https://motion.dev): entrada dos blocos
// ao rolar, brilho do hero, pulsos do circuito; e ainda o topo com fundo
// depois de rolar, doações e o ampliador das capturas. Sem JS (ou com
// "reduzir movimento") a página aparece inteira e parada.
import { setTheme } from "./vendor/fluent-web-components-3.1.3.min.js";
import theme from "./vendor/reemu-theme.js";
// Bundle UMD do Motion: registra `globalThis.Motion` (copiado de
// node_modules por scripts/vendor-motion.mjs no build).
import "./vendor/motion.js";
import { t } from "./i18n.js";

const { animate } = globalThis.Motion;

setTheme(theme);

const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
// curva "sai rápido, assenta devagar" da identidade (ease-soft no tema)
const EASE = [0.2, 0.7, 0.2, 1];

// Topo: transparente no hero, com fundo desfocado depois de rolar.
const header = document.querySelector(".top");
const onScroll = () => header.toggleAttribute("data-scrolled", scrollY > 12);
addEventListener("scroll", onScroll, { passive: true });
onScroll();

// Entrada ao rolar: os `.reveal` começam transparentes (`js:opacity-0` no
// HTML) e sobem 22 px ao entrar na tela. Irmãos entram em cascata (60 ms
// entre um e outro).
const reveals = document.querySelectorAll(".reveal");
const delayOf = new Map();
{
  const siblingsIndex = new Map();
  for (const el of reveals) {
    const i = siblingsIndex.get(el.parentElement) ?? 0;
    siblingsIndex.set(el.parentElement, i + 1);
    delayOf.set(el, Math.min(i, 8) * 0.06);
  }
}
const reveal = (el) =>
  animate(el, { opacity: [0, 1], y: [22, 0] }, { duration: 0.7, delay: delayOf.get(el), ease: EASE });
const showAll = () => reveals.forEach((el) => (el.style.opacity = "1"));

if ("IntersectionObserver" in window && !reduceMotion) {
  let observed = false;
  const io = new IntersectionObserver(
    (entries) => {
      observed = true;
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        reveal(e.target);
        io.unobserve(e.target);
      }
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.12 },
  );
  // O hero já está na tela ao abrir: entra em cascata no próximo quadro.
  const hero = document.querySelector("main > section");
  requestAnimationFrame(() => hero.querySelectorAll(".reveal").forEach(reveal));
  reveals.forEach((el) => {
    if (!hero.contains(el)) io.observe(el);
  });
  // Rede de segurança: se o observer nunca responder, nada fica invisível.
  setTimeout(() => {
    if (observed) return;
    io.disconnect();
    showAll();
  }, 2500);
} else {
  showAll();
}

if (!reduceMotion) {
  // Brilhos do hero: vão e voltam devagar, cada um num ritmo.
  const drift = { ease: "easeInOut", repeat: Infinity, repeatType: "reverse" };
  const orbA = document.querySelector('[data-orb="a"]');
  const orbB = document.querySelector('[data-orb="b"]');
  if (orbA) animate(orbA, { x: 140, y: 90, scale: 1.15 }, { ...drift, duration: 18 });
  if (orbB) animate(orbB, { x: -120, y: 120, scale: 0.9 }, { ...drift, duration: 22 });

  // Pulso de sinal correndo em cada trilha do circuito, da borda pro centro.
  // `pathLength=1` + traço `0.06 1.4`: o deslocamento vai de 1.46 (um
  // período) a 0, igual em qualquer trilha. Duração e atraso de cada uma
  // vêm do SVG (`data-dur`, `data-delay`).
  for (const path of document.querySelectorAll("[data-pulses] path")) {
    animate(1.46, 0, {
      duration: Number(path.dataset.dur),
      delay: Number(path.dataset.delay),
      ease: "linear",
      repeat: Infinity,
      onUpdate: (v) => (path.style.strokeDashoffset = String(v)),
    });
  }

  // "moderna" do título: o gradiente verde → azul corre pelo texto. O i18n
  // troca o HTML do título ao mudar o idioma, então a animação recomeça.
  let shine = null;
  const startShine = () => {
    shine?.stop();
    const grad = document.querySelector("h1 .grad");
    if (grad)
      shine = animate(grad, { backgroundPosition: ["0% 0%", "-200% 0%"] }, { duration: 6, ease: "linear", repeat: Infinity });
  };
  startShine();
  addEventListener("reemu-lang", startShine);
}

// Ampliador das capturas (só as que existem — quadros "em breve" ignoram).
const lightbox = document.getElementById("lightbox");
const big = lightbox?.querySelector(".lightbox-img");
document.querySelectorAll(".shot-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    if ("missing" in btn.closest("figure").dataset || !big) return;
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
    // "Copia e cola" (BR Code) do mesmo Pix, o que está no QR code.
    const code = card.dataset.pixCode?.trim();
    const codeBtn = card.querySelector(".pix-code-copy");
    if (code && codeBtn) {
      codeBtn.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(code);
          codeBtn.textContent = t("pixcode.copied");
        } catch {
          codeBtn.textContent = t("pixcode.manual");
        }
        setTimeout(() => (codeBtn.textContent = t("pixcode.copy")), 2500);
      });
    } else {
      codeBtn?.remove();
      card.querySelector("img[src='pix-qr.svg']")?.remove();
    }
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

// Galeria: enquanto nenhuma captura existe em site/screens/, o bloco inteiro
// fica escondido — cinco quadros "Captura em breve" parecem site inacabado.
// As imagens são `loading="lazy"`: o erro de cada uma chega quando ela entra
// perto da tela, então a conferência roda a cada erro e no `load`.
const gallery = document.querySelector(".gallery");
if (gallery) {
  const check = () => {
    const imgs = [...gallery.querySelectorAll("img")];
    gallery.hidden = imgs.every(
      (i) => "missing" in (i.closest("figure")?.dataset ?? {}) || (i.complete && i.naturalWidth === 0),
    );
  };
  gallery.addEventListener("error", check, true);
  addEventListener("load", check);
}
