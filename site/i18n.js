// Idiomas do site (pt-BR, en, es) — mesma regra do app: o português é a
// origem e fica no próprio HTML; os elementos com `data-i18n="chave"` trocam
// o conteúdo pelo dicionário do idioma ativo, e `data-i18n-attr="attr=chave;…"`
// troca atributos (alt, aria-label, content). Textos montados em JS usam `t()`.
//
// Idioma: `?lang=` na URL, a escolha salva em localStorage (`reemu.site.lang`)
// ou o 1º de `navigator.languages` que casar pela língua-base; senão pt-BR.

export const LANGS = ["pt-BR", "en", "es"];
const STORE = "reemu.site.lang";

// Só os textos montados em JS precisam de pt-BR aqui; os do HTML vêm do HTML.
const JS_PT = {
  "dl.exe": "Instalador (.exe)",
  "dl.exeHint": "recomendado",
  "dl.msi": "Pacote MSI (.msi)",
  "dl.msiHint": "instalação gerenciada",
  "dl.appimage": "AppImage",
  "dl.appimageHint": "qualquer distro · recomendado",
  "dl.deb": "Pacote .deb",
  "dl.debHint": "Debian, Ubuntu, Mint",
  "dl.rpm": "Pacote .rpm",
  "dl.rpmHint": "Fedora, openSUSE",
  "dl.for": "Baixar para {os} — {label}",
  "dl.none": "Ainda não há versão publicada. Acompanhe em ",
  "dl.otherOs": "O ReEmu roda em Windows e Linux.",
  "dl.viewTag": "Ver a versão {tag} no GitHub",
  "dl.latest": "Mais recente",
  "dl.pre": "Pré-lançamento",
  "dl.changes": "O que mudou",
  "dl.older": "Versões anteriores",
  "dl.more": "Outras opções de download",
  "dl.viewGithub": "Ver no GitHub",
  "dl.noReleases": "Nenhuma versão publicada ainda.",
  "dl.fallback": "Baixar no GitHub Releases",
  "dl.fallbackSub": "abre a versão mais recente",
  "dl.viewAll": "Ver todas no GitHub",
  "dl.rate": "O GitHub limitou as consultas deste endereço por agora. Tente de novo em alguns minutos.",
  "dl.failed": "Não foi possível carregar a lista de versões.",
  "pix.copied": "Chave copiada!",
  "pix.manual": "Copie a chave acima",
  "pix.copy": "Copiar chave Pix",
  "pixcode.copied": "Código copiado!",
  "pixcode.manual": "Use o QR code",
  "pixcode.copy": "Copiar código Pix",
  "share.text": "ReEmu: seus jogos clássicos com cara de console. Grátis para Linux e Windows.",
  "share.copied": "Link copiado!",
  "share.button": "Compartilhar",
  "car.slide": "Imagem {n} de {total}",
  "car.goto": "Ir para a imagem {n}",
};

