//! Interface VFS do libretro, v3 (`RETRO_ENVIRONMENT_GET_VFS_INTERFACE`):
//! o core faz E/S de arquivo e pasta pelo frontend. Implementada sobre
//! `std::fs`, seguindo a semântica documentada em `libretro.h`
//! (libretro-common) para cada função — ver os comentários de cada uma.
//!
//! Alguns cores só funcionam com ela: o Stella 8 só considera a ROM um
//! arquivo se `stat` da VFS disser que é (`FSNodeLIBRETRO::setFlags`, em
//! `src/os/libretro/FSNodeLIBRETRO.cxx`); sem VFS, nenhum jogo abria.
//!
//! As v4/v5 (stat de 64 bits, mtime, cópia) não entram: devolvemos a versão
//! 3 no `required_interface_version`, e o core que pedir mais recebe `false`.

use std::ffi::{c_char, c_int, c_uint, c_void, CStr, CString};
use std::fs::{File, OpenOptions, ReadDir};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// Versão da VFS que oferecemos.
pub(crate) const VFS_VERSION: u32 = 3;

const ACCESS_READ: c_uint = 1 << 0;
const ACCESS_WRITE: c_uint = 1 << 1;
const ACCESS_UPDATE_EXISTING: c_uint = 1 << 2;

const SEEK_START: c_int = 0;
const SEEK_CURRENT: c_int = 1;
const SEEK_END: c_int = 2;

const STAT_IS_VALID: c_int = 1 << 0;
const STAT_IS_DIRECTORY: c_int = 1 << 1;
const STAT_IS_CHARACTER_SPECIAL: c_int = 1 << 2;

/// `struct retro_vfs_interface_info`.
#[repr(C)]
pub(crate) struct VfsInterfaceInfo {
    pub required_interface_version: u32,
    pub iface: *const VfsInterface,
}

/// `struct retro_vfs_interface` até a v3, na ordem do cabeçalho.
#[repr(C)]
pub(crate) struct VfsInterface {
    get_path: unsafe extern "C" fn(*mut VfsFile) -> *const c_char,
    open: unsafe extern "C" fn(*const c_char, c_uint, c_uint) -> *mut VfsFile,
    close: unsafe extern "C" fn(*mut VfsFile) -> c_int,
    size: unsafe extern "C" fn(*mut VfsFile) -> i64,
    tell: unsafe extern "C" fn(*mut VfsFile) -> i64,
    seek: unsafe extern "C" fn(*mut VfsFile, i64, c_int) -> i64,
    read: unsafe extern "C" fn(*mut VfsFile, *mut c_void, u64) -> i64,
    write: unsafe extern "C" fn(*mut VfsFile, *const c_void, u64) -> i64,
    flush: unsafe extern "C" fn(*mut VfsFile) -> c_int,
    remove: unsafe extern "C" fn(*const c_char) -> c_int,
    rename: unsafe extern "C" fn(*const c_char, *const c_char) -> c_int,
    // v2
    truncate: unsafe extern "C" fn(*mut VfsFile, i64) -> i64,
    // v3
    stat: unsafe extern "C" fn(*const c_char, *mut i32) -> c_int,
    mkdir: unsafe extern "C" fn(*const c_char) -> c_int,
    opendir: unsafe extern "C" fn(*const c_char, bool) -> *mut VfsDir,
    readdir: unsafe extern "C" fn(*mut VfsDir) -> bool,
    dirent_get_name: unsafe extern "C" fn(*mut VfsDir) -> *const c_char,
    dirent_is_dir: unsafe extern "C" fn(*mut VfsDir) -> bool,
    closedir: unsafe extern "C" fn(*mut VfsDir) -> c_int,
}

pub(crate) static VFS: VfsInterface = VfsInterface {
    get_path,
    open,
    close,
    size,
    tell,
    seek,
    read,
    write,
    flush,
    remove,
    rename,
    truncate,
    stat,
    mkdir,
    opendir,
    readdir,
    dirent_get_name,
    dirent_is_dir,
    closedir,
};

