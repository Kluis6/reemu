//! Configuração de exibição de vídeo (backlog: integer scaling).

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoConfig {
    /// Trava a ALTURA do retângulo do jogo num múltiplo INTEIRO da altura
    /// nativa do core (em vez do letterbox fracionário livre) — evita o
    /// borrão vertical de escala não-inteira em pixel art. Com moldura/bezel
    /// ativa, o fator usa `round(altura_da_moldura / altura_nativa)` — o
    /// mais próximo, não sempre pra cima — pra minimizar o erro em pixels:
    /// barra preta fina quando o fator de baixo é mais perto, corte pequeno
    /// do jogo quando o de cima é mais perto. Sempre arredondar pra cima
    /// (testado antes) causava cortes grandes de HUD/texto quando a conta
    /// caía perto do meio do caminho entre dois fatores. O excesso que ainda
    /// passar da altura do canvas é cortado pelo clipping normal da GPU, não
    /// redimensionado.
    /// A LARGURA é derivada da proporção de exibição (`aspect_ratio`, já
    /// corrigida de pixel não-quadrado), não de `native_width × fator` —
    /// necessário porque cores com PAR ≠ 1 (PS1, N64, Mega Drive…) têm
    /// resolução nativa crua desproporcional à exibição real; usar o fator
    /// puro nos dois eixos deixava a imagem mais larga que o vidro e
    /// cortava texto/HUD nas bordas. A moldura em si (foto, não pixel art)
    /// nunca é redimensionada. `false` por padrão.
    pub integer_scaling: bool,
    /// Saída HDR na janela do jogo (superfície FP16 scRGB) quando o monitor e
    /// o sistema permitem; senão segue SDR. `false` por padrão.
    #[serde(default)]
    pub hdr: bool,
    /// Branco SDR em nits — o brilho do branco do jogo no HDR. `0` =
    /// automático (o "brilho do conteúdo SDR" configurado no Windows).
    #[serde(default)]
    pub hdr_paper_white_nits: u32,
    /// Realce de brilhos: expande os tons claros do jogo (SDR) até o pico
    /// (`hdr_peak_nits`). `true` por padrão.
    #[serde(default = "yes")]
    pub hdr_highlights: bool,
    /// Pico do realce em nits. `0` = automático (o brilho que o monitor
    /// sustenta em tela inteira).
    #[serde(default)]
    pub hdr_peak_nits: u32,
}

fn yes() -> bool {
    true
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            integer_scaling: false,
            hdr: false,
            hdr_paper_white_nits: 0,
            hdr_highlights: true,
            hdr_peak_nits: 0,
        }
    }
}

/// Persistência da configuração de vídeo. Linha única por instalação (sem
/// multi-perfil), mesmo padrão de `AudioConfigRepository`.
#[async_trait]
pub trait VideoConfigRepository: Send + Sync {
    async fn get(&self) -> Result<VideoConfig, RepoError>;
    async fn update(&self, config: &VideoConfig) -> Result<(), RepoError>;
}
