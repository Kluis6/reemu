//! Leitores virtuais (`rc_hash_filereader_t` e `rc_hash_cdreader_t`) e suporte
//! a arquivos compactados (`.zip` e `.7z`) para cálculo de hash oficial do
//! RetroAchievements em mídias de disco (`.iso`, `.cue`/`.bin`, `.gdi`, `.chd`,
//! `.cso`, `.gcm`, `.gcz`, `.pbp`, `.m3u`).

use crate::console::{
    is_disc_console, RC_CONSOLE_3DO, RC_CONSOLE_ARCADE, RC_CONSOLE_ARDUBOY, RC_CONSOLE_DREAMCAST,
    RC_CONSOLE_GAMECUBE, RC_CONSOLE_MS_DOS, RC_CONSOLE_NEO_GEO_CD, RC_CONSOLE_PCFX,
    RC_CONSOLE_PC_ENGINE_CD, RC_CONSOLE_PLAYSTATION, RC_CONSOLE_PLAYSTATION_2, RC_CONSOLE_PSP,
    RC_CONSOLE_SATURN, RC_CONSOLE_SEGA_CD, RC_CONSOLE_WII,
};
use crate::ffi;
use chd::header::Header;
use chd::map::{CompressionTypeLegacy, CompressionTypeV5, Map, MapEntry};
use chd::metadata::KnownMetadata;
use chd::Chd;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use sevenz_rust2::{ArchiveReader, Password};
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const USER_DATA: usize = 2048;
const CD_FRAME: u64 = 2448;
const CD_TRACK_PADDING: u64 = 4;
const CHD_MAX_HUNK: u32 = 1024 * 1024;
const CHD_MAX_HUNKS: u32 = 1 << 22;
const CHD_MAP_BYTES_PER_HUNK: u64 = 16;
const CHD_MAP_SLACK: u64 = 64 * 1024;
const CHD_MAX_METADATA: usize = 64 * 1024;
const END_OF_MAP: &[u8; 16] = b"EndOfListCookie\0";
const RA_MAX_ROM_BUFFER: usize = 64 * 1024 * 1024;

const SYNC_PATTERN: [u8; 12] = [
    0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
];

const DISC_ARCHIVE_SHEET_EXTS: &[&str] = &["m3u", "cue", "gdi", "ccd"];

const DISC_SINGLE_EXTS: &[&str] = &[
    "iso", "gcm", "chd", "cso", "gcz", "pbp", "cdi", "img", "mdf", "nrg", "rvz", "wia", "tgc",
];

const CARTRIDGE_ROM_EXTS: &[&str] = &[
    "nes", "fds", "unif", "unf", "sfc", "smc", "swc", "fig", "bs", "gb", "gbc", "gba", "srl",
    "n64", "z64", "v64", "ndd", "md", "smd", "gen", "sgd", "sms", "gg", "pce", "sgx", "a26", "a78",
    "lnx", "ws", "wsc", "ngp", "ngc", "32x", "vb", "col", "int", "nds", "dsi", "ids", "sg", "a52",
    "atr", "xfd", "atx", "xex", "j64", "jag", "min", "sv", "mx1", "mx2", "vec", "d64", "d71",
    "d81", "g64", "t64", "x64", "crt", "adf", "adz", "dms", "hdf", "tzx", "z80", "rzx", "szx",
    "scl", "trd", "cdt", "cpr", "msa", "stx", "ch8", "sc8", "xo8", "hc8", "t81", "uze", "duck",
    "neo", "wad",
];

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

pub(crate) fn is_archive_path(path: &Path) -> bool {
    matches!(ext_lower(path).as_str(), "zip" | "7z")
}

pub fn is_disc_like_path(path: &Path) -> bool {
    let ext = ext_lower(path);
    DISC_ARCHIVE_SHEET_EXTS.contains(&ext.as_str())
        || DISC_SINGLE_EXTS.contains(&ext.as_str())
        || (ext == "bin"
            && std::fs::metadata(path)
                .map(|m| m.len() > 8 * 1024 * 1024)
                .unwrap_or(false))
}

/// Calcula o hash oficial do `rcheevos` para `console_id` em `path`, suportando
/// discos soltos, contêineres comprimidos (`.chd`, `.cso`, `.gcz`) e arquivos
/// `.zip`/`.7z` (tanto ROMs de cartucho quanto imagens de disco `.iso`/`.cue`+`.bin`).
pub fn hash_path_for_console(console_id: u32, path: &Path) -> Option<String> {
    if is_archive_path(path)
        && !matches!(
            console_id,
            RC_CONSOLE_ARCADE | RC_CONSOLE_MS_DOS | RC_CONSOLE_ARDUBOY
        )
    {
        return hash_archive(path, Some(console_id)).map(|(_, h)| h);
    }
    hash_direct_file_for_console(console_id, path)
}

/// Resolve `(console_id, hash)` tentando `preferred_console` primeiro e, para
/// mídias de disco (ou quando `preferred_console` é `None`, como em
/// `system_id == "disc"`), sondando os sistemas de disco suportados pelo
/// `rcheevos` pelas assinaturas internas da imagem.
pub fn resolve_console_and_hash(
    preferred_console: Option<u32>,
    path: &Path,
) -> Option<(u32, String)> {
    if is_archive_path(path) {
        if let Some(cid) = preferred_console {
            if matches!(
                cid,
                RC_CONSOLE_ARCADE | RC_CONSOLE_MS_DOS | RC_CONSOLE_ARDUBOY
            ) {
                return hash_direct_file_for_console(cid, path).map(|h| (cid, h));
            }
        }
        return hash_archive(path, preferred_console);
    }

    resolve_direct_file(preferred_console, path)
}

fn resolve_direct_file(preferred_console: Option<u32>, path: &Path) -> Option<(u32, String)> {
    if let Some(cid) = preferred_console {
        if let Some(hash) = hash_direct_file_for_console(cid, path) {
            let resolved = refine_sega_disc_console(cid, path).unwrap_or(cid);
            return Some((resolved, hash));
        }
        // Se não é console de disco nem extensão de disco, não há fallback.
        if !is_disc_console(cid) && !is_disc_like_path(path) {
            return None;
        }
    }

    for &candidate in probe_disc_consoles(path, preferred_console).iter() {
        if Some(candidate) == preferred_console {
            continue;
        }
        if let Some(hash) = hash_direct_file_for_console(candidate, path) {
            let resolved = refine_sega_disc_console(candidate, path).unwrap_or(candidate);
            return Some((resolved, hash));
        }
    }
    None
}

fn probe_disc_consoles(path: &Path, skip: Option<u32>) -> Vec<u32> {
    let ext = ext_lower(path);
    let candidates: &[u32] = match ext.as_str() {
        "gcm" | "gcz" | "tgc" => &[RC_CONSOLE_GAMECUBE, RC_CONSOLE_WII],
        "pbp" => &[RC_CONSOLE_PSP],
        "gdi" | "cdi" => &[RC_CONSOLE_DREAMCAST],
        "cso" => &[RC_CONSOLE_PSP, RC_CONSOLE_PLAYSTATION_2],
        _ => &[
            RC_CONSOLE_PLAYSTATION_2,
            RC_CONSOLE_GAMECUBE,
            RC_CONSOLE_PLAYSTATION,
            RC_CONSOLE_PSP,
            RC_CONSOLE_DREAMCAST,
            RC_CONSOLE_SATURN,
            RC_CONSOLE_SEGA_CD,
            RC_CONSOLE_WII,
            RC_CONSOLE_3DO,
            RC_CONSOLE_PC_ENGINE_CD,
            RC_CONSOLE_PCFX,
            RC_CONSOLE_NEO_GEO_CD,
        ],
    };
    candidates
        .iter()
        .copied()
        .filter(|c| Some(*c) != skip)
        .collect()
}