/// Responde `RETRO_ENVIRONMENT_GET_VFS_INTERFACE`. `false` se o core pede
/// uma versão mais nova que a nossa ("the frontend must return false").
///
/// # Safety
/// `data` aponta pra um `retro_vfs_interface_info` válido (ou é nulo).
pub(crate) unsafe fn get_interface(data: *mut c_void) -> bool {
    let Some(info) = (data as *mut VfsInterfaceInfo).as_mut() else {
        return false;
    };
    if info.required_interface_version > VFS_VERSION {
        return false;
    }
    info.required_interface_version = VFS_VERSION;
    info.iface = &VFS;
    true
}

/// Handle de arquivo (`struct retro_vfs_file_handle`, opaco pro core).
pub(crate) struct VfsFile {
    file: File,
    path: CString,
}

/// Handle de pasta (`struct retro_vfs_dir_handle`).
pub(crate) struct VfsDir {
    entries: ReadDir,
    include_hidden: bool,
    name: Option<CString>,
    is_dir: bool,
}

unsafe fn path_of(p: *const c_char) -> Option<PathBuf> {
    if p.is_null() {
        return None;
    }
    let bytes = CStr::from_ptr(p).to_bytes();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
    // No Windows os caminhos do libretro são UTF-8.
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes).ok().map(PathBuf::from)
    }
}

/// "Returns the path that was used to open this file."
unsafe extern "C" fn get_path(stream: *mut VfsFile) -> *const c_char {
    stream
        .as_ref()
        .map_or(std::ptr::null(), |f| f.path.as_ptr())
}

