//! Cheats por código (docs/ai-context/17-cheats-rewind.md, fase 1): a lista
//! de cada jogo fica no banco (`cheats`) e vai pro core ao carregar o jogo e a
//! cada mudança no jogo aberto (`EmuSession::set_cheats` → `retro_cheat_set`).

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheatDto {
    pub id: i64,
    pub rom_id: String,
    pub description: String,
    pub code: String,
    pub enabled: bool,
}

impl From<domain::cheats::Cheat> for CheatDto {
    fn from(c: domain::cheats::Cheat) -> Self {
        Self {
            id: c.id,
            rom_id: c.rom_id,
            description: c.description,
            code: c.code,
            enabled: c.enabled,
        }
    }
}

/// Manda a lista do jogo pro core. Chamado no load e depois de cada mudança.
pub(super) async fn apply_cheats(state: &AppState, rom_id: &str) {
    let Some(pool) = state.db.clone() else {
        return;
    };
    match db::CheatsRepo::new(pool).list(rom_id).await {
        Ok(list) => {
            let codes: Vec<_> = list.iter().map(|c| c.to_code()).collect();
            let on = codes.iter().filter(|c| c.enabled).count();
            if !codes.is_empty() {
                log::info!("cheats: {on} de {} ligados", codes.len());
            }
            let session = Arc::clone(&state.session);
            let _ = tauri::async_runtime::spawn_blocking(move || session.set_cheats(codes)).await;
        }
        Err(e) => log::warn!("cheats: lendo a lista do jogo: {e}"),
    }
}

/// Reaplica só se a mudança foi no jogo que está aberto.
async fn reapply_if_current(state: &AppState, rom_id: &str) {
    let current = state
        .current_rom
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if current.as_deref() == Some(rom_id) {
        apply_cheats(state, rom_id).await;
    }
}

/// Código sem espaços nas pontas; vazio é recusado. Descrição vazia vira o
/// próprio código.
fn clean(description: &str, code: &str) -> Result<(String, String), String> {
    let code = code.trim();
    if code.is_empty() {
        return Err("o código do cheat está vazio".into());
    }
    let description = description.trim();
    let description = if description.is_empty() {
        code
    } else {
        description
    };
    Ok((description.to_string(), code.to_string()))
}

#[tauri::command]
pub async fn list_cheats(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<Vec<CheatDto>, String> {
    let repo = db::CheatsRepo::new(pool(&state)?);
    let list = repo.list(&rom_id).await.map_err(|e| e.to_string())?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn add_cheat(
    state: State<'_, AppState>,
    rom_id: String,
    description: String,
    code: String,
) -> Result<CheatDto, String> {
    let (description, code) = clean(&description, &code)?;
    let repo = db::CheatsRepo::new(pool(&state)?);
    // entra ligado: quem acrescenta um código quer usar
    let cheat = repo
        .add(&rom_id, &description, &code, true)
        .await
        .map_err(|e| e.to_string())?;
    reapply_if_current(&state, &rom_id).await;
    Ok(cheat.into())
}

#[tauri::command]
pub async fn update_cheat(
    state: State<'_, AppState>,
    id: i64,
    description: String,
    code: String,
    enabled: bool,
) -> Result<CheatDto, String> {
    let (description, code) = clean(&description, &code)?;
    let repo = db::CheatsRepo::new(pool(&state)?);
    let cheat = repo
        .update(id, &description, &code, enabled)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("cheat não encontrado")?;
    reapply_if_current(&state, &cheat.rom_id).await;
    Ok(cheat.into())
}

#[tauri::command]
pub async fn delete_cheat(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let repo = db::CheatsRepo::new(pool(&state)?);
    if let Some(rom_id) = repo.delete(id).await.map_err(|e| e.to_string())? {
        reapply_if_current(&state, &rom_id).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clean;

    #[test]
    fn code_is_trimmed_and_empty_description_falls_back_to_code() {
        assert_eq!(
            clean("  ", " 7E0DBF09 ").unwrap(),
            ("7E0DBF09".to_string(), "7E0DBF09".to_string())
        );
        assert_eq!(
            clean(" Vidas ", "7E0DBF09").unwrap(),
            ("Vidas".to_string(), "7E0DBF09".to_string())
        );
        assert!(clean("Vidas", "   ").is_err());
    }
}