fn hash_direct_file_for_console(console_id: u32, path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let ext = ext_lower(path);
    // Evita recursão infinita em `rcheevos` v12.5.0 (`hash.c`: `RC_CONSOLE_PC_ENGINE_CD`
    // chama `rc_hash_buffered_file` quando a extensão não é `cue`/`chd`/`m3u`, e
    // `rc_hash_from_buffer` volta para `rc_hash_from_file` em `.iso`).
    if console_id == RC_CONSOLE_PC_ENGINE_CD && !matches!(ext.as_str(), "cue" | "chd" | "m3u") {
        return None;
    }
    // `rc_hash_psp` faz whole-file hash apenas para `.pbp`; em outras extensões exige
    // `PSP_GAME\PARAM.SFO` via ISO9660, que é seguro. Evita whole-file acidental se não for `.pbp`.
    if console_id == RC_CONSOLE_GAMECUBE || console_id == RC_CONSOLE_WII {
        // `rc_hash_gamecube` / `rc_hash_wii` validam magia em 0x1C / 0x18 via filereader.
    }

    let c_path = path_to_cstring(path)?;
    let mut iter: ffi::rc_hash_iterator_t = unsafe { std::mem::zeroed() };
    unsafe {
        ffi::rc_hash_initialize_iterator(&mut iter, c_path.as_ptr(), std::ptr::null(), 0);
    }
    install_custom_io(&mut iter);

    let mut out = [0 as c_char; 33];
    let ok = unsafe { ffi::rc_hash_generate(out.as_mut_ptr(), console_id, &iter) };
    unsafe {
        ffi::rc_hash_destroy_iterator(&mut iter);
    }
    if ok == 0 {
        return None;
    }
    let s = unsafe { CStr::from_ptr(out.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    (!s.is_empty()).then_some(s)
}

fn hash_buffer_with_name(console_id: u32, entry_name: &str, bytes: &[u8]) -> Option<String> {
    let c_name = CString::new(entry_name).ok();
    let name_ptr = c_name.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
    let mut iter: ffi::rc_hash_iterator_t = unsafe { std::mem::zeroed() };
    unsafe {
        ffi::rc_hash_initialize_iterator(&mut iter, name_ptr, bytes.as_ptr(), bytes.len());
    }
    let mut out = [0 as c_char; 33];
    let ok = unsafe { ffi::rc_hash_generate(out.as_mut_ptr(), console_id, &iter) };
    unsafe {
        ffi::rc_hash_destroy_iterator(&mut iter);
    }
    if ok == 0 {
        return None;
    }
    let s = unsafe { CStr::from_ptr(out.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    (!s.is_empty()).then_some(s)
}

/// `rc_hash_sega_cd` no `rcheevos` aceita tanto Sega CD (`SEGADISCSYSTEM  `)
/// quanto Saturn (`SEGA SEGASATURN `). Refina o `console_id` pelo setor 0 para
/// que o mapa de memória do core (`LibretroMemory`) use a plataforma exata.
fn refine_sega_disc_console(console_id: u32, path: &Path) -> Option<u32> {
    if !matches!(console_id, RC_CONSOLE_SEGA_CD | RC_CONSOLE_SATURN) {
        return None;
    }
    let c_path = path_to_cstring(path)?;
    let mut iter: ffi::rc_hash_iterator_t = unsafe { std::mem::zeroed() };
    unsafe {
        ffi::rc_hash_initialize_iterator(&mut iter, c_path.as_ptr(), std::ptr::null(), 0);
    }
    install_custom_io(&mut iter);
    let detected = unsafe {
        let open = iter.callbacks.cdreader.open_track_iterator?;
        let handle = open(iter.path, 1, &iter);
        if handle.is_null() {
            ffi::rc_hash_destroy_iterator(&mut iter);
            return None;
        }
        let first_fn = iter.callbacks.cdreader.first_track_sector;
        let read_fn = iter.callbacks.cdreader.read_sector;
        let close_fn = iter.callbacks.cdreader.close_track;
        let first = first_fn.map_or(0, |f| f(handle));
        let mut head = [0u8; 16];
        let n = read_fn.map_or(0, |r| {
            r(handle, first, head.as_mut_ptr().cast(), head.len())
        });
        if let Some(close) = close_fn {
            close(handle);
        }
        if n == 16 && &head == b"SEGA SEGASATURN " {
            Some(RC_CONSOLE_SATURN)
        } else if n == 16 && &head == b"SEGADISCSYSTEM  " {
            Some(RC_CONSOLE_SEGA_CD)
        } else {
            None
        }
    };
    unsafe {
        ffi::rc_hash_destroy_iterator(&mut iter);
    }
    detected
}

fn path_to_cstring(path: &Path) -> Option<CString> {
    CString::new(path.to_string_lossy().into_owned().into_bytes()).ok()
}

fn install_custom_io(iter: &mut ffi::rc_hash_iterator_t) {
    iter.callbacks.filereader = ffi::rc_hash_filereader_t {
        open: Some(vfs_open),
        seek: Some(vfs_seek),
        tell: Some(vfs_tell),
        read: Some(vfs_read),
        close: Some(vfs_close),
    };
    iter.callbacks.cdreader = ffi::rc_hash_cdreader_t {
        open_track: Some(cd_open_track),
        read_sector: Some(cd_read_sector),
        close_track: Some(cd_close_track),
        first_track_sector: Some(cd_first_track_sector),
        open_track_iterator: Some(cd_open_track_iterator),
    };
}

// ---------------------------------------------------------------------------
// Extração temporária de `.zip` / `.7z` para discos e ROMs grandes
// ---------------------------------------------------------------------------

struct TempExtractDir {
    dir: PathBuf,
}

impl TempExtractDir {
    fn create() -> io::Result<Self> {
        let base = std::env::temp_dir();
        let pid = std::process::id();
        for _ in 0..10_000 {
            let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let candidate = base.join(format!("reemu-ra-disc-{pid}-{seq}"));
            match std::fs::create_dir(&candidate) {
                Ok(()) => return Ok(Self { dir: candidate }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "não foi possível criar pasta temporária para hash de disco",
        ))
    }
}

impl Drop for TempExtractDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Clone, Debug)]
struct ArchiveEntryMeta {
    name: String,
    rel_path: PathBuf,
    ext: String,
    size: u64,
}

fn sanitize_archive_rel_path(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let rel = Path::new(&normalized);
    let mut clean = PathBuf::new();
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

fn select_primary_archive_entry(entries: &[ArchiveEntryMeta]) -> Option<(usize, bool)> {
    // 1. Playlist/sheet multi-arquivo (.m3u > .cue/.gdi/.ccd) -> requer extrair irmãos.
    for sheet_ext in DISC_ARCHIVE_SHEET_EXTS {
        if let Some(idx) = entries.iter().position(|e| e.ext == *sheet_ext) {
            return Some((idx, true));
        }
    }
    // 2. Imagem única de disco (.iso, .gcm, .chd, .cso, .gcz, .pbp...).
    if let Some(idx) = entries
        .iter()
        .position(|e| DISC_SINGLE_EXTS.contains(&e.ext.as_str()))
    {
        return Some((idx, false));
    }
    // 3. ROM de cartucho conhecida.
    if let Some(idx) = entries
        .iter()
        .position(|e| CARTRIDGE_ROM_EXTS.contains(&e.ext.as_str()))
    {
        return Some((idx, false));
    }
    // 4. Trilha `.bin` solta grande (> 8 MiB) ou binário único de cartucho.
    if let Some(idx) = entries.iter().position(|e| e.ext == "bin") {
        return Some((idx, false));
    }
    None
}

fn is_cartridge_fast_path(entry: &ArchiveEntryMeta, preferred_console: Option<u32>) -> bool {
    let Some(cid) = preferred_console else {
        return false;
    };
    if is_disc_console(cid) {
        return false;
    }
    if DISC_ARCHIVE_SHEET_EXTS.contains(&entry.ext.as_str())
        || DISC_SINGLE_EXTS.contains(&entry.ext.as_str())
    {
        return false;
    }
    if entry.ext == "bin" && entry.size > 8 * 1024 * 1024 {
        return false;
    }
    entry.size <= RA_MAX_ROM_BUFFER as u64
}

fn hash_archive(path: &Path, preferred_console: Option<u32>) -> Option<(u32, String)> {
    if ext_lower(path) == "7z" {
        hash_7z_archive(path, preferred_console)
    } else {
        hash_zip_archive(path, preferred_console)
    }
}

fn hash_zip_archive(path: &Path, preferred_console: Option<u32>) -> Option<(u32, String)> {
    let file = File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).ok()?;
    let mut metas = Vec::new();
    for i in 0..zip.len() {
        let Ok(entry) = zip.by_index(i) else {
            continue;
        };
        if !entry.is_file() {
            continue;
        }
        let Ok(name) = entry.name() else {
            continue;
        };
        let name = name.into_owned();
        let Some(rel_path) = sanitize_archive_rel_path(&name) else {
            continue;
        };
        let ext = ext_lower(&rel_path);
        metas.push(ArchiveEntryMeta {
            name,
            rel_path,
            ext,
            size: entry.size(),
        });
    }

    let (primary_idx, extract_all) = select_primary_archive_entry(&metas)?;
    let primary = &metas[primary_idx];

    if !extract_all && is_cartridge_fast_path(primary, preferred_console) {
        let cid = preferred_console?;
        let mut entry = zip.by_name(&primary.name).ok()?;
        let mut bytes = Vec::with_capacity(primary.size as usize);
        entry
            .by_ref()
            .take(RA_MAX_ROM_BUFFER as u64)
            .read_to_end(&mut bytes)
            .ok()?;
        if let Some(h) = hash_buffer_with_name(cid, &primary.name, &bytes) {
            return Some((cid, h));
        }
    }

    let temp = TempExtractDir::create().ok()?;
    if extract_all {
        for meta in &metas {
            let dest = temp.dir.join(&meta.rel_path);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).ok()?;
            }
            let mut src = zip.by_name(&meta.name).ok()?;
            let mut out = File::create(&dest).ok()?;
            io::copy(&mut src, &mut out).ok()?;
            out.flush().ok()?;
        }
    } else {
        let dest = temp.dir.join(&primary.rel_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).ok()?;
        }
        let mut src = zip.by_name(&primary.name).ok()?;
        let mut out = File::create(&dest).ok()?;
        io::copy(&mut src, &mut out).ok()?;
        out.flush().ok()?;
    }

    let extracted_primary = temp.dir.join(&primary.rel_path);
    resolve_direct_file(preferred_console, &extracted_primary)
}

