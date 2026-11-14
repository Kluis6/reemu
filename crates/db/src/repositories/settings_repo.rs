//! As tabelas dos grupos de configuração que uma máquina copia de outra
//! (cloud save, D114), lidas e escritas dentro de UMA transação por grupo.
//!
//! Aqui só o SQL. O que é portátil, o formato do documento e o mapa de jogo
//! por CRC32 ficam no app (`apps/desktop/src-tauri/src/sync/settings_groups.rs`).
//! Os `id` das linhas novas são os mesmos que os repositórios de cada tabela
//! gravam, então uma linha escrita aqui é igual à que a tela escreveria.

use super::core_options_repo::{override_id, row_id};
use super::decoration_repo::assign_id;
use super::shader_chain_repo::{assignment_id, param_id};
use crate::cascade::be;
use crate::convert::{scope_from_db, scope_to_db};
use crate::pool::Db;
use domain::decoration::{DecorationAssignment, DecorationPack, DecorationPackSource};
use domain::error::RepoError;
use domain::shader_chain::{AssignmentScope, ShaderFormat, ShaderPreset};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use std::collections::HashMap;

/// Um valor de `core_option_overrides` (opção de core de um jogo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionOverrideRow {
    pub rom_id: String,
    pub core_id: String,
    pub key: String,
    pub value: String,
}

/// Uma linha de `shader_chain_assignments` com os parâmetros ajustados nela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderAssignmentRow {
    pub id: String,
    pub scope: AssignmentScope,
    pub system_id: Option<String>,
    pub rom_id: Option<String>,
    pub preset_id: String,
    /// `shader_parameter_overrides` da atribuição, em ordem de chave.
    pub params: Vec<(String, String)>,
}

/// Uma linha de `decoration_assignments`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationAssignmentRow {
    pub id: String,
    pub scope: AssignmentScope,
    pub system_id: Option<String>,
    pub rom_id: Option<String>,
    pub pack_id: String,
    pub asset_path: String,
}

/// Uma linha de `system_hotkeys`, crua (o `trigger_json` não é lido aqui).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRow {
    pub id: String,
    pub action: String,
    pub trigger_json: String,
    pub device_guid: Option<String>,
}

/// A parte de `audio_config` que não depende do aparelho de saída: o id e o
/// nome do aparelho não estão aqui.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSettings {
    pub rate_control_enabled: bool,
    pub rate_control_delta: f64,
    pub sample_rate_preference: Option<i64>,
}

/// A parte de `video_config` que não depende da tela: os nits de HDR não
/// estão aqui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSettings {
    pub integer_scaling: bool,
    pub hdr: bool,
    pub hdr_highlights: bool,
}

/// Uma linha de `aspect_assignments`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AspectRow {
    pub scope: String,
    pub target: String,
    pub aspect: String,
}

pub struct SettingsRepo {
    db: Db,
}

impl SettingsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Transação de leitura: todas as tabelas vistas no mesmo instante. Sem
    /// `commit`, ela termina quando o valor sai de escopo.
    pub async fn read(&self) -> Result<SettingsTx, RepoError> {
        let tx = self.db.begin().await.map_err(be)?;
        Ok(SettingsTx { tx })
    }

    /// Transação de escrita. `BEGIN IMMEDIATE` pega a trava de escrita no
    /// começo, então o que se lê dentro dela ainda vale quando se escreve
    /// (SQLite, "BEGIN TRANSACTION": uma transação adiada que lê e depois
    /// escreve pode falhar com `SQLITE_BUSY` se outra conexão escreveu no
    /// meio). Sem `commit`, nada do que foi feito fica.
    pub async fn write(&self) -> Result<SettingsTx, RepoError> {
        let tx = self.db.begin_with("BEGIN IMMEDIATE").await.map_err(be)?;
        Ok(SettingsTx { tx })
    }
}

/// Uma transação aberta sobre as tabelas de configuração.
pub struct SettingsTx {
    tx: Transaction<'static, Sqlite>,
}

fn col<T>(r: &SqliteRow, name: &str) -> Result<T, RepoError>
where
    T: for<'r> sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
{
    r.try_get(name).map_err(be)
}

fn pack_source(s: &str) -> Result<DecorationPackSource, RepoError> {
    match s {
        "bundled" => Ok(DecorationPackSource::Bundled),
        "user_imported" => Ok(DecorationPackSource::UserImported),
        other => Err(RepoError::Corrupt(format!(
            "decoration_packs.source desconhecido: {other:?}"
        ))),
    }
}

impl SettingsTx {
    pub async fn commit(self) -> Result<(), RepoError> {
        self.tx.commit().await.map_err(be)
    }

