use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

/// Item guardado em `alvanista_home_feed` para a tela inicial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlvanistaHomeFeedRow {
    pub id: String,
    pub lang: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub image_url: Option<String>,
    pub url: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub sponsored: bool,
    pub position: i64,
    pub fetched_at: i64,
}

fn row_to_home_item(row: &SqliteRow) -> Result<AlvanistaHomeFeedRow, RepoError> {
    Ok(AlvanistaHomeFeedRow {
        id: row.try_get("id").map_err(be)?,
        lang: row.try_get("lang").map_err(be)?,
        kind: row.try_get("kind").map_err(be)?,
        title: row.try_get("title").map_err(be)?,
        summary: row.try_get("summary").map_err(be)?,
        image_url: row.try_get("image_url").map_err(be)?,
        url: row.try_get("url").map_err(be)?,
        starts_at: row.try_get("starts_at").map_err(be)?,
        ends_at: row.try_get("ends_at").map_err(be)?,
        sponsored: row.try_get::<i64, _>("sponsored").map_err(be)? != 0,
        position: row.try_get("position").map_err(be)?,
        fetched_at: row.try_get("fetched_at").map_err(be)?,
    })
}

/// Estado local da integração com o Alvanista: pares chave/valor
/// (`alvanista_state`). Os tokens não passam por aqui.
#[derive(Clone)]
pub struct AlvanistaRepo {
    db: Db,
}

impl AlvanistaRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub async fn state_get(&self, key: &str) -> Result<Option<String>, RepoError> {
        let row = sqlx::query("SELECT value FROM alvanista_state WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.map(|r| r.try_get("value").map_err(be)).transpose()
    }

