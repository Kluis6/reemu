//! Saída de áudio. Implementações: `cpal` (desktop), Oboe via JNI (mobile).
//! Dynamic Rate Control decidido como abordagem de sincronia áudio/vídeo
//! (não resample fixo) — evita drift acumulado em sessões longas.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub output_device_id: Option<String>,
    pub output_device_name: Option<String>,
    pub rate_control_enabled: bool,
    /// Margem de ajuste, ex: 0.005 = ±0.5%.
    pub rate_control_delta: f32,
    pub sample_rate_preference: Option<u32>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device_id: None,
            output_device_name: None,
            rate_control_enabled: true,
            rate_control_delta: 0.005,
            sample_rate_preference: None,
        }
    }
}

/// Persistência da configuração de áudio. É sempre uma linha única por
/// instalação (sem multi-perfil) — daí `get`/`update`, nunca `insert`/`list`.
#[async_trait]
pub trait AudioConfigRepository: Send + Sync {
    async fn get(&self) -> Result<AudioConfig, RepoError>;
    async fn update(&self, config: &AudioConfig) -> Result<(), RepoError>;
}

/// Saída de áudio. NÃO é `Send`/`Sync`: a stream do `cpal` é confinada à
/// thread que dirige o core (`emu-session`), que é quem chama estes métodos.
/// O adapter é construído nessa thread (via factory), nunca movido.
pub trait AudioSink {
    /// Envia áudio do core na taxa nativa dele; a implementação faz o
    /// resample dinâmico baseado no nível de preenchimento do buffer
    /// (Dynamic Rate Control).
    fn push_samples(&mut self, samples: &[i16], core_sample_rate: u32);

    fn pause(&mut self);
    fn resume(&mut self);

    /// Troca de dispositivo em runtime. Se o device_id salvo não for
    /// encontrado, o adapter deve cair pro padrão do sistema e notificar
    /// via toast (decisão: Abordagem B, fallback + aviso não-bloqueante).
    fn set_output_device(&mut self, device_id: Option<&str>) -> Result<(), String>;
}
