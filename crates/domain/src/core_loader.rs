//! Porta de carregamento de cores libretro.
//!
//! Implementações:
//! - desktop: `libloading` (dlopen/LoadLibrary), sem restrição de origem
//! - mobile: cores empacotados na APK (targetSdkVersion baixo, ver decisão
//!   sobre distribuição de cores no Android — Abordagem A, download dinâmico
//!   igual ao desktop, viável por não distribuir via Google Play)

use crate::error::RepoError;
use crate::frame_source::FrameSource;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRenderRequirements {
    pub render_backend: RenderBackend,
    pub gl_version_min: Option<String>,
    pub gl_profile: Option<String>,
    pub needs_depth_stencil: bool,
}

/// Um core instalado localmente. `render_requirements` fica `None` até o
/// primeiro load detectar (decisão: runtime, sem curadoria).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledCore {
    pub core_id: String,
    pub version: String,
    /// Unix timestamp (segundos).
    pub installed_at: i64,
    pub render_requirements: Option<CoreRenderRequirements>,
}

/// Persistência do catálogo local de cores instalados (tabela
/// `installed_cores`). Não confundir com o catálogo remoto do buildbot
/// (etapa 10) nem com o loader em si (`CoreLoader`).
#[async_trait]
pub trait InstalledCoreRepository: Send + Sync {
    /// Registra ou atualiza a identidade do core (idempotente por `core_id`).
    /// Não mexe em `render_requirements` já persistidos.
    async fn register(&self, core: &InstalledCore) -> Result<(), RepoError>;
    async fn get(&self, core_id: &str) -> Result<Option<InstalledCore>, RepoError>;
    async fn list(&self) -> Result<Vec<InstalledCore>, RepoError>;
    /// Upsert dos requisitos de render detectados no primeiro load.
    async fn set_render_requirements(
        &self,
        core_id: &str,
        reqs: &CoreRenderRequirements,
    ) -> Result<(), RepoError>;
    async fn remove(&self, core_id: &str) -> Result<(), RepoError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreId(pub String);

/// Geometria da imagem do core (espelha `retro_game_geometry`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SystemGeometry {
    pub base_width: u32,
    pub base_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    /// `<= 0` no libretro significa "use base_width/base_height".
    pub aspect_ratio: f32,
}

/// Timing do core (espelha `retro_system_timing`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SystemTiming {
    pub fps: f64,
    pub sample_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SystemAvInfo {
    pub geometry: SystemGeometry,
    pub timing: SystemTiming,
}

#[derive(Debug, Error)]
pub enum CoreLoadError {
    #[error("core não encontrado: {0}")]
    NotFound(String),
    #[error("falha ao carregar core: {0}")]
    LoadFailed(String),
    #[error("core incompatível com a plataforma atual: {0}")]
    IncompatiblePlatform(String),
    /// O core exige HW render (GL/Vulkan) e essa negociação ainda não está
    /// implementada (etapa 02 passo 4 / etapa 12). Os requisitos detectados
    /// já foram persistidos — ver `CoreRenderRequirements`.
    #[error("core exige HW render ainda não suportado: {0}")]
    HwRenderUnsupported(String),
}

/// Uma instância de core carregada e pronta pra rodar. Substitui o antigo
/// marker `LoadedCoreHandle`: agora é a própria `FrameSource` (cada
/// `next_frame` roda um `retro_run`), mais a metadata técnica pós-load.
/// A struct concreta (`DesktopCore` etc.) vive no adapter.
pub trait LoadedCore: FrameSource {
    fn system_av_info(&self) -> SystemAvInfo;
    fn render_requirements(&self) -> CoreRenderRequirements;
}

#[async_trait]
pub trait CoreLoader: Send + Sync {
    async fn load(
        &self,
        core_id: &CoreId,
        rom_path: &str,
    ) -> Result<Box<dyn LoadedCore>, CoreLoadError>;

    async fn unload(&self, core: Box<dyn LoadedCore>) -> Result<(), CoreLoadError>;

    /// Requisitos de renderização descobertos na primeira vez que esse core
    /// foi carregado (cache; None se ainda não foi carregado nenhuma vez).
    fn known_render_requirements(&self, core_id: &CoreId) -> Option<CoreRenderRequirements>;
}
