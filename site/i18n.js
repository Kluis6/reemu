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
  "meta.description":
    "ReEmu: an emulation frontend with libretro cores for Linux and Windows. A library with box art, CRT shaders, bezels, save states and full controller navigation.",
  "meta.og": "Your classic games with a console look. Free and open source, for Linux and Windows.",
  skip: "Skip to content",
  "nav.aria": "Sections",
  "nav.features": "Features",
  "nav.ease": "Ease of use",
  "nav.screens": "Screens",
  "nav.support": "Support",
  "nav.download": "Download",
  "nav.home": "ReEmu — home",
  "hero.badge": "Emulation frontend · Linux and Windows · Free",
  "hero.title": 'Your classic games, with a <span class="grad">console</span> look.',
  "hero.lead":
    'ReEmu gathers your collection in a beautiful library with box art and runs everything with <fluent-link inline href="https://www.libretro.com/">libretro</fluent-link> cores, the same ones RetroArch uses. CRT shaders, bezels, save states with thumbnails and full controller navigation, without opening a single config file.',
  "hero.loading": "Looking for the latest version…",
  "hero.note": "Open source (MIT). Once installed, ReEmu updates itself.",
  "stats.cores": "cores in the catalog",
  "stats.systems": "recognized systems",
  "stats.shaders": "of shader presets",
  "stats.pad": "controller for everything",
  "features.kicker": "Features",
  "features.title": "Everything a modern frontend should have",
  "features.lead":
    "Made for the living-room TV and the desktop PC: a fast, console-style interface with the details that make a difference every day.",
  "f1.t": "Library with box art",
  "f1.p": "Point to your ROM folder and ReEmu identifies each game (by hash, too) and fetches box art and info from ScreenScraper, with TheGamesDB as a fallback.",
  "f2.t": "CRT and LCD shaders",
  "f2.p": "99.7% of RetroArch's <code>.slangp</code> presets work: pick a tube, handheld or arcade look from the interface, with parameter tweaking.",
  "f3.t": "Bezels",
  "f3.p": "Download The Bezel Project pack or import your own. The bezel fits around the game image, like a real cabinet.",
  "f4.t": "Save states with thumbnails",
  "f4.p": "Several save slots, each with a picture of the moment. The game's own save (SRAM) is written automatically, even when you close the window.",
  "f5.t": "All from the controller",
  "f5.p": "Menus, library, settings and the pause menu are navigable with a gamepad, with button hints on screen. Per-controller mapping and save/load shortcuts.",
  "f6.t": "Cores and BIOS made simple",
  "f6.p": "A catalog of 120+ libretro cores, downloaded with one click. The BIOS screen shows, per system, what's missing and puts the right file in the right place.",
  "f7.t": "Themes and wallpaper",
  "f7.p": "Themes with an animated background in four colors, a wallpaper on the home screen and a high-contrast theme for those who need more legibility.",
  "f8.t": "Play time",
  "f8.p": "ReEmu tracks how long you've played each title and shows it on the game page. A profile with avatar, quick search and highlights on the home screen.",
  "f9.t": "Automatic updates",
  "f9.p": "When a new version comes out, a notice appears and the sidebar bell shows a summary of the changes. One click and ReEmu updates and restarts.",
  "ease.kicker": "Ease of use",
  "ease.title": "From download to first game in three steps",
  "s1.t": "Install",
  "s1.p": "Download the Windows installer or the Linux AppImage. No dependencies to hunt down.",
  "s2.t": "Point to your ROMs",
  "s2.p": "Pick the folder. ReEmu recognizes the systems, opens compressed files and fetches the box art by itself.",
  "s3.t": "Play",
  "s3.p": "Download the system's core from the catalog, pick the game and press A. Your progress is saved.",
  "cmp.title": "What ReEmu does for you",
  "cmp.task": "Task",
  "cmp.manual": "Setting it all up by hand",
  "cmp.us": "With ReEmu",
  "cmp.r1": "Emulators",
  "cmp.r1a": "Find, download and configure one for each system",
  "cmp.r1b": "Catalog of libretro cores, installed with one click",
  "cmp.r2": "BIOS",
  "cmp.r2a": "Figure out names, versions and folders for each file",
  "cmp.r2b": "Per-system list of what's missing, with guided import",
  "cmp.r3": "Box art and info",
  "cmp.r3a": "Search for images and rename them one by one",
  "cmp.r3b": "Automatic lookup on ScreenScraper and TheGamesDB",
  "cmp.r4": "Retro look",
  "cmp.r4a": "Edit presets and shader paths in files",
  "cmp.r4b": "Presets picked and tuned from the interface",
  "cmp.r5": "Navigation",
  "cmp.r5a": "Keyboard and mouse to configure",
  "cmp.r5b": "All from the controller, on the couch",
  "cmp.r6": "Updates",
  "cmp.r6a": "Go back to the website and reinstall",
  "cmp.r6b": "In-app notice and one-click update",
  "tech.title": "Modern technology under the hood",
  "t1.t": "Rust core",
  "t1.p": "Fast and memory-safe, from loading cores to audio.",
  "t2.t": "Core in a separate process",
  "t2.p": "If a core crashes, the interface stays up and the rest of the app doesn't go down with it.",
  "t3.t": "Vulkan and OpenGL",
  "t3.p": "Hardware rendering for 3D cores, with a GPU shader pipeline.",
  "t4.t": "Crackle-free audio",
  "t4.p": "Dynamic rate control (DRC) keeps sound and picture in sync.",
  "t5.t": "Fluent 2 interface",
  "t5.p": "React with Microsoft's design system, in the spirit of the Xbox console mode.",
  "t6.t": "Passwords in the system keychain",
  "t6.p": "ScreenScraper and TheGamesDB credentials stored by the operating system itself.",
  "screens.kicker": "Screens",
  "screens.title": "See ReEmu in action",
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
  "sys.kicker": "Systems",
  "sys.title": "From Atari to PlayStation",
  "sys.aria": "Some of the supported systems",
  "sys.more": "and more…",
  "sys.fine": "ReEmu does not include games or BIOS. Use copies of your own games.",
  "sup.kicker": "Support",
  "sup.title": "Help ReEmu keep evolving",
  "sup.lead":
    "ReEmu is free, ad-free and open source. Donations pay for development time, testing on real hardware (Windows, Linux, controllers, graphics cards) and hosting. Any amount helps.",
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
  "dl.kicker": "Download",
  "dl.title": "All versions",
  "foot.top": "ReEmu — back to top",
  "foot.by": "developed by Luis Julio",
  "foot.code": "Code on GitHub",
  "foot.releases": "Releases",
  "foot.issue": "Report a problem",
  "foot.legal":
    "MIT license. libretro cores have their own licenses and are downloaded by the app. ReEmu does not include games or BIOS. Site interface built with Fluent UI 2.",
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
};

