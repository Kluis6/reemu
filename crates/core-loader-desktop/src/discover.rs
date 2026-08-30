//! Descoberta de cores instalados: varre um diretório atrás de
//! `*_libretro.<suf>` e espia `retro_get_system_info` / `retro_api_version`
//! de cada um.
//!
//! É seguro fazer isso em sequência pra vários cores: a API libretro garante
//! que essas duas funções podem ser chamadas a qualquer momento, antes de
//! `retro_init`, e não tocam em estado global (a regra "um core por
//! processo" só vale a partir de `set_environment`/`init`).

use crate::raw::RawCore;
use crate::sys;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredCore {
    /// Nome do arquivo sem o sufixo de plataforma (ex: `snes9x_libretro.so`
    /// → `snes9x_libretro`). É o que `DesktopCoreLoader` resolve de volta
    /// pra um caminho.
    pub core_id: String,
    pub path: PathBuf,
    /// `retro_system_info.library_name` (ex: "Snes9x").
    pub library_name: String,
    /// `retro_system_info.library_version`.
    pub library_version: String,
    /// Extensões aceitas, sem ponto (ex: `["sfc", "smc", "fig"]`).
    pub valid_extensions: Vec<String>,
    pub api_version: u32,
}

fn dylib_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// Varre `dir` (não-recursivo). Diretório inexistente → lista vazia.
/// Arquivos que não abrem como core libretro são ignorados em silêncio.
pub fn discover_cores(dir: &Path) -> Vec<DiscoveredCore> {
    let suffix = dylib_suffix();
    let marker = format!("_libretro{suffix}");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<DiscoveredCore> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(&marker))
        })
        .filter_map(|p| peek(&p))
        .collect();
    out.sort_by(|a, b| a.core_id.cmp(&b.core_id));
    out
}

fn peek(path: &Path) -> Option<DiscoveredCore> {
    let raw = RawCore::open(path).ok()?;
    let api_version = unsafe { (raw.api_version)() };

    let mut info: sys::retro_system_info = unsafe { std::mem::zeroed() };
    unsafe { (raw.get_system_info)(&mut info) };

    let file = path.file_name()?.to_str()?;
    let core_id = file
        .strip_suffix(dylib_suffix())
        .unwrap_or(file)
        .to_string();

    Some(DiscoveredCore {
        core_id,
        path: path.to_path_buf(),
        library_name: cstr(info.library_name),
        library_version: cstr(info.library_version),
        valid_extensions: cstr(info.valid_extensions)
            .split('|')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        api_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dir_is_empty() {
        assert!(discover_cores(Path::new("/nao/existe/aqui")).is_empty());
    }

    #[cfg(feature = "test-fixtures")]
    #[test]
    fn finds_the_test_core() {
        let testcore = std::path::Path::new(crate::testcore_path());
        let dir = testcore.parent().unwrap();
        // O fixture é `libreemu_testcore.so` — não casa com `*_libretro.so`,
        // então copiamos pra um tmp com o nome certo.
        let tmp = std::env::temp_dir().join(format!("reemu-discover-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let dest = tmp.join(format!("fake_libretro{}", dylib_suffix()));
        std::fs::copy(testcore, &dest).unwrap();
        let _ = dir; // (só documentando de onde veio)

        let found = discover_cores(&tmp);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].core_id, "fake_libretro");
        assert!(found[0].api_version >= 1);

        std::fs::remove_dir_all(&tmp).ok();
    }
}
