//! Catálogo de cores (etapa 10, MVP).

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCoreDto {
    pub core_id: String,
    pub name: String,
    pub systems: String,
    pub license: String,
    pub installed: bool,
    /// `"software"` = buffer cru; `"opengl"` = renderiza em GL (o frontend cria
    /// o contexto offscreen; precisa de GPU + libEGL).
    pub hw: &'static str,
    /// Tem versão anterior guardada, de uma atualização.
    pub has_previous: bool,
}

#[tauri::command]
pub fn list_core_catalog(state: State<'_, AppState>) -> Vec<CatalogCoreDto> {
    // Só os nomes dos arquivos: o catálogo não precisa carregar as DLLs
    // (ver `core_loader_desktop::discover`).
    let installed: std::collections::HashSet<String> =
        emu_session::installed_core_ids(&state.cores_dir)
            .into_iter()
            .collect();
    crate::core_catalog::CATALOG
        .iter()
        .map(|e| CatalogCoreDto {
            core_id: e.id.to_string(),
            name: e.name.to_string(),
            systems: e.systems.to_string(),
            license: e.license.to_string(),
            installed: installed.contains(e.id),
            hw: e.hw.as_str(),
            has_previous: crate::core_catalog::has_previous(&state.cores_dir, e.id),
        })
        .collect()
}

#[tauri::command]
pub async fn download_core(state: State<'_, AppState>, core_id: String) -> Result<(), String> {
    let dir = state.cores_dir.clone();
    crate::core_catalog::download(&dir, &core_id).await?;
    log::info!("core instalado: {core_id}");
    Ok(())
}

#[tauri::command]
pub fn remove_core(state: State<'_, AppState>, core_id: String) -> Result<(), String> {
    crate::core_catalog::remove(&state.cores_dir, &core_id)
}

/// Os cores instalados com versão nova no buildbot (`core_updates`).
#[tauri::command]
pub async fn check_core_updates(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let dir = state.cores_dir.clone();
    crate::core_updates::check(&dir).await
}

/// Volta um core para a versão guardada na última atualização.
#[tauri::command]
pub fn restore_previous_core(state: State<'_, AppState>, core_id: String) -> Result<(), String> {
    crate::core_catalog::restore_previous(&state.cores_dir, &core_id)?;
    log::info!("core de volta à versão anterior: {core_id}");
    Ok(())
}
