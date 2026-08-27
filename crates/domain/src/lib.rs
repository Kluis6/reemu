//! Crate `domain`: regras de negócio puras do emulador libretro.
//!
//! Nenhum tipo aqui depende de I/O de plataforma. Cada módulo define uma
//! "porta" (trait) que os crates `core-loader-desktop` e `core-loader-mobile`
//! implementam como adapters — arquitetura hexagonal.
//!
//! Convenção: `Ports` = traits que o domínio expõe pra fora (implementadas
//! pelos adapters). `Model` = tipos de dados puros (sem comportamento de I/O).

pub mod core_loader;
pub mod frame_source;
pub mod shader_chain;
pub mod decoration;
pub mod core_options;
pub mod audio;
pub mod input;
pub mod hotkeys;
pub mod save_state;
pub mod metadata;
pub mod focus;