const ES = {
  "meta.description":
    "ReEmu: frontend de emulación con cores libretro para Linux y Windows. Biblioteca con carátulas, shaders CRT, marcos, save states y navegación completa con el mando.",
  "meta.og": "Tus juegos clásicos con aspecto de consola. Gratis y de código abierto, para Linux y Windows.",
  skip: "Saltar al contenido",
  "nav.aria": "Secciones",
  "nav.features": "Funciones",
  "nav.ease": "Facilidad",
  "nav.screens": "Pantallas",
  "nav.support": "Apoya",
  "nav.download": "Descargar",
  "nav.home": "ReEmu — inicio",
  "hero.badge": "Frontend de emulación · Linux y Windows · Gratis",
  "hero.title": 'Tus juegos clásicos, con aspecto de <span class="grad">consola</span>.',
  "hero.lead":
    'ReEmu reúne tu colección en una biblioteca bonita, con carátulas, y lo ejecuta todo con los cores <fluent-link inline href="https://www.libretro.com/">libretro</fluent-link>, los mismos de RetroArch. Shaders CRT, marcos, save states con miniatura y navegación completa con el mando, sin abrir un archivo de configuración.',
  "hero.loading": "Buscando la versión más reciente…",
  "hero.note": "Código abierto (MIT). Una vez instalado, ReEmu se actualiza solo.",
  "stats.cores": "cores en el catálogo",
  "stats.systems": "sistemas reconocidos",
  "stats.shaders": "de los presets de shader",
  "stats.pad": "mando para todo",
  "features.kicker": "Funciones",
  "features.title": "Todo lo que un frontend moderno necesita",
  "features.lead":
    "Pensado para la TV del salón y para el PC de escritorio: interfaz estilo consola, rápida y con los detalles que marcan la diferencia en el día a día.",
  "f1.t": "Biblioteca con carátulas",
  "f1.p": "Indica la carpeta de las ROMs y ReEmu identifica cada juego (también por hash) y busca carátulas e información en ScreenScraper, con TheGamesDB de respaldo.",
  "f2.t": "Shaders CRT y LCD",
  "f2.p": "El 99,7% de los presets <code>.slangp</code> de RetroArch funcionan: elige el aspecto de tubo, de portátil o de arcade desde la interfaz, con ajuste de parámetros.",
  "f3.t": "Marcos (bezels)",
  "f3.p": "Descarga el paquete de The Bezel Project o importa los tuyos. El marco encaja alrededor de la imagen del juego, como en una recreativa de verdad.",
  "f4.t": "Save states con miniatura",
  "f4.p": "Varios espacios de guardado, cada uno con la imagen del momento. El guardado interno del juego (SRAM) se graba solo, incluso al cerrar la ventana.",
  "f5.t": "Todo con el mando",
  "f5.p": "Menús, biblioteca, ajustes y el menú de pausa navegables con el gamepad, con ayudas de botones en pantalla. Asignación por mando y atajos de guardar/cargar.",
  "f6.t": "Cores y BIOS sin misterio",
  "f6.p": "Catálogo con más de 120 cores libretro, descargados con un clic. La pantalla de BIOS muestra, por sistema, lo que falta e importa el archivo correcto al lugar correcto.",
  "f7.t": "Temas y fondo de pantalla",
  "f7.p": "Temas con fondo animado en cuatro colores, fondo de pantalla en el inicio y un tema de alto contraste para quien necesita más legibilidad.",
  "f8.t": "Tiempo de juego",
  "f8.p": "ReEmu cuenta cuánto jugaste a cada título y lo muestra en la página del juego. Perfil con avatar, búsqueda rápida y destacados en el inicio.",
  "f9.t": "Actualización automática",
  "f9.p": "Cuando sale una versión nueva, aparece un aviso y la campana de la barra lateral muestra el resumen de cambios. Un clic y ReEmu se actualiza y se reinicia.",
  "ease.kicker": "Facilidad",
  "ease.title": "De la descarga al primer juego en tres pasos",
  "s1.t": "Instala",
  "s1.p": "Descarga el instalador para Windows o el AppImage para Linux. Sin dependencias que buscar.",
  "s2.t": "Indica tus ROMs",
  "s2.p": "Elige la carpeta. ReEmu reconoce los sistemas, abre archivos comprimidos y busca las carátulas solo.",
  "s3.t": "Juega",
  "s3.p": "Descarga el core del sistema desde el catálogo, elige el juego y pulsa A. Tu progreso queda guardado.",
  "cmp.title": "Lo que ReEmu hace por ti",
  "cmp.task": "Tarea",
  "cmp.manual": "Montándolo todo a mano",
  "cmp.us": "Con ReEmu",
  "cmp.r1": "Emuladores",
  "cmp.r1a": "Encontrar, descargar y configurar uno para cada sistema",
  "cmp.r1b": "Catálogo de cores libretro, instalados con un clic",
  "cmp.r2": "BIOS",
  "cmp.r2a": "Averiguar nombres, versiones y carpetas de cada archivo",
  "cmp.r2b": "Lista por sistema de lo que falta, con importación guiada",
  "cmp.r3": "Carátulas e información",
  "cmp.r3a": "Buscar imágenes y renombrarlas una por una",
  "cmp.r3b": "Búsqueda automática en ScreenScraper y TheGamesDB",
  "cmp.r4": "Aspecto retro",
  "cmp.r4a": "Editar presets y rutas de shader en archivos",
  "cmp.r4b": "Presets elegidos y ajustados desde la interfaz",
  "cmp.r5": "Navegación",
  "cmp.r5a": "Teclado y ratón para configurar",
  "cmp.r5b": "Todo con el mando, desde el sofá",
  "cmp.r6": "Actualizaciones",
  "cmp.r6a": "Volver al sitio y reinstalar",
  "cmp.r6b": "Aviso en la app y actualización con un clic",
  "tech.title": "Tecnología moderna por dentro",
  "t1.t": "Núcleo en Rust",
  "t1.p": "Rápido y con gestión de memoria segura, desde la carga de los cores hasta el audio.",
  "t2.t": "Core en un proceso aparte",
  "t2.p": "Si un core falla, la interfaz sigue en pie y el resto de la app no se cae con él.",
  "t3.t": "Vulkan y OpenGL",
  "t3.p": "Renderizado por hardware para los cores 3D, con pipeline de shaders en la GPU.",
  "t4.t": "Audio sin chasquidos",
  "t4.p": "El control dinámico de frecuencia (DRC) mantiene sonido e imagen sincronizados.",
  "t5.t": "Interfaz Fluent 2",
  "t5.p": "React con el sistema de diseño de Microsoft, en el espíritu del modo consola de Xbox.",
  "t6.t": "Contraseñas en el llavero del sistema",
  "t6.p": "Credenciales de ScreenScraper y TheGamesDB guardadas por el propio sistema operativo.",
  "screens.kicker": "Pantallas",
  "screens.title": "Mira ReEmu en acción",
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
  "sys.kicker": "Sistemas",
  "sys.title": "Del Atari a PlayStation",
  "sys.aria": "Algunos de los sistemas compatibles",
  "sys.more": "y más…",
  "sys.fine": "ReEmu no incluye juegos ni BIOS. Usa copias de tus propios juegos.",
  "sup.kicker": "Apoya",
  "sup.title": "Ayuda a que ReEmu siga evolucionando",
  "sup.lead":
    "ReEmu es gratuito, sin anuncios y de código abierto. Las donaciones pagan el tiempo de desarrollo, las pruebas en hardware real (Windows, Linux, mandos, tarjetas gráficas) y el alojamiento. Cualquier cantidad ayuda.",
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
  "dl.kicker": "Descarga",
  "dl.title": "Todas las versiones",
  "foot.top": "ReEmu — volver arriba",
  "foot.by": "desarrollado por Luis Julio",
  "foot.code": "Código en GitHub",
  "foot.releases": "Versiones",
  "foot.issue": "Informar de un problema",
  "foot.legal":
    "Licencia MIT. Los cores libretro tienen sus propias licencias y los descarga la app. ReEmu no incluye juegos ni BIOS. Interfaz del sitio con Fluent UI 2.",
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