fn hash_7z_archive(path: &Path, preferred_console: Option<u32>) -> Option<(u32, String)> {
    let mut reader = ArchiveReader::open(path, Password::empty()).ok()?;
    reader.set_thread_count(1);

    let metas: Vec<ArchiveEntryMeta> = reader
        .archive()
        .files
        .iter()
        .filter(|f| !f.is_directory())
        .filter_map(|f| {
            let rel_path = sanitize_archive_rel_path(f.name())?;
            let ext = ext_lower(&rel_path);
            Some(ArchiveEntryMeta {
                name: f.name().to_string(),
                rel_path,
                ext,
                size: f.size(),
            })
        })
        .collect();

    let (primary_idx, extract_all) = select_primary_archive_entry(&metas)?;
    let primary = metas[primary_idx].clone();

    if !extract_all && is_cartridge_fast_path(&primary, preferred_console) {
        let cid = preferred_console?;
        let mut bytes_out: Option<Vec<u8>> = None;
        let ok = reader.for_each_entries(|e, r| {
            if bytes_out.is_some() {
                return Ok(false);
            }
            if e.name() == primary.name {
                let mut buf = Vec::with_capacity(primary.size as usize);
                r.take(RA_MAX_ROM_BUFFER as u64).read_to_end(&mut buf)?;
                bytes_out = Some(buf);
                return Ok(false);
            }
            io::copy(r, &mut io::sink())?;
            Ok(true)
        });
        if ok.is_ok() {
            if let Some(bytes) = bytes_out {
                if let Some(h) = hash_buffer_with_name(cid, &primary.name, &bytes) {
                    return Some((cid, h));
                }
            }
        }
        // Reabre caso precise extrair em disco abaixo.
        reader = ArchiveReader::open(path, Password::empty()).ok()?;
        reader.set_thread_count(1);
    }

    let temp = TempExtractDir::create().ok()?;
    let target_name = primary.name.clone();
    let dir_root = temp.dir.clone();
    let mut extracted_ok = false;

    reader
        .for_each_entries(|e, r| {
            if e.is_directory() {
                return Ok(true);
            }
            let Some(rel_path) = sanitize_archive_rel_path(e.name()) else {
                io::copy(r, &mut io::sink())?;
                return Ok(true);
            };
            if !extract_all && e.name() != target_name {
                io::copy(r, &mut io::sink())?;
                return Ok(true);
            }
            let dest = dir_root.join(&rel_path);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&dest)?;
            io::copy(r, &mut out)?;
            out.flush()?;
            if e.name() == target_name {
                extracted_ok = true;
                if !extract_all {
                    return Ok(false);
                }
            }
            Ok(true)
        })
        .ok()?;

    if !extracted_ok {
        return None;
    }
    let extracted_primary = temp.dir.join(&primary.rel_path);
    resolve_direct_file(preferred_console, &extracted_primary)
}

// ---------------------------------------------------------------------------
// Virtual Filereader: arquivos comuns + `.cso` (CISO) + `.gcz` + `.chd` DVD
// ---------------------------------------------------------------------------

enum VirtualFile {
    Plain(File),
    Ciso(Box<CisoReader>),
    Gcz(Box<GczReader>),
    ChdStream(Box<ChdByteStream>),
}

impl VirtualFile {
    fn open(path: &Path) -> Option<Self> {
        let ext = ext_lower(path);
        match ext.as_str() {
            "cso" => CisoReader::open(path).map(|r| Self::Ciso(Box::new(r))),
            "gcz" => GczReader::open(path).map(|r| Self::Gcz(Box::new(r))),
            "chd" => ChdByteStream::open(path)
                .map(|r| Self::ChdStream(Box::new(r)))
                .or_else(|| File::open(path).ok().map(Self::Plain)),
            _ => File::open(path).ok().map(Self::Plain),
        }
    }

    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Plain(f) => f.seek(pos),
            Self::Ciso(r) => r.seek(pos),
            Self::Gcz(r) => r.seek(pos),
            Self::ChdStream(r) => r.seek(pos),
        }
    }

    fn tell(&mut self) -> u64 {
        match self {
            Self::Plain(f) => f.stream_position().unwrap_or(0),
            Self::Ciso(r) => r.pos,
            Self::Gcz(r) => r.pos,
            Self::ChdStream(r) => r.pos,
        }
    }

    fn read_bytes(&mut self, mut out: &mut [u8]) -> usize {
        let mut total = 0usize;
        while !out.is_empty() {
            let r = match self {
                Self::Plain(f) => f.read(out),
                Self::Ciso(c) => c.read(out),
                Self::Gcz(g) => g.read(out),
                Self::ChdStream(c) => c.read(out),
            };
            match r {
                Ok(0) => break,
                Ok(n) => {
                    total += n;
                    out = &mut out[n..];
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        total
    }
}

unsafe extern "C" fn vfs_open(path_utf8: *const c_char) -> *mut c_void {
    if path_utf8.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(path_str) = CStr::from_ptr(path_utf8).to_str() else {
        return std::ptr::null_mut();
    };
    match VirtualFile::open(Path::new(path_str)) {
        Some(vf) => Box::into_raw(Box::new(vf)).cast(),
        None => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn vfs_seek(file_handle: *mut c_void, offset: i64, origin: c_int) {
    if file_handle.is_null() {
        return;
    }
    let vf = &mut *(file_handle as *mut VirtualFile);
    let from = match origin {
        0 => {
            if offset < 0 {
                return;
            }
            SeekFrom::Start(offset as u64)
        }
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return,
    };
    let _ = vf.seek(from);
}

unsafe extern "C" fn vfs_tell(file_handle: *mut c_void) -> i64 {
    if file_handle.is_null() {
        return 0;
    }
    let vf = &mut *(file_handle as *mut VirtualFile);
    i64::try_from(vf.tell()).unwrap_or(i64::MAX)
}

unsafe extern "C" fn vfs_read(
    file_handle: *mut c_void,
    buffer: *mut c_void,
    requested_bytes: usize,
) -> usize {
    if file_handle.is_null() || buffer.is_null() || requested_bytes == 0 {
        return 0;
    }
    let vf = &mut *(file_handle as *mut VirtualFile);
    let out = std::slice::from_raw_parts_mut(buffer.cast::<u8>(), requested_bytes);
    vf.read_bytes(out)
}

unsafe extern "C" fn vfs_close(file_handle: *mut c_void) {
    if !file_handle.is_null() {
        drop(Box::from_raw(file_handle as *mut VirtualFile));
    }
}

// ---------------------------------------------------------------------------
// Leitor `.cso` (CISO v1: deflate por bloco de 2048 bytes)
// ---------------------------------------------------------------------------

struct CisoReader {
    file: File,
    total_bytes: u64,
    block_size: u32,
    align: u8,
    index: Vec<u32>,
    pos: u64,
    cached_block: Option<usize>,
    block_buf: Vec<u8>,
    comp_buf: Vec<u8>,
}

impl CisoReader {
    fn open(path: &Path) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut hdr = [0u8; 24];
        file.read_exact(&mut hdr).ok()?;
        if &hdr[0..4] != b"CISO" {
            return None;
        }
        let total_bytes = u64::from_le_bytes(hdr[8..16].try_into().ok()?);
        let block_size = u32::from_le_bytes(hdr[16..20].try_into().ok()?);
        let ver = hdr[20];
        let align = hdr[21];
        if ver > 1 || !(512..=1024 * 1024).contains(&block_size) || align > 16 || total_bytes == 0 {
            return None;
        }
        let num_blocks = usize::try_from(total_bytes.div_ceil(u64::from(block_size))).ok()?;
        if num_blocks > 4_000_000 {
            return None;
        }
        let mut raw_idx = vec![0u8; (num_blocks + 1) * 4];
        file.read_exact(&mut raw_idx).ok()?;
        let index: Vec<u32> = raw_idx
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Some(Self {
            file,
            total_bytes,
            block_size,
            align,
            index,
            pos: 0,
            cached_block: None,
            block_buf: vec![0u8; block_size as usize],
            comp_buf: Vec::new(),
        })
    }

    fn load_block(&mut self, block: usize) -> io::Result<()> {
        if self.cached_block == Some(block) {
            return Ok(());
        }
        let cur = *self
            .index
            .get(block)
            .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
        let next = *self
            .index
            .get(block + 1)
            .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
        let plain = (cur & 0x8000_0000) != 0;
        let off = u64::from(cur & 0x7FFF_FFFF) << u32::from(self.align);
        let next_off = u64::from(next & 0x7FFF_FFFF) << u32::from(self.align);
        let comp_len = usize::try_from(
            next_off
                .checked_sub(off)
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?,
        )
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        if comp_len > (self.block_size as usize) * 2 + 4096 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        self.file.seek(SeekFrom::Start(off))?;
        self.block_buf.fill(0);
        if plain {
            let to_read = comp_len.min(self.block_buf.len());
            self.file.read_exact(&mut self.block_buf[..to_read])?;
        } else {
            self.comp_buf.resize(comp_len, 0);
            self.file.read_exact(&mut self.comp_buf)?;
            let mut dec = DeflateDecoder::new(&self.comp_buf[..]);
            let mut filled = 0usize;
            while filled < self.block_buf.len() {
                let n = dec.read(&mut self.block_buf[filled..])?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
        }
        self.cached_block = Some(block);
        Ok(())
    }
}

impl Seek for CisoReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => self.total_bytes.checked_add_signed(d),
        }
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        self.pos = next.min(self.total_bytes);
        Ok(self.pos)
    }
}

impl Read for CisoReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.total_bytes || out.is_empty() {
            return Ok(0);
        }
        let bsize = u64::from(self.block_size);
        let block = usize::try_from(self.pos / bsize)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let in_block = (self.pos % bsize) as usize;
        self.load_block(block)?;
        let rem_file = (self.total_bytes - self.pos) as usize;
        let avail = (self.block_buf.len() - in_block)
            .min(rem_file)
            .min(out.len());
        out[..avail].copy_from_slice(&self.block_buf[in_block..in_block + avail]);
        self.pos += avail as u64;
        Ok(avail)
    }
}

