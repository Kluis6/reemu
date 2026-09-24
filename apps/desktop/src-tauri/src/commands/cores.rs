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
}

#[tauri::command]
pub fn list_core_catalog(state: State<'_, AppState>) -> Vec<CatalogCoreDto> {
    let installed: std::collections::HashSet<String> =
        emu_session::discover_cores(&state.cores_dir)
            .into_iter()
            .map(|c| c.core_id)
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
