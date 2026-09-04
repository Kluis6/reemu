//! Crate `domain`: regras de negócio puras do emulador libretro.
//!
//! Nenhum tipo aqui depende de I/O de plataforma. Cada módulo define uma
//! "porta" (trait) que os crates `core-loader-desktop` e `core-loader-mobile`
//! implementam como adapters — arquitetura hexagonal.
//!
//! Convenção: `Ports` = traits que o domínio expõe pra fora (implementadas
//! pelos adapters). `Model` = tipos de dados puros (sem comportamento de I/O).

pub mod error;

pub mod audio;
pub mod bios;
pub mod core_loader;
pub mod core_options;
pub mod decoration;
pub mod focus;
pub mod frame_source;
pub mod hotkeys;
pub mod input;
pub mod library;
pub mod metadata;
pub mod save_state;
pub mod shader_chain;
