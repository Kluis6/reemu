//! Erro do catálogo. O app converte em `AppError` (`app_error.rs`), com o
//! código que a interface mostra.

/// Falha ao baixar, instalar, voltar ou remover um core, ou ao consultar
/// atualizações.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// Rede ou resposta HTTP de erro; `context` diz o que se baixava.
    #[error("{context}: {source}")]
    Http {
        context: &'static str,
        #[source]
        source: reqwest::Error,
    },
    /// O zip baixado não serve.
    #[error("{0}")]
    Corrupt(String),
    /// Não há o que se pediu (ex.: versão anterior).
    #[error("{0}")]
    NotFound(String),
    /// O core não está instalado.
    #[error("{0}")]
    NoCore(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl CatalogError {
    pub fn http(context: &'static str, source: reqwest::Error) -> Self {
        Self::Http { context, source }
    }
}

impl From<String> for CatalogError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}
