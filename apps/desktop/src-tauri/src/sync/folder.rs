//! Empacotamento determinístico e extração segura de saves em pasta (D171-D173).
//!
//! Emuladores como PPSSPP (`PSP/SAVEDATA/<SLOT>`) e cores de Nintendo DS
//! (`melonDS DS/<PASTA>`, `DeSmuME/<PASTA>`) gravam cada save como um diretório
//! com vários arquivos internos (`PARAM.SFO`, `DATA.BIN`, ícones). Para subir
//! na API de Cloud Save como `kind = "file"` sem falsos conflitos entre
//! máquinas, cada pasta vira um `.zip` determinístico em memória.

use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Sufixo reservado aos temporários do motor de sync.
pub const TMP_SUFFIX: &str = ".sync-tmp";

/// Limite máximo de arquivos regulares dentro de uma pasta de save.
pub const MAX_FOLDER_FILES: usize = 512;

/// Limite máximo descomprimido (64 MiB, alinhado ao teto de `SavePlatform::Other`).
pub const MAX_FOLDER_BYTES: u64 = 64 * 1024 * 1024;

/// Profundidade máxima de subpastas relativas dentro de um slot.
pub const MAX_FOLDER_DEPTH: usize = 16;

fn invalid_data(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

fn invalid_input(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

fn zip_io(err: zip::result::ZipError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("zip inválido: {err}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Valida o caminho relativo (separado por `/`) de um arquivo dentro da pasta
/// de save, bloqueando componentes vazios, `.`, `..`, `:`, `\` e `.sync-tmp`.
fn validate_rel_parts(raw: &str) -> io::Result<Vec<&str>> {
    if raw.is_empty() || raw.starts_with('/') || raw.contains(['\\', ':', '\0']) {
        return Err(invalid_data("caminho relativo inválido no pacote de save"));
    }
    let parts: Vec<&str> = raw.split('/').collect();
    if parts.is_empty() || parts.len() > MAX_FOLDER_DEPTH {
        return Err(invalid_data(
            "profundidade inválida no pacote de save em pasta",
        ));
    }
    for part in &parts {
        if part.is_empty() || *part == "." || *part == ".." || part.ends_with(TMP_SUFFIX) {
            return Err(invalid_data(
                "componente proibido no pacote de save em pasta",
            ));
        }
    }
    Ok(parts)
}

/// Retorna `true` se `dir` existe e contém ao menos um arquivo regular (ignorando
/// `.sync-tmp`). Pasta inexistente devolve `Ok(false)`.
pub fn has_regular_files(dir: &Path) -> io::Result<bool> {
    fn search(cur: &Path, depth: usize) -> io::Result<bool> {
        if depth > MAX_FOLDER_DEPTH {
            return Err(invalid_data("pasta de save excede a profundidade máxima"));
        }
        let entries = match std::fs::read_dir(cur) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(TMP_SUFFIX) {
                continue;
            }
            let ft = entry.file_type()?;
            if ft.is_symlink() {
                return Err(invalid_input(
                    "link simbólico não permitido na pasta de save",
                ));
            }
            if ft.is_file() {
                return Ok(true);
            }
            if ft.is_dir() && search(&entry.path(), depth + 1)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    match std::fs::symlink_metadata(dir) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(invalid_input("pasta de save não pode ser link simbólico"));
            }
            if !meta.is_dir() {
                return Ok(false);
            }
            search(dir, 1)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

struct CollectedFile {
    rel: String,
    path: PathBuf,
    mtime: i64,
}

fn collect_files(
    root: &Path,
    cur: &Path,
    depth: usize,
    total_bytes: &mut u64,
    out: &mut Vec<CollectedFile>,
) -> io::Result<()> {
    if depth > MAX_FOLDER_DEPTH {
        return Err(invalid_data("pasta de save excede a profundidade máxima"));
    }
    let mut entries: Vec<std::fs::DirEntry> = Vec::new();
    for entry in std::fs::read_dir(cur)? {
        entries.push(entry?);
    }
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let file_name = entry.file_name();
        let Some(name_str) = file_name.to_str() else {
            return Err(invalid_data("nome de arquivo não UTF-8 na pasta de save"));
        };
        if name_str.ends_with(TMP_SUFFIX) {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            return Err(invalid_input(
                "link simbólico não permitido na pasta de save",
            ));
        }
        let path = entry.path();
        if ft.is_dir() {
            collect_files(root, &path, depth + 1, total_bytes, out)?;
        } else if ft.is_file() {
            if out.len() >= MAX_FOLDER_FILES {
                return Err(invalid_data("pasta de save excede o limite de arquivos"));
            }
            let meta = std::fs::metadata(&path)?;
            *total_bytes = total_bytes.saturating_add(meta.len());
            if *total_bytes > MAX_FOLDER_BYTES {
                return Err(invalid_data("pasta de save excede o limite de 64 MiB"));
            }
            let rel_path = path
                .strip_prefix(root)
                .map_err(|_| invalid_data("caminho fora da raiz do save"))?;
            let mut parts = Vec::new();
            for comp in rel_path.components() {
                match comp {
                    std::path::Component::Normal(c) => {
                        let s = c
                            .to_str()
                            .ok_or_else(|| invalid_data("componente não UTF-8"))?;
                        parts.push(s);
                    }
                    _ => return Err(invalid_data("componente inválido na pasta de save")),
                }
            }
            let rel = parts.join("/");
            validate_rel_parts(&rel)?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push(CollectedFile { rel, path, mtime });
        }
    }
    Ok(())
}

/// Empacota `dir` em um `.zip` determinístico em memória, devolvendo
/// `(bytes_zip, sha256_hex, maior_mtime_unix)`.
///
/// Pasta ausente ou sem arquivos regulares devolve `ErrorKind::NotFound`.
pub fn pack_folder(dir: &Path) -> io::Result<(Vec<u8>, String, i64)> {
    let meta = std::fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() {
        return Err(invalid_input("pasta de save não pode ser link simbólico"));
    }
    if !meta.is_dir() {
        return Err(invalid_input("caminho de save em pasta não é diretório"));
    }

    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    collect_files(dir, dir, 1, &mut total_bytes, &mut files)?;
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "pasta de save vazia",
        ));
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));

    let max_mtime = files.iter().map(|f| f.mtime).max().unwrap_or(0);
    let mut buf = Vec::new();
    {
        let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        for item in &files {
            zw.start_file(&item.rel, opts).map_err(zip_io)?;
            let mut reader = std::fs::File::open(&item.path)?;
            io::copy(&mut reader, &mut zw)?;
        }
        zw.finish().map_err(zip_io)?;
    }

    let hash = sha256_hex(&buf);
    Ok((buf, hash, max_mtime))
}