const EN = {
  "skip": "Skip to content",
  "cta.all": "See all versions and what changed",
  "meta.description": "Free, open-source frontend to organize and play your classic games on Windows and Linux, with an interface built for the controller.",
  "meta.og": "Your classics. Your collection. A modern experience. Free and open source, for Windows and Linux.",
  "nav.aria": "Sections",
  "nav.support": "Support",
  "nav.download": "Download",
  "nav.home": "ReEmu — home",
  "hero.title": "Your classics. Your collection. A <span class=\"grad\">modern</span> experience.",
  "hero.lead": "ReEmu takes care of everything: emulators, covers, controller and looks. You just pick the game.",
  "hero.loading": "Looking for the latest version…",
  "sc2.btn": "Enlarge: library",
  "sc2.alt": "PC Engine library with box art and jump-to-letter",
  "sc3.btn": "Enlarge: game page",
  "sc3.alt": "MSR game page with core, aspect ratio, 16:9 widescreen and presets",
  "sc4.btn": "Enlarge: shaders",
  "sc4.alt": "Shader list with the preview beside it, on a 2D scene",
  "sup.title": "Support ReEmu",
  "d1.p": "Monthly or one-time support, straight through GitHub.",
  "d1.b": "Support on GitHub",
  "d2.p": "Scan the QR code with your bank app (Brazil) or copy the code.",
  "d2.b": "Copy Pix key",
  "d2.qr": "Pix QR code to support ReEmu",
  "d2.key": "Or use the random key:",
  "d3.p": "Buy the project a coffee, no sign-up.",
  "d3.b": "Support on Ko-fi",
  "d4.p": "Credit card from any country.",
  "d4.b": "Donate via PayPal",
  "sup.soon": "Donation options are being set up. Meanwhile, you can help in other ways:",
  "dl.title": "All versions",
  "dl.older": "Earlier versions",
  "foot.top": "ReEmu — back to top",
  "foot.releases": "Releases",
  "foot.issue": "Report a problem",
  "foot.community": "Community on Alvanista",
  "foot.contact": "Email contact: reemucontact@gmail.com",
  "foot.legal": "Open source, MIT license.",
  "foot.libretro": "ReEmu uses <a class='text-green underline-offset-2 hover:underline' href='https://www.libretro.com' target='_blank' rel='noopener'>libretro</a> cores, downloaded from the official buildbot, each with its own authors and license. libretro and RetroArch are trademarks of their respective owners; ReEmu is not affiliated with or endorsed by them.",
  "foot.bezels": "The bezels used by ReEmu come from <a class='text-green underline-offset-2 hover:underline' href='https://github.com/thebezelproject/BezelProject' target='_blank' rel='noopener'>The Bezel Project</a> and are downloaded from the project’s GitHub when you pick a system. The images belong to their respective authors; ReEmu is not affiliated with or endorsed by The Bezel Project.",
  "lightbox.aria": "Enlarged screenshot",
  "lightbox.close": "Close",
  "dl.exe": "Installer (.exe)",
  "dl.exeHint": "recommended",
  "dl.msi": "MSI package (.msi)",
  "dl.msiHint": "managed install",
  "dl.appimage": "AppImage",
  "dl.appimageHint": "any distro · recommended",
  "dl.deb": ".deb package",
  "dl.debHint": "Debian, Ubuntu, Mint",
  "dl.rpm": ".rpm package",
  "dl.rpmHint": "Fedora, openSUSE",
  "dl.for": "Download for {os} — {label}",
  "dl.none": "No version has been published yet. Follow along on ",
  "dl.otherOs": "ReEmu runs on Windows and Linux.",
  "dl.viewTag": "See version {tag} on GitHub",
  "dl.latest": "Latest",
  "dl.pre": "Pre-release",
  "dl.changes": "What changed",
  "dl.viewGithub": "See on GitHub",
  "dl.noReleases": "No version published yet.",
  "dl.fallback": "Download from GitHub Releases",
  "dl.fallbackSub": "opens the latest version",
  "dl.viewAll": "See all on GitHub",
  "dl.rate": "GitHub is rate-limiting requests from this address for now. Try again in a few minutes.",
  "dl.failed": "Couldn't load the list of versions.",
  "pix.copied": "Key copied!",
  "pix.manual": "Copy the key above",
  "pix.copy": "Copy Pix key",
  "pixcode.copied": "Code copied!",
  "pixcode.manual": "Use the QR code",
  "pixcode.copy": "Copy Pix code",
  "share.text": "ReEmu: your classic games with a console look. Free for Linux and Windows.",
  "share.copied": "Link copied!",
  "share.button": "Share",
  "meta.title": "ReEmu — Your classics. Your collection.",
  "more.title": "From Atari to PlayStation 2",
  "more.lead": "Nearly 50 systems, one controller.",
  "cta.button": "Download ReEmu",
  "sup.more": "Other ways to help",
  "own.p1": "ReEmu does not include games or BIOS.",
  "foot.tagline": "Emulator frontend for Windows and Linux.",
  "nav.features": "Features",
  "nav.systems": "Systems",
  "dl2.title": "Download ReEmu",
  "dl2.lead": "Free. It updates itself, so you only download it once.",
  "sup.lead": "No ads and no subscription. If it helped you, give back however you can.",
  "car.aria": "ReEmu screenshots",
  "car.prev": "Previous image",
  "car.next": "Next image",
  "car.slide": "Image {n} of {total}",
  "car.goto": "Go to image {n}",
  "dl.more": "Other download options",
  "show1.t": "Your collection, organized",
  "show1.p": "Point to your folders: covers, systems and search ready, with thousands of games scrolling smoothly.",
  "show2.t": "Press Play. Or tune everything.",
  "show2.p": "Core, aspect ratio, widescreen, presets, save states, shader, cheats and achievements on each game’s page.",
  "show3.t": "The retro look",
  "show3.p": "CRT and LCD shaders with a live preview, plus bezels from The Bezel Project.",
  "more2.title": "Much more than playing",
  "m1.t": "Controller only",
  "m1.p": "Menus, on-screen keyboard and file browser without touching the keyboard.",
  "m2.t": "One-click emulators",
  "m2.p": "libretro cores come from the catalog, already matched to each system.",
  "m3.t": "Save states with thumbnails",
  "m3.p": "Save and load from the pause menu.",
  "m4.t": "BIOS checker",
  "m4.p": "Shows what each system needs and what is already in place. BIOS not included.",
  "m5.t": "Eleven themes",
  "m5.p": "Light, dark, high contrast and one that’s all yours.",
  "m6.t": "Cores kept up to date",
  "m6.p": "The bell tells you about new versions, and you can roll back.",
  "m7.t": "Updates itself",
  "m7.p": "ReEmu tells you and installs the new version.",
  "m8.t": "Free and open",
  "m8.p": "MIT license, no ads, no subscription.",
  "req.line": "Windows 10 and 11 (64-bit) · 64-bit Linux, AppImage or .deb · Xbox, PlayStation or generic controller",
  "sup.star": "Star on GitHub",
  "car.zoom": "Enlarge image",
  "car1.alt": "SoulCalibur on the Dreamcast with the console bezel",
  "car2.alt": "Home screen with Continue and recently added games",
  "car3.alt": "The Legend of Zelda: Ocarina of Time on the Nintendo 64 with a bezel",
  "car4.alt": "Chrono Trigger page, Super Nintendo, with core, aspect ratio and presets",
  "car5.alt": "Aladdin on the Super Nintendo with a themed bezel",
  "car6.alt": "Pause menu with QuickSave and QuickLoad",
  "car7.alt": "The Adventures of Batman & Robin on the Super Nintendo with a themed bezel",
  "car8.alt": "Library with the Master System and Mega Drive rows",
  "car9.alt": "International Superstar Soccer '98 on the Nintendo 64 with a themed bezel",
  "car10.alt": "AMD FSR shader parameters with the preview beside them",
  "car11.alt": "Settings as blocks, in a light theme",
  "car12.alt": "Castlevania: Circle of the Moon page, Game Boy Advance, in a light theme",
  "car13.alt": "International Superstar Soccer '98 page with the 16:9 widescreen patch",
  "sc8.btn": "Enlarge: core options",
  "sc8.alt": "Core options for this game only, each showing its inherited value",
  "sc9.btn": "Enlarge: game shader",
  "sc9.alt": "The game’s shader with the ScaleFX parameters beside it",
};

