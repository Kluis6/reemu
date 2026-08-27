# 01 — Domain + DB: Implementar Repositórios

## Objetivo desta etapa

Implementar os repositórios concretos em `crates/db` que satisfazem as
traits definidas em `crates/domain` (ex: `ShaderChainResolver`,
`DecorationResolver`, `CoreOptionsStore`), usando `sqlx` sobre o schema
já definido em `crates/db/migrations/0001_init.sql`.

## Estado atual (2026-08-27 — `done`)

Implementado em `crates/db`: `ShaderChainRepo`, `DecorationRepo`,
`CoreOptionsRepo`, `AudioConfigRepo`, `InstalledCoresRepo`, `RomsRepo`,
`SaveStateRepo` + `pool.rs`, `cascade.rs`, `convert.rs`. 18 testes de
integração com SQLite in-memory.

**Mudança de contrato**: as traits de DB em `domain` agora são `async`
(`#[async_trait]`) — o doc abaixo foi escrito assumindo sync. Erro comum:
`domain::error::RepoError` (`Backend` / `Corrupt`), retornado como
`Result<Option<T>, RepoError>` (o "não encontrado" é `Ok(None)`, não erro).

Save state: port dividido em `SaveStateManager` (alto nível, dispara
`retro_serialize`, fica no core-loader — etapa 08) e `SaveStateRepository`
(só metadata, no `db`). Ambos os models de save ganharam `id`.

**Não implementado (de propósito, entra depois)**: métodos de *escrita* de
assignment (criar/editar preset de shader/decoração por rom/sistema) —
entram com a UI (etapa 04/07). O `db` hoje só resolve (lê) a cascata.

## Decisões relevantes

- Nenhuma tabela tem `user_profile_id`.
- Resolução em cascata (`shader_chain_assignments`,
  `decoration_assignments`): a query deve buscar primeiro `scope='rom'`
  com o `rom_id` exato; se não achar, `scope='system'` com o `system_id`;
  se não achar, `scope='default'`. Implemente isso como uma função de
  resolução única e reaproveitada pelas duas tabelas (a lógica é idêntica),
  não duplicada.
- `installed_cores.render_backend` e campos relacionados são escritos em
  runtime (primeiro load do core), não pré-populados — o repositório deve
  ter um método de `upsert` para isso, não só `insert`.
- `audio_config` é sempre uma única linha (`id = 1`, já tem `CHECK`
  no schema) — o repositório deve expor `get()`/`update()`, nunca
  `insert()`/`list()`.

## Setup

```bash
cd crates/db
cargo add sqlx --features sqlite,runtime-tokio,migrate
cargo add tokio --features rt,macros
```

## Estrutura sugerida

```
crates/db/src/
  lib.rs
  pool.rs                    -- criação do SqlitePool, roda migrations
  repositories/
    mod.rs
    shader_chain_repo.rs      -- implementa ShaderChainResolver
    decoration_repo.rs         -- implementa DecorationResolver
    core_options_repo.rs        -- implementa CoreOptionsStore
    audio_config_repo.rs
    installed_cores_repo.rs
    roms_repo.rs
    save_state_repo.rs         -- implementa SaveStateManager (metadata só;
                                   escrita do arquivo de state fica no
                                   core-loader, não aqui)
```

## Critério de pronto

- Cada repositório tem teste de integração usando SQLite in-memory
  (`sqlite::memory:`) rodando a migration antes de cada teste
- Resolução em cascata testada com os três casos (rom, system, default)
  e o caso de "nenhum encontrado" (retorna `None`, não erro)
- Nenhum repositório vaza tipo do `sqlx` (`sqlx::Row`, etc.) pra fora do
  crate `db` — a interface pública retorna só os tipos de `domain`
