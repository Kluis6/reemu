use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::input::{
    ControllerLayoutEntry, ControllerMapping, ControllerMappingRepository, MappingSource,
};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

pub struct ControllerMappingsRepo {
    db: Db,
}

impl ControllerMappingsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

const COLS: &str = "SELECT guid, display_name, layout_json, source FROM controller_mappings";

fn row_to_mapping(row: &SqliteRow) -> Result<ControllerMapping, RepoError> {
    let layout_json: String = row.try_get("layout_json").map_err(be)?;
    let layout: Vec<ControllerLayoutEntry> = serde_json::from_str(&layout_json).map_err(|e| {
        RepoError::Corrupt(format!("controller_mappings.layout_json inválido: {e}"))
    })?;
    let source: String = row.try_get("source").map_err(be)?;
    let source = MappingSource::from_wire(&source).ok_or_else(|| {
        RepoError::Corrupt(format!("controller_mappings.source desconhecida: {source}"))
    })?;
    Ok(ControllerMapping {
        guid: row.try_get("guid").map_err(be)?,
        display_name: row.try_get("display_name").map_err(be)?,
        layout,
        source,
    })
}

#[async_trait]
impl ControllerMappingRepository for ControllerMappingsRepo {
    async fn list(&self) -> Result<Vec<ControllerMapping>, RepoError> {
        let rows = sqlx::query(&format!("{COLS} ORDER BY display_name"))
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter().map(row_to_mapping).collect()
    }

    async fn get(&self, guid: &str) -> Result<Option<ControllerMapping>, RepoError> {
        let row = sqlx::query(&format!("{COLS} WHERE guid = ?1"))
            .bind(guid)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.as_ref().map(row_to_mapping).transpose()
    }

    async fn upsert(&self, mapping: &ControllerMapping) -> Result<(), RepoError> {
        let layout_json = serde_json::to_string(&mapping.layout)
            .map_err(|e| RepoError::Backend(e.to_string()))?;
        sqlx::query(
            "INSERT INTO controller_mappings (guid, display_name, layout_json, source) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(guid) DO UPDATE SET \
               display_name = excluded.display_name, \
               layout_json = excluded.layout_json, \
               source = excluded.source",
        )
        .bind(&mapping.guid)
        .bind(&mapping.display_name)
        .bind(&layout_json)
        .bind(mapping.source.as_wire())
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn delete(&self, guid: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM controller_mappings WHERE guid = ?1")
            .bind(guid)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
