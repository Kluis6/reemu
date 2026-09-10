use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::profile::{Profile, ProfileRepository};
use sqlx::Row;

pub struct ProfileRepo {
    db: Db,
}

impl ProfileRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ProfileRepository for ProfileRepo {
    async fn get(&self) -> Result<Profile, RepoError> {
        // A linha id=1 é inserida pela migration; se sumir, é corrupção.
        let row = sqlx::query("SELECT name, bio, avatar, onboarded FROM profile WHERE id = 1")
            .fetch_optional(&self.db)
            .await
            .map_err(be)?
            .ok_or_else(|| RepoError::Corrupt("profile sem a linha id=1".into()))?;

        Ok(Profile {
            name: row.try_get("name").map_err(be)?,
            bio: row
                .try_get::<Option<String>, _>("bio")
                .map_err(be)?
                .filter(|s| !s.is_empty()),
            avatar: row.try_get("avatar").map_err(be)?,
            onboarded: row.try_get::<i64, _>("onboarded").map_err(be)? != 0,
        })
    }

    async fn update(&self, p: &Profile) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE profile SET name = ?1, bio = ?2, avatar = ?3, onboarded = ?4 WHERE id = 1",
        )
        .bind(&p.name)
        .bind(p.bio.as_deref())
        .bind(&p.avatar)
        .bind(p.onboarded as i64)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }
}