// ---------------------------------------------------------------------------
// Leitor `.gcz` (Dolphin CompressedBlob: zlib por bloco para GameCube / Wii)
// ---------------------------------------------------------------------------

struct GczReader {
    file: File,
    data_size: u64,
    compressed_data_size: u64,
    block_size: u32,
    data_start: u64,
    offsets: Vec<u64>,
    pos: u64,
    cached_block: Option<usize>,
    block_buf: Vec<u8>,
    comp_buf: Vec<u8>,
}

impl GczReader {
    const MAGIC: u32 = 0xB10B_C001;
    const UNCOMPRESSED_FLAG: u64 = 1 << 63;

    fn open(path: &Path) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut hdr = [0u8; 32];
        file.read_exact(&mut hdr).ok()?;
        let magic = u32::from_le_bytes(hdr[0..4].try_into().ok()?);
        if magic != Self::MAGIC {
            return None;
        }
        let compressed_data_size = u64::from_le_bytes(hdr[8..16].try_into().ok()?);
        let data_size = u64::from_le_bytes(hdr[16..24].try_into().ok()?);
        let block_size = u32::from_le_bytes(hdr[24..28].try_into().ok()?);
        let num_blocks = usize::try_from(u32::from_le_bytes(hdr[28..32].try_into().ok()?)).ok()?;
        if !(512..=4 * 1024 * 1024).contains(&block_size)
            || num_blocks == 0
            || num_blocks > 2_000_000
            || data_size == 0
        {
            return None;
        }
        let mut raw_offsets = vec![0u8; num_blocks * 8];
        file.read_exact(&mut raw_offsets).ok()?;
        let offsets: Vec<u64> = raw_offsets
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let data_start = 32u64.checked_add((num_blocks as u64).checked_mul(12)?)?;
        Some(Self {
            file,
            data_size,
            compressed_data_size,
            block_size,
            data_start,
            offsets,
            pos: 0,
            cached_block: None,
            block_buf: vec![0u8; block_size as usize],
            comp_buf: Vec::new(),
        })
    }

    fn load_block(&mut self, block: usize) -> io::Result<()> {
        if self.cached_block == Some(block) {
            return Ok(());
        }
        let entry = *self
            .offsets
            .get(block)
            .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
        let uncompressed = (entry & Self::UNCOMPRESSED_FLAG) != 0;
        let rel_off = entry & !Self::UNCOMPRESSED_FLAG;
        let next_rel = match self.offsets.get(block + 1) {
            Some(&n) => n & !Self::UNCOMPRESSED_FLAG,
            None => self.compressed_data_size,
        };
        let comp_len = usize::try_from(
            next_rel
                .checked_sub(rel_off)
                .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?,
        )
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
        if comp_len > (self.block_size as usize) * 2 + 4096 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let file_off = self
            .data_start
            .checked_add(rel_off)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        self.file.seek(SeekFrom::Start(file_off))?;
        self.block_buf.fill(0);
        if uncompressed {
            let to_read = comp_len.min(self.block_buf.len());
            self.file.read_exact(&mut self.block_buf[..to_read])?;
        } else {
            self.comp_buf.resize(comp_len, 0);
            self.file.read_exact(&mut self.comp_buf)?;
            let mut dec = ZlibDecoder::new(&self.comp_buf[..]);
            let mut filled = 0usize;
            while filled < self.block_buf.len() {
                let n = dec.read(&mut self.block_buf[filled..])?;
                if n == 0 {
                    break;
                }
                filled += n;
            }
        }
        self.cached_block = Some(block);
        Ok(())
    }
}

impl Seek for GczReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => self.data_size.checked_add_signed(d),
        }
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        self.pos = next.min(self.data_size);
        Ok(self.pos)
    }
}

impl Read for GczReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.data_size || out.is_empty() {
            return Ok(0);
        }
        let bsize = u64::from(self.block_size);
        let block = usize::try_from(self.pos / bsize)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let in_block = (self.pos % bsize) as usize;
        self.load_block(block)?;
        let rem_file = (self.data_size - self.pos) as usize;
        let avail = (self.block_buf.len() - in_block)
            .min(rem_file)
            .min(out.len());
        out[..avail].copy_from_slice(&self.block_buf[in_block..in_block + avail]);
        self.pos += avail as u64;
        Ok(avail)
    }
}

// ---------------------------------------------------------------------------
// Leitor `.chd` (CD 2448-byte frames, GD-ROM e DVD 2048-byte sectors)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChdSectorMode {
    Cooked2048,
    Raw2352 { mode2: bool },
    Mode2Headerless2336,
    Audio2352,
}

impl ChdSectorMode {
    fn from_chd_type(kind: &str) -> Option<Self> {
        Some(match kind {
            "MODE1/2048" | "MODE1" | "MODE2/2048" | "MODE2_FORM1" => Self::Cooked2048,
            "MODE1/2352" | "MODE1_RAW" => Self::Raw2352 { mode2: false },
            "MODE2/2352" | "MODE2_RAW" | "CDI/2352" => Self::Raw2352 { mode2: true },
            "MODE2/2336" | "MODE2" | "MODE2_FORM_MIX" => Self::Mode2Headerless2336,
            "AUDIO" => Self::Audio2352,
            _ => return None,
        })
    }

    fn is_data(self) -> bool {
        !matches!(self, Self::Audio2352)
    }

    fn payload_size(self) -> usize {
        match self {
            Self::Audio2352 => 2352,
            _ => USER_DATA,
        }
    }

    fn user_offset(self, frame: &[u8]) -> usize {
        match self {
            Self::Cooked2048 | Self::Audio2352 => 0,
            Self::Mode2Headerless2336 => 8,
            Self::Raw2352 { mode2 } => match frame.get(15) {
                Some(1) => 16,
                Some(2) => 24,
                _ if mode2 => 24,
                _ => 16,
            },
        }
    }
}

#[derive(Debug, Clone)]
struct ChdCdTrackMeta {
    number: u32,
    mode: ChdSectorMode,
    frames: u64,
    pregap: u64,
    pregap_in_data: bool,
}

fn parse_chd_cd_track(text: &str) -> Option<ChdCdTrackMeta> {
    let mut number = 0u32;
    let mut kind = String::new();
    let mut frames = 0u64;
    let mut pregap = 0u64;
    let mut pregap_in_data = false;
    for word in text.split(|c: char| c.is_whitespace() || c == '\0') {
        let Some((k, v)) = word.split_once(':') else {
            continue;
        };
        match k {
            "TRACK" => number = v.parse().ok()?,
            "TYPE" => kind = v.to_ascii_uppercase(),
            "FRAMES" => frames = v.parse().ok()?,
            "PREGAP" => pregap = v.parse().ok()?,
            "PGTYPE" => pregap_in_data = v.starts_with('V'),
            _ => {}
        }
    }
    let mode = ChdSectorMode::from_chd_type(&kind)?;
    (number > 0 && frames > 0).then_some(ChdCdTrackMeta {
        number,
        mode,
        frames,
        pregap,
        pregap_in_data,
    })
}

fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be_u64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

struct SafeChdCore {
    chd: Chd<BufReader<File>>,
    file_len: u64,
    hunk_size: u32,
    hunk_buf: Vec<u8>,
    cached_hunk: Option<u32>,
    comp_buf: Vec<u8>,
    poisoned: bool,
}

enum ChdMediaKind {
    Dvd,
    Cd {
        tracks: Vec<ChdCdTrackMeta>,
        gd: bool,
    },
}

impl SafeChdCore {
    fn open(path: &Path) -> Option<(Self, ChdMediaKind)> {
        let path_buf = path.to_path_buf();
        catch_unwind(AssertUnwindSafe(move || Self::open_checked(&path_buf)))
            .ok()
            .flatten()
    }

    fn open_checked(path: &Path) -> Option<(Self, ChdMediaKind)> {
        let mut raw_file = File::open(path).ok()?;
        let file_len = raw_file.seek(SeekFrom::End(0)).ok()?;
        let mut raw = [0u8; 56];
        raw_file.seek(SeekFrom::Start(0)).ok()?;
        raw_file.read_exact(&mut raw).ok()?;
        if &raw[..8] != b"MComprHD" {
            return None;
        }
        let meta_offset = match be_u32(&raw, 12)? {
            3 | 4 => be_u64(&raw, 36)?,
            5 => be_u64(&raw, 48)?,
            _ => 0,
        };
        let media = Self::read_media_kind(&mut raw_file, meta_offset)?;

        raw_file.seek(SeekFrom::Start(0)).ok()?;
        let mut reader = BufReader::new(raw_file);
        let header = Header::try_read_header(&mut reader).ok()?;
        let hunk_size = header.hunk_size();
        if header.has_parent()
            || hunk_size == 0
            || hunk_size > CHD_MAX_HUNK
            || header.hunk_count() > CHD_MAX_HUNKS
        {
            return None;
        }
        match &media {
            ChdMediaKind::Dvd if !(hunk_size as usize).is_multiple_of(USER_DATA) => return None,
            ChdMediaKind::Cd { .. } if !u64::from(hunk_size).is_multiple_of(CD_FRAME) => {
                return None
            }
            _ => {}
        }

        let hunks = u64::from(header.hunk_count());
        let map_end = match &header {
            Header::V5Header(v5) if header.is_compressed() => {
                let mut len = [0u8; 4];
                reader.seek(SeekFrom::Start(v5.map_offset)).ok()?;
                reader.read_exact(&mut len).ok()?;
                let len = u64::from(u32::from_be_bytes(len));
                if len > hunks * CHD_MAP_BYTES_PER_HUNK + CHD_MAP_SLACK {
                    return None;
                }
                v5.map_offset.checked_add(16)?.checked_add(len)?
            }
            Header::V5Header(v5) => v5.map_offset.checked_add(hunks * 4)?,
            Header::V1Header(_)
            | Header::V2Header(_)
            | Header::V3Header(_)
            | Header::V4Header(_) => {
                let entry: u64 = match &header {
                    Header::V3Header(_) | Header::V4Header(_) => 16,
                    _ => 8,
                };
                let cookie_at = u64::from(header.len()).checked_add(hunks * entry)?;
                let mut cookie = [0u8; 16];
                let cookie_slice = cookie.get_mut(..entry as usize)?;
                reader.seek(SeekFrom::Start(cookie_at)).ok()?;
                reader.read_exact(cookie_slice).ok()?;
                if *cookie_slice != END_OF_MAP[..entry as usize] {
                    return None;
                }
                cookie_at.checked_add(entry)?
            }
        };
        if map_end > file_len {
            return None;
        }

        reader.seek(SeekFrom::Start(0)).ok()?;
        let chd = Chd::open(reader, None).ok()?;
        Some((
            Self {
                chd,
                file_len,
                hunk_size,
                hunk_buf: vec![0u8; hunk_size as usize],
                cached_hunk: None,
                comp_buf: Vec::new(),
                poisoned: false,
            },
            media,
        ))
    }

    fn read_media_kind(file: &mut File, first: u64) -> Option<ChdMediaKind> {
        const MAX_ENTRIES: usize = 128;
        const MAX_TRACK_TEXT: usize = 1024;
        let cd_tags = [
            KnownMetadata::CdRomTrack2 as u32,
            KnownMetadata::CdRomTrack as u32,
            KnownMetadata::GdRomTrack as u32,
            KnownMetadata::GdRomOld as u32,
        ];
        let gd_tags = [
            KnownMetadata::GdRomTrack as u32,
            KnownMetadata::GdRomOld as u32,
        ];
        let mut tracks = Vec::new();
        let (mut is_cd, mut gd) = (false, false);
        let mut visited = Vec::new();
        let mut offset = first;
        while offset != 0 {
            if visited.len() >= MAX_ENTRIES || visited.contains(&offset) {
                return None;
            }
            visited.push(offset);
            let mut head = [0u8; 16];
            file.seek(SeekFrom::Start(offset)).ok()?;
            file.read_exact(&mut head).ok()?;
            let tag = u32::from_be_bytes(head[0..4].try_into().ok()?);
            let len = u32::from_be_bytes([0, head[5], head[6], head[7]]) as usize;
            offset = u64::from_be_bytes(head[8..16].try_into().ok()?);
            if len > CHD_MAX_METADATA {
                return None;
            }
            is_cd |= KnownMetadata::is_cdrom(tag);
            if !cd_tags.contains(&tag) {
                continue;
            }
            if len > MAX_TRACK_TEXT {
                return None;
            }
            gd |= gd_tags.contains(&tag);
            let mut text = vec![0u8; len];
            file.read_exact(&mut text).ok()?;
            tracks.push(parse_chd_cd_track(&String::from_utf8_lossy(&text))?);
        }
        if !is_cd {
            return Some(ChdMediaKind::Dvd);
        }
        if tracks.is_empty() {
            return None;
        }
        tracks.sort_by_key(|t| t.number);
        Some(ChdMediaKind::Cd { tracks, gd })
    }

    fn resolve_hunk(&self, mut n: u32) -> Option<u32> {
        const MAX_HOPS: usize = 16;
        let limit = u64::from(self.hunk_size) * 2;
        let fits = |offset: u64, size: u32| {
            u64::from(size) <= limit
                && offset
                    .checked_add(u64::from(size))
                    .is_some_and(|end| end <= self.file_len)
        };
        for _ in 0..MAX_HOPS {
            let map: &Map = self.chd.map();
            let target = match map.get_entry(n as usize)? {
                MapEntry::V5Compressed(e) => match e.hunk_type().ok()? {
                    CompressionTypeV5::CompressionSelf => e.block_offset(),
                    CompressionTypeV5::CompressionType0
                    | CompressionTypeV5::CompressionType1
                    | CompressionTypeV5::CompressionType2
                    | CompressionTypeV5::CompressionType3
                    | CompressionTypeV5::CompressionNone => {
                        return fits(e.block_offset(), e.block_size()).then_some(n);
                    }
                    _ => return None,
                },
                MapEntry::V5Uncompressed(e) => {
                    let ok = e.block_offset() == 0 || fits(e.block_offset(), e.block_size());
                    return ok.then_some(n);
                }
                MapEntry::LegacyEntry(e) => match e.hunk_type().ok()? {
                    CompressionTypeLegacy::SelfHunk => e.block_offset(),
                    CompressionTypeLegacy::Compressed | CompressionTypeLegacy::Uncompressed => {
                        return fits(e.block_offset(), e.block_size()).then_some(n);
                    }
                    CompressionTypeLegacy::Mini => return Some(n),
                    _ => return None,
                },
            };
            let target = u32::try_from(target).ok()?;
            if target >= n {
                return None;
            }
            n = target;
        }
        None
    }

    fn ensure_hunk(&mut self, hunk_idx: u32) -> Option<()> {
        if self.cached_hunk == Some(hunk_idx) {
            return Some(());
        }
        if self.poisoned || hunk_idx >= self.chd.header().hunk_count() {
            return None;
        }
        let resolved = self.resolve_hunk(hunk_idx)?;
        let ok = catch_unwind(AssertUnwindSafe(|| {
            let mut hunk = self.chd.hunk(resolved).ok()?;
            hunk.read_hunk_in(&mut self.comp_buf, &mut self.hunk_buf)
                .ok()
        }));
        match ok {
            Ok(Some(_)) => {
                self.cached_hunk = Some(hunk_idx);
                Some(())
            }
            Ok(None) => None,
            Err(_) => {
                self.poisoned = true;
                None
            }
        }
    }

