//! Porta de carregamento de cores libretro.
//!
//! Implementações:
//! - desktop: `libloading` (dlopen/LoadLibrary), sem restrição de origem
//! - mobile: cores empacotados na APK (targetSdkVersion baixo, ver decisão
//!   sobre distribuição de cores no Android — Abordagem A, download dinâmico
//!   igual ao desktop, viável por não distribuir via Google Play)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RenderBackend {
    Software,
    OpenGl,
    Vulkan,
}

/// Metadata técnica de um core, detectada em runtime no primeiro load
/// (decisão: sem curadoria manual estática para esses campos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreRenderRequirements {
    pub render_backend: RenderBackend,
    pub gl_version_min: Option<String>,
    pub gl_profile: Option<String>,
    pub needs_depth_stencil: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreId(pub String);

#[derive(Debug, Error)]
pub enum CoreLoadError {
    #[error("core não encontrado: {0}")]
    NotFound(String),
    #[error("falha ao carregar core: {0}")]
    LoadFailed(String),
    #[error("core incompatível com a plataforma atual: {0}")]
    IncompatiblePlatform(String),
}

/// Handle opaco pra uma instância de core carregada. O domínio não sabe
/// o que tem dentro — só o adapter concreto (desktop/mobile) conhece o tipo
/// real por trás disso.
pub trait LoadedCoreHandle: Send {}

#[async_trait]
pub trait CoreLoader: Send + Sync {
    async fn load(&self, core_id: &CoreId, rom_path: &str)
        -> Result<Box<dyn LoadedCoreHandle>, CoreLoadError>;

    async fn unload(&self, handle: Box<dyn LoadedCoreHandle>) -> Result<(), CoreLoadError>;

    /// Requisitos de renderização descobertos na primeira vez que esse core
    /// foi carregado (cache; None se ainda não foi carregado nenhuma vez).
    fn known_render_requirements(&self, core_id: &CoreId) -> Option<CoreRenderRequirements>;
}
