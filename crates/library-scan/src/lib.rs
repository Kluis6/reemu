//! `library-scan`: hash de ROMs (`RomHashService`) + varredura de diretório
//! populando `domain::library::RomRepository`. Match automático de metadata
//! é sempre por hash exato (ver `docs/ai-context/09`).

mod archive;
mod decoration;
mod hash;
mod scan;
mod systems;

pub use archive::{is_supported_archive, peek_zip, read_zip_entry, ArchivedRom};
pub use decoration::{
    scan_decoration_pack, viewport_for_image, DecoScope, ScannedDecoration, Viewport,
};
pub use hash::FileRomHasher;
pub use scan::{count_roms, scan_into, ScanError, ScanProgress, ScanReport};
pub use systems::{
    libretro_boxart_url, system_for_extension, system_from_folder_name, AMBIGUOUS_DISC_EXTS,
};
