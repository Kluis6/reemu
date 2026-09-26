use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use sqlx::Row;

/// Teclas que o usuário trocou por cima do padrão do teclado
/// (`input_desktop::keymap`). Pares `(alvo, KeyboardEvent.code)`.
pub struct KeyboardBindingsRepo {
    db: Db,
}

impl KeyboardBindingsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> Result<Vec<(String, String)>, RepoError> {
        let rows = sqlx::query("SELECT target, code FROM keyboard_bindings ORDER BY target")
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter()
            .map(|r| {
                Ok((
                    r.try_get("target").map_err(be)?,
                    r.try_get("code").map_err(be)?,
                ))
            })
            .collect()
    }

    pub async fn set(&self, target: &str, code: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO keyboard_bindings (target, code) VALUES (?1, ?2)
             ON CONFLICT(target) DO UPDATE SET code = excluded.code",
        )
        .bind(target)
        .bind(code)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// Volta tudo ao padrão.
    pub async fn clear(&self) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM keyboard_bindings")
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_overwrites_and_clear_resets() {
        let db = crate::connect_in_memory().await.unwrap();
        let repo = KeyboardBindingsRepo::new(db);
        repo.set("R2", "KeyR").await.unwrap();
        repo.set("R2", "Space").await.unwrap();
        repo.set("LStickUp", "").await.unwrap();
        assert_eq!(
            repo.list().await.unwrap(),
            vec![
                ("LStickUp".to_string(), String::new()),
                ("R2".to_string(), "Space".to_string())
            ]
        );
        repo.clear().await.unwrap();
        assert!(repo.list().await.unwrap().is_empty());
    }
}
