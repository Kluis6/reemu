//! Save states (etapa 08).

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStateDto {
    pub id: String,
    pub slot: Option<u32>,
    pub created_at: i64,
    pub file_path: String,
    pub has_thumbnail: bool,
}

fn save_dto(m: domain::save_state::SaveStateMetadata) -> SaveStateDto {
    SaveStateDto {
        id: m.id,
        slot: m.slot,
        created_at: m.created_at,
        file_path: m.file_path,
        has_thumbnail: m.thumbnail_path.is_some(),
    }
}

/// Reduz o frame RGBA8 pra no máximo `max_w` de largura (nearest) e codifica
/// como PNG — o thumbnail do save state.
pub(super) fn thumbnail_png(w: u32, h: u32, rgba: &[u8], max_w: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
        return None;
    }
    let scale = (w as f32 / max_w as f32).max(1.0);
    let (tw, th) = ((w as f32 / scale) as u32, (h as f32 / scale) as u32);
    let (tw, th) = (tw.max(1), th.max(1));
    let mut small = vec![0u8; (tw * th * 4) as usize];
    for y in 0..th {
        let sy = (y as f32 * scale) as u32;
        for x in 0..tw {
            let sx = (x as f32 * scale) as u32;
            let si = ((sy.min(h - 1) * w + sx.min(w - 1)) * 4) as usize;
            let di = ((y * tw + x) * 4) as usize;
            small[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, tw, th);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().ok()?.write_image_data(&small).ok()?;
    Some(out)
}

#[tauri::command]
pub async fn save_state(
    state: State<'_, AppState>,
    rom_id: String,
    slot: Option<u32>,
) -> Result<SaveStateDto, String> {
    let core_id = state
        .session
        .loaded_core()
        .ok_or_else(|| "nenhum core carregado".to_string())?;
    let bytes = state
        .session
        .save_state()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "o core não suporta save state".to_string())?;
    let thumb = {
        let f = state.last_frame.lock().unwrap_or_else(|p| p.into_inner());
        f.as_ref()
            .and_then(|c| thumbnail_png(c.w, c.h, &c.rgba, 320))
    };
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::save(
        &repo,
        &state.save_dir,
        &rom_id,
        &core_id,
        slot,
        &bytes,
        thumb.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(save_dto(meta))
}

/// PNG do thumbnail de um save state (corpo vazio se não tem).
#[tauri::command]
pub async fn read_save_thumbnail(
    state: State<'_, AppState>,
    state_id: String,
) -> Result<tauri::ipc::Response, String> {
    use domain::save_state::SaveStateRepository;
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let path = repo
        .get_state(&state_id)
        .await
        .map_err(|e| e.to_string())?
        .and_then(|m| m.thumbnail_path);
    let bytes = match path {
        Some(p) => std::fs::read(&p).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn list_save_states(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<Vec<SaveStateDto>, String> {
    let repo = db::SaveStateRepo::new(pool(&state)?);
    Ok(save_svc::list(&repo, &rom_id)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(save_dto)
        .collect())
}

#[tauri::command]
pub async fn load_save_state(state: State<'_, AppState>, state_id: String) -> Result<(), String> {
    let running = state.session.loaded_core();
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::load_bytes(&repo, &state_id, running.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&meta.file_path).map_err(|e| e.to_string())?;
    if state
        .session
        .restore_state(bytes)
        .map_err(|e| e.to_string())?
    {
        Ok(())
    } else {
        Err("retro_unserialize recusou o state".to_string())
    }
}

#[tauri::command]
pub async fn delete_save_state(state: State<'_, AppState>, state_id: String) -> Result<(), String> {
    let repo = db::SaveStateRepo::new(pool(&state)?);
    save_svc::delete(&repo, &state_id)
        .await
        .map_err(|e| e.to_string())
}
