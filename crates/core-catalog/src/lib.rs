//! Cores libretro do buildbot (Fase 6, item 12 da análise de 2026-11-12): o
//! catálogo curado e a instalação ([`catalog`], reexportado na raiz), o aviso
//! de versão nova ([`updates`]) e o erro próprio ([`CatalogError`]).

mod catalog;
mod error;
#[cfg(test)]
mod smoke_roms;
pub mod updates;

pub use catalog::*;
pub use error::CatalogError;
