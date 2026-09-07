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

/// Handles crus de um `VkDevice` **já criado pelo compositor**, pra um core de
/// HW render Vulkan (etapa 12) usar o MESMO device — a `VkImage` que ele
/// entrega vira textura do compositor sem cópia.
///
/// Tudo `usize` de propósito: o `domain` não depende de `ash`/`wgpu`. O adapter
/// (`core-loader-desktop`) reconstrói os tipos `ash` com `Instance::load` /
/// `Device::load` a partir do `get_instance_proc_addr`. Handles Vulkan
/// despacháveis são ponteiros, então `usize` os representa sem perda.
///
/// **Posse:** quem recebe isto NÃO destrói nada — o compositor é o dono.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VulkanSharedDevice {
    /// `PFN_vkGetInstanceProcAddr` do loader, como inteiro.
    pub get_instance_proc_addr: usize,
    pub instance: usize,
    pub physical_device: usize,
    pub device: usize,
    /// Queue com GRAPHICS+COMPUTE (exigência da spec do libretro Vulkan).
    pub queue: usize,
    pub queue_family_index: u32,
}

/// Ponteiros da negociação de HW render Vulkan v1 de um core que EXIGE criar o
/// `VkDevice` ele mesmo (Beetle PSX HW — o `context_reset` dele aborta se o
/// frontend não chamou o `create_device`). O adapter (`core-loader-desktop`)
/// passa isto pro `VulkanDeviceNegotiator` do shell, que constrói a
/// `ash::Instance` com as extensões que o `wgpu-hal` quer, chama o
/// `create_device` do core, reconstrói o `FrameProcessor` sobre esse device, e
/// devolve os handles como [`VulkanSharedDevice`].
#[derive(Debug, Clone, Copy)]
pub struct VkNegotiation {
    /// `retro_vulkan_get_application_info_t` como inteiro (`0` = o core não deu).
    pub get_application_info: usize,
    /// `retro_vulkan_create_device_t` como inteiro (nunca `0` aqui).
    pub create_device: usize,
}

/// Fábrica que o shell fornece: dada a negociação de um core "dono do device",
/// cria o `VkDevice` (via o `create_device` do core), reconstrói o compositor
/// wgpu sobre ele, e devolve os handles. `Err` = não deu (o loader cai pro
/// caminho "frontend cria o device" ou pro bring-up).
pub type VulkanDeviceNegotiator =
    std::sync::Arc<dyn Fn(VkNegotiation) -> Result<VulkanSharedDevice, String> + Send + Sync>;

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

/// Core preferido por plataforma (`system_id`). O default do seletor de core;
/// o override por ROM (state local do RomDetail) continua vencendo.
#[async_trait]
pub trait SystemCoreRepository: Send + Sync {
    /// `(system_id, core_id)` de cada plataforma que tem preferência salva.
    async fn all(&self) -> Result<Vec<(String, String)>, RepoError>;
    async fn set(&self, system_id: &str, core_id: &str) -> Result<(), RepoError>;
    async fn clear(&self, system_id: &str) -> Result<(), RepoError>;
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
