use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use sqlx::Row;

/// Pastas adicionadas à biblioteca (raízes da varredura).
pub struct LibraryFoldersRepo {
    db: Db,
}

/// `path` está em `dir` ou é o próprio `dir` (aceita `/` e `\`).
pub fn is_under(path: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches(['/', '\\']);
    path == dir
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with(['/', '\\']))
}

impl LibraryFoldersRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> Result<Vec<String>, RepoError> {
        let rows = sqlx::query("SELECT path FROM library_folders ORDER BY path")
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter().map(|r| r.try_get("path").map_err(be)).collect()
    }

    /// Guarda a pasta. Se ela já está dentro de outra guardada, não muda
    /// nada; se contém pastas guardadas, fica no lugar delas.
    pub async fn add(&self, path: &str, at_unix: i64) -> Result<(), RepoError> {
        let path = path.trim_end_matches(['/', '\\']);
        let all = self.list().await?;
        if all.iter().any(|d| is_under(path, d)) {
            return Ok(());
        }
        for d in all.iter().filter(|d| is_under(d, path)) {
            self.remove(d).await?;
        }
        sqlx::query("INSERT OR IGNORE INTO library_folders (path, added_at) VALUES (?1, ?2)")
            .bind(path)
            .bind(at_unix)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    async fn remove(&self, path: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM library_folders WHERE path = ?1")
            .bind(path)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Tira as pastas que estão em `dir` (ou são ele).
    pub async fn remove_under(&self, dir: &str) -> Result<(), RepoError> {
        for d in self.list().await?.iter().filter(|d| is_under(d, dir)) {
            self.remove(d).await?;
        }
        Ok(())
    }

    pub async fn clear(&self) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM library_folders")
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_under_accepts_both_separators() {
        assert!(is_under(r"E:\roms\naomi\a.zip", r"E:\roms"));
        assert!(is_under(r"E:\roms", r"E:\roms\"));
        assert!(is_under("/home/u/roms/nes/x.nes", "/home/u/roms/"));
        assert!(!is_under(r"E:\roms2\a.zip", r"E:\roms"));
    }

    #[tokio::test]
    async fn add_keeps_only_the_outermost_folder() {
        let repo = LibraryFoldersRepo::new(crate::connect_in_memory().await.unwrap());
        repo.add(r"E:\roms\naomi", 0).await.unwrap();
        repo.add(r"E:\roms\dc", 0).await.unwrap();
        repo.add(r"E:\roms\", 0).await.unwrap();
        repo.add(r"E:\roms\snes", 0).await.unwrap();
        repo.add("D:/jogos", 0).await.unwrap();
        assert_eq!(repo.list().await.unwrap(), ["D:/jogos", r"E:\roms"]);
        repo.remove_under(r"E:\roms").await.unwrap();
        assert_eq!(repo.list().await.unwrap(), ["D:/jogos"]);
    }
}