/// SHA-256 do `.zip` determinístico, tamanho comprimido e maior `mtime` interno.
pub fn hash_folder(dir: &Path) -> io::Result<(String, u64, i64)> {
    let (bytes, hash, mtime) = pack_folder(dir)?;
    Ok((hash, bytes.len() as u64, mtime))
}

fn sibling_tmp(dir: &Path) -> io::Result<PathBuf> {
    let mut name = dir
        .file_name()
        .ok_or_else(|| invalid_input("pasta de save sem nome"))?
        .to_os_string();
    name.push(TMP_SUFFIX);
    Ok(dir.with_file_name(name))
}

/// Remove arquivo, link simbólico ou diretório recursivamente, ignorando `NotFound`.
pub fn remove_any(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_dir() && !meta.file_type().is_symlink() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Extrai `zip_bytes` em `<dir>.sync-tmp` validando cada entrada contra
/// path-traversal, links simbólicos e zip-bomb, faz `fsync` dos arquivos e só
/// então substitui `dir` (que quem chama já copiou para `sync-backups`).
pub fn unpack_folder_atomic(dir: &Path, zip_bytes: &[u8]) -> io::Result<()> {
    let parent = dir
        .parent()
        .ok_or_else(|| invalid_input("pasta de save sem diretório pai"))?;
    std::fs::create_dir_all(parent)?;

    let tmp = sibling_tmp(dir)?;
    remove_any(&tmp)?;

    let extracted = (|| -> io::Result<()> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(zip_io)?;
        std::fs::create_dir_all(&tmp)?;

        let mut file_count = 0usize;
        let mut total_uncompressed = 0u64;

        for idx in 0..archive.len() {
            let mut entry = archive.by_index(idx).map_err(zip_io)?;
            if entry.is_symlink() {
                return Err(invalid_data("link simbólico recusado no pacote de save"));
            }
            let raw_name = entry.name().map_err(zip_io)?.into_owned();
            if entry.is_dir() {
                let trimmed = raw_name.trim_end_matches('/');
                if !trimmed.is_empty() {
                    validate_rel_parts(trimmed)?;
                }
                continue;
            }
            let parts = validate_rel_parts(&raw_name)?;
            file_count += 1;
            if file_count > MAX_FOLDER_FILES {
                return Err(invalid_data("pacote de save excede o limite de arquivos"));
            }

            let mut out_path = tmp.clone();
            for part in parts {
                out_path.push(part);
            }
            if let Some(out_parent) = out_path.parent() {
                std::fs::create_dir_all(out_parent)?;
            }

            let remaining = MAX_FOLDER_BYTES.saturating_sub(total_uncompressed);
            let mut limited = (&mut entry).take(remaining + 1);
            let mut out_file = std::fs::File::create(&out_path)?;
            let written = io::copy(&mut limited, &mut out_file)?;
            if written > remaining {
                return Err(invalid_data(
                    "pacote de save excede o limite descomprimido de 64 MiB",
                ));
            }
            total_uncompressed += written;
            out_file.write_all(&[])?;
            out_file.sync_all()?;
        }

        if file_count == 0 {
            return Err(invalid_data("pacote de save sem arquivos"));
        }

        remove_any(dir)?;
        std::fs::rename(&tmp, dir)
    })();

    if extracted.is_err() {
        let _ = remove_any(&tmp);
    }
    extracted
}