const ES = {
  "skip": "Saltar al contenido",
  "cta.all": "Ver todas las versiones y qué cambió",
  "meta.description": "Frontend gratuito y de código abierto para organizar y jugar tus juegos clásicos en Windows y Linux, con una interfaz pensada para el mando.",
  "meta.og": "Tus clásicos. Tu colección. Una experiencia moderna. Gratis y de código abierto, para Windows y Linux.",
  "nav.aria": "Secciones",
  "nav.support": "Apoya",
  "nav.download": "Descargar",
  "nav.home": "ReEmu — inicio",
  "hero.title": "Tus clásicos. Tu colección. Una experiencia <span class=\"grad\">moderna</span>.",
  "hero.lead": "ReEmu se encarga de todo: emuladores, carátulas, mando y aspecto. Tú solo eliges el juego.",
  "hero.loading": "Buscando la versión más reciente…",
  "sc2.btn": "Ampliar: biblioteca",
  "sc2.alt": "Biblioteca de PC Engine con portadas y salto por letra",
  "sc3.btn": "Ampliar: página del juego",
  "sc3.alt": "Página de MSR con core, relación de aspecto, panorámico 16:9 y ajustes predefinidos",
  "sc4.btn": "Ampliar: shaders",
  "sc4.alt": "Lista de shaders con la vista previa al lado, en una escena 2D",
  "sup.title": "Apoya ReEmu",
  "d1.p": "Apoyo mensual o único, directamente por GitHub.",
  "d1.b": "Apoyar en GitHub",
  "d2.p": "Escanea el código QR con la app del banco (Brasil) o copia el código.",
  "d2.b": "Copiar clave Pix",
  "d2.qr": "Código QR Pix para apoyar ReEmu",
  "d2.key": "O usa la clave aleatoria:",
  "d3.p": "Invita un café al proyecto, sin registro.",
  "d3.b": "Apoyar en Ko-fi",
  "d4.p": "Tarjeta de crédito de cualquier país.",
  "d4.b": "Donar con PayPal",
  "sup.soon": "Las formas de donación se están configurando. Mientras tanto, puedes ayudar de otras maneras:",
  "dl.title": "Todas las versiones",
  "dl.older": "Versiones anteriores",
  "foot.top": "ReEmu — volver arriba",
  "foot.releases": "Versiones",
  "foot.issue": "Informar de un problema",
  "foot.community": "Comunidad en Alvanista",
  "foot.contact": "Contacto por correo: reemucontact@gmail.com",
  "foot.legal": "Código abierto, licencia MIT.",
  "foot.libretro": "ReEmu usa cores <a class='text-green underline-offset-2 hover:underline' href='https://www.libretro.com' target='_blank' rel='noopener'>libretro</a>, descargados del buildbot oficial, cada uno con sus autores y su licencia. libretro y RetroArch son marcas de sus respectivos dueños; ReEmu no está afiliado ni respaldado por ellos.",
  "foot.bezels": "Los marcos que usa ReEmu vienen de <a class='text-green underline-offset-2 hover:underline' href='https://github.com/thebezelproject/BezelProject' target='_blank' rel='noopener'>The Bezel Project</a> y se descargan del GitHub del proyecto cuando eliges un sistema. Las imágenes pertenecen a sus respectivos autores; ReEmu no está afiliado ni respaldado por The Bezel Project.",
  "lightbox.aria": "Captura ampliada",
  "lightbox.close": "Cerrar",
  "dl.exe": "Instalador (.exe)",
  "dl.exeHint": "recomendado",
  "dl.msi": "Paquete MSI (.msi)",
  "dl.msiHint": "instalación gestionada",
  "dl.appimage": "AppImage",
  "dl.appimageHint": "cualquier distro · recomendado",
  "dl.deb": "Paquete .deb",
  "dl.debHint": "Debian, Ubuntu, Mint",
  "dl.rpm": "Paquete .rpm",
  "dl.rpmHint": "Fedora, openSUSE",
  "dl.for": "Descargar para {os} — {label}",
  "dl.none": "Todavía no hay versión publicada. Síguelo en ",
  "dl.otherOs": "ReEmu funciona en Windows y Linux.",
  "dl.viewTag": "Ver la versión {tag} en GitHub",
  "dl.latest": "Más reciente",
  "dl.pre": "Prelanzamiento",
  "dl.changes": "Qué cambió",
  "dl.viewGithub": "Ver en GitHub",
  "dl.noReleases": "Todavía no hay versiones publicadas.",
  "dl.fallback": "Descargar desde GitHub Releases",
  "dl.fallbackSub": "abre la versión más reciente",
  "dl.viewAll": "Ver todas en GitHub",
  "dl.rate": "GitHub ha limitado por ahora las consultas desde esta dirección. Inténtalo de nuevo en unos minutos.",
  "dl.failed": "No se pudo cargar la lista de versiones.",
  "pix.copied": "¡Clave copiada!",
  "pix.manual": "Copia la clave de arriba",
  "pix.copy": "Copiar clave Pix",
  "pixcode.copied": "¡Código copiado!",
  "pixcode.manual": "Usa el código QR",
  "pixcode.copy": "Copiar código Pix",
  "share.text": "ReEmu: tus juegos clásicos con aspecto de consola. Gratis para Linux y Windows.",
  "share.copied": "¡Enlace copiado!",
  "share.button": "Compartir",
  "meta.title": "ReEmu — Tus clásicos. Tu colección.",
  "more.title": "De Atari a PlayStation 2",
  "more.lead": "Casi 50 sistemas, un solo mando.",
  "cta.button": "Descargar ReEmu",
  "sup.more": "Otras formas de ayudar",
  "own.p1": "ReEmu no incluye juegos ni BIOS.",
  "foot.tagline": "Frontend de emuladores para Windows y Linux.",
  "nav.features": "Funciones",
  "nav.systems": "Sistemas",
  "dl2.title": "Descarga ReEmu",
  "dl2.lead": "Gratis. Se actualiza solo, así que solo lo descargas una vez.",
  "sup.lead": "Sin anuncios ni suscripción. Si te ayudó, devuélvelo como puedas.",
  "car.aria": "Capturas de ReEmu",
  "car.prev": "Imagen anterior",
  "car.next": "Imagen siguiente",
  "car.slide": "Imagen {n} de {total}",
  "car.goto": "Ir a la imagen {n}",
  "dl.more": "Otras opciones de descarga",
  "show1.t": "Tu colección organizada",
  "show1.p": "Indica las carpetas: carátulas, sistemas y búsqueda listos, con miles de juegos sin trabarse.",
  "show2.t": "Pulsa Jugar. O ajústalo todo.",
  "show2.p": "Core, relación de aspecto, panorámico, ajustes, save states, shader, trucos y logros en la página de cada juego.",
  "show3.t": "El estilo retro",
  "show3.p": "Shaders de tele de tubo y LCD con vista previa, y marcos de The Bezel Project.",
  "more2.title": "Mucho más que jugar",
  "m1.t": "Solo con el mando",
  "m1.p": "Menús, teclado virtual y explorador de archivos sin tocar el teclado.",
  "m2.t": "Emuladores en un clic",
  "m2.p": "Los cores libretro salen del catálogo, ya listos para cada sistema.",
  "m3.t": "Save states con miniatura",
  "m3.p": "Guarda y carga desde el menú de pausa.",
  "m4.t": "Verificador de BIOS",
  "m4.p": "Muestra lo que pide cada sistema y lo que ya está en su lugar. BIOS no incluidas.",
  "m5.t": "Once temas",
  "m5.p": "Claros, oscuros, alto contraste y uno a tu gusto.",
  "m6.t": "Cores siempre al día",
  "m6.p": "La campana avisa cuando hay versión nueva, y puedes volver a la anterior.",
  "m7.t": "Se actualiza solo",
  "m7.p": "ReEmu avisa e instala la versión nueva.",
  "m8.t": "Gratis y abierto",
  "m8.p": "Licencia MIT, sin anuncios ni suscripción.",
  "req.line": "Windows 10 y 11 (64 bits) · Linux de 64 bits, AppImage o .deb · Mando Xbox, PlayStation o genérico",
  "sup.star": "Dar una estrella en GitHub",
  "car.zoom": "Ampliar la imagen",
  "car1.alt": "SoulCalibur en Dreamcast con el marco de la consola",
  "car2.alt": "Pantalla de inicio con Continuar y los juegos añadidos recientemente",
  "car3.alt": "The Legend of Zelda: Ocarina of Time en Nintendo 64 con marco",
  "car4.alt": "Página de Chrono Trigger, de Super Nintendo, con core, relación de aspecto y ajustes",
  "car5.alt": "Aladdin en Super Nintendo con marco temático",
  "car6.alt": "Menú de pausa con QuickSave y QuickLoad",
  "car7.alt": "The Adventures of Batman & Robin en Super Nintendo con marco temático",
  "car8.alt": "Biblioteca con las filas de Master System y Mega Drive",
  "car9.alt": "International Superstar Soccer '98 en Nintendo 64 con marco temático",
  "car10.alt": "Parámetros del shader AMD FSR con la vista previa al lado",
  "car11.alt": "Configuración en bloques, con un tema claro",
  "car12.alt": "Página de Castlevania: Circle of the Moon, de Game Boy Advance, con un tema claro",
  "car13.alt": "Página de International Superstar Soccer '98 con el parche panorámico 16:9",
  "sc8.btn": "Ampliar: opciones del core",
  "sc8.alt": "Opciones del core solo para este juego, con el valor heredado de cada una",
  "sc9.btn": "Ampliar: shader del juego",
  "sc9.alt": "Shader del juego con los parámetros de ScaleFX al lado",
};

