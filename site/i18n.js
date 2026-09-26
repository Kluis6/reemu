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
  "share.text": "ReEmu: seus jogos clássicos com cara de console. Grátis para Linux e Windows.",
  "share.copied": "Link copiado!",
  "share.button": "Compartilhar",
};

const EN = {
  "cta.all": "See all versions and what changed",
  "meta.description": "ReEmu: your classics, your collection, a modern experience. A free, open-source frontend to organize and play your classic games on Windows and Linux.",
  "meta.og": "Your classics. Your collection. A modern experience. Free and open source, for Windows and Linux.",
  skip: "Skip to content",
  "nav.aria": "Sections",
  "nav.support": "Support",
  "nav.download": "Download",
  "nav.home": "ReEmu — home",
  "hero.title": "Your classics. Your collection. A <span class=\"grad\">modern</span> experience.",
  "hero.lead": "ReEmu is a modern frontend to organize, discover and play your classic games, in an interface made for people who love playing — not configuring.",
  "hero.loading": "Looking for the latest version…",
  "hero.note": "Free download · Windows · Linux · Open source",
  "f1.t": "Your collection, your way",
  "f1.p": "Point ReEmu to your folders and let it organize the rest.",
  "f2.t": "An interface that feels like a console",
  "f2.p": "Browse your collection quickly and intuitively.",
  "f3.t": "Your games, looking the way you want",
  "f3.p": "Transform how the classics look.",
  "f4.t": "Pick up exactly where you left off",
  "f4.p": "Save your progress whenever you want.",
  "f5.t": "Your favorites always close by",
  "f5.p": "Build your own selection of games.",
  "f6.t": "Know how long you've played",
  "f6.p": "ReEmu tracks your play time and shows your history right on each title's page.",
  "s1.t": "Install",
  "s1.p": "Download ReEmu for Windows or Linux. A simple install, without hunting for dependencies.",
  "s2.t": "Add your collection",
  "s2.p": "Choose where your games are. ReEmu identifies the systems and organizes your library automatically.",
  "s3.t": "Sit. Choose. Play.",
  "s3.p": "Grab your controller, pick a game and start.",
  "sc1.btn": "Enlarge: home screen",
  "sc1.alt": "ReEmu home screen with highlights and recent games",
  "sc1.cap": "Home screen with highlights and recent games",
  "sc2.btn": "Enlarge: library",
  "sc2.alt": "Game library with box art",
  "sc2.cap": "Library by system, with box art",
  "sc3.btn": "Enlarge: game with shader",
  "sc3.alt": "Game running with a CRT shader and bezel",
  "sc3.cap": "Game with CRT shader and bezel",
  "sc4.btn": "Enlarge: pause menu",
  "sc4.alt": "Pause menu with save states and thumbnails",
  "sc4.cap": "Pause menu and save states",
  "sc5.btn": "Enlarge: core catalog",
  "sc5.alt": "Catalog of cores to download",
  "sc5.cap": "Core and BIOS catalog",
  "sys.aria": "Some of the supported systems",
  "sys.more": "and more…",
  "sup.title": "Free. Open. Made for players.",
  "sup.lead": "ReEmu is free and open source.",
  "d1.p": "Monthly or one-time support, straight through GitHub.",
  "d1.b": "Support on GitHub",
  "d2.p": "Instant transfer, no fee (Brazil).",
  "d2.b": "Copy Pix key",
  "d3.p": "Buy the project a coffee, no sign-up.",
  "d3.b": "Support on Ko-fi",
  "d4.p": "Credit card from any country.",
  "d4.b": "Donate via PayPal",
  "sup.soon": "Donation options are being set up. Meanwhile, you can help in other ways:",
  "h1.t": "Give it a star",
  "h1.p": "It helps more people find the project on GitHub.",
  "h1.b": "Open on GitHub",
  "h2.t": "Report problems",
  "h2.p": "A game that won't open or a controller that doesn't respond: tell us what happened.",
  "h2.b": "Open a report",
  "h3.t": "Spread the word",
  "h3.p": "Show ReEmu to people who love classic games.",
  "dl.title": "All versions",
  "foot.top": "ReEmu — back to top",
  "foot.releases": "Releases",
  "foot.issue": "Report a problem",
  "foot.legal": "Free · Open source · Windows · Linux",
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
  "share.text": "ReEmu: your classic games with a console look. Free for Linux and Windows.",
  "share.copied": "Link copied!",
  "share.button": "Share",
  "meta.title": "ReEmu — Your classics. Your collection.",
  "nav.library": "Library",
  "nav.start": "Get started",
  "hero.lead2": "Your collection gets box art, info, favorites, progress, themes and navigation designed for the controller.",
  "life.title": "Give your collection a new life",
  "life.p1": "Your classic games deserve more than a folder full of files.",
  "life.p2": "ReEmu turns your collection into a beautiful, organized library that is easy to explore.",
  "life.punch": "Find. Choose. Play.",
  "life.punch2": "No hassle.",
  "lib.title": "A library made for playing",
  "f1.p2": "Your games are identified automatically and shown with box art, info and organization by system.",
  "f2.p2": "Everything was designed to work great with a controller — from the library to the settings and the in-game menu.",
  "f3.p2": "Use shaders, bezels and different presentation styles to recreate the feeling of playing on an old TV, a handheld or an arcade.",
  "f4.p2": "Save states show a thumbnail of the saved moment so you can quickly find the right point in your adventure.",
  "f5.p2": "Favorites, recent games, highlights and quick search help you get to what you really want to play.",
  "lib.close1": "Your collection stops being just a library.",
  "lib.close2": "It starts telling your story.",
  "start.title": "From download to first game",
  "start.punch": "This is how it should be.",
  "less.title": "Less configuring. More playing.",
  "less.lead": "You don't need to turn a night of nostalgia into a configuration session.",
  "less.with": "With ReEmu",
  "w1.t": "Your collection",
  "w1.p": "Automatically organized into a visual library.",
  "w2.t": "Box art and info",
  "w2.p": "Your games become much easier to find and explore.",
  "w3.t": "Systems",
  "w3.p": "Many platforms gathered in a single place.",
  "w4.t": "Controllers",
  "w4.p": "Full navigation with a gamepad.",
  "w5.t": "Look",
  "w5.p": "Shaders, bezels and themes to personalize your experience.",
  "w6.t": "Progress",
  "w6.p": "Save states, history and play time always at hand.",
  "w7.t": "Updates",
  "w7.p": "ReEmu itself lets you know when a new version is out.",
  "room.title": "Made for the living room. Perfect for the PC.",
  "room.lead": "ReEmu was designed to work both at your desk and on the living-room TV.",
  "room.b1": "Clean interface.",
  "room.b2": "Controller navigation.",
  "room.b3": "Easy-to-read text.",
  "room.b4": "Quick access to your collection.",
  "room.punch": "Plug in the controller. Sit on the couch. Pick a game.",
  "more.title": "More than a library",
  "more.sub": "An experience for your whole collection.",
  "more.lead": "From Atari to PlayStation, from handhelds to arcades, ReEmu brings different generations of games together in a single experience.",
  "more.punch": "Classics from many generations.<br />One single library.",
  "cta.title": "Your classic games have never felt so current.",
  "cta.p1": "The nostalgia is in the games.",
  "cta.p2": "The experience can be modern.",
  "cta.sub": "Discover ReEmu",
  "cta.lead": "Download it for free and turn your collection into a library made for playing.",
  "cta.button": "Download ReEmu",
  "cta.note": "Windows · Linux · Open source · Free",
  "sup.b1": "No ads.",
  "sup.b2": "No subscription.",
  "sup.b3": "No turning your collection into a service.",
  "sup.help": "If you like the project, you can help keep it evolving — contributing code, reporting problems, spreading the word or supporting its development.",
  "sup.cta": "Support ReEmu",
  "own.title": "Your collection belongs to you.",
  "own.p1": "ReEmu does not include games or BIOS.",
  "own.p2": "Use copies of games you own and respect the laws that apply in your region.",
  "foot.tagline": "Your classics. Your collection. A modern experience.",
};

