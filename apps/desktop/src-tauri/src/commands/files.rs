//! Navegador de arquivos dentro do app: a janela do sistema não funciona com controle.

use crate::app_error::AppError;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DirListing {
    /// Pasta listada; `null` = a lista de unidades e da pasta pessoal.
    pub path: Option<String>,
    /// Pasta de cima; `null` = volta à lista de unidades.
    pub parent: Option<String>,
    pub entries: Vec<DirEntry>,
}

fn entry(name: String, path: &Path, is_dir: bool) -> DirEntry {
    DirEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        is_dir,
    }
}

/// Pasta pessoal e unidades do sistema, onde a navegação começa.
fn roots() -> DirListing {
    let mut entries = Vec::new();
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    if let Some(home) = home.map(PathBuf::from).filter(|p| p.is_dir()) {
        let name = home.file_name().map_or_else(
            || home.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        entries.push(entry(name, &home, true));
    }
    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let path = Path::new(&drive);
        if path.is_dir() {
            entries.push(entry(drive.clone(), path, true));
        }
    }
    #[cfg(not(windows))]
    entries.push(entry("/".into(), Path::new("/"), true));
    DirListing {
        path: None,
        parent: None,
        entries,
    }
}

#[cfg(windows)]
fn is_hidden(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Pastas primeiro, em ordem alfabética. Arquivos só no modo de arquivo e, com
/// `extensions`, só os dessas extensões. Ocultos ficam de fora.
fn list_dir(dir: &Path, extensions: &[String], dirs_only: bool) -> std::io::Result<DirListing> {
    let mut entries = Vec::new();
    for item in std::fs::read_dir(dir)?.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        let path = item.path();
        // `metadata` segue atalhos: um link para pasta se comporta como pasta.
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if name.starts_with('.') || is_hidden(&meta) {
            continue;
        }
        let is_dir = meta.is_dir();
        let wanted = is_dir
            || (!dirs_only
                && (extensions.is_empty()
                    || path.extension().is_some_and(|e| {
                        let e = e.to_string_lossy();
                        extensions.iter().any(|x| x.eq_ignore_ascii_case(&e))
                    })));
        if wanted {
            entries.push(entry(name, &path, is_dir));
        }
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(DirListing {
        path: Some(dir.to_string_lossy().into_owned()),
        parent: dir.parent().map(|p| p.to_string_lossy().into_owned()),
        entries,
    })
}

/// Lista uma pasta, ou o ponto de partida quando `path` vem vazio.
#[tauri::command]
pub async fn browse_dir(
    path: Option<String>,
    extensions: Vec<String>,
    dirs_only: bool,
) -> Result<DirListing, AppError> {
    match path {
        None => Ok(roots()),
        Some(p) => list_dir(Path::new(&p), &extensions, dirs_only).map_err(AppError::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reemu-files-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("zeta")).unwrap();
        std::fs::create_dir_all(dir.join(".oculta")).unwrap();
        std::fs::write(dir.join("Alfa.PNG"), b"").unwrap();
        std::fs::write(dir.join("beta.txt"), b"").unwrap();
        dir
    }

    fn names(listing: &DirListing) -> Vec<&str> {
        listing.entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn pastas_primeiro_filtro_por_extensao_e_sem_ocultos() {
        let dir = fixture("filtro");
        let listing = list_dir(&dir, &["png".into()], false).unwrap();
        assert_eq!(names(&listing), ["zeta", "Alfa.PNG"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn modo_de_pasta_nao_lista_arquivos() {
        let dir = fixture("pastas");
        let listing = list_dir(&dir, &[], true).unwrap();
        assert_eq!(names(&listing), ["zeta"]);
        assert!(listing.parent.is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pasta_inexistente_vira_erro() {
        assert!(list_dir(Path::new("/reemu/nao/existe"), &[], false).is_err());
    }
}
