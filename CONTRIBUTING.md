# Como contribuir com o ReEmu

Toda mudança entra na `main` por pull request e passa pela revisão do dono do
projeto. Não é possível enviar direto para a `main`.

## Fluxo

1. Crie uma branch a partir da `main` atualizada. Quem não é colaborador trabalha
   em um fork.
2. Faça a mudança e abra um pull request para a `main`, preenchendo o modelo.
3. Espere o CI passar. Em pull request de fork, o dono do projeto precisa liberar
   a execução.
4. O dono do projeto revisa. Um envio novo depois da aprovação pede nova revisão.
5. Com a aprovação, o CI verde e as conversas resolvidas, o dono do projeto faz o
   merge em squash.

## Antes de abrir o pull request

No frontend (`packages/app-desktop`):

```sh
npx tsc -b
npx oxlint src
npx vitest run
npx vite build
```

No Rust:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Regras do projeto

As regras estão em [`CLAUDE.md`](CLAUDE.md). As principais:

- Decisões sobre OpenGL, Vulkan, wgpu ou shaders seguem a documentação oficial, e
  o pull request cita qual foi usada.
- Texto da interface passa por `t('chave')`, em pt-BR, en e es.
- Não envie chaves, senhas nem tokens, e não copie código com licença incompatível.

## Versões publicadas

Só o dono do projeto cria tags `v*`. O build de uma versão só roda depois da
aprovação dele no ambiente `release` do GitHub, e é esse ambiente que guarda a
chave de assinatura.