    fn read_at(&mut self, byte_offset: u64, len: usize) -> Option<&[u8]> {
        let hsize = u64::from(self.hunk_size);
        let hunk = u32::try_from(byte_offset / hsize).ok()?;
        let in_hunk = (byte_offset % hsize) as usize;
        self.ensure_hunk(hunk)?;
        self.hunk_buf.get(in_hunk..in_hunk + len)
    }
}

/// Exposição de `.chd` de DVD como stream contíguo de bytes (usado quando um
/// console como GameCube/Wii lê o disco via `filereader` em vez de `cdreader`).
struct ChdByteStream {
    core: SafeChdCore,
    logical_bytes: u64,
    pos: u64,
}

impl ChdByteStream {
    fn open(path: &Path) -> Option<Self> {
        let (core, media) = SafeChdCore::open(path)?;
        if !matches!(media, ChdMediaKind::Dvd) {
            return None;
        }
        let logical_bytes = core.chd.header().logical_bytes();
        (logical_bytes > 0).then_some(Self {
            core,
            logical_bytes,
            pos: 0,
        })
    }
}

impl Seek for ChdByteStream {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let next = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => self.logical_bytes.checked_add_signed(d),
        }
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        self.pos = next.min(self.logical_bytes);
        Ok(self.pos)
    }
}

impl Read for ChdByteStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.logical_bytes || out.is_empty() {
            return Ok(0);
        }
        let hsize = u64::from(self.core.hunk_size);
        let in_hunk = (self.pos % hsize) as usize;
        let rem_hunk = (self.core.hunk_size as usize) - in_hunk;
        let rem_file = (self.logical_bytes - self.pos) as usize;
        let take = out.len().min(rem_hunk).min(rem_file);
        let slice = self
            .core
            .read_at(self.pos, take)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        out[..take].copy_from_slice(slice);
        self.pos += take as u64;
        Ok(take)
    }
}

// ---------------------------------------------------------------------------
// Custom `cdreader` (`TrackHandle`): `.chd` (CD/GD-ROM/DVD) + fallback nativo
// ---------------------------------------------------------------------------

struct ChdCdTrackHandle {
    core: SafeChdCore,
    is_dvd: bool,
    mode: ChdSectorMode,
    first_frame_in_chd: u64,
    data_frames: u64,
    first_sector_lba: u32,
}

impl ChdCdTrackHandle {
    fn open(path: &Path, requested_track: u32) -> Option<Self> {
        let track_req = if requested_track == 0 {
            ffi::RC_HASH_CDTRACK_LARGEST
        } else {
            requested_track
        };
        let (mut core, media) = SafeChdCore::open(path)?;
        match media {
            ChdMediaKind::Dvd => {
                if !matches!(
                    track_req,
                    1 | ffi::RC_HASH_CDTRACK_FIRST_DATA
                        | ffi::RC_HASH_CDTRACK_LAST
                        | ffi::RC_HASH_CDTRACK_LARGEST
                ) {
                    return None;
                }
                let total_sectors = core.chd.header().logical_bytes() / (USER_DATA as u64);
                (total_sectors > 0).then_some(Self {
                    core,
                    is_dvd: true,
                    mode: ChdSectorMode::Cooked2048,
                    first_frame_in_chd: 0,
                    data_frames: total_sectors,
                    first_sector_lba: 0,
                })
            }
            ChdMediaKind::Cd { tracks, gd } => {
                let chosen = select_chd_track(&tracks, gd, track_req)?;
                let mut chd_frame = 0u64;
                let mut disc_lba = 0u64;
                for t in &tracks {
                    if gd && t.number == 3 && disc_lba < 45_000 {
                        disc_lba = 45_000;
                    }
                    if t.number == chosen.number {
                        let recorded_pregap = if t.pregap_in_data { t.pregap } else { 0 };
                        let first_frame_in_chd = chd_frame.checked_add(recorded_pregap)?;
                        let data_frames = t.frames.checked_sub(recorded_pregap)?;
                        if data_frames == 0 {
                            return None;
                        }
                        let fallback_lba = u32::try_from(disc_lba.checked_add(t.pregap)?).ok()?;
                        let first_sector_lba =
                            detect_raw_msf_lba(&mut core, t.mode, first_frame_in_chd, data_frames)
                                .unwrap_or(fallback_lba);
                        return Some(Self {
                            core,
                            is_dvd: false,
                            mode: t.mode,
                            first_frame_in_chd,
                            data_frames,
                            first_sector_lba,
                        });
                    }
                    let padded = t.frames.checked_next_multiple_of(CD_TRACK_PADDING)?;
                    chd_frame = chd_frame.checked_add(padded)?;
                    let unrecorded_pregap = if t.pregap_in_data { 0 } else { t.pregap };
                    disc_lba = disc_lba
                        .checked_add(unrecorded_pregap)?
                        .checked_add(t.frames)?;
                }
                None
            }
        }
    }

    fn read_sector_bytes(&mut self, sector: u32, mut out: &mut [u8]) -> usize {
        if sector < self.first_sector_lba || out.is_empty() {
            return 0;
        }
        let mut rel_sector = u64::from(sector - self.first_sector_lba);
        let payload_len = self.mode.payload_size();
        let mut total_read = 0usize;

        while !out.is_empty() && rel_sector < self.data_frames {
            let take = out.len().min(payload_len);
            if self.is_dvd {
                let Some(byte_off) = rel_sector.checked_mul(USER_DATA as u64) else {
                    break;
                };
                let Some(slice) = self.core.read_at(byte_off, take) else {
                    break;
                };
                out[..take].copy_from_slice(slice);
            } else {
                let Some(frame_idx) = self.first_frame_in_chd.checked_add(rel_sector) else {
                    break;
                };
                let Some(byte_off) = frame_idx.checked_mul(CD_FRAME) else {
                    break;
                };
                let Some(frame) = self.core.read_at(byte_off, CD_FRAME as usize) else {
                    break;
                };
                let user_off = self.mode.user_offset(frame);
                let Some(slice) = frame.get(user_off..user_off + take) else {
                    break;
                };
                out[..take].copy_from_slice(slice);
            }
            total_read += take;
            out = &mut out[take..];
            rel_sector += 1;
        }
        total_read
    }
}

fn select_chd_track(tracks: &[ChdCdTrackMeta], gd: bool, track_req: u32) -> Option<ChdCdTrackMeta> {
    match track_req {
        ffi::RC_HASH_CDTRACK_FIRST_DATA => tracks.iter().find(|t| t.mode.is_data()).cloned(),
        ffi::RC_HASH_CDTRACK_LAST => tracks.last().cloned(),
        ffi::RC_HASH_CDTRACK_LARGEST => tracks
            .iter()
            .filter(|t| t.mode.is_data())
            .max_by_key(|t| t.frames)
            .or_else(|| tracks.iter().max_by_key(|t| t.frames))
            .cloned(),
        ffi::RC_HASH_CDTRACK_FIRST_OF_SECOND_SESSION => {
            if gd {
                tracks.iter().find(|t| t.number >= 3).cloned()
            } else {
                None
            }
        }
        n => tracks.iter().find(|t| t.number == n).cloned(),
    }
}

fn bcd_byte(b: u8) -> Option<u32> {
    let hi = b >> 4;
    let lo = b & 0x0F;
    (hi < 10 && lo < 10).then_some(u32::from(hi) * 10 + u32::from(lo))
}

fn detect_raw_msf_lba(
    core: &mut SafeChdCore,
    mode: ChdSectorMode,
    first_frame_in_chd: u64,
    data_frames: u64,
) -> Option<u32> {
    if !matches!(mode, ChdSectorMode::Raw2352 { .. }) {
        return None;
    }
    let probe_rel = if data_frames > 16 { 16 } else { 0 };
    let byte_off = first_frame_in_chd
        .checked_add(probe_rel)?
        .checked_mul(CD_FRAME)?;
    let frame = core.read_at(byte_off, 16)?;
    if frame[..12] != SYNC_PATTERN {
        return None;
    }
    let m = bcd_byte(frame[12])?;
    let s = bcd_byte(frame[13])?;
    let f = bcd_byte(frame[14])?;
    let lba = (m * 60 + s) * 75 + f;
    lba.checked_sub(150)?.checked_sub(probe_rel as u32)
}

enum TrackHandle {
    Default {
        raw: *mut c_void,
        cdreader: ffi::rc_hash_cdreader_t,
    },
    Chd(Box<ChdCdTrackHandle>),
}

unsafe extern "C" fn cd_open_track(path: *const c_char, track: u32) -> *mut c_void {
    cd_open_track_iterator(path, track, std::ptr::null())
}