const ES = {
  "cta.all": "Ver todas las versiones y qué cambió",
  "meta.description": "ReEmu: tus clásicos, tu colección, una experiencia moderna. Frontend gratuito y de código abierto para organizar y jugar tus juegos clásicos en Windows y Linux.",
  "meta.og": "Tus clásicos. Tu colección. Una experiencia moderna. Gratis y de código abierto, para Windows y Linux.",
  skip: "Saltar al contenido",
  "nav.aria": "Secciones",
  "nav.support": "Apoya",
  "nav.download": "Descargar",
  "nav.home": "ReEmu — inicio",
  "hero.title": "Tus clásicos. Tu colección. Una experiencia <span class=\"grad\">moderna</span>.",
  "hero.lead": "ReEmu es un frontend moderno para organizar, descubrir y jugar tus juegos clásicos en una interfaz hecha para quien disfruta jugando — no configurando.",
  "hero.loading": "Buscando la versión más reciente…",
  "hero.note": "Descarga gratis · Windows · Linux · Código abierto",
  "f1.t": "Tu colección, a tu manera",
  "f1.p": "Indica a ReEmu tus carpetas y deja que organice el resto.",
  "f2.t": "Una interfaz que parece una consola",
  "f2.p": "Navega por tu colección de forma rápida e intuitiva.",
  "f3.t": "Tus juegos, con el aspecto que quieras",
  "f3.p": "Transforma la experiencia visual de los clásicos.",
  "f4.t": "Continúa justo donde lo dejaste",
  "f4.p": "Guarda tu progreso cuando quieras.",
  "f5.t": "Tus favoritos siempre a mano",
  "f5.p": "Crea tu propia selección de juegos.",
  "f6.t": "Sabe cuánto has jugado",
  "f6.p": "ReEmu registra tu tiempo de juego y muestra tu historial en la página de cada título.",
  "s1.t": "Instala",
  "s1.p": "Descarga ReEmu para Windows o Linux. Instalación sencilla, sin buscar dependencias.",
  "s2.t": "Añade tu colección",
  "s2.p": "Elige dónde están tus juegos. ReEmu identifica los sistemas y organiza tu biblioteca automáticamente.",
  "s3.t": "Siéntate. Elige. Juega.",
  "s3.p": "Toma tu mando, elige un juego y empieza.",
  "sc1.btn": "Ampliar: pantalla de inicio",
  "sc1.alt": "Pantalla de inicio de ReEmu con destacados y juegos recientes",
  "sc1.cap": "Pantalla de inicio con destacados y juegos recientes",
  "sc2.btn": "Ampliar: biblioteca",
  "sc2.alt": "Biblioteca de juegos con carátulas",
  "sc2.cap": "Biblioteca por sistema, con carátulas",
  "sc3.btn": "Ampliar: juego con shader",
  "sc3.alt": "Juego ejecutándose con shader CRT y marco",
  "sc3.cap": "Juego con shader CRT y marco",
  "sc4.btn": "Ampliar: menú de pausa",
  "sc4.alt": "Menú de pausa con save states y miniaturas",
  "sc4.cap": "Menú de pausa y save states",
  "sc5.btn": "Ampliar: catálogo de cores",
  "sc5.alt": "Catálogo de cores para descargar",
  "sc5.cap": "Catálogo de cores y BIOS",
  "sys.aria": "Algunos de los sistemas compatibles",
  "sys.more": "y más…",
  "sup.title": "Gratis. Abierto. Hecho para jugadores.",
  "sup.lead": "ReEmu es gratuito y de código abierto.",
  "d1.p": "Apoyo mensual o único, directamente por GitHub.",
  "d1.b": "Apoyar en GitHub",
  "d2.p": "Transferencia instantánea, sin comisión (Brasil).",
  "d2.b": "Copiar clave Pix",
  "d3.p": "Invita un café al proyecto, sin registro.",
  "d3.b": "Apoyar en Ko-fi",
  "d4.p": "Tarjeta de crédito de cualquier país.",
  "d4.b": "Donar con PayPal",
  "sup.soon": "Las formas de donación se están configurando. Mientras tanto, puedes ayudar de otras maneras:",
  "h1.t": "Dale una estrella",
  "h1.p": "Ayuda a que más gente encuentre el proyecto en GitHub.",
  "h1.b": "Abrir en GitHub",
  "h2.t": "Informa de problemas",
  "h2.p": "Un juego que no abre o un mando que no responde: cuéntanos qué pasó.",
  "h2.b": "Abrir un informe",
  "h3.t": "Difúndelo",
  "h3.p": "Enseña ReEmu a quien disfruta de los juegos clásicos.",
  "dl.title": "Todas las versiones",
  "foot.top": "ReEmu — volver arriba",
  "foot.releases": "Versiones",
  "foot.issue": "Informar de un problema",
  "foot.legal": "Gratis · Código abierto · Windows · Linux",
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
  "share.text": "ReEmu: tus juegos clásicos con aspecto de consola. Gratis para Linux y Windows.",
  "share.copied": "¡Enlace copiado!",
  "share.button": "Compartir",
  "meta.title": "ReEmu — Tus clásicos. Tu colección.",
  "nav.library": "Biblioteca",
  "nav.start": "Cómo empezar",
  "hero.lead2": "Tu colección gana carátulas, información, favoritos, progreso, temas y una navegación pensada para el mando.",
  "life.title": "Dale una nueva vida a tu colección",
  "life.p1": "Tus juegos clásicos merecen más que una carpeta llena de archivos.",
  "life.p2": "ReEmu convierte tu colección en una biblioteca bonita, organizada y fácil de explorar.",
  "life.punch": "Encuentra. Elige. Juega.",
  "life.punch2": "Sin complicaciones.",
  "lib.title": "Una biblioteca hecha para jugar",
  "f1.p2": "Tus juegos se identifican automáticamente y se muestran con carátulas, información y organización por sistema.",
  "f2.p2": "Todo está pensado para funcionar muy bien con mando — desde la biblioteca hasta los ajustes y el menú durante el juego.",
  "f3.p2": "Usa shaders, marcos y distintos estilos de presentación para recrear la sensación de jugar en una TV antigua, en una portátil o en una recreativa.",
  "f4.p2": "Los save states muestran una miniatura del momento guardado para que encuentres rápido el punto justo de tu aventura.",
  "f5.p2": "Favoritos, juegos recientes, destacados y búsqueda rápida te ayudan a llegar a lo que de verdad quieres jugar.",
  "lib.close1": "Tu colección deja de ser solo una biblioteca.",
  "lib.close2": "Empieza a contar tu historia.",
  "start.title": "De la descarga al primer juego",
  "start.punch": "Así es como debería ser.",
  "less.title": "Menos configuración. Más juego.",
  "less.lead": "No tienes que convertir una noche de nostalgia en una sesión de configuración.",
  "less.with": "Con ReEmu",
  "w1.t": "Tu colección",
  "w1.p": "Organizada automáticamente en una biblioteca visual.",
  "w2.t": "Carátulas e información",
  "w2.p": "Tus juegos son mucho más fáciles de encontrar y explorar.",
  "w3.t": "Sistemas",
  "w3.p": "Diversas plataformas reunidas en un solo lugar.",
  "w4.t": "Mandos",
  "w4.p": "Navegación completa con el gamepad.",
  "w5.t": "Aspecto",
  "w5.p": "Shaders, marcos y temas para personalizar tu experiencia.",
  "w6.t": "Progreso",
  "w6.p": "Save states, historial y tiempo de juego siempre a mano.",
  "w7.t": "Actualizaciones",
  "w7.p": "El propio ReEmu te avisa cuando hay una versión nueva.",
  "room.title": "Hecho para el salón. Perfecto para el PC.",
  "room.lead": "ReEmu está pensado para funcionar tanto en tu escritorio como en la TV del salón.",
  "room.b1": "Interfaz limpia.",
  "room.b2": "Navegación con mando.",
  "room.b3": "Textos fáciles de leer.",
  "room.b4": "Acceso rápido a tu colección.",
  "room.punch": "Conecta el mando. Siéntate en el sofá. Elige un juego.",
  "more.title": "Más que una biblioteca",
  "more.sub": "Una experiencia para toda tu colección.",
  "more.lead": "Del Atari a PlayStation, de las portátiles a las recreativas, ReEmu reúne distintas generaciones de juegos en una sola experiencia.",
  "more.punch": "Clásicos de varias generaciones.<br />Una sola biblioteca.",
  "cta.title": "Tus juegos clásicos nunca se vieron tan actuales.",
  "cta.p1": "La nostalgia está en los juegos.",
  "cta.p2": "La experiencia puede ser moderna.",
  "cta.sub": "Descubre ReEmu",
  "cta.lead": "Descárgalo gratis y convierte tu colección en una biblioteca hecha para jugar.",
  "cta.button": "Descargar ReEmu",
  "cta.note": "Windows · Linux · Código abierto · Gratis",
  "sup.b1": "Sin anuncios.",
  "sup.b2": "Sin suscripción.",
  "sup.b3": "Sin convertir tu colección en un servicio.",
  "sup.help": "Si te gusta el proyecto, puedes ayudar a que siga evolucionando — contribuyendo con código, informando de problemas, difundiendo el proyecto o apoyando su desarrollo.",
  "sup.cta": "Apoya ReEmu",
  "own.title": "Tu colección te pertenece.",
  "own.p1": "ReEmu no incluye juegos ni BIOS.",
  "own.p2": "Usa copias de los juegos que posees y respeta las leyes aplicables en tu región.",
  "foot.tagline": "Tus clásicos. Tu colección. Una experiencia moderna.",
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
