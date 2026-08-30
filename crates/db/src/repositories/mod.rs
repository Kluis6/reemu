//! Repositórios concretos. Um arquivo por porta do `domain`.
//!
//! Todos guardam um `Db` (pool clonável) e convertem erro de sqlx para
//! `domain::error::RepoError`. Nenhum tipo do sqlx aparece na assinatura
//! pública.

mod audio_config_repo;
mod controller_mappings_repo;
mod core_options_repo;
mod decoration_repo;
mod device_ports_repo;
mod installed_cores_repo;
mod metadata_repo;
mod roms_repo;
mod save_state_repo;
mod shader_chain_repo;
mod system_hotkeys_repo;

pub use audio_config_repo::AudioConfigRepo;
pub use controller_mappings_repo::ControllerMappingsRepo;
pub use core_options_repo::CoreOptionsRepo;
pub use decoration_repo::DecorationRepo;
pub use device_ports_repo::DevicePortsRepo;
pub use installed_cores_repo::InstalledCoresRepo;
pub use metadata_repo::MetadataRepo;
pub use roms_repo::RomsRepo;
pub use save_state_repo::SaveStateRepo;
pub use shader_chain_repo::ShaderChainRepo;
pub use system_hotkeys_repo::SystemHotkeysRepo;
