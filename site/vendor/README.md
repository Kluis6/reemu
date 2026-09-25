# Dependências do site (sem CDN)

- `fluent-web-components-3.1.3.min.js`: bundle `web-components-all.min.js` do
  pacote [`@fluentui/web-components`](https://www.npmjs.com/package/@fluentui/web-components)
  3.1.3 (Fluent UI 2 para web, licença MIT, © Microsoft). Registra todos os
  componentes `fluent-*` e exporta `setTheme`.
- `reemu-theme.js`: tokens do tema escuro (ver o comentário no arquivo).

Para atualizar o Fluent: `npm i @fluentui/web-components@<versão>`, copie
`dist/web-components-all.min.js` para cá com a versão no nome e ajuste o
`import` em `ui.js`.
