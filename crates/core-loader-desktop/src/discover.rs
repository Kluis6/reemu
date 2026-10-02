//! Descoberta de cores instalados: varre um diretório atrás de
//! `*_libretro.<suf>` e espia `retro_get_system_info` / `retro_api_version`
//! de cada um.
//!
//! É seguro fazer isso em sequência pra vários cores: a API libretro garante
//! que essas duas funções podem ser chamadas a qualquer momento, antes de
//! `retro_init`, e não tocam em estado global (a regra "um core por
//! processo" só vale a partir de `set_environment`/`init`).
//!
//! Mas espiar = carregar a DLL no processo do app, e cada carga roda a
//! inicialização dela. No Windows várias reservam índices de TLS e não
//! devolvem no `FreeLibrary`; a lista de cores era refeita a cada
//! atualização da tela, recarregando todas, e o app caiu com "fatal runtime
//! error: out of TLS indexes" instalando cores (2026-10-02). Por isso:
//! - [`installed_core_ids`] só olha os nomes dos arquivos;
//! - [`discover_cores`] guarda o que espiou por (caminho, tamanho, data), e
//!   cada DLL é carregada uma vez por execução (de novo só se o arquivo
//!   mudar).

use crate::raw::RawCore;
use crate::sys;
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

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

/// Arquivos `*_libretro.<suf>` em `dir` (não-recursivo).
fn core_files(dir: &Path) -> Vec<PathBuf> {
    let marker = format!("_libretro{}", dylib_suffix());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(&marker))
        })
        .collect()
}

fn core_id_of(path: &Path) -> Option<String> {
    let file = path.file_name()?.to_str()?;
    Some(
        file.strip_suffix(dylib_suffix())
            .unwrap_or(file)
            .to_string(),
    )
}

/// Ids dos cores instalados em `dir`, só pelo nome do arquivo — sem carregar
/// nenhuma DLL (ver o cabeçalho do módulo). Ordenado.
pub fn installed_core_ids(dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = core_files(dir)
        .iter()
        .filter_map(|p| core_id_of(p))
        .collect();
    ids.sort();
    ids
}

type PeekKey = (u64, Option<SystemTime>);
type PeekCache = Mutex<HashMap<PathBuf, (PeekKey, Option<DiscoveredCore>)>>;

/// O que já foi espiado: caminho → (tamanho, data, resultado).
fn peek_cache() -> &'static PeekCache {
    static CACHE: std::sync::OnceLock<PeekCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `peek` com cache por (caminho, tamanho, data de modificação).
fn peek_cached(path: &Path) -> Option<DiscoveredCore> {
    let meta = std::fs::metadata(path).ok()?;
    let key: PeekKey = (meta.len(), meta.modified().ok());
    let mut cache = peek_cache().lock().unwrap_or_else(|p| p.into_inner());
    if let Some((k, found)) = cache.get(path) {
        if *k == key {
            return found.clone();
        }
    }
    let found = peek(path);
    cache.insert(path.to_path_buf(), (key, found.clone()));
    found
}

/// Varre `dir` (não-recursivo). Diretório inexistente → lista vazia.
/// Arquivos que não abrem como core libretro são ignorados em silêncio.
/// Cada DLL é espiada uma vez por execução (ver o cabeçalho do módulo).
pub fn discover_cores(dir: &Path) -> Vec<DiscoveredCore> {
    let mut out: Vec<DiscoveredCore> = core_files(dir)
        .iter()
        .filter_map(|p| peek_cached(p))
        .collect();
    out.sort_by(|a, b| a.core_id.cmp(&b.core_id));
    out
}

fn peek(path: &Path) -> Option<DiscoveredCore> {
    let raw = RawCore::open(path).ok()?;
    let api_version = unsafe { (raw.api_version)() };

    let mut info: sys::retro_system_info = unsafe { std::mem::zeroed() };
    unsafe { (raw.get_system_info)(&mut info) };

    let core_id = core_id_of(path)?;

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

    /// Os ids saem dos nomes dos arquivos, sem carregar nada: um arquivo que
    /// nem é DLL entra na lista de ids e só some no `discover_cores`.
    #[test]
    fn installed_ids_come_from_file_names_only() {
        let tmp = std::env::temp_dir().join(format!("reemu-ids-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        for name in ["b_libretro", "a_libretro", "outro"] {
            std::fs::write(tmp.join(format!("{name}{}", dylib_suffix())), b"nao e dll").unwrap();
        }
        assert_eq!(installed_core_ids(&tmp), ["a_libretro", "b_libretro"]);
        assert!(discover_cores(&tmp).is_empty());
        std::fs::remove_dir_all(&tmp).ok();
    }

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