    async fn all(&mut self, sql: &str) -> Result<Vec<SqliteRow>, RepoError> {
        sqlx::query(sql).fetch_all(&mut *self.tx).await.map_err(be)
    }

    /// `(id, crc32)` de cada ROM da biblioteca.
    pub async fn rom_crc32s(&mut self) -> Result<Vec<(String, String)>, RepoError> {
        let rows = self.all("SELECT id, crc32 FROM roms ORDER BY id").await?;
        rows.iter()
            .map(|r| Ok((col(r, "id")?, col(r, "crc32")?)))
            .collect()
    }

    // --- opções de core -----------------------------------------------------

    /// `(core_id, option_key, value)` de `core_options_values`.
    pub async fn option_values(&mut self) -> Result<Vec<(String, String, String)>, RepoError> {
        let rows = self
            .all(
                "SELECT core_id, option_key, value FROM core_options_values \
                 ORDER BY core_id, option_key",
            )
            .await?;
        rows.iter()
            .map(|r| Ok((col(r, "core_id")?, col(r, "option_key")?, col(r, "value")?)))
            .collect()
    }

    pub async fn clear_option_values(&mut self) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM core_options_values")
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Insere sem substituir: a mesma opção duas vezes é erro.
    pub async fn insert_option_value(
        &mut self,
        core_id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO core_options_values (id, core_id, option_key, value) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(row_id(core_id, key))
        .bind(core_id)
        .bind(key)
        .bind(value)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn option_overrides(&mut self) -> Result<Vec<OptionOverrideRow>, RepoError> {
        let rows = self
            .all(
                "SELECT rom_id, core_id, option_key, value FROM core_option_overrides \
                 ORDER BY rom_id, core_id, option_key",
            )
            .await?;
        rows.iter()
            .map(|r| {
                Ok(OptionOverrideRow {
                    rom_id: col(r, "rom_id")?,
                    core_id: col(r, "core_id")?,
                    key: col(r, "option_key")?,
                    value: col(r, "value")?,
                })
            })
            .collect()
    }

    /// Apaga as opções por jogo de uma ROM (de todos os cores).
    pub async fn delete_option_overrides(&mut self, rom_id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM core_option_overrides WHERE rom_id = ?1")
            .bind(rom_id)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Insere sem substituir: a mesma opção do mesmo jogo duas vezes é erro.
    pub async fn insert_option_override(&mut self, o: &OptionOverrideRow) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO core_option_overrides (id, rom_id, core_id, option_key, value) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(override_id(&o.rom_id, &o.core_id, &o.key))
        .bind(&o.rom_id)
        .bind(&o.core_id)
        .bind(&o.key)
        .bind(&o.value)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        Ok(())
    }

    // --- shaders --------------------------------------------------------------

    pub async fn shader_presets(&mut self) -> Result<Vec<ShaderPreset>, RepoError> {
        let rows = self
            .all(
                "SELECT id, name, source_path, is_builtin, includes_bezel FROM shader_presets \
                 ORDER BY id",
            )
            .await?;
        rows.iter()
            .map(|r| {
                Ok(ShaderPreset {
                    id: col(r, "id")?,
                    name: col(r, "name")?,
                    source_path: col(r, "source_path")?,
                    format: ShaderFormat::Slang,
                    is_builtin: col::<i64>(r, "is_builtin")? != 0,
                    includes_bezel: col::<i64>(r, "includes_bezel")? != 0,
                })
            })
            .collect()
    }

    /// Cria o preset quando não há um com o mesmo `id`; o que já existe fica
    /// como está.
    pub async fn ensure_shader_preset(&mut self, p: &ShaderPreset) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO shader_presets (id, name, source_path, format, is_builtin, includes_bezel) \
             VALUES (?1, ?2, ?3, 'slang', ?4, ?5) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&p.id)
        .bind(&p.name)
        .bind(&p.source_path)
        .bind(p.is_builtin as i64)
        .bind(p.includes_bezel as i64)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn shader_assignments(&mut self) -> Result<Vec<ShaderAssignmentRow>, RepoError> {
        let params = self
            .all(
                "SELECT assignment_id, parameter_key, value FROM shader_parameter_overrides \
                 ORDER BY assignment_id, parameter_key",
            )
            .await?;
        let mut by_assignment: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for r in &params {
            by_assignment
                .entry(col(r, "assignment_id")?)
                .or_default()
                .push((col(r, "parameter_key")?, col(r, "value")?));
        }
        let rows = self
            .all(
                "SELECT id, scope, system_id, rom_id, preset_id FROM shader_chain_assignments \
                 ORDER BY id",
            )
            .await?;
        rows.iter()
            .map(|r| {
                let id: String = col(r, "id")?;
                Ok(ShaderAssignmentRow {
                    params: by_assignment.remove(&id).unwrap_or_default(),
                    scope: scope_from_db(&col::<String>(r, "scope")?)?,
                    system_id: col(r, "system_id")?,
                    rom_id: col(r, "rom_id")?,
                    preset_id: col(r, "preset_id")?,
                    id,
                })
            })
            .collect()
    }

