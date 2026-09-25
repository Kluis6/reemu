import type { Messages } from '../types'

const es: Messages = {
  nav: {
    home: 'Inicio',
    library: 'Mis juegos',
    settings: 'Configuración',
    achievements: 'Mis logros',
    network: 'Mi red',
  },
  hints: {
    select: 'Seleccionar',
    search: 'Buscar',
    options: 'Opciones',
    back: 'Volver',
  },
  shell: {
    profile: 'Perfil',
    status: 'Estado',
    online: 'En línea',
    away: 'Ausente',
    playing: 'Jugando',
    quit: 'Salir',
    shutdown: 'Apagar',
    backTooltip: 'Volver a la pantalla anterior',
    searchPlaceholder: 'Buscar en la biblioteca…',
    fullscreen: 'Pantalla completa',
    exitFullscreen: 'Salir de pantalla completa',
    pendingMetadata: '{{label}} — metadatos para revisar: {{count}}',
  },
  settings: {
    title: 'Configuración',
    tabs: {
      profile: 'Perfil',
      appearance: 'Apariencia',
      library: 'Administrar biblioteca',
      audio: 'Audio',
      video: 'Video',
      metadata: 'Metadatos',
      hotkeys: 'Atajos',
      controllers: 'Controles',
      cores: 'Núcleos',
      bios: 'BIOS',
    },
  },
  language: {
    title: 'Idioma',
    description: 'Idioma de la interfaz. Automático sigue el idioma del sistema.',
    auto: 'Automático (sistema)',
    'pt-BR': 'Português (Brasil)',
    en: 'English',
    es: 'Español',
  },
  appearance: {
    uiScale: {
      title: 'Tamaño de la interfaz',
      description:
        'Agranda textos, botones y portadas por igual. Predeterminado para el monitor, Grande para un portátil de lejos o una TV pequeña, Más grande para una TV vista desde el sofá.',
      default: 'Predeterminado',
      large: 'Grande',
      larger: 'Más grande',
    },
    theme: {
      title: 'Tema de color',
      description: 'Cambia el color de acento y el fondo de la app.',
      card: 'Tema {{name}}',
      lightMode: '{{name}}: modo claro',
      hue: 'Tono del tema personalizado',
      names: {
        xboxGreen: 'Verde Xbox',
        xboxClassic: 'Xbox Clásico',
        psBlue: 'Azul PlayStation',
        psClassic: 'PlayStation Clásico',
        alva: 'Alva',
        snes: 'Super Nintendo',
        highContrast: 'Alto contraste',
        custom: 'Personalizado',
      },
    },
    wallpaper: {
      title: 'Fondo de pantalla',
      description: 'Una imagen de fondo para la pantalla de inicio. Opcional.',
      choose: 'Elegir imagen…',
      change: 'Cambiar imagen…',
      remove: 'Quitar',
      pickTitle: 'Elige un fondo de pantalla',
    },
  },
}

export default es
