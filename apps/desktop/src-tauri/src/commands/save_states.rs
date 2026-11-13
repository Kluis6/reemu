//! Save states (etapa 08).

use super::*;
use crate::app_error::AppError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "SaveState"))]
pub struct SaveStateDto {
    pub id: String,
    pub slot: Option<u32>,
    pub created_at: i64,
    pub file_path: String,
    pub has_thumbnail: bool,
    /// Tempo de jogo da ROM (s) quando o estado foi salvo. `None` = save
    /// antigo, de antes do relógio existir.
    pub play_time_at_save: Option<u64>,
}

/// Um save state em disco precisa carregar numa sessão futura: o de core com
/// `SINGLE_SESSION` "can only be loaded during the same session"
/// (libretro.h), então não é gravado.
pub(super) fn check_disk_savable(state: &AppState) -> Result<(), AppError> {
    use domain::core_loader::serialization_quirk as q;
    if state.session.serialization_quirks() & q::SINGLE_SESSION != 0 {
        return Err(
            "este core não permite salvar o estado em disco: o estado dele só vale enquanto o \
             jogo está aberto"
                .into(),
        );
    }
    Ok(())
}

/// Por que o core não devolveu um estado: o de `MUST_INITIALIZE` falha
/// "initially" (libretro.h), antes de terminar de iniciar o jogo.
pub(super) fn no_state_error(state: &AppState) -> String {
    use domain::core_loader::serialization_quirk as q;
    if state.session.serialization_quirks() & q::MUST_INITIALIZE != 0 {
        "o core ainda está iniciando o jogo; tente salvar de novo em instantes".into()
    } else {
        "o core não suporta save state".into()
    }
}

fn save_dto(m: domain::save_state::SaveStateMetadata) -> SaveStateDto {
    SaveStateDto {
        id: m.id,
        slot: m.slot,
        created_at: m.created_at,
        file_path: m.file_path,
        has_thumbnail: m.thumbnail_path.is_some(),
        play_time_at_save: m.play_time_at_save,
    }
}

/// PNG da miniatura do save state: o jogo no momento do save.
///
/// No modo canvas é o último frame mandado pro `<canvas>` (`last_frame`). No
/// vídeo nativo (janela/subsurface própria) o frame não passa pelo canvas e o
/// `last_frame` fica vazio — aí vale o frame capturado quando o menu de pausa
/// abriu (`pause_bg`, o que estava na tela) ou, fora do menu (QuickSave pela
/// hotkey) e no protótipo de overlay, uma captura na hora da saída da cadeia
/// de shader (`capture_surface_frame`).
pub(super) fn save_thumbnail(state: &AppState) -> Option<Vec<u8>> {
    let canvas = {
        let f = state.last_frame.lock().unwrap_or_else(|p| p.into_inner());
        f.as_ref()
            .and_then(|c| thumbnail_png(c.w, c.h, &c.rgba, 320))
    };
    if canvas.is_some() {
        return canvas;
    }
    let menu_up = matches!(
        *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()),
        VideoMenu::MenuUp
    );
    let paused = if menu_up {
        state
            .pause_bg
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    } else {
        None
    };
    let (w, h, rgba) = paused.or_else(|| {
        state
            .gpu
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
            .and_then(|fp| fp.capture_surface_frame())
    })?;
    thumbnail_png(w, h, &rgba, 320)
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
) -> Result<SaveStateDto, AppError> {
    let core_id = state
        .session
        .loaded_core()
        .ok_or_else(|| "nenhum core carregado".to_string())?;
    check_disk_savable(&state)?;
    let bytes = state
        .session
        .save_state()
        .map_err(AppError::from)?
        .ok_or_else(|| no_state_error(&state))?;
    let thumb = save_thumbnail(&state);
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::save(
        &repo,
        &state.save_dir,
        &rom_id,
        &core_id,
        slot,
        &bytes,
        thumb.as_deref(),
        crate::play_clock::total(&state, &rom_id).await,
    )
    .await
    .map_err(AppError::from)?;
    Ok(save_dto(meta))
}

/// PNG do thumbnail de um save state (corpo vazio se não tem).
#[tauri::command]
pub async fn read_save_thumbnail(
    state: State<'_, AppState>,
    state_id: String,
) -> Result<tauri::ipc::Response, AppError> {
    use domain::save_state::SaveStateRepository;
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let path = repo
        .get_state(&state_id)
        .await
        .map_err(AppError::from)?
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
) -> Result<Vec<SaveStateDto>, AppError> {
    let repo = db::SaveStateRepo::new(pool(&state)?);
    Ok(save_svc::list(&repo, &rom_id)
        .await
        .map_err(AppError::from)?
        .into_iter()
        .map(save_dto)
        .collect())
}