/// Modos como na implementação de referência do libretro-common
/// (`vfs/vfs_implementation.c`, `retro_vfs_file_open_impl`):
/// - `READ` → `rb` (não cria);
/// - `WRITE` → `wb` (cria e zera);
/// - `READ_WRITE` → `w+b` (cria e zera, lê e escreve);
/// - `WRITE|UPDATE_EXISTING` e `READ_WRITE|UPDATE_EXISTING` → `r+b` (lê e
///   escreve, não cria nem zera) — o flycast abre o `.cue` assim;
/// - qualquer outra combinação falha.
///
/// Pasta → `NULL` ("this will return NULL if path names a directory").
unsafe extern "C" fn open(path: *const c_char, mode: c_uint, _hints: c_uint) -> *mut VfsFile {
    let Some(p) = path_of(path) else {
        return std::ptr::null_mut();
    };
    if p.is_dir() {
        return std::ptr::null_mut();
    }
    let mut opts = OpenOptions::new();
    const RW: c_uint = ACCESS_READ | ACCESS_WRITE;
    match mode {
        ACCESS_READ => opts.read(true),
        ACCESS_WRITE => opts.write(true).create(true).truncate(true),
        RW => opts.read(true).write(true).create(true).truncate(true),
        m if m == ACCESS_WRITE | ACCESS_UPDATE_EXISTING || m == RW | ACCESS_UPDATE_EXISTING => {
            opts.read(true).write(true)
        }
        _ => return std::ptr::null_mut(),
    };
    let result = opts.open(&p);
    log::debug!(
        "vfs open {} (modo {mode}): {}",
        p.display(),
        if result.is_ok() { "ok" } else { "falhou" }
    );
    match result {
        Ok(file) => Box::into_raw(Box::new(VfsFile {
            file,
            path: CStr::from_ptr(path).to_owned(),
        })),
        Err(_) => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn close(stream: *mut VfsFile) -> c_int {
    if stream.is_null() {
        return -1;
    }
    drop(Box::from_raw(stream));
    0
}

unsafe extern "C" fn size(stream: *mut VfsFile) -> i64 {
    stream
        .as_ref()
        .and_then(|f| f.file.metadata().ok())
        .map_or(-1, |m| m.len() as i64)
}

unsafe extern "C" fn tell(stream: *mut VfsFile) -> i64 {
    stream
        .as_mut()
        .and_then(|f| f.file.stream_position().ok())
        .map_or(-1, |p| p as i64)
}

/// 0 em sucesso, -1 em falha (o retorno é `int64_t`, mas não é a posição).
unsafe extern "C" fn seek(stream: *mut VfsFile, offset: i64, whence: c_int) -> i64 {
    let Some(f) = stream.as_mut() else { return -1 };
    let to = match whence {
        SEEK_START if offset >= 0 => SeekFrom::Start(offset as u64),
        SEEK_CURRENT => SeekFrom::Current(offset),
        SEEK_END => SeekFrom::End(offset),
        _ => return -1,
    };
    f.file.seek(to).map_or(-1, |_| 0)
}

/// Como `fread`: lê até `len` ou o fim do arquivo (curto só no fim).
unsafe extern "C" fn read(stream: *mut VfsFile, s: *mut c_void, len: u64) -> i64 {
    let Some(f) = stream.as_mut() else { return -1 };
    if len == 0 {
        return 0;
    }
    if s.is_null() {
        return -1;
    }
    let buf = std::slice::from_raw_parts_mut(s as *mut u8, len as usize);
    let mut done = 0;
    while done < buf.len() {
        match f.file.read(&mut buf[done..]) {
            Ok(0) => break,
            Ok(n) => done += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return -1,
        }
    }
    done as i64
}

unsafe extern "C" fn write(stream: *mut VfsFile, s: *const c_void, len: u64) -> i64 {
    let Some(f) = stream.as_mut() else { return -1 };
    if len == 0 {
        return 0;
    }
    if s.is_null() {
        return -1;
    }
    let buf = std::slice::from_raw_parts(s as *const u8, len as usize);
    f.file.write_all(buf).map_or(-1, |_| len as i64)
}

unsafe extern "C" fn flush(stream: *mut VfsFile) -> c_int {
    stream
        .as_mut()
        .map_or(-1, |f| f.file.flush().map_or(-1, |_| 0))
}

/// Apaga o arquivo; pasta vazia também sai (como o `remove()` do C, que o
/// `filestream_delete` do libretro-common usa).
unsafe extern "C" fn remove(path: *const c_char) -> c_int {
    let Some(p) = path_of(path) else { return -1 };
    let r = if p.is_dir() {
        std::fs::remove_dir(&p)
    } else {
        std::fs::remove_file(&p)
    };
    r.map_or(-1, |_| 0)
}

unsafe extern "C" fn rename(old: *const c_char, new: *const c_char) -> c_int {
    match (path_of(old), path_of(new)) {
        (Some(a), Some(b)) => std::fs::rename(a, b).map_or(-1, |_| 0),
        _ => -1,
    }
}

unsafe extern "C" fn truncate(stream: *mut VfsFile, length: i64) -> i64 {
    let Some(f) = stream.as_mut() else { return -1 };
    if length < 0 {
        return -1;
    }
    f.file.set_len(length as u64).map_or(-1, |_| 0)
}

/// Bits `RETRO_VFS_STAT_*`, ou 0 se o caminho não existe. Segue links.
unsafe extern "C" fn stat(path: *const c_char, size: *mut i32) -> c_int {
    let Some(meta) = path_of(path).and_then(|p| std::fs::metadata(p).ok()) else {
        return 0;
    };
    if let Some(out) = size.as_mut() {
        *out = i32::try_from(meta.len()).unwrap_or(i32::MAX);
    }
    let mut flags = STAT_IS_VALID;
    if meta.is_dir() {
        flags |= STAT_IS_DIRECTORY;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        if meta.file_type().is_char_device() {
            flags |= STAT_IS_CHARACTER_SPECIAL;
        }
    }
    flags
}

/// 0 se criou, -2 se já existe, -1 em outro erro (não recursivo).
unsafe extern "C" fn mkdir(dir: *const c_char) -> c_int {
    let Some(p) = path_of(dir) else { return -1 };
    match std::fs::create_dir(&p) {
        Ok(()) => 0,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => -2,
        Err(_) => -1,
    }
}

unsafe extern "C" fn opendir(dir: *const c_char, include_hidden: bool) -> *mut VfsDir {
    let Some(entries) = path_of(dir).and_then(|p| std::fs::read_dir(p).ok()) else {
        return std::ptr::null_mut();
    };
    Box::into_raw(Box::new(VfsDir {
        entries,
        include_hidden,
        name: None,
        is_dir: false,
    }))
}

/// Avança pra próxima entrada; `false` no fim. Oculto = nome começando com
/// `.` (convenção Unix), pulado sem `include_hidden`.
unsafe extern "C" fn readdir(dirstream: *mut VfsDir) -> bool {
    let Some(d) = dirstream.as_mut() else {
        return false;
    };
    for entry in d.entries.by_ref() {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        #[cfg(unix)]
        let bytes = {
            use std::os::unix::ffi::OsStrExt;
            name.as_bytes().to_vec()
        };
        #[cfg(not(unix))]
        let bytes = name.to_string_lossy().into_owned().into_bytes();
        if !d.include_hidden && bytes.first() == Some(&b'.') {
            continue;
        }
        let Ok(cname) = CString::new(bytes) else {
            continue;
        };
        // segue link: um link pra pasta conta como pasta
        d.is_dir = std::fs::metadata(entry.path()).is_ok_and(|m| m.is_dir());
        d.name = Some(cname);
        return true;
    }
    d.name = None;
    false
}

unsafe extern "C" fn dirent_get_name(dirstream: *mut VfsDir) -> *const c_char {
    dirstream
        .as_ref()
        .and_then(|d| d.name.as_ref())
        .map_or(std::ptr::null(), |n| n.as_ptr())
}

unsafe extern "C" fn dirent_is_dir(dirstream: *mut VfsDir) -> bool {
    dirstream
        .as_ref()
        .is_some_and(|d| d.name.is_some() && d.is_dir)
}

unsafe extern "C" fn closedir(dirstream: *mut VfsDir) -> c_int {
    if dirstream.is_null() {
        return -1;
    }
    drop(Box::from_raw(dirstream));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(p: &std::path::Path) -> CString {
        CString::new(p.to_str().unwrap()).unwrap()
    }

    #[test]
    fn version_negotiation() {
        let mut info = VfsInterfaceInfo {
            required_interface_version: 3,
            iface: std::ptr::null(),
        };
        assert!(unsafe { get_interface(&mut info as *mut _ as *mut c_void) });
        assert_eq!(info.required_interface_version, 3);
        assert!(!info.iface.is_null());
        let mut newer = VfsInterfaceInfo {
            required_interface_version: 5,
            iface: std::ptr::null(),
        };
        assert!(!unsafe { get_interface(&mut newer as *mut _ as *mut c_void) });
    }

    #[test]
    fn file_roundtrip_and_open_modes() {
        let dir = std::env::temp_dir().join(format!("reemu-vfs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.bin");
        let cp = c(&path);
        unsafe {
            // só leitura não cria
            assert!(open(cp.as_ptr(), ACCESS_READ, 0).is_null());
            // escrita + UPDATE_EXISTING também não
            assert!(open(cp.as_ptr(), ACCESS_WRITE | ACCESS_UPDATE_EXISTING, 0).is_null());
            // escrita simples cria
            let f = open(cp.as_ptr(), ACCESS_WRITE, 0);
            assert!(!f.is_null());
            assert_eq!(write(f, b"hello world".as_ptr().cast(), 11), 11);
            assert_eq!(CStr::from_ptr(get_path(f)), cp.as_c_str());
            assert_eq!(close(f), 0);

            let f = open(cp.as_ptr(), ACCESS_READ, 0);
            assert_eq!(size(f), 11);
            assert_eq!(seek(f, 6, SEEK_START), 0);
            assert_eq!(tell(f), 6);
            let mut buf = [0u8; 32];
            assert_eq!(read(f, buf.as_mut_ptr().cast(), 32), 5); // curto no fim
            assert_eq!(&buf[..5], b"world");
            assert_eq!(seek(f, -5, SEEK_END), 0);
            assert_eq!(tell(f), 6);
            assert_eq!(close(f), 0);

            // WRITE|UPDATE_EXISTING = r+b: lê também, sem zerar
            let f = open(cp.as_ptr(), ACCESS_WRITE | ACCESS_UPDATE_EXISTING, 0);
            let mut two = [0u8; 2];
            assert_eq!(read(f, two.as_mut_ptr().cast(), 2), 2);
            assert_eq!(&two, b"he");
            assert_eq!(close(f), 0);
            // combinação fora da tabela falha
            assert!(open(cp.as_ptr(), ACCESS_READ | ACCESS_UPDATE_EXISTING, 0).is_null());

            // UPDATE_EXISTING não zera
            let f = open(cp.as_ptr(), ACCESS_READ_WRITE_UPDATE, 0);
            assert!(!f.is_null());
            assert_eq!(size(f), 11);
            assert_eq!(truncate(f, 5), 0);
            assert_eq!(size(f), 5);
            assert_eq!(close(f), 0);

            // escrita sem UPDATE_EXISTING zera
            let f = open(cp.as_ptr(), ACCESS_WRITE, 0);
            assert_eq!(size(f), 0);
            assert_eq!(close(f), 0);

            // pasta não abre como arquivo
            assert!(open(c(&dir).as_ptr(), ACCESS_READ, 0).is_null());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    const ACCESS_READ_WRITE_UPDATE: c_uint = ACCESS_READ | ACCESS_WRITE | ACCESS_UPDATE_EXISTING;

    #[test]
    fn stat_mkdir_and_listing() {
        let dir = std::env::temp_dir().join(format!("reemu-vfs-d-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            let cd = c(&dir);
            assert_eq!(stat(cd.as_ptr(), std::ptr::null_mut()), 0);
            assert_eq!(mkdir(cd.as_ptr()), 0);
            assert_eq!(mkdir(cd.as_ptr()), -2);
            let st = stat(cd.as_ptr(), std::ptr::null_mut());
            assert_eq!(st, STAT_IS_VALID | STAT_IS_DIRECTORY);

            std::fs::write(dir.join("rom.a26"), [0u8; 4096]).unwrap();
            std::fs::write(dir.join(".oculto"), b"x").unwrap();
            std::fs::create_dir(dir.join("sub")).unwrap();
            let mut sz = 0i32;
            let st = stat(c(&dir.join("rom.a26")).as_ptr(), &mut sz);
            assert_eq!((st, sz), (STAT_IS_VALID, 4096));
            #[cfg(unix)]
            assert_ne!(
                stat(c"/dev/null".as_ptr(), std::ptr::null_mut()) & STAT_IS_CHARACTER_SPECIAL,
                0
            );

            let list = |hidden: bool| {
                let d = opendir(cd.as_ptr(), hidden);
                assert!(!d.is_null());
                let mut out = Vec::new();
                while readdir(d) {
                    let n = CStr::from_ptr(dirent_get_name(d))
                        .to_str()
                        .unwrap()
                        .to_string();
                    out.push((n, dirent_is_dir(d)));
                }
                assert!(dirent_get_name(d).is_null());
                assert_eq!(closedir(d), 0);
                out.sort();
                out
            };
            assert_eq!(
                list(false),
                vec![("rom.a26".to_string(), false), ("sub".to_string(), true)]
            );
            assert_eq!(list(true).len(), 3);

            assert_eq!(remove(c(&dir.join(".oculto")).as_ptr()), 0);
            assert_eq!(
                rename(
                    c(&dir.join("rom.a26")).as_ptr(),
                    c(&dir.join("b.a26")).as_ptr()
                ),
                0
            );
            assert_eq!(
                stat(c(&dir.join("rom.a26")).as_ptr(), std::ptr::null_mut()),
                0
            );
            assert!(opendir(c(&dir.join("nada")).as_ptr(), false).is_null());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
