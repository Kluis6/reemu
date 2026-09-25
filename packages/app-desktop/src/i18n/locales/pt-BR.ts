// Português (Brasil) — idioma de origem: as CHAVES e o formato saem daqui
// (`Messages` em ../types.ts). en/es têm que ter exatamente as mesmas chaves
// (o TypeScript e o teste `locales.test.ts` cobram).
const ptBR = {
  nav: {
    home: 'Início',
    library: 'Meus jogos',
    settings: 'Configurações',
    achievements: 'Minhas conquistas',
    network: 'Minha rede',
  },
  hints: {
    select: 'Selecionar',
    search: 'Buscar',
    options: 'Opções',
    back: 'Voltar',
  },
  shell: {
    profile: 'Perfil',
    status: 'Status',
    online: 'Online',
    away: 'Ausente',
    playing: 'Jogando',
    quit: 'Sair',
    shutdown: 'Encerrar',
    backTooltip: 'Voltar para a tela anterior',
    searchPlaceholder: 'Buscar na biblioteca…',
    fullscreen: 'Tela cheia',
    exitFullscreen: 'Sair da tela cheia',
    pendingMetadata: '{{label}} — metadados para revisar: {{count}}',
  },
  settings: {
    title: 'Configurações',
    tabs: {
      profile: 'Perfil',
      appearance: 'Aparência',
      library: 'Gerenciar biblioteca',
      audio: 'Áudio',
      video: 'Vídeo',
      metadata: 'Metadados',
      hotkeys: 'Atalhos',
      controllers: 'Controles',
      cores: 'Cores',
      bios: 'BIOS',
    },
  },
  language: {
    title: 'Idioma',
    description: 'Idioma da interface. Automático segue o idioma do sistema.',
    auto: 'Automático (sistema)',
    'pt-BR': 'Português (Brasil)',
    en: 'English',
    es: 'Español',
  },
  appearance: {
    uiScale: {
      title: 'Tamanho da interface',
      description:
        'Aumenta textos, botões e capas por igual. Padrão pro monitor, Grande pra notebook de longe ou TV pequena, Maior pra TV vista do sofá.',
      default: 'Padrão',
      large: 'Grande',
      larger: 'Maior',
    },
    theme: {
      title: 'Tema de cor',
      description: 'Muda a cor de destaque e do fundo do app.',
      card: 'Tema {{name}}',
      lightMode: '{{name}}: modo claro',
      hue: 'Matiz do tema personalizado',
      names: {
        xboxGreen: 'Verde Xbox',
        xboxClassic: 'Xbox Clássico',
        psBlue: 'Azul PlayStation',
        psClassic: 'PlayStation Clássico',
        alva: 'Alva',
        snes: 'Super Nintendo',
        highContrast: 'Alto contraste',
        custom: 'Personalizado',
      },
    },
    wallpaper: {
      title: 'Papel de parede',
      description: 'Uma imagem de fundo pra tela inicial. Opcional.',
      choose: 'Escolher imagem…',
      change: 'Trocar imagem…',
      remove: 'Remover',
      pickTitle: 'Escolha um papel de parede',
    },
  },
}

export default ptBR
