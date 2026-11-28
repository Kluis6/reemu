//! Verificação/importação de arquivos de sistema (BIOS) — ver
//! `domain::bios` pra tabela de quais cores precisam do quê e onde.
//! Puro I/O local: nunca baixa nada (BIOS é copyright da fabricante), só
//! confere `system_dir` e copia o que o usuário escolher no picker.

use domain::bios::{bios_files_for_system, known_systems, BiosFile};
use md5::{Digest, Md5};
use std::collections::{BTreeSet, HashMap};
use std::io::{self, Read, Write};
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

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct BiosScanResult {
    pub imported: usize,
    pub skipped: usize,
    pub unreadable: usize,
    pub review: Vec<String>,
}

// Cobre os firmwares conhecidos de hash fixo (incluindo PS2 e 64DD).
// Evita ler imagens grandes quando a pasta contém também jogos.
const BIOS_SCAN_MAX_BYTES: u64 = 16 * 1024 * 1024;

fn read_scan_candidate(path: &Path, data: &mut Vec<u8>) -> io::Result<bool> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > BIOS_SCAN_MAX_BYTES {
        return Ok(false);
    }
    data.clear();
    file.take(BIOS_SCAN_MAX_BYTES + 1).read_to_end(data)?;
    Ok(!data.is_empty() && data.len() as u64 <= BIOS_SCAN_MAX_BYTES)
}

/// Procura BIOS na pasta selecionada e copia somente as correspondências
/// por MD5 para os caminhos publicados no catálogo.
pub fn scan_bios_folder(system_dir: &Path, folder: &Path) -> io::Result<BiosScanResult> {
    scan_bios(
        system_dir,
        folder,
        known_systems().into_iter().flat_map(bios_files_for_system),
    )
}

