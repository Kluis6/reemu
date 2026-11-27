//! `library-scan`: hash de ROMs (`RomHashService`) + varredura de diretório
//! populando `domain::library::RomRepository`. Match automático de metadata
//! é sempre por hash exato (ver `docs/ai-context/09`).

mod archive;
mod chd_reader;
mod decoration;
mod disc_sniff;
mod hash;
mod identity;
mod iso9660;
mod scan;
mod sector;
mod serial;
mod systems;
#[cfg(test)]
mod test_io;

pub use archive::{is_supported_archive, peek_archive, read_archive_entry_capped, ArchivedRom};
pub use decoration::{
    scan_decoration_pack, viewport_for_image, DecoScope, ScannedDecoration, Viewport,
};
pub use hash::{FileRomHasher, RomHashes};
pub use identity::{identify, Digests, Identity};
pub use scan::{count_roms, identify_known_rom, scan_into, ScanError, ScanProgress, ScanReport};
pub use serial::{read_serial, SERIAL_SYSTEMS};
pub use systems::{
    libretro_boxart_url, system_for_extension, system_from_folder_name, AMBIGUOUS_DISC_EXTS,
};
