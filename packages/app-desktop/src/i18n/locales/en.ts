import type { Messages } from '../types'

const en: Messages = {
  nav: {
    home: 'Home',
    library: 'My games',
    settings: 'Settings',
    achievements: 'My achievements',
    network: 'My network',
  },
  hints: {
    select: 'Select',
    search: 'Search',
    options: 'Options',
    back: 'Back',
  },
  shell: {
    profile: 'Profile',
    status: 'Status',
    online: 'Online',
    away: 'Away',
    playing: 'Playing',
    quit: 'Quit',
    shutdown: 'Shut down',
    backTooltip: 'Go back to the previous screen',
    searchPlaceholder: 'Search the library…',
    fullscreen: 'Full screen',
    exitFullscreen: 'Exit full screen',
    pendingMetadata: '{{label}} — metadata to review: {{count}}',
  },
  settings: {
    title: 'Settings',
    tabs: {
      profile: 'Profile',
      appearance: 'Appearance',
      library: 'Manage library',
      audio: 'Audio',
      video: 'Video',
      metadata: 'Metadata',
      hotkeys: 'Hotkeys',
      controllers: 'Controllers',
      cores: 'Cores',
      bios: 'BIOS',
    },
  },
  language: {
    title: 'Language',
    description: 'Interface language. Automatic follows the system language.',
    auto: 'Automatic (system)',
    'pt-BR': 'Português (Brasil)',
    en: 'English',
    es: 'Español',
  },
  appearance: {
    uiScale: {
      title: 'Interface size',
      description:
        'Scales text, buttons and covers evenly. Default for a monitor, Large for a laptop from afar or a small TV, Larger for a TV seen from the couch.',
      default: 'Default',
      large: 'Large',
      larger: 'Larger',
    },
    theme: {
      title: 'Color theme',
      description: 'Changes the accent and background colors of the app.',
      card: '{{name}} theme',
      lightMode: '{{name}}: light mode',
      hue: 'Custom theme hue',
      names: {
        xboxGreen: 'Xbox Green',
        xboxClassic: 'Classic Xbox',
        psBlue: 'PlayStation Blue',
        psClassic: 'Classic PlayStation',
        alva: 'Alva',
        snes: 'Super Nintendo',
        highContrast: 'High contrast',
        custom: 'Custom',
      },
    },
    wallpaper: {
      title: 'Wallpaper',
      description: 'A background image for the home screen. Optional.',
      choose: 'Choose image…',
      change: 'Change image…',
      remove: 'Remove',
      pickTitle: 'Choose a wallpaper',
    },
  },
}

export default en