unsafe extern "C" fn cd_open_track_iterator(
    path: *const c_char,
    track: u32,
    iterator: *const ffi::rc_hash_iterator_t,
) -> *mut c_void {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(path_str) = CStr::from_ptr(path).to_str() else {
        return std::ptr::null_mut();
    };
    let fs_path = Path::new(path_str);
    if ext_lower(fs_path) == "chd" {
        return match ChdCdTrackHandle::open(fs_path, track) {
            Some(h) => Box::into_raw(Box::new(TrackHandle::Chd(Box::new(h)))).cast(),
            None => std::ptr::null_mut(),
        };
    }

    let mut default_cd = ffi::rc_hash_cdreader_t::default();
    ffi::rc_hash_get_default_cdreader(&mut default_cd);
    let raw = if !iterator.is_null() {
        if let Some(open_iter) = default_cd.open_track_iterator {
            open_iter(path, track, iterator)
        } else if let Some(open) = default_cd.open_track {
            open(path, track)
        } else {
            std::ptr::null_mut()
        }
    } else if let Some(open) = default_cd.open_track {
        open(path, track)
    } else {
        std::ptr::null_mut()
    };
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(TrackHandle::Default {
        raw,
        cdreader: default_cd,
    }))
    .cast()
}

unsafe extern "C" fn cd_read_sector(
    track_handle: *mut c_void,
    sector: u32,
    buffer: *mut c_void,
    requested_bytes: usize,
) -> usize {
    if track_handle.is_null() || buffer.is_null() || requested_bytes == 0 {
        return 0;
    }
    match &mut *(track_handle as *mut TrackHandle) {
        TrackHandle::Default { raw, cdreader } => cdreader
            .read_sector
            .map_or(0, |f| f(*raw, sector, buffer, requested_bytes)),
        TrackHandle::Chd(chd_track) => {
            let out = std::slice::from_raw_parts_mut(buffer.cast::<u8>(), requested_bytes);
            chd_track.read_sector_bytes(sector, out)
        }
    }
}

unsafe extern "C" fn cd_first_track_sector(track_handle: *mut c_void) -> u32 {
    if track_handle.is_null() {
        return 0;
    }
    match &mut *(track_handle as *mut TrackHandle) {
        TrackHandle::Default { raw, cdreader } => {
            cdreader.first_track_sector.map_or(0, |f| f(*raw))
        }
        TrackHandle::Chd(chd_track) => chd_track.first_sector_lba,
    }
}