#[tauri::command]
pub async fn load_save_state(state: State<'_, AppState>, state_id: String) -> Result<(), AppError> {
    let running = state.session.loaded_core();
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::load_bytes(&repo, &state_id, running.as_deref())
        .await
        .map_err(AppError::from)?;
    let bytes = std::fs::read(&meta.file_path).map_err(AppError::from)?;
    if state.session.restore_state(bytes).map_err(AppError::from)? {
        Ok(())
    } else {
        Err("retro_unserialize recusou o state".into())
    }
}

#[tauri::command]
pub async fn delete_save_state(
    state: State<'_, AppState>,
    state_id: String,
) -> Result<(), AppError> {
    let repo = db::SaveStateRepo::new(pool(&state)?);
    save_svc::delete(&repo, &state_id)
        .await
        .map_err(AppError::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSaveStateDto {
    pub id: String,
    pub rom_id: String,
    pub rom_title: String,
    pub slot: Option<u32>,
    pub created_at: i64,
    pub play_time_at_save: Option<u64>,
}

/// Os save states mais recentes com miniatura, de todos os jogos. Estado cuja
/// ROM saiu da biblioteca fica de fora (no SQL), e o de miniatura que sumiu do
/// disco também (aqui). A miniatura carrega por `read_save_thumbnail`.
#[tauri::command]
pub async fn list_recent_save_states(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<RecentSaveStateDto>, AppError> {
    let pool = pool(&state)?;
    recent_states_with_titles(
        &db::SaveStateRepo::new(pool.clone()),
        &db::RomsRepo::new(pool),
        limit,
    )
    .await
}

/// Junta cada estado recente ao título da sua ROM. Pula o estado cuja
/// miniatura não está mais no disco (seria um quadro vazio na fileira).
async fn recent_states_with_titles(
    states: &db::SaveStateRepo,
    roms: &db::RomsRepo,
    limit: u32,
) -> Result<Vec<RecentSaveStateDto>, AppError> {
    use domain::library::RomRepository;
    let recent = states
        .recent_with_thumbnail(limit.clamp(1, 24))
        .await
        .map_err(AppError::from)?;
    let mut out = Vec::with_capacity(recent.len());
    for m in recent {
        let thumbnail_on_disk = m
            .thumbnail_path
            .as_deref()
            .is_some_and(|p| std::path::Path::new(p).exists());
        if !thumbnail_on_disk {
            continue;
        }
        // O SQL já tira o estado de ROM ausente; este `continue` cobre a ROM
        // removida entre a lista e esta leitura.
        let Some(rom) = roms.get(&m.rom_id).await.map_err(AppError::from)? else {
            continue;
        };
        out.push(RecentSaveStateDto {
            rom_title: rom_title(&rom),
            id: m.id,
            rom_id: m.rom_id,
            slot: m.slot,
            created_at: m.created_at,
            play_time_at_save: m.play_time_at_save,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::library::{Rom, RomRepository};
    use domain::save_state::{SaveStateMetadata, SaveStateRepository};

    fn meta(
        id: &str,
        rom_id: &str,
        created_at: i64,
        thumbnail: &std::path::Path,
    ) -> SaveStateMetadata {
        SaveStateMetadata {
            id: id.into(),
            rom_id: rom_id.into(),
            core_id: "core".into(),
            slot: Some(0),
            file_path: format!("/saves/{id}.state"),
            thumbnail_path: Some(thumbnail.to_string_lossy().into_owned()),
            created_at,
            play_time_at_save: None,
        }
    }

    /// Miniatura apagada do disco: o estado some da lista e o resto volta
    /// com o título da ROM.
    #[tokio::test]
    async fn recent_states_skip_a_state_whose_thumbnail_file_is_gone() {
        let dir = std::env::temp_dir().join(format!("reemu-recent-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let present = dir.join("ok.png");
        std::fs::write(&present, b"png").unwrap();
        let missing = dir.join("sumiu.png");
        let _ = std::fs::remove_file(&missing);

        let pool = db::connect_in_memory().await.unwrap();
        let roms = db::RomsRepo::new(pool.clone());
        let states = db::SaveStateRepo::new(pool);
        roms.add(&Rom {
            id: "vivo".into(),
            file_path: "/roms/Super Game.zip".into(),
            crc32: "C".into(),
            md5: "M".into(),
            system_id: "nes".into(),
            added_at: 0,
            last_played_at: None,
            is_favorite: false,
            user_title: None,
        })
        .await
        .unwrap();
        states
            .record_state(&meta("ok", "vivo", 100, &present))
            .await
            .unwrap();
        states
            .record_state(&meta("sumiu", "vivo", 200, &missing))
            .await
            .unwrap();

        let out = recent_states_with_titles(&states, &roms, 10).await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            out.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["ok"]
        );
        assert_eq!(out[0].rom_title, "Super Game");
    }
}
