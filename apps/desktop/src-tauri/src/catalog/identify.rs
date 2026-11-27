//! Identificação preguiçosa em segundo plano para linhas antigas ou
//! provisórias (`identified_at IS NULL`, Review Focus 1).
//!
//! Delega a regra de arquivo e compactado para `library_scan::identify_known_rom`,
//! rodando uma ROM por vez em `spawn_blocking` e parando assim que houver um
//! jogo abrindo ou rodando. Nunca registra caminhos, nomes, seriais nem hashes.

use domain::library::Rom;
use std::collections::HashSet;
use std::path::Path;

pub fn identify_rom_file(rom: &Rom) -> Option<library_scan::Identity> {
    library_scan::identify_known_rom(Path::new(&rom.file_path), &rom.system_id).ok()
}

pub async fn identify_unidentified(
    pool: &db::Db,
    only: Option<&HashSet<String>>,
    now: i64,
    is_playing: &(dyn Fn() -> bool + Send + Sync),
) -> Result<usize, String> {
    if is_playing() {
        return Ok(0);
    }
    let repo = db::RomsRepo::new(pool.clone());
    let rows = repo.unidentified().await.map_err(|e| e.to_string())?;
    let mut done = 0usize;

    for rom in rows {
        if is_playing() {
            break;
        }
        if let Some(allowed) = only {
            if !allowed.contains(&rom.id) {
                continue;
            }
        }
        let rom_for_io = rom.clone();
        let computed = tokio::task::spawn_blocking(move || identify_rom_file(&rom_for_io))
            .await
            .ok()
            .flatten();
        let Some(id) = computed else {
            continue;
        };
        if is_playing() {
            break;
        }
        let (crc32, md5, sha1) = match &id.hashes {
            Some(d) => (d.crc32.as_str(), d.md5.as_str(), Some(d.sha1.as_str())),
            None => ("", "", None),
        };
        let identified_at = (!id.provisional).then_some(now);
        repo.save_identity(
            &rom.id,
            crc32,
            md5,
            sha1,
            id.serial.as_deref(),
            &id.name,
            i64::try_from(id.size).ok(),
            identified_at,
        )
        .await
        .map_err(|e| e.to_string())?;
        if !id.provisional {
            done += 1;
        }
        tokio::task::yield_now().await;
    }
    Ok(done)
}