fn scan_bios(
    system_dir: &Path,
    folder: &Path,
    bios_files: impl IntoIterator<Item = BiosFile>,
) -> io::Result<BiosScanResult> {
    if !std::fs::metadata(folder)?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Selecione uma pasta de BIOS",
        ));
    }

    let mut by_hash: HashMap<&str, Vec<PathBuf>> = HashMap::new();
    let mut known_names = BTreeSet::new();
    for entry in bios_files {
        let dir = dir_for(system_dir, &entry);
        for (name, hash) in entry.any_of.iter().copied() {
            known_names.insert(name.to_ascii_lowercase());
            by_hash.entry(hash).or_default().push(dir.join(name));
        }
        if entry.any_of.is_empty() {
            let leaf = Path::new(entry.filename)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(entry.filename);
            known_names.insert(leaf.to_ascii_lowercase());
            for hash in entry.md5 {
                by_hash
                    .entry(hash)
                    .or_default()
                    .push(dir.join(entry.filename));
            }
        }
    }

    let mut result = BiosScanResult::default();
    let mut review = BTreeSet::new();
    let mut candidates = HashMap::new();
    let mut data = Vec::new();
    for entry in walkdir::WalkDir::new(folder).sort_by_file_name() {
        let entry = match entry {
            Ok(entry) if entry.file_type().is_file() => entry,
            Ok(_) => continue,
            Err(_) => {
                result.unreadable += 1;
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy();
        let loaded = match read_scan_candidate(entry.path(), &mut data) {
            Ok(loaded) => loaded,
            Err(_) => {
                result.unreadable += 1;
                continue;
            }
        };
        let hash = loaded.then(|| format!("{:x}", Md5::digest(&data)));
        let Some((&hash, targets)) = hash.as_deref().and_then(|h| by_hash.get_key_value(h)) else {
            if known_names.contains(&name.to_ascii_lowercase()) {
                review.insert(name.into_owned());
            }
            continue;
        };
        let same_name = |p: &Path| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(&name))
        };
        for target in targets {
            // Nomes alternativos com o mesmo hash podem atender cores diferentes.
            // Só prioriza o nome de origem se houver conflito entre arquivo e pasta.
            if !same_name(target)
                && targets
                    .iter()
                    .any(|p| same_name(p) && (p.starts_with(target) || target.starts_with(p)))
            {
                continue;
            }
            candidates
                .entry(target)
                .or_insert_with(|| (entry.path().to_path_buf(), hash));
        }
    }

    // SGB1.sfc e SGB2.sfc podem ser arquivos ou pastas em cores diferentes.
    // Quando ambos aparecem como candidatos, dá precedência ao arquivo.
    let mut candidates: Vec<_> = candidates.into_iter().collect();
    candidates.sort_by(|a, b| a.0.cmp(b.0));
    for (dst, (src, hash)) in candidates {
        let copied = (|| -> io::Result<bool> {
            if dst.symlink_metadata().is_ok() {
                return Ok(false);
            }
            if !read_scan_candidate(&src, &mut data)? || format!("{:x}", Md5::digest(&data)) != hash
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Arquivo mudou durante a busca de BIOS",
                ));
            }
            let parent = dst.parent().expect("destino de BIOS tem pasta pai");
            std::fs::create_dir_all(parent)?;
            let mut staged = tempfile::NamedTempFile::new_in(parent)?;
            staged.write_all(&data)?;
            staged.persist_noclobber(dst).map_err(|e| e.error)?;
            Ok(true)
        })();
        match copied {
            Ok(true) => result.imported += 1,
            Ok(false) => result.skipped += 1,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::NotADirectory
                ) =>
            {
                result.skipped += 1;
            }
            Err(_) => result.unreadable += 1,
        }
    }
    result.review = review.into_iter().collect();
    Ok(result)
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
        md5_hex(&path).is_some_and(|got| file.md5.iter().any(|want| got.eq_ignore_ascii_case(want)))
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
            let alternatives: Option<(&[&str], u64)> = match (system_id, file.filename) {
                ("x68000", "iplrom.dat") => Some((
                    &domain::bios::X68000_IPL_NAMES[1..],
                    domain::bios::X68000_IPL_SIZE,
                )),
                ("x68000", "cgrom.dat") => Some((
                    &domain::bios::X68000_FONT_NAMES[1..],
                    domain::bios::X68000_FONT_SIZE,
                )),
                _ => None,
            };
            let required = file.required
                && !alternatives.is_some_and(|(names, size)| {
                    let dir = dir_for(system_dir, &file);
                    names.iter().any(|name| {
                        std::fs::metadata(dir.join(name))
                            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == size)
                    })
                });
            out.push(BiosStatus {
                system_id: system_id.to_string(),
                filename: file.filename.to_string(),
                required,
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

    #[test]
    fn scan_fills_nonconflicting_firmware_names_for_different_cores() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let system = temp.path().join("system");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("bios.bin"), b"abc").unwrap();
        let bios = BiosFile {
            filename: "bios.bin",
            subfolder: None,
            md5: &["900150983cd24fb0d6963f7d28e17f72"],
            required: false,
            note: "",
            any_of: &[],
        };
        let result = scan_bios(
            &system,
            &source,
            [
                bios,
                BiosFile {
                    filename: "other.sfc",
                    ..bios
                },
                BiosFile {
                    filename: "program.rom",
                    subfolder: Some("bios.bin"),
                    ..bios
                },
            ],
        )
        .unwrap();
        assert_eq!(
            (result.imported, result.skipped, result.unreadable),
            (2, 0, 0)
        );
        for name in ["bios.bin", "other.sfc"] {
            assert_eq!(std::fs::read(system.join(name)).unwrap(), b"abc");
        }
    }

    #[test]
    fn scan_matches_renamed_files_marks_known_names_for_review_and_preserves_targets() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let system = temp.path().join("system");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        std::fs::write(source.join("nested/renamed.img"), b"abc").unwrap();
        std::fs::write(source.join("bios.zip"), b"not verified").unwrap();
        std::fs::write(source.join("wrong.bin"), b"incorrect hash").unwrap();
        std::fs::write(source.join("rename-again.bin"), b"hello").unwrap();
        std::fs::write(system.join("wrong.bin"), b"preserve this file").unwrap();

        let bios = BiosFile {
            filename: "expected.bin",
            subfolder: Some("core"),
            md5: &["900150983cd24fb0d6963f7d28e17f72"], // abc
            required: false,
            note: "",
            any_of: &[],
        };
        let catalog = [
            bios,
            BiosFile {
                filename: "bios.zip",
                md5: &[],
                ..bios
            },
            BiosFile {
                filename: "wrong.bin",
                subfolder: None,
                md5: &["5d41402abc4b2a76b9719d911017c592"], // hello
                ..bios
            },
        ];
        let result = scan_bios(&system, &source, catalog).unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.skipped, 1);
        assert_eq!(result.unreadable, 0);
        assert_eq!(result.review, ["bios.zip", "wrong.bin"]);
        assert_eq!(
            std::fs::read(system.join("core/expected.bin")).unwrap(),
            b"abc"
        );
        assert_eq!(
            std::fs::read(system.join("wrong.bin")).unwrap(),
            b"preserve this file"
        );

        let repeated = scan_bios(&system, &source, catalog).unwrap();
        assert_eq!((repeated.imported, repeated.skipped), (0, 2));
    }
}
