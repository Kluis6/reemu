//! Verificação/importação de arquivos de sistema (BIOS) — ver
//! `domain::bios` pra tabela de quais cores precisam do quê e onde.
//! Puro I/O local: nunca baixa nada (BIOS é copyright da fabricante), só
//! confere `system_dir` e copia o que o usuário escolher no picker.

use domain::bios::{bios_files_for_system, BiosFile, KNOWN_SYSTEMS};
use md5::{Digest, Md5};
use std::io;
use std::path::{Path, PathBuf};

pub struct BiosStatus {
    pub system_id: String,
    pub filename: String,
    pub required: bool,
    pub note: String,
    pub present: bool,
    /// `Some(true/false)` = arquivo presente e MD5 conhecido conferido;
    /// `None` = ausente (nada pra conferir) ou sem MD5 documentado pra esse
    /// arquivo (arcade — cada jogo pede um BIOS diferente).
    pub hash_ok: Option<bool>,
}

fn path_for(system_dir: &Path, file: &BiosFile) -> PathBuf {
    match file.subfolder {
        Some(sub) => system_dir.join(sub).join(file.filename),
        None => system_dir.join(file.filename),
    }
}

fn md5_hex(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Md5::new();
    hasher.update(&bytes);
    Some(format!("{:x}", hasher.finalize()))
}

fn find_file(system_id: &str, filename: &str) -> io::Result<&'static BiosFile> {
    bios_files_for_system(system_id)
        .iter()
        .find(|f| f.filename == filename)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{system_id}/{filename} não é um BIOS conhecido"),
            )
        })
}

/// Confere todo `system_dir` contra a tabela de `domain::bios` — presença +
/// MD5 quando documentado. Não é caro (poucos arquivos, alguns MB no máximo
/// pros `.zip` de arcade) — chamado sob demanda, sem cache.
pub fn check_all(system_dir: &Path) -> Vec<BiosStatus> {
    let mut out = Vec::new();
    for &system_id in KNOWN_SYSTEMS {
        for file in bios_files_for_system(system_id) {
            let path = path_for(system_dir, file);
            let present = path.is_file();
            let hash_ok = present
                .then(|| {
                    file.md5.map(|want| {
                        md5_hex(&path).is_some_and(|got| got.eq_ignore_ascii_case(want))
                    })
                })
                .flatten();
            out.push(BiosStatus {
                system_id: system_id.to_string(),
                filename: file.filename.to_string(),
                required: file.required,
                note: file.note.to_string(),
                present,
                hash_ok,
            });
        }
    }
    out
}

/// Copia `src` pra `<system_dir>/[subfolder/]<filename esperado>` —
/// renomeia pro nome canônico independente de como o arquivo do usuário se
/// chamava (o core procura pelo nome exato).
pub fn import_bios_file(
    system_dir: &Path,
    system_id: &str,
    filename: &str,
    src: &Path,
) -> io::Result<()> {
    let file = find_file(system_id, filename)?;
    let dst = path_for(system_dir, file);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, &dst)?;
    Ok(())
}

/// Remove um BIOS já importado (corrigir um import errado sem caçar o
/// arquivo na mão). Idempotente — `Ok(())` mesmo se já não existir.
pub fn remove_bios_file(system_dir: &Path, system_id: &str, filename: &str) -> io::Result<()> {
    let file = find_file(system_id, filename)?;
    let path = path_for(system_dir, file);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "reemu-bios-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn missing_files_report_absent() {
        let dir = tmp();
        let status = check_all(&dir);
        assert!(!status.is_empty());
        assert!(status.iter().all(|s| !s.present && s.hash_ok.is_none()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn import_then_check_reports_present_and_hash() {
        let dir = tmp();
        let src = dir.join("meu_arquivo_qualquer.bin");
        std::fs::write(&src, b"not the real saturn bios").unwrap();

        import_bios_file(&dir, "saturn", "saturn_bios.bin", &src).unwrap();
        assert!(
            dir.join("kronos/saturn_bios.bin").is_file(),
            "renomeou + subpasta"
        );

        let status = check_all(&dir);
        let saturn = status
            .iter()
            .find(|s| s.system_id == "saturn" && s.filename == "saturn_bios.bin")
            .unwrap();
        assert!(saturn.present);
        assert_eq!(
            saturn.hash_ok,
            Some(false),
            "não é o BIOS real, hash não bate"
        );

        remove_bios_file(&dir, "saturn", "saturn_bios.bin").unwrap();
        assert!(!dir.join("kronos/saturn_bios.bin").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn import_unknown_bios_errors() {
        let dir = tmp();
        let src = dir.join("x.bin");
        std::fs::write(&src, b"x").unwrap();
        assert!(import_bios_file(&dir, "nes", "whatever.bin", &src).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
