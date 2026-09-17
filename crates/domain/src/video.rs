//! Configuração de exibição de vídeo (backlog: integer scaling).

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VideoConfig {
    /// Trava o retângulo do jogo num múltiplo INTEIRO da resolução nativa do
    /// core (em vez do letterbox fracionário livre) — evita o borrão de
    /// escala não-inteira em pixel art. Com moldura/bezel ativa, o fator usa
    /// `ceil(altura_da_moldura / altura_nativa)` — arredonda pra CIMA, não
    /// pra baixo — pra eliminar a barra preta acima/abaixo da tela; o
    /// excesso resultante (jogo maior que o vidro da moldura) é cortado pelo
    /// próprio clipping da GPU, não redimensionado. A moldura em si (foto,
    /// não pixel art) nunca é redimensionada. `false` por padrão
    /// (`bool::default()`).
    pub integer_scaling: bool,
}

/// Persistência da configuração de vídeo. Linha única por instalação (sem
/// multi-perfil), mesmo padrão de `AudioConfigRepository`.
#[async_trait]
pub trait VideoConfigRepository: Send + Sync {
    async fn get(&self) -> Result<VideoConfig, RepoError>;
    async fn update(&self, config: &VideoConfig) -> Result<(), RepoError>;
}