unsafe extern "C" fn cd_close_track(track_handle: *mut c_void) {
    if track_handle.is_null() {
        return;
    }
    let handle = Box::from_raw(track_handle as *mut TrackHandle);
    if let TrackHandle::Default { raw, cdreader } = *handle {
        if let Some(close) = cdreader.close_track {
            close(raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::{DeflateEncoder, ZlibEncoder};
    use flate2::Compression;

    fn dir_record(name: &[u8], lba: u32, size: u32, dir: bool) -> Vec<u8> {
        let mut len = 33 + name.len();
        len += len % 2;
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[6..10].copy_from_slice(&lba.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[25] = if dir { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    fn build_iso(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut total_sectors = 24usize;
        for (_, data) in files {
            total_sectors += data.len().max(1).div_ceil(USER_DATA);
        }
        let mut img = vec![0u8; total_sectors * USER_DATA];
        let mut root = Vec::new();
        root.extend(dir_record(&[0], 18, USER_DATA as u32, true));
        root.extend(dir_record(&[1], 18, USER_DATA as u32, true));
        let mut next = 20u32;
        for (name, data) in files {
            let at = (next as usize) * USER_DATA;
            img[at..at + data.len()].copy_from_slice(data);
            root.extend(dir_record(
                format!("{name};1").as_bytes(),
                next,
                data.len() as u32,
                false,
            ));
            next += data.len().max(1).div_ceil(USER_DATA) as u32;
        }
        img[18 * USER_DATA..18 * USER_DATA + root.len()].copy_from_slice(&root);
        let pvd = &mut img[16 * USER_DATA..17 * USER_DATA];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        pvd[128..130].copy_from_slice(&(USER_DATA as u16).to_le_bytes());
        pvd[156..156 + 34].copy_from_slice(&dir_record(&[0], 18, USER_DATA as u32, true));
        let term = &mut img[17 * USER_DATA..18 * USER_DATA];
        term[0] = 255;
        term[1..6].copy_from_slice(b"CD001");
        img
    }

    fn to_bcd(n: u32) -> u8 {
        (((n / 10) % 10) << 4 | (n % 10)) as u8
    }

    fn to_raw_2352(cooked: &[u8], mode: u8, start_lba: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((cooked.len() / USER_DATA) * 2352);
        for (i, s) in cooked.chunks(USER_DATA).enumerate() {
            let mut raw = vec![0u8; 2352];
            raw[..12].copy_from_slice(&SYNC_PATTERN);
            let msf_frames = start_lba + (i as u32) + 150;
            raw[12] = to_bcd(msf_frames / (60 * 75));
            raw[13] = to_bcd((msf_frames / 75) % 60);
            raw[14] = to_bcd(msf_frames % 75);
            raw[15] = mode;
            let off = if mode == 2 { 24 } else { 16 };
            raw[off..off + s.len()].copy_from_slice(s);
            out.extend(raw);
        }
        out
    }

    fn make_ps2_iso() -> Vec<u8> {
        let cnf = b"BOOT2 = cdrom0:\\SLUS_202.28;1\r\nVER = 1.20\r\nVMODE = NTSC\r\n";
        let mut elf = vec![0u8; 4096];
        elf[0..4].copy_from_slice(b"\x7FELF");
        elf[4..12].copy_from_slice(b"PS2-TEST");
        build_iso(&[("SYSTEM.CNF", cnf), ("SLUS_202.28", &elf)])
    }

    fn make_psx_iso() -> Vec<u8> {
        let cnf = b"BOOT = cdrom:\\SLUS_007.48;1\r\nTCB = 4\r\nEVENT = 10\r\n";
        let mut exe = vec![0u8; 4096];
        exe[0..8].copy_from_slice(b"PS-X EXE");
        exe[28..32].copy_from_slice(&2048u32.to_le_bytes());
        build_iso(&[("SYSTEM.CNF", cnf), ("SLUS_007.48", &exe)])
    }

    fn make_gamecube_iso() -> Vec<u8> {
        // Partição mínima válida para `rc_hash_gamecube`:
        // - magia 0xC2339F3D em 0x1C..0x20
        // - dol_offset em 0x420..0x424 (0x3000)
        // - apploader sizes em 0x2440 + 0x14 (body=0x40, trailer=0x20)
        // - main.dol em 0x3000 com 1 segmento de código em 0x3100 de 0x200 bytes
        let mut gc = vec![0u8; 0x4000];
        gc[0..6].copy_from_slice(b"GALE01");
        gc[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
        gc[0x420..0x424].copy_from_slice(&0x3000u32.to_be_bytes());
        gc[0x2440 + 0x14..0x2440 + 0x18].copy_from_slice(&0x40u32.to_be_bytes());
        gc[0x2440 + 0x18..0x2440 + 0x1C].copy_from_slice(&0x20u32.to_be_bytes());
        gc[0x3000..0x3004].copy_from_slice(&0x3100u32.to_be_bytes());
        gc[0x3090..0x3094].copy_from_slice(&0x200u32.to_be_bytes());
        for (i, b) in gc[0x3100..0x3300].iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(3);
        }
        gc
    }

    #[test]
    fn iterator_struct_matches_rcheevos_abi() {
        let c_path = CString::new("test.iso").unwrap();
        let mut iter: ffi::rc_hash_iterator_t = unsafe { std::mem::zeroed() };
        unsafe {
            ffi::rc_hash_initialize_iterator(&mut iter, c_path.as_ptr(), std::ptr::null(), 0);
        }
        assert_eq!(iter.index, -1);
        assert!(iter.callbacks.filereader.open.is_some());
        assert!(iter.callbacks.cdreader.open_track_iterator.is_some());
        unsafe {
            ffi::rc_hash_destroy_iterator(&mut iter);
        }
    }

    #[test]
    fn hashes_ps2_iso_and_7z_and_cso_identically() {
        let dir = tempfile::tempdir().unwrap();
        let ps2_iso = make_ps2_iso();
        let iso_path = dir.path().join("Game.iso");
        std::fs::write(&iso_path, &ps2_iso).unwrap();

        let expected =
            hash_path_for_console(RC_CONSOLE_PLAYSTATION_2, &iso_path).expect("hash ps2 iso");
        assert_eq!(expected.len(), 32);

        // Mesmo disco dentro de `.7z`.
        let sz_path = dir.path().join("Game.7z");
        {
            let mut w = sevenz_rust2::ArchiveWriter::create(&sz_path).unwrap();
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Game.iso"),
                Some(io::Cursor::new(ps2_iso.clone())),
            )
            .unwrap();
            w.finish().unwrap();
        }
        assert_eq!(
            hash_path_for_console(RC_CONSOLE_PLAYSTATION_2, &sz_path).as_deref(),
            Some(expected.as_str())
        );

        // Mesmo disco comprimido em `.cso` (CISO v1 deflate, blocos de 2048).
        let cso_path = dir.path().join("Game.cso");
        {
            let num_blocks = ps2_iso.len() / USER_DATA;
            let mut offsets = Vec::with_capacity(num_blocks + 1);
            let mut payloads = Vec::new();
            let data_base = 24 + (num_blocks + 1) * 4;
            for chunk in ps2_iso.chunks_exact(USER_DATA) {
                offsets.push((data_base + payloads.len()) as u32);
                let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
                enc.write_all(chunk).unwrap();
                payloads.extend(enc.finish().unwrap());
            }
            offsets.push((data_base + payloads.len()) as u32);
            let mut cso = Vec::new();
            cso.extend_from_slice(b"CISO");
            cso.extend_from_slice(&24u32.to_le_bytes());
            cso.extend_from_slice(&(ps2_iso.len() as u64).to_le_bytes());
            cso.extend_from_slice(&(USER_DATA as u32).to_le_bytes());
            cso.push(1); // ver = 1
            cso.push(0); // align = 0
            cso.extend_from_slice(&[0, 0]);
            for off in offsets {
                cso.extend_from_slice(&off.to_le_bytes());
            }
            cso.extend_from_slice(&payloads);
            std::fs::write(&cso_path, &cso).unwrap();
        }
        assert_eq!(
            hash_path_for_console(RC_CONSOLE_PLAYSTATION_2, &cso_path).as_deref(),
            Some(expected.as_str())
        );

        // Mesmo disco em `.chd` DVD (`chdman createdvd`).
        let chd_path = dir.path().join("Game.chd");
        {
            use chd::write::{ChdWriteOptions, ChdWriter};
            let f = File::create(&chd_path).unwrap();
            let mut w = ChdWriter::new(f, ChdWriteOptions::dvd()).unwrap();
            w.write_all(&ps2_iso).unwrap();
            w.finish().unwrap();
        }
        assert_eq!(
            hash_path_for_console(RC_CONSOLE_PLAYSTATION_2, &chd_path).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn hashes_multitrack_psx_cue_inside_zip_and_chd_and_resolves_wrong_folder_console() {
        let dir = tempfile::tempdir().unwrap();
        let psx_cooked = make_psx_iso();
        let psx_raw = to_raw_2352(&psx_cooked, 2, 0);
        let audio_raw = vec![0u8; 2352 * 4];
        let cue_text = "FILE \"Track 1.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\nFILE \"Track 2.bin\" BINARY\r\n  TRACK 02 AUDIO\r\n    INDEX 01 00:00:00\r\n";

        let cue_path = dir.path().join("Game.cue");
        std::fs::write(dir.path().join("Track 1.bin"), &psx_raw).unwrap();
        std::fs::write(dir.path().join("Track 2.bin"), &audio_raw).unwrap();
        std::fs::write(&cue_path, cue_text.as_bytes()).unwrap();

        let expected =
            hash_path_for_console(RC_CONSOLE_PLAYSTATION, &cue_path).expect("hash psx cue");

        // `Silent Hill (v1.1).cue` numa pasta `ps2/`: `preferred_console = PS2`
        // falha na assinatura `BOOT2` e resolve automaticamente como `PS1`!
        assert_eq!(
            resolve_console_and_hash(Some(RC_CONSOLE_PLAYSTATION_2), &cue_path),
            Some((RC_CONSOLE_PLAYSTATION, expected.clone()))
        );

        // Multi-arquivo `.cue` + `.bin` dentro de `.zip`, mesmo quando `Track 1.bin`
        // aparece antes do `.cue` e `preferred_console` é `None` (`system_id == "disc"`).
        let zip_path = dir.path().join("Resident Evil 2.zip");
        {
            let f = File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zw.start_file("Track 1.bin", opts).unwrap();
            zw.write_all(&psx_raw).unwrap();
            zw.start_file("Track 2.bin", opts).unwrap();
            zw.write_all(&audio_raw).unwrap();
            zw.start_file("Game.cue", opts).unwrap();
            zw.write_all(cue_text.as_bytes()).unwrap();
            zw.finish().unwrap();
        }
        assert_eq!(
            resolve_console_and_hash(None, &zip_path),
            Some((RC_CONSOLE_PLAYSTATION, expected.clone()))
        );

        // Mesmo CD em `.chd` (frames de 2448 bytes com metadado `CHT2`).
        let chd_path = dir.path().join("Game.chd");
        {
            use chd::write::{ChdWriteOptions, ChdWriter};
            let f = File::create(&chd_path).unwrap();
            let mut w = ChdWriter::new(f, ChdWriteOptions::cd()).unwrap();
            let frames = psx_raw.len() / 2352;
            let meta = CString::new(format!(
                "TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:{frames} PAD:0 PREGAP:0 PGTYPE:MODE1 PGSUB:NONE POSTGAP:0"
            ))
            .unwrap();
            w.add_metadata(KnownMetadata::CdRomTrack2, meta.as_c_str(), true)
                .unwrap();
            for s in psx_raw.chunks_exact(2352) {
                let mut frame = vec![0u8; 2448];
                frame[..2352].copy_from_slice(s);
                w.write_all(&frame).unwrap();
            }
            for _ in frames..frames.next_multiple_of(4) {
                w.write_all(&[0u8; 2448]).unwrap();
            }
            w.finish().unwrap();
        }
        assert_eq!(
            resolve_console_and_hash(None, &chd_path),
            Some((RC_CONSOLE_PLAYSTATION, expected))
        );
    }

    #[test]
    fn hashes_gamecube_iso_gcm_gcz_and_resolves_generic_disc() {
        let dir = tempfile::tempdir().unwrap();
        let gc = make_gamecube_iso();
        let iso_path = dir.path().join("Smash.iso");
        let gcm_path = dir.path().join("Smash.gcm");
        std::fs::write(&iso_path, &gc).unwrap();
        std::fs::write(&gcm_path, &gc).unwrap();

        let expected =
            hash_path_for_console(RC_CONSOLE_GAMECUBE, &iso_path).expect("hash gamecube iso");
        assert_eq!(
            resolve_console_and_hash(None, &iso_path),
            Some((RC_CONSOLE_GAMECUBE, expected.clone()))
        );
        assert_eq!(
            resolve_console_and_hash(None, &gcm_path),
            Some((RC_CONSOLE_GAMECUBE, expected.clone()))
        );

        // Formato `.gcz` comprimido com zlib.
        let gcz_path = dir.path().join("Smash.gcz");
        {
            let block_size = 4096usize;
            let num_blocks = gc.len() / block_size;
            let mut offsets = Vec::with_capacity(num_blocks);
            let mut payloads = Vec::new();
            for chunk in gc.chunks_exact(block_size) {
                offsets.push(payloads.len() as u64);
                let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
                enc.write_all(chunk).unwrap();
                payloads.extend(enc.finish().unwrap());
            }
            let mut out = Vec::new();
            out.extend_from_slice(&GczReader::MAGIC.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes()); // GC
            out.extend_from_slice(&(payloads.len() as u64).to_le_bytes());
            out.extend_from_slice(&(gc.len() as u64).to_le_bytes());
            out.extend_from_slice(&(block_size as u32).to_le_bytes());
            out.extend_from_slice(&(num_blocks as u32).to_le_bytes());
            for off in offsets {
                out.extend_from_slice(&off.to_le_bytes());
            }
            for _ in 0..num_blocks {
                out.extend_from_slice(&0u32.to_le_bytes());
            }
            out.extend_from_slice(&payloads);
            std::fs::write(&gcz_path, &out).unwrap();
        }
        assert_eq!(
            resolve_console_and_hash(None, &gcz_path),
            Some((RC_CONSOLE_GAMECUBE, expected))
        );
    }

    #[test]
    fn distinguishes_saturn_from_segacd_when_probing_generic_disc() {
        let dir = tempfile::tempdir().unwrap();
        let mut saturn = vec![0u8; 16 * USER_DATA];
        saturn[..16].copy_from_slice(b"SEGA SEGASATURN ");
        let sat_path = dir.path().join("Saturn.iso");
        std::fs::write(&sat_path, &saturn).unwrap();

        let (cid, hash) = resolve_console_and_hash(None, &sat_path).expect("saturn resolved");
        assert_eq!(cid, RC_CONSOLE_SATURN);
        assert_eq!(hash.len(), 32);

        let mut segacd = vec![0u8; 16 * USER_DATA];
        segacd[..16].copy_from_slice(b"SEGADISCSYSTEM  ");
        let scd_path = dir.path().join("SegaCD.iso");
        std::fs::write(&scd_path, &segacd).unwrap();

        let (cid2, _) = resolve_console_and_hash(None, &scd_path).expect("segacd resolved");
        assert_eq!(cid2, RC_CONSOLE_SEGA_CD);
    }
}