    pub async fn state_set(&self, key: &str, value: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO alvanista_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// Grava só se a chave ainda não existe (duas chamadas simultâneas não
    /// se sobrescrevem); quem chama relê o valor que ficou.
    pub async fn state_insert_if_absent(&self, key: &str, value: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO alvanista_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO NOTHING",
        )
        .bind(key)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn state_delete(&self, key: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM alvanista_state WHERE key = ?1")
            .bind(key)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Chave "Usar o catálogo do Alvanista para identificar meus jogos" (ligada por padrão).
    pub async fn catalog_enabled(&self) -> Result<bool, RepoError> {
        Ok(self.state_get("catalog.enabled").await?.as_deref() != Some("0"))
    }

    pub async fn set_catalog_enabled(&self, enabled: bool) -> Result<(), RepoError> {
        self.state_set("catalog.enabled", if enabled { "1" } else { "0" })
            .await?;
        if enabled {
            self.state_delete("catalog.disabled").await?;
        }
        Ok(())
    }

    pub async fn catalog_disabled_by_server(&self) -> Result<bool, RepoError> {
        Ok(self.state_get("catalog.disabled").await?.as_deref() == Some("1"))
    }

    pub async fn set_catalog_disabled_by_server(&self, disabled: bool) -> Result<(), RepoError> {
        if disabled {
            self.state_set("catalog.disabled", "1").await
        } else {
            self.state_delete("catalog.disabled").await
        }
    }

    /// O catálogo está ativo (chave ligada e rota habilitada no servidor)?
    pub async fn catalog_active(&self) -> Result<bool, RepoError> {
        Ok(self.catalog_enabled().await? && !self.catalog_disabled_by_server().await?)
    }

    /// Chave "Mostrar destaques do Alvanista na tela inicial" (ligada por padrão).
    pub async fn home_highlights_enabled(&self) -> Result<bool, RepoError> {
        Ok(self.state_get("home.highlights_enabled").await?.as_deref() != Some("0"))
    }

    pub async fn set_home_highlights_enabled(&self, enabled: bool) -> Result<(), RepoError> {
        self.state_set("home.highlights_enabled", if enabled { "1" } else { "0" })
            .await
    }

    /// Instante Unix (segundos) da última busca bem-sucedida das novidades para `lang`.
    pub async fn home_feed_fetched_at(&self, lang: &str) -> Result<Option<i64>, RepoError> {
        let state_ts = self
            .state_get(&format!("home.fetched_at.{lang}"))
            .await?
            .and_then(|v| v.parse::<i64>().ok());
        let row = sqlx::query(
            "SELECT MAX(fetched_at) AS fetched_at FROM alvanista_home_feed WHERE lang = ?1",
        )
        .bind(lang)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        let table_ts: Option<i64> = match row {
            Some(r) => r.try_get("fetched_at").map_err(be)?,
            None => None,
        };
        Ok(match (state_ts, table_ts) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        })
    }

    /// Substitui os itens de novidades guardados para `lang` e registra `fetched_at`.
    pub async fn replace_home_feed(
        &self,
        lang: &str,
        items: &[AlvanistaHomeFeedRow],
        fetched_at: i64,
    ) -> Result<(), RepoError> {
        let mut tx = self.db.begin().await.map_err(be)?;
        sqlx::query("DELETE FROM alvanista_home_feed WHERE lang = ?1")
            .bind(lang)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        for item in items {
            sqlx::query(
                "INSERT INTO alvanista_home_feed \
                 (id, lang, kind, title, summary, image_url, url, starts_at, ends_at, sponsored, position, fetched_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )
            .bind(&item.id)
            .bind(lang)
            .bind(&item.kind)
            .bind(&item.title)
            .bind(&item.summary)
            .bind(&item.image_url)
            .bind(&item.url)
            .bind(&item.starts_at)
            .bind(&item.ends_at)
            .bind(i64::from(item.sponsored))
            .bind(item.position)
            .bind(item.fetched_at)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        }
        sqlx::query(
            "INSERT INTO alvanista_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(format!("home.fetched_at.{lang}"))
        .bind(fetched_at.to_string())
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        tx.commit().await.map_err(be)?;
        Ok(())
    }

    pub async fn list_home_feed(&self, lang: &str) -> Result<Vec<AlvanistaHomeFeedRow>, RepoError> {
        let rows = sqlx::query(
            "SELECT id, lang, kind, title, summary, image_url, url, starts_at, ends_at, \
             sponsored, position, fetched_at \
             FROM alvanista_home_feed WHERE lang = ?1 ORDER BY position ASC, id ASC",
        )
        .bind(lang)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(row_to_home_item).collect()
    }

    pub async fn home_feed_item_by_id(
        &self,
        id: &str,
    ) -> Result<Option<AlvanistaHomeFeedRow>, RepoError> {
        let row = sqlx::query(
            "SELECT id, lang, kind, title, summary, image_url, url, starts_at, ends_at, \
             sponsored, position, fetched_at \
             FROM alvanista_home_feed WHERE id = ?1 ORDER BY fetched_at DESC, position ASC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_home_item).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connect_in_memory;

    fn sample_row(
        id: &str,
        lang: &str,
        kind: &str,
        pos: i64,
        fetched_at: i64,
    ) -> AlvanistaHomeFeedRow {
        AlvanistaHomeFeedRow {
            id: id.into(),
            lang: lang.into(),
            kind: kind.into(),
            title: format!("Titulo {id}"),
            summary: format!("Resumo {id}"),
            image_url: Some(format!("https://cdn.alvanista.com/{id}.jpg")),
            url: format!("https://www.alvanista.com/articles/{id}"),
            starts_at: Some("2026-10-01T00:00:00Z".into()),
            ends_at: None,
            sponsored: false,
            position: pos,
            fetched_at,
        }
    }

    #[tokio::test]
    async fn home_highlights_preference_defaults_to_true_and_toggles() {
        let db = connect_in_memory().await.unwrap();
        let repo = AlvanistaRepo::new(db);
        assert!(repo.home_highlights_enabled().await.unwrap());
        repo.set_home_highlights_enabled(false).await.unwrap();
        assert!(!repo.home_highlights_enabled().await.unwrap());
        repo.set_home_highlights_enabled(true).await.unwrap();
        assert!(repo.home_highlights_enabled().await.unwrap());
    }

    #[tokio::test]
    async fn home_feed_persists_per_language_orders_by_position_and_records_empty_fetch() {
        let db = connect_in_memory().await.unwrap();
        let repo = AlvanistaRepo::new(db);

        assert_eq!(repo.home_feed_fetched_at("pt-BR").await.unwrap(), None);
        repo.replace_home_feed(
            "pt-BR",
            &[
                sample_row("b", "pt-BR", "review", 1, 1000),
                sample_row("a", "pt-BR", "article", 0, 1000),
            ],
            1000,
        )
        .await
        .unwrap();
        repo.replace_home_feed("en", &[sample_row("en1", "en", "article", 0, 1200)], 1200)
            .await
            .unwrap();

        let pt = repo.list_home_feed("pt-BR").await.unwrap();
        assert_eq!(pt.len(), 2);
        assert_eq!(pt[0].id, "a");
        assert_eq!(pt[1].id, "b");
        assert_eq!(
            repo.home_feed_fetched_at("pt-BR").await.unwrap(),
            Some(1000)
        );
        assert_eq!(
            repo.home_feed_item_by_id("b")
                .await
                .unwrap()
                .map(|r| r.kind),
            Some("review".to_owned())
        );

        // Resposta vazia ainda atualiza o carimbo para respeitar o intervalo de 4h,
        // sem apagar os itens de outro idioma.
        repo.replace_home_feed("pt-BR", &[], 2000).await.unwrap();
        assert!(repo.list_home_feed("pt-BR").await.unwrap().is_empty());
        assert_eq!(
            repo.home_feed_fetched_at("pt-BR").await.unwrap(),
            Some(2000)
        );
        assert_eq!(repo.list_home_feed("en").await.unwrap().len(), 1);
    }
}
