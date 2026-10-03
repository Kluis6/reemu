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
  "pixcode.copied": "Código copiado!",
  "pixcode.manual": "Use o QR code",
  "pixcode.copy": "Copiar código Pix",
  "share.text": "ReEmu: seus jogos clássicos com cara de console. Grátis para Linux e Windows.",
  "share.copied": "Link copiado!",
  "share.button": "Compartilhar",
};

const EN = {
  "skip": "Skip to content",
  "cta.all": "See all versions and what changed",
  "meta.description": "Free, open-source frontend to organize and play your classic games on Windows and Linux, with an interface built for the controller.",
  "meta.og": "Your classics. Your collection. A modern experience. Free and open source, for Windows and Linux.",
  skip: "Skip to content",
  "nav.aria": "Sections",
  "nav.support": "Support",
  "nav.download": "Download",
  "nav.home": "ReEmu — home",
  "hero.title": "Your classics. Your collection. A <span class=\"grad\">modern</span> experience.",
  "hero.lead": "Organize, discover and play your classics in an interface built for the controller — for people who love playing, not configuring.",
  "hero.loading": "Looking for the latest version…",
  "hero.note": "Free download · Windows · Linux · Open source",
  "f1.t": "Automatic organization",
  "f1.p": "Point ReEmu to your folders: it identifies each game and builds the library by system, with covers and info.",
  "f2.t": "Built for the controller",
  "f2.p": "Library, settings and pause menu: everything works with the controller, no keyboard needed.",
  "f3.t": "The screen of each era",
  "f3.p": "Shaders and bezels recreate the CRT TV, the handheld and the arcade.",
  "f4.t": "Pick up where you left off",
  "f4.p": "Save states with a thumbnail of the saved moment, so you find the right spot at a glance.",
  "f5.t": "Favorites close at hand",
  "f5.p": "Favorites, recent games and search take you straight to what you want to play.",
  "f6.t": "Your play history",
  "f6.p": "ReEmu tracks the time you spend on each game and shows it on the game’s page.",
  "s1.t": "Install",
  "s1.p": "Download for Windows or Linux and install — no dependencies to hunt down.",
  "s2.t": "Add your games",
  "s2.p": "Pick your folders: ReEmu identifies the systems and builds the library.",
  "s3.t": "Play",
  "s3.p": "Grab the controller and pick a game.",
  "sc1.btn": "Enlarge: game with shader",
  "sc1.alt": "Jet Set Radio on the Dreamcast with a CRT shader and bezel",
  "sc1.cap": "Game with CRT shader and bezel",
  "sc2.btn": "Enlarge: library",
  "sc2.alt": "PC Engine library with box art",
  "sc2.cap": "Library by system, with box art",
  "sc3.btn": "Enlarge: game page",
  "sc3.alt": "MSR game page with cover, background art, play time and emulator options",
  "sc3.cap": "Game page: cover, play time and emulator options",
  "sc4.btn": "Enlarge: game with themed bezel",
  "sc4.alt": "The Adventures of Batman & Robin with a themed bezel and CRT shader",
  "sc4.cap": "Classics with themed bezels and CRT shader",
  "sc5.btn": "Enlarge: pause menu",
  "sc5.alt": "Pause menu with QuickSave and QuickLoad over the blurred game",
  "sc5.cap": "Pause menu and save states",
  "sc6.btn": "Enlarge: settings",
  "sc6.alt": "Settings in categories: profile, appearance, video, controllers, cores, BIOS and system",
  "sc6.cap": "Settings in categories, navigable with a controller",
  "sys.aria": "Some of the supported systems",
  "sys.more": "and more…",
  "sup.title": "Free. Open. Made for players.",
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
  "nav.library": "Library",
  "nav.start": "Get started",
  "hero.lead2": "Your collection gets box art, info, favorites, progress, themes and navigation designed for the controller.",
  "life.title": "Give your collection a new life",
  "life.p1": "Your classic games deserve more than a folder full of files.",
  "life.p2": "ReEmu turns your collection into a dynamic, organized library that is easy to explore.",
  "life.punch": "Find. Choose. Play.",
  "life.punch2": "No hassle.",
  "lib.title": "A library made for playing",
  "start.title": "From download to first game",
  "less.title": "Less configuring. More fun.",
  "less.lead": "You don't need to turn a night of nostalgia into a configuration session.",
  "less.with": "ReEmu takes care of it",
  "w1.t": "Emulators",
  "w1.p": "libretro cores come from the catalog with one click, and ReEmu picks the right one for each system.",
  "w2.t": "BIOS",
  "w2.p": "Shows which files each system needs and whether they’re in place.",
  "w3.t": "Covers and info",
  "w3.p": "Fetched from the internet for every game in the library.",
  "w4.t": "Controllers",
  "w4.p": "Recognized when you plug them in, with buttons already mapped.",
  "w5.t": "Updates",
  "w5.p": "ReEmu tells you when a new version is out and updates itself.",
  "room.title": "Made for the living room. Perfect for the PC.",
  "room.lead": "Works at your desk and on the living-room TV, fullscreen or windowed.",
  "room.b1": "Console proportions on any screen.",
  "room.b2": "Text readable from the couch.",
  "room.b3": "Adjustable interface size.",
  "more.title": "From Atari to PlayStation",
  "more.lead": "Consoles, handhelds, arcades and computers from many generations, in the same library.",
  "cta.title": "Your classic games have never felt so current.",
  "cta.lead": "Free, open source and ready for Windows and Linux.",
  "cta.button": "Download ReEmu",
  "sup.b1": "No ads.",
  "sup.b2": "No subscription.",
  "sup.b3": "No turning your collection into a service.",
  "sup.help": "Like the project? Help with a donation, by reporting problems or by spreading the word.",
  "sup.cta": "Support ReEmu",
  "own.title": "Your collection belongs to you.",
  "own.p1": "ReEmu does not include games or BIOS.",
  "own.p2": "Use copies of games you own and respect the laws that apply in your region.",
  "foot.tagline": "Emulator frontend for Windows and Linux.",
};

