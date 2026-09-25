// Movimento da página: entrada dos blocos ao rolar, contadores, topo com
// fundo depois de rolar e o ampliador das capturas. Tudo opcional — sem JS
// (ou com "reduzir movimento") a página aparece inteira e parada.

const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

// Topo: transparente no hero, com fundo desfocado depois de rolar.
// (não `top`: é `window.top` no escopo global e redeclarar dá SyntaxError)
const header = document.querySelector(".top");
const onScroll = () => header.classList.toggle("scrolled", scrollY > 12);
addEventListener("scroll", onScroll, { passive: true });
onScroll();

// Contador de 0 até `data-to` (com `data-decimals` casas, vírgula pt-BR).
function countUp(el) {
  const to = Number(el.dataset.to);
  const decimals = Number(el.dataset.decimals || 0);
  const fmt = (v) =>
    v.toLocaleString("pt-BR", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  if (reduceMotion) {
    el.textContent = fmt(to);
    return;
  }
  const start = performance.now();
  const dur = 1400;
  const tick = (now) => {
    const t = Math.min(1, (now - start) / dur);
    const eased = 1 - Math.pow(1 - t, 3);
    el.textContent = fmt(to * eased);
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
  // O hero já está na tela ao abrir: entra em cascata no próximo quadro,
  // sem esperar o observer. O resto entra conforme a rolagem.
  const heroReveals = document.querySelectorAll(".hero .reveal");
  requestAnimationFrame(() => {
    heroReveals.forEach((el) => {
      el.classList.add("in");
      el.querySelectorAll(".count").forEach(countUp);
    });
  });
  reveals.forEach((el) => {
    if (!el.closest(".hero")) io.observe(el);
  });
  // Rede de segurança: se o observer nunca responder (navegador/aba em
  // estado estranho), nada pode ficar invisível — mostra tudo.
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
if (lightbox && typeof lightbox.showModal === "function") {
  const big = lightbox.querySelector("img");
  document.querySelectorAll(".shot-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const fig = btn.closest("figure");
      if (fig.classList.contains("missing")) return;
      const img = btn.querySelector("img");
      big.src = img.currentSrc || img.src;
      big.alt = img.alt;
      lightbox.showModal();
    });
  });
  // clicar fora da imagem fecha
  lightbox.addEventListener("click", (e) => {
    if (e.target === lightbox) lightbox.close();
  });
}
