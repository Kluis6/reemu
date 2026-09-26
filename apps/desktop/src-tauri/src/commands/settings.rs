//! Configs persistidas (crates/db): áudio e vídeo.

use super::*;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfigDto {
    pub output_device_id: Option<String>,
    pub output_device_name: Option<String>,
    pub rate_control_enabled: bool,
    pub rate_control_delta: f32,
    pub sample_rate_preference: Option<u32>,
}

impl From<AudioConfig> for AudioConfigDto {
    fn from(c: AudioConfig) -> Self {
        Self {
            output_device_id: c.output_device_id,
            output_device_name: c.output_device_name,
            rate_control_enabled: c.rate_control_enabled,
            rate_control_delta: c.rate_control_delta,
            sample_rate_preference: c.sample_rate_preference,
        }
    }
}

impl From<AudioConfigDto> for AudioConfig {
    fn from(c: AudioConfigDto) -> Self {
        Self {
            output_device_id: c.output_device_id,
            output_device_name: c.output_device_name,
            rate_control_enabled: c.rate_control_enabled,
            rate_control_delta: c.rate_control_delta,
            sample_rate_preference: c.sample_rate_preference,
        }
    }
}

#[tauri::command]
pub async fn get_audio_config(state: State<'_, AppState>) -> Result<AudioConfigDto, String> {
    let repo = db::AudioConfigRepo::new(pool(&state)?);
    repo.get().await.map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_audio_config(
    state: State<'_, AppState>,
    config: AudioConfigDto,
) -> Result<(), String> {
    let cfg: AudioConfig = config.into();
    let repo = db::AudioConfigRepo::new(pool(&state)?);
    repo.update(&cfg).await.map_err(|e| e.to_string())?;

    // Aplica ao vivo: recria o sink na thread do core com a config nova.
    let cfg2 = cfg.clone();
    let session = state.session.clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        session.reload_audio(Box::new(move || {
            match audio_desktop::CpalAudioSink::new(&cfg2) {
                Ok(s) => Some(Box::new(s) as _),
                Err(e) => {
                    log::error!("áudio indisponível ({e}) — rodando sem som");
                    None
                }
            }
        }))
    })
    .await
    .map_err(|e| e.to_string())?;
    r.map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoConfigDto {
    pub integer_scaling: bool,
}

impl From<VideoConfig> for VideoConfigDto {
    fn from(c: VideoConfig) -> Self {
        Self {
            integer_scaling: c.integer_scaling,
        }
    }
}

impl From<VideoConfigDto> for VideoConfig {
    fn from(c: VideoConfigDto) -> Self {
        Self {
            integer_scaling: c.integer_scaling,
        }
    }
}

#[tauri::command]
pub async fn get_video_config(state: State<'_, AppState>) -> Result<VideoConfigDto, String> {
    let repo = db::VideoConfigRepo::new(pool(&state)?);
    repo.get().await.map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_video_config(
    state: State<'_, AppState>,
    config: VideoConfigDto,
) -> Result<(), String> {
    let cfg: VideoConfig = config.into();
    let repo = db::VideoConfigRepo::new(pool(&state)?);
    repo.update(&cfg).await.map_err(|e| e.to_string())?;

    // Aplica ao vivo: o próximo `render_to_surface` já lê o valor novo.
    if let Some(fp) = state.gpu.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
        fp.set_integer_scaling(cfg.integer_scaling);
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCoreDto {
    pub core_id: String,
    /// Nome amigável do core (`retro_system_info.library_name`).
    pub name: String,
    pub version: String,
    /// Extensões de ROM aceitas, sem ponto.
    pub extensions: Vec<String>,
    /// Backend de render detectado num load anterior (`installed_cores`, se houver).
    pub render_backend: Option<String>,
    /// Sistemas (ids da varredura) que o core atende, pelo catálogo — pra
    /// escolher o core de um jogo pelo sistema antes da extensão.
    pub systems: Vec<String>,
}

/// Cores disponíveis: varre `<dados>/cores/*_libretro.<suf>` e cruza com a
/// tabela `installed_cores` (que guarda o backend de render detectado no
/// primeiro load).
#[tauri::command]
pub async fn list_installed_cores(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledCoreDto>, String> {
    let discovered = emu_session::discover_cores(&state.cores_dir);

    let backends: std::collections::HashMap<String, Option<String>> = match state.db.as_ref() {
        Some(pool) => db::InstalledCoresRepo::new(pool.clone())
            .list()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| {
                (
                    c.core_id,
                    c.render_requirements
                        .map(|r| format!("{:?}", r.render_backend)),
                )
            })
            .collect(),
        None => Default::default(),
    };

    Ok(discovered
        .into_iter()
        .map(|c| InstalledCoreDto {
            name: if c.library_name.is_empty() {
                c.core_id.clone()
            } else {
                c.library_name
            },
            version: c.library_version,
            extensions: c.valid_extensions,
            render_backend: backends.get(&c.core_id).cloned().flatten(),
            systems: crate::core_catalog::system_ids(&c.core_id)
                .into_iter()
                .map(String::from)
                .collect(),
            core_id: c.core_id,
        })
        .collect())
}