const ES = {
  "skip": "Saltar al contenido",
  "cta.all": "Ver todas las versiones y qué cambió",
  "meta.description": "Frontend gratuito y de código abierto para organizar y jugar tus juegos clásicos en Windows y Linux, con una interfaz pensada para el mando.",
  "meta.og": "Tus clásicos. Tu colección. Una experiencia moderna. Gratis y de código abierto, para Windows y Linux.",
  skip: "Saltar al contenido",
  "nav.aria": "Secciones",
  "nav.support": "Apoya",
  "nav.download": "Descargar",
  "nav.home": "ReEmu — inicio",
  "hero.title": "Tus clásicos. Tu colección. Una experiencia <span class=\"grad\">moderna</span>.",
  "hero.lead": "Organiza, descubre y juega tus clásicos en una interfaz pensada para el mando — para quien disfruta jugando, no configurando.",
  "hero.loading": "Buscando la versión más reciente…",
  "hero.note": "Descarga gratis · Windows · Linux · Código abierto",
  "f1.t": "Organización automática",
  "f1.p": "Apunta ReEmu a tus carpetas: identifica cada juego y arma la biblioteca por sistema, con carátulas e información.",
  "f2.t": "Pensado para el mando",
  "f2.p": "Biblioteca, ajustes y menú de pausa: todo se maneja con el mando, sin teclado.",
  "f3.t": "La pantalla de cada época",
  "f3.p": "Shaders y marcos recrean la tele de tubo, la portátil y la recreativa.",
  "f4.t": "Sigue donde lo dejaste",
  "f4.p": "Save states con miniatura del momento guardado, para encontrar el punto justo de un vistazo.",
  "f5.t": "Tus favoritos a mano",
  "f5.p": "Favoritos, juegos recientes y búsqueda te llevan directo a lo que quieres jugar.",
  "f6.t": "Tu historial de juego",
  "f6.p": "ReEmu cuenta el tiempo de cada juego y lo muestra en su página.",
  "s1.t": "Instala",
  "s1.p": "Descárgalo para Windows o Linux e instálalo — sin dependencias que buscar.",
  "s2.t": "Añade tus juegos",
  "s2.p": "Elige las carpetas: ReEmu identifica los sistemas y arma la biblioteca.",
  "s3.t": "Juega",
  "s3.p": "Toma el mando y elige un juego.",
  "sc1.btn": "Ampliar: juego con shader",
  "sc1.alt": "Jet Set Radio en Dreamcast con shader CRT y marco",
  "sc1.cap": "Juego con shader CRT y marco",
  "sc2.btn": "Ampliar: biblioteca",
  "sc2.alt": "Biblioteca de PC Engine con portadas",
  "sc2.cap": "Biblioteca por sistema, con portadas",
  "sc3.btn": "Ampliar: página del juego",
  "sc3.alt": "Página de MSR con portada, arte de fondo, tiempo de juego y opciones del emulador",
  "sc3.cap": "Página del juego: portada, tiempo de juego y opciones del emulador",
  "sc4.btn": "Ampliar: juego con marco temático",
  "sc4.alt": "The Adventures of Batman & Robin con marco temático y shader CRT",
  "sc4.cap": "Clásicos con marco temático y shader CRT",
  "sc5.btn": "Ampliar: menú de pausa",
  "sc5.alt": "Menú de pausa con QuickSave y QuickLoad sobre el juego desenfocado",
  "sc5.cap": "Menú de pausa y save states",
  "sc6.btn": "Ampliar: configuración",
  "sc6.alt": "Configuración en categorías: perfil, apariencia, video, mandos, cores, BIOS y sistema",
  "sc6.cap": "Configuración en categorías, navegable con el mando",
  "sys.aria": "Algunos de los sistemas compatibles",
  "sys.more": "y más…",
  "sup.title": "Gratis. Abierto. Hecho para jugadores.",
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
  "nav.library": "Biblioteca",
  "nav.start": "Cómo empezar",
  "hero.lead2": "Tu colección gana carátulas, información, favoritos, progreso, temas y una navegación pensada para el mando.",
  "life.title": "Dale una nueva vida a tu colección",
  "life.p1": "Tus juegos clásicos merecen más que una carpeta llena de archivos.",
  "life.p2": "ReEmu convierte tu colección en una biblioteca dinámica, organizada y fácil de explorar.",
  "life.punch": "Encuentra. Elige. Juega.",
  "life.punch2": "Sin complicaciones.",
  "lib.title": "Una biblioteca hecha para jugar",
  "start.title": "De la descarga al primer juego",
  "less.title": "Menos configuración. Más diversión.",
  "less.lead": "No tienes que convertir una noche de nostalgia en una sesión de configuración.",
  "less.with": "ReEmu se encarga",
  "w1.t": "Emuladores",
  "w1.p": "Los cores de libretro se descargan del catálogo con un clic, y ReEmu elige el adecuado para cada sistema.",
  "w2.t": "BIOS",
  "w2.p": "Muestra qué archivos necesita cada sistema y si ya están en su lugar.",
  "w3.t": "Carátulas e información",
  "w3.p": "Se buscan en internet para cada juego de la biblioteca.",
  "w4.t": "Mandos",
  "w4.p": "Se reconocen al conectarlos, con los botones ya asignados.",
  "w5.t": "Actualizaciones",
  "w5.p": "ReEmu avisa cuando sale una versión nueva y se actualiza solo.",
  "room.title": "Hecho para el salón. Perfecto para el PC.",
  "room.lead": "Funciona en el escritorio y en la tele del salón, a pantalla completa o en ventana.",
  "room.b1": "Proporciones de consola en cualquier pantalla.",
  "room.b2": "Textos legibles desde el sofá.",
  "room.b3": "Tamaño de la interfaz ajustable.",
  "more.title": "Del Atari a PlayStation",
  "more.lead": "Consolas, portátiles, recreativas y ordenadores de varias generaciones, en la misma biblioteca.",
  "cta.title": "Tus juegos clásicos nunca se vieron tan actuales.",
  "cta.lead": "Gratis, de código abierto y listo para Windows y Linux.",
  "cta.button": "Descargar ReEmu",
  "sup.b1": "Sin anuncios.",
  "sup.b2": "Sin suscripción.",
  "sup.b3": "Sin convertir tu colección en un servicio.",
  "sup.help": "¿Te gusta el proyecto? Ayuda con una donación, reportando problemas o difundiéndolo.",
  "sup.cta": "Apoya ReEmu",
  "own.title": "Tu colección te pertenece.",
  "own.p1": "ReEmu no incluye juegos ni BIOS.",
  "own.p2": "Usa copias de los juegos que posees y respeta las leyes aplicables en tu región.",
  "foot.tagline": "Frontend de emuladores para Windows y Linux.",
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
