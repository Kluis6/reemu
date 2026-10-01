# Dependências do site (sem CDN)

- `fluent-web-components-3.1.3.min.js`: bundle `web-components-all.min.js` do
  pacote [`@fluentui/web-components`](https://www.npmjs.com/package/@fluentui/web-components)
  3.1.3 (Fluent UI 2 para web, licença MIT, © Microsoft). Registra todos os
  componentes `fluent-*` e exporta `setTheme`.
- `reemu-theme.js`: tokens do tema escuro (ver o comentário no arquivo).
- `motion.js` (não versionado): bundle UMD do [Motion](https://motion.dev)
  (licença MIT), copiado de `node_modules` por `scripts/vendor-motion.mjs`
  no `build`/`dev`. A versão fica travada em `site/package.json`.

O CSS é Tailwind v4: classes no HTML/JS, tokens em `src/input.css`, saída
em `tailwind.css` (não versionado). Para ver o site localmente:
`pnpm --filter reemu-site dev` e sirva a pasta `site/` (ex.:
`python3 -m http.server -d site`).

Para atualizar o Fluent: `npm i @fluentui/web-components@<versão>`, copie
`dist/web-components-all.min.js` para cá com a versão no nome e ajuste o
`import` em `ui.js`.
