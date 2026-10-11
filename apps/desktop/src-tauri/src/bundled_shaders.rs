//! Shaders próprios do ReEmu (pasta `shaders/` do crate), embutidos no
//! binário: os que o pacote `slang-shaders` da libretro não tem — hoje o CAS
//! da AMD (`shaders/cas`). São gravados em `<dados>/shaders/reemu` no
//! startup, fora da pasta do pacote (uma atualização do pacote não os apaga),
//! e funcionam sem o pacote baixado. O compilador de presets lê arquivos
//! (`#include` relativo), por isso o disco e não só a memória.

use std::path::{Path, PathBuf};

/// `(caminho relativo, conteúdo)`.
const FILES: &[(&str, &str)] = &[
    ("cas/cas.slangp", include_str!("../shaders/cas/cas.slangp")),
    ("cas/cas.slang", include_str!("../shaders/cas/cas.slang")),
];

/// Pasta dos shaders próprios dentro de `<dados>/shaders`.
pub fn dir(shaders_dir: &Path) -> PathBuf {
    shaders_dir.join("reemu")
}

/// Grava os arquivos que faltam ou mudaram (o app pode ter sido atualizado).
/// Falha só registra: os presets embutidos ficam indisponíveis.
pub fn install(shaders_dir: &Path) {
    let root = dir(shaders_dir);
    for (rel, body) in FILES {
        let path = root.join(rel);
        if std::fs::read_to_string(&path).is_ok_and(|cur| cur == *body) {
            continue;
        }
        let r = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, body));
        if let Err(e) = r {
            log::warn!("shaders próprios: gravar {}: {e}", path.display());
        }
    }
}