const DICT = { "pt-BR": JS_PT, en: EN, es: ES };

function stored() {
  try {
    return localStorage.getItem(STORE);
  } catch {
    return null;
  }
}

/** Idioma do navegador casado pela língua-base (pt-PT → pt-BR, es-MX → es). */
function fromNavigator() {
  for (const tag of navigator.languages || [navigator.language || ""]) {
    const base = tag.toLowerCase().split("-")[0];
    const hit = LANGS.find((l) => l.toLowerCase().split("-")[0] === base);
    if (hit) return hit;
  }
  return "pt-BR";
}

// `?lang=en` na URL vence (link compartilhado já no idioma certo).
const fromUrl = new URLSearchParams(location.search).get("lang");
let current = LANGS.includes(fromUrl) ? fromUrl : LANGS.includes(stored()) ? stored() : fromNavigator();

export const lang = () => current;

/** Texto de `key` no idioma ativo, com `{var}` trocado por `vars.var`. */
export function t(key, vars = {}) {
  const s = DICT[current][key] ?? JS_PT[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}

/** Aplica o idioma ativo aos elementos marcados no HTML. */
function applyDom() {
  document.documentElement.lang = current;
  const dict = DICT[current];
  for (const node of document.querySelectorAll("[data-i18n]")) {
    // guarda o pt-BR original na 1ª troca, pra poder voltar
    if (node.dataset.i18nPt == null) node.dataset.i18nPt = node.innerHTML;
    node.innerHTML = current === "pt-BR" ? node.dataset.i18nPt : (dict[node.dataset.i18n] ?? node.dataset.i18nPt);
  }
  for (const node of document.querySelectorAll("[data-i18n-attr]")) {
    for (const pair of node.dataset.i18nAttr.split(";")) {
      const [attr, key] = pair.split("=").map((x) => x.trim());
      const saved = `i18nPt${attr.replace(/[^a-z]/gi, "")}`;
      if (node.dataset[saved] == null) node.dataset[saved] = node.getAttribute(attr) ?? "";
      node.setAttribute(attr, current === "pt-BR" ? node.dataset[saved] : (dict[key] ?? node.dataset[saved]));
    }
  }
  for (const b of document.querySelectorAll("[data-lang]")) {
    b.setAttribute("appearance", b.dataset.lang === current ? "primary" : "subtle");
    b.setAttribute("aria-pressed", String(b.dataset.lang === current));
  }
}

/** Troca o idioma, salva a escolha e avisa quem monta texto em JS. */
export function setLang(l) {
  if (!LANGS.includes(l)) return;
  current = l;
  try {
    localStorage.setItem(STORE, l);
  } catch {
    // sem storage: vale só nesta visita
  }
  applyDom();
  dispatchEvent(new CustomEvent("reemu-lang", { detail: l }));
}

for (const b of document.querySelectorAll("[data-lang]")) {
  b.addEventListener("click", () => setLang(b.dataset.lang));
}
applyDom();
