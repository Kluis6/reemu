//! As cópias de segurança em `<dados>/sync-backups/<carimbo>/` e a limpeza
//! dos temporários de uma escrita que não terminou. Saiu de `engine.rs` sem
//! mudança de comportamento.

use super::*;

impl Engine {
    pub(super) fn new_stamp(&self) {
        *lock(&self.stamp) = local_stamp();
    }

    /// Apaga os `.sync-tmp` que uma escrita interrompida deixou (o app fechou
    /// entre o temporário e o `rename`). Só esses: nenhum outro arquivo.
    pub(super) fn sweep_temps(&self) {
        let mut dirs = vec![self.dirs.saves.clone()];
        dirs.extend(local::system_dirs(&self.dirs.system));
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let is_ours = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.ends_with(TMP_SUFFIX));
                if is_ours && entry.file_type().is_ok_and(|t| t.is_file()) {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!("sync: temporário velho não saiu: {e}");
                    }
                }
            }
        }
    }

    /// Onde o backup de `path` fica: `<dados>/sync-backups/<carimbo>/<caminho
    /// relativo aos dados>`, sem nunca sobrescrever um backup.
    pub(super) fn backup_target(&self, path: &Path) -> io::Result<PathBuf> {
        let stamp = {
            let mut s = lock(&self.stamp);
            if s.is_empty() {
                *s = local_stamp();
            }
            s.clone()
        };
        self.backup_target_in(&stamp, path)
    }

    /// O mesmo com um carimbo dado, sem tocar no da passada em andamento.
    pub(super) fn backup_target_in(&self, stamp: &str, path: &Path) -> io::Result<PathBuf> {
        let rel = match path.strip_prefix(&self.dirs.data) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
            _ => PathBuf::from(path.file_name().unwrap_or(path.as_os_str())),
        };
        let first = self.dirs.data.join("sync-backups").join(stamp).join(rel);
        if let Some(parent) = first.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut target = first.clone();
        let mut n = 2;
        while present(&target)? {
            let name = first
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            target = first.with_file_name(format!("{name} ({n})"));
            n += 1;
        }
        Ok(target)
    }

    /// Copia o arquivo para a pasta de backup da passada (o original fica). A
    /// cópia vai ao disco (`fsync`) antes de o original ser substituído.
    pub fn backup(&self, path: &Path) -> io::Result<PathBuf> {
        let target = self.backup_target(path)?;
        copy_synced(path, &target)?;
        Ok(target)
    }

    /// Muda o arquivo para a pasta de backup da passada.
    pub(super) fn move_to_backup(&self, path: &Path) -> io::Result<PathBuf> {
        let target = self.backup_target(path)?;
        if std::fs::rename(path, &target).is_err() {
            // Outro volume: copia, grava no disco e só então tira o original.
            copy_synced(path, &target)?;
            std::fs::remove_file(path)?;
        }
        Ok(target)
    }
}
