//! Verificação/importação de arquivos de sistema (BIOS) — ver
//! `domain::bios` pra tabela de quais cores precisam do quê e onde.
//! Puro I/O local: nunca baixa nada (BIOS é copyright da fabricante), só
//! confere `system_dir` e copia o que o usuário escolher no picker.

use domain::bios::{bios_files_for_system, known_systems, BiosFile};
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
    /// Entrada "qualquer um destes": a pasta onde basta um dump (a UI mostra
    /// ela no lugar do `filename`, que aí é só a chave `"*"`).
    pub any_of_folder: Option<String>,
}

fn dir_for(system_dir: &Path, file: &BiosFile) -> PathBuf {
    match file.subfolder {
        Some(sub) => system_dir.join(sub),
        None => system_dir.to_path_buf(),
    }
}

fn path_for(system_dir: &Path, file: &BiosFile) -> PathBuf {
    dir_for(system_dir, file).join(file.filename)
}

fn md5_hex(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Md5::new();
    hasher.update(&bytes);
    Some(format!("{:x}", hasher.finalize()))
}

/// Arquivos soltos na pasta de uma entrada "qualquer um destes".
fn files_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default()
}

fn find_file(system_id: &str, filename: &str) -> io::Result<BiosFile> {
    bios_files_for_system(system_id)
        .into_iter()
        .find(|f| f.filename == filename)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{system_id}/{filename} não é um BIOS conhecido"),
            )
        })
}

fn status_of(system_dir: &Path, file: &BiosFile) -> (bool, Option<bool>) {
    if !file.any_of.is_empty() {
        // basta um dump na pasta; hash ok se algum for um dos conhecidos
        let files = files_in(&dir_for(system_dir, file));
        if files.is_empty() {
            return (false, None);
        }
        let ok = files.iter().any(|p| {
            md5_hex(p).is_some_and(|got| {
                file.any_of
                    .iter()
                    .any(|(_, want)| got.eq_ignore_ascii_case(want))
            })
        });
        return (true, Some(ok));
    }
    let path = path_for(system_dir, file);
    let present = path.is_file();
    let hash_ok = (present && !file.md5.is_empty()).then(|| {
        md5_hex(&path)
            .is_some_and(|got| file.md5.iter().any(|want| got.eq_ignore_ascii_case(want)))
    });
    (present, hash_ok)
}

/// Confere todo `system_dir` contra a tabela de `domain::bios` — presença +
/// MD5 quando documentado. Chamado sob demanda, sem cache: são arquivos
/// pequenos (o maior, o BIOS do PS2, tem 4 MB).
pub fn check_all(system_dir: &Path) -> Vec<BiosStatus> {
    let mut out = Vec::new();
    for system_id in known_systems() {
        for file in bios_files_for_system(system_id) {
            let (present, hash_ok) = status_of(system_dir, &file);
            out.push(BiosStatus {
                system_id: system_id.to_string(),
                filename: file.filename.to_string(),
                required: file.required,
                note: file.note.to_string(),
                present,
                hash_ok,
                any_of_folder: (!file.any_of.is_empty())
                    .then(|| file.subfolder.unwrap_or_default().to_string()),
            });
        }
    }
    out
}

/// Copia `src` pra `<system_dir>/[subfolder/]<filename esperado>` —
/// renomeia pro nome canônico independente de como o arquivo do usuário se
/// chamava (o core procura pelo nome exato). Entrada "qualquer um destes":
/// o dump é reconhecido pelo MD5 e ganha o nome oficial dele; um dump que
/// não está na lista mantém o nome original (o core aceita qualquer BIOS
/// válido na pasta).
pub fn import_bios_file(
    system_dir: &Path,
    system_id: &str,
    filename: &str,
    src: &Path,
) -> io::Result<()> {
    let file = find_file(system_id, filename)?;
    let dst = if file.any_of.is_empty() {
        path_for(system_dir, &file)
    } else {
        let got = md5_hex(src).unwrap_or_default();
        let name = file
            .any_of
            .iter()
            .find(|(_, want)| got.eq_ignore_ascii_case(want))
            .map(|(n, _)| std::ffi::OsString::from(n))
            .or_else(|| src.file_name().map(|n| n.to_os_string()))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "arquivo sem nome"))?;
        dir_for(system_dir, &file).join(name)
    };
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, &dst)?;
    Ok(())
}

/// Remove um BIOS já importado (corrigir um import errado sem caçar o
/// arquivo na mão). Entrada "qualquer um destes": limpa os arquivos da
/// pasta, que é só de BIOS. Idempotente — `Ok(())` mesmo se já não existir.
pub fn remove_bios_file(system_dir: &Path, system_id: &str, filename: &str) -> io::Result<()> {
    let file = find_file(system_id, filename)?;
    let paths = if file.any_of.is_empty() {
        vec![path_for(system_dir, &file)]
    } else {
        files_in(&dir_for(system_dir, &file))
    };
    for path in paths {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
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

    #[test]
    fn ps2_any_of_entry_accepts_any_dump_and_removes_all() {
        let dir = tmp();
        let src = dir.join("SCPH-70012.bin");
        std::fs::write(&src, b"um dump que nao esta na lista").unwrap();

        // MD5 desconhecido: mantém o nome original na pasta do core
        import_bios_file(&dir, "ps2", "*", &src).unwrap();
        assert!(dir.join("pcsx2/bios/SCPH-70012.bin").is_file());
        let ps2 = check_all(&dir)
            .into_iter()
            .find(|s| s.system_id == "ps2" && s.filename == "*")
            .unwrap();
        assert!(ps2.present && ps2.required);
        assert_eq!(ps2.hash_ok, Some(false));

        remove_bios_file(&dir, "ps2", "*").unwrap();
        assert!(super::files_in(&dir.join("pcsx2/bios")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
