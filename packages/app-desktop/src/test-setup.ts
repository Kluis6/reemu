import i18n from './i18n'

// Os testes esperam os textos em pt-BR (a origem das chaves). Sem isto o
// idioma segue o `navigator.language` do Node, que no CI é en-US.
await i18n.changeLanguage('pt-BR')
