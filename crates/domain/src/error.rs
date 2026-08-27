//! Erro comum das portas de repositório (implementadas pelo crate `db`).
//!
//! O domínio não conhece `sqlx`/`rusqlite` — os adapters convertem o erro
//! concreto de armazenamento para uma destas variantes.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    /// Falha do backend de armazenamento (conexão, SQL, constraint, etc).
    #[error("erro de armazenamento: {0}")]
    Backend(String),

    /// A linha existe mas está em um formato que o domínio não consegue
    /// interpretar (ex: enum fora do conjunto esperado, JSON inválido).
    /// Indica corrupção ou incompatibilidade de schema, não "não encontrado".
    #[error("dado inconsistente no armazenamento: {0}")]
    Corrupt(String),
}