/// Copia recursivamente os arquivos regulares de `from` para `to`, com `fsync`
/// em cada arquivo copiado (para `sync-backups/<carimbo>/`).
pub fn copy_dir_synced(from: &Path, to: &Path) -> io::Result<()> {
    fn walk_copy(src: &Path, dst: &Path, depth: usize) -> io::Result<()> {
        if depth > MAX_FOLDER_DEPTH {
            return Err(invalid_data("pasta de save excede a profundidade máxima"));
        }
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(TMP_SUFFIX) {
                continue;
            }
            let ft = entry.file_type()?;
            if ft.is_symlink() {
                return Err(invalid_input("link simbólico não permitido no backup"));
            }
            let src_path = entry.path();
            let dst_path = dst.join(&name);
            if ft.is_dir() {
                walk_copy(&src_path, &dst_path, depth + 1)?;
            } else if ft.is_file() {
                std::fs::copy(&src_path, &dst_path)?;
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(&dst_path)?
                    .sync_all()?;
            }
        }
        Ok(())
    }

    let meta = std::fs::symlink_metadata(from)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(invalid_input("origem do backup em pasta inválida"));
    }
    walk_copy(from, to, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("reemu-sync-folder-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pack_is_deterministic_across_creation_order_and_ignores_sync_tmp() {
        let root = temp("deterministic");
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(a.join("sub")).unwrap();
        std::fs::create_dir_all(b.join("sub")).unwrap();

        std::fs::write(a.join("PARAM.SFO"), b"sfo-bytes").unwrap();
        std::fs::write(a.join("DATA.BIN"), b"save-payload").unwrap();
        std::fs::write(a.join("sub").join("ICON0.PNG"), b"png").unwrap();
        std::fs::write(a.join("DATA.BIN.sync-tmp"), b"interrupted").unwrap();

        // Ordem inversa de criação na segunda máquina.
        std::fs::write(b.join("sub").join("ICON0.PNG"), b"png").unwrap();
        std::fs::write(b.join("DATA.BIN"), b"save-payload").unwrap();
        std::fs::write(b.join("PARAM.SFO"), b"sfo-bytes").unwrap();

        let (bytes_a, hash_a, _) = pack_folder(&a).unwrap();
        let (bytes_b, hash_b, _) = pack_folder(&b).unwrap();
        assert_eq!(hash_a, hash_b);
        assert_eq!(bytes_a, bytes_b);

        let dest = root.join("unpacked");
        unpack_folder_atomic(&dest, &bytes_a).unwrap();
        assert_eq!(
            std::fs::read(dest.join("DATA.BIN")).unwrap(),
            b"save-payload"
        );
        assert_eq!(
            std::fs::read(dest.join("sub").join("ICON0.PNG")).unwrap(),
            b"png"
        );
        assert!(!dest.join("DATA.BIN.sync-tmp").exists());

        let (_, hash_unpacked, _) = pack_folder(&dest).unwrap();
        assert_eq!(hash_unpacked, hash_a);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_or_missing_folder_returns_not_found() {
        let root = temp("empty");
        let empty = root.join("empty_slot");
        std::fs::create_dir_all(empty.join("nested")).unwrap();
        assert_eq!(
            pack_folder(&empty).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            pack_folder(&root.join("missing")).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unpack_rejects_path_traversal_and_keeps_existing_folder_intact() {
        let root = temp("traversal");
        let slot = root.join("ULUS10041DATA00");
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::write(slot.join("DATA.BIN"), b"good-local-save").unwrap();

        let mut bad_zip = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut bad_zip));
            zw.start_file("../escaped.bin", zip::write::SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"evil").unwrap();
            zw.finish().unwrap();
        }

        let err = unpack_folder_atomic(&slot, &bad_zip).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(!root.join("escaped.bin").exists());
        assert_eq!(
            std::fs::read(slot.join("DATA.BIN")).unwrap(),
            b"good-local-save"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
