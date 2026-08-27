//! `library-scan`: hash de ROMs (`RomHashService`) + varredura de diretório
//! populando `domain::library::RomRepository`. Match automático de metadata
//! é sempre por hash exato (ver `docs/ai-context/09`).

mod hash;
mod scan;
mod systems;

pub use hash::FileRomHasher;
pub use scan::{scan_into, ScanError, ScanReport};
pub use systems::system_for_extension;
