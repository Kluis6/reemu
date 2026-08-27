//! Resolução em cascata `rom -> system -> default`, idêntica para
//! `shader_chain_assignments` e `decoration_assignments` (mesma coluna
//! `scope` e mesma forma travada por CHECK no schema). Uma função só,
//! reusada pelos dois repositórios.

use crate::pool::Db;
use domain::error::RepoError;
use sqlx::sqlite::SqliteRow;

/// Converte um erro do sqlx em `RepoError::Backend`.
pub(crate) fn be(e: sqlx::Error) -> RepoError {
    RepoError::Backend(e.to_string())
}

/// `select` deve ser `"SELECT <colunas> FROM <tabela>"` sem `WHERE` — as
/// três cláusulas de escopo são anexadas aqui, na ordem da cascata.
/// Retorna `Ok(None)` quando nenhum escopo tem atribuição (não é erro).
pub(crate) async fn resolve_cascade<T>(
    db: &Db,
    select: &str,
    system_id: &str,
    rom_id: Option<&str>,
    map: impl Fn(&SqliteRow) -> Result<T, RepoError>,
) -> Result<Option<T>, RepoError> {
    if let Some(rid) = rom_id {
        let sql = format!("{select} WHERE scope = 'rom' AND rom_id = ?1 LIMIT 1");
        if let Some(row) = sqlx::query(&sql)
            .bind(rid)
            .fetch_optional(db)
            .await
            .map_err(be)?
        {
            return Ok(Some(map(&row)?));
        }
    }

    let sql = format!("{select} WHERE scope = 'system' AND system_id = ?1 LIMIT 1");
    if let Some(row) = sqlx::query(&sql)
        .bind(system_id)
        .fetch_optional(db)
        .await
        .map_err(be)?
    {
        return Ok(Some(map(&row)?));
    }

    let sql = format!("{select} WHERE scope = 'default' LIMIT 1");
    if let Some(row) = sqlx::query(&sql).fetch_optional(db).await.map_err(be)? {
        return Ok(Some(map(&row)?));
    }

    Ok(None)
}