    /// Apaga a atribuição e os parâmetros dela.
    pub async fn delete_shader_assignment(&mut self, id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM shader_parameter_overrides WHERE assignment_id = ?1")
            .bind(id)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        sqlx::query("DELETE FROM shader_chain_assignments WHERE id = ?1")
            .bind(id)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Insere a atribuição e os parâmetros dela, sem substituir nada: o
    /// escopo já ocupado é erro.
    pub async fn insert_shader_assignment(
        &mut self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
        preset_id: &str,
        params: &[(String, String)],
    ) -> Result<(), RepoError> {
        let aid = assignment_id(scope, system_id, rom_id);
        sqlx::query(
            "INSERT INTO shader_chain_assignments (id, scope, system_id, rom_id, preset_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&aid)
        .bind(scope_to_db(scope))
        .bind(system_id)
        .bind(rom_id)
        .bind(preset_id)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        for (key, value) in params {
            sqlx::query(
                "INSERT INTO shader_parameter_overrides (id, assignment_id, parameter_key, value) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(param_id(&aid, key))
            .bind(&aid)
            .bind(key)
            .bind(value)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        }
        Ok(())
    }

    // --- decoração ------------------------------------------------------------

    pub async fn decoration_packs(&mut self) -> Result<Vec<DecorationPack>, RepoError> {
        let rows = self
            .all("SELECT id, name, source, base_path FROM decoration_packs ORDER BY id")
            .await?;
        rows.iter()
            .map(|r| {
                Ok(DecorationPack {
                    id: col(r, "id")?,
                    name: col(r, "name")?,
                    source: pack_source(&col::<String>(r, "source")?)?,
                    base_path: col(r, "base_path")?,
                })
            })
            .collect()
    }

    pub async fn decoration_assignments(
        &mut self,
    ) -> Result<Vec<DecorationAssignmentRow>, RepoError> {
        let rows = self
            .all(
                "SELECT id, scope, system_id, rom_id, pack_id, asset_path \
                 FROM decoration_assignments ORDER BY id",
            )
            .await?;
        rows.iter()
            .map(|r| {
                Ok(DecorationAssignmentRow {
                    id: col(r, "id")?,
                    scope: scope_from_db(&col::<String>(r, "scope")?)?,
                    system_id: col(r, "system_id")?,
                    rom_id: col(r, "rom_id")?,
                    pack_id: col(r, "pack_id")?,
                    asset_path: col(r, "asset_path")?,
                })
            })
            .collect()
    }

    pub async fn delete_decoration_assignment(&mut self, id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM decoration_assignments WHERE id = ?1")
            .bind(id)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Insere sem substituir: o escopo já ocupado é erro.
    pub async fn insert_decoration_assignment(
        &mut self,
        a: &DecorationAssignment,
    ) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO decoration_assignments \
             (id, scope, system_id, rom_id, pack_id, asset_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(assign_id(a))
        .bind(scope_to_db(a.scope))
        .bind(&a.system_id)
        .bind(&a.rom_id)
        .bind(&a.pack_id)
        .bind(&a.asset_path)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        Ok(())
    }

    // --- hotkeys e teclado ----------------------------------------------------

    pub async fn hotkeys(&mut self) -> Result<Vec<HotkeyRow>, RepoError> {
        let rows = self
            .all("SELECT id, action, trigger_json, device_guid FROM system_hotkeys ORDER BY id")
            .await?;
        rows.iter()
            .map(|r| {
                Ok(HotkeyRow {
                    id: col(r, "id")?,
                    action: col(r, "action")?,
                    trigger_json: col(r, "trigger_json")?,
                    device_guid: col(r, "device_guid")?,
                })
            })
            .collect()
    }

    pub async fn delete_hotkey(&mut self, id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM system_hotkeys WHERE id = ?1")
            .bind(id)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// `id` = ação, como no `SystemHotkeysRepo`, e sem `device_guid`. Insere
    /// sem substituir: a mesma ação duas vezes é erro.
    pub async fn insert_hotkey(
        &mut self,
        action: &str,
        trigger_json: &str,
    ) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO system_hotkeys (id, action, trigger_json, device_guid) \
             VALUES (?1, ?1, ?2, NULL)",
        )
        .bind(action)
        .bind(trigger_json)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// `(alvo, KeyboardEvent.code)` de `keyboard_bindings`.
    pub async fn keyboard_bindings(&mut self) -> Result<Vec<(String, String)>, RepoError> {
        let rows = self
            .all("SELECT target, code FROM keyboard_bindings ORDER BY target")
            .await?;
        rows.iter()
            .map(|r| Ok((col(r, "target")?, col(r, "code")?)))
            .collect()
    }

    pub async fn clear_keyboard_bindings(&mut self) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM keyboard_bindings")
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Insere sem substituir: o mesmo alvo duas vezes é erro.
    pub async fn insert_keyboard_binding(
        &mut self,
        target: &str,
        code: &str,
    ) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO keyboard_bindings (target, code) VALUES (?1, ?2)")
            .bind(target)
            .bind(code)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    // --- áudio e vídeo --------------------------------------------------------

    pub async fn audio(&mut self) -> Result<AudioSettings, RepoError> {
        let r = sqlx::query(
            "SELECT rate_control_enabled, rate_control_delta, sample_rate_preference \
             FROM audio_config WHERE id = 1",
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(be)?
        .ok_or_else(|| RepoError::Corrupt("audio_config sem a linha id=1".into()))?;
        Ok(AudioSettings {
            rate_control_enabled: col(&r, "rate_control_enabled")?,
            rate_control_delta: col(&r, "rate_control_delta")?,
            sample_rate_preference: col(&r, "sample_rate_preference")?,
        })
    }

    /// Só as três colunas portáteis; o aparelho de saída fica.
    pub async fn set_audio(&mut self, a: &AudioSettings) -> Result<(), RepoError> {
        let done = sqlx::query(
            "UPDATE audio_config SET rate_control_enabled = ?1, rate_control_delta = ?2, \
             sample_rate_preference = ?3 WHERE id = 1",
        )
        .bind(a.rate_control_enabled)
        .bind(a.rate_control_delta)
        .bind(a.sample_rate_preference)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::Corrupt("audio_config sem a linha id=1".into()));
        }
        Ok(())
    }

    pub async fn video(&mut self) -> Result<VideoSettings, RepoError> {
        let r = sqlx::query(
            "SELECT integer_scaling, hdr, hdr_highlights FROM video_config WHERE id = 1",
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(be)?
        .ok_or_else(|| RepoError::Corrupt("video_config sem a linha id=1".into()))?;
        Ok(VideoSettings {
            integer_scaling: col(&r, "integer_scaling")?,
            hdr: col(&r, "hdr")?,
            hdr_highlights: col(&r, "hdr_highlights")?,
        })
    }

    /// Só as três colunas portáteis; os nits de HDR ficam.
    pub async fn set_video(&mut self, v: &VideoSettings) -> Result<(), RepoError> {
        let done = sqlx::query(
            "UPDATE video_config SET integer_scaling = ?1, hdr = ?2, hdr_highlights = ?3 \
             WHERE id = 1",
        )
        .bind(v.integer_scaling)
        .bind(v.hdr)
        .bind(v.hdr_highlights)
        .execute(&mut *self.tx)
        .await
        .map_err(be)?;
        if done.rows_affected() == 0 {
            return Err(RepoError::Corrupt("video_config sem a linha id=1".into()));
        }
        Ok(())
    }

    pub async fn aspects(&mut self) -> Result<Vec<AspectRow>, RepoError> {
        let rows = self
            .all("SELECT scope, target, aspect FROM aspect_assignments ORDER BY scope, target")
            .await?;
        rows.iter()
            .map(|r| {
                Ok(AspectRow {
                    scope: col(r, "scope")?,
                    target: col(r, "target")?,
                    aspect: col(r, "aspect")?,
                })
            })
            .collect()
    }

    pub async fn delete_aspect(&mut self, scope: &str, target: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM aspect_assignments WHERE scope = ?1 AND target = ?2")
            .bind(scope)
            .bind(target)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Insere sem substituir: o mesmo escopo e alvo duas vezes é erro.
    pub async fn insert_aspect(&mut self, a: &AspectRow) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO aspect_assignments (scope, target, aspect) VALUES (?1, ?2, ?3)")
            .bind(&a.scope)
            .bind(&a.target)
            .bind(&a.aspect)
            .execute(&mut *self.tx)
            .await
            .map_err(be)?;
        Ok(())
    }
}
