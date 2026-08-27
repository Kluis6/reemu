//! Crate `db`: implementações concretas dos repositórios (sqlx/rusqlite)
//! que satisfazem as portas definidas em `domain` (ex: ShaderChainResolver,
//! DecorationResolver, CoreOptionsStore) usando o schema em
//! `migrations/0001_init.sql`.
//!
//! Ainda não implementado — este crate hoje só carrega a migration.
//! Próximo passo natural: adicionar sqlx e implementar os primeiros
//! repositórios (sugestão: começar por `installed_cores` e `roms`,
//! que são a base de que tudo mais depende).

pub const INIT_MIGRATION: &str = include_str!("../migrations/0001_init.sql");
