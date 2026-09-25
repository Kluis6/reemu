//! Carregar/descarregar jogo, frames de vídeo, decoração/bezel e janela.

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedGame {
    base_width: u32,
    base_height: u32,
    fps: f64,
    sample_rate: f64,
    /// Proporção de exibição pedida pelo core (ex: ~1.306 pra SNES 4:3). `0`
    /// = usar `base_width/base_height`.
    aspect_ratio: f32,
}

#[tauri::command]
pub async fn load_game(
    app: AppHandle,
    core_id: String,
    rom_path: String,
    rom_id: Option<String>,
) -> Result<LoadedGame, String> {
    use domain::core_options::CoreOptionsStore;
    let state = app.state::<AppState>();
    let session = Arc::clone(&state.session);

    // Vídeo nativo: volta pro estado "jogando" e descarta o print do menu. A
    // subsurface fica escondida pelo `reemu-video-pump` (dono único da conexão
    // Wayland) durante todo o load — senão o último frame do jogo anterior fica
    // grudado no `wl_surface` por cima da tela de "Carregando…".
    state
        .loading_game
        .store(true, std::sync::atomic::Ordering::Relaxed);
    *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()) = VideoMenu::Playing;
    *state.pause_bg.lock().unwrap_or_else(|p| p.into_inner()) = None;

    // Valores de opção salvos — o filho os aplica já durante o load (ele
    // pede via `GET_VARIABLE`), mandados junto no `EmuSession::load`. Cascata:
    // valores por core, com os overrides deste jogo por cima.
    let initial_option_values = match state.db.clone() {
        Some(pool) => {
            let repo = db::CoreOptionsRepo::new(pool);
            let mut v = repo.values_for(&core_id).await.unwrap_or_default();
            if let Some(rid) = &rom_id {
                for (k, val) in repo
                    .overrides_for_rom(rid, &core_id)
                    .await
                    .unwrap_or_default()
                {
                    v.insert(k, val);
                }
            }
            v
        }
        None => Default::default(),
    };

    // O load é bloqueante (spawna o processo filho + espera o `Loaded`);
    // tira da thread async.
    let av = {
        let core_id = core_id.clone();
        let joined = tauri::async_runtime::spawn_blocking(move || {
            session.load(&core_id, &rom_path, initial_option_values)
        })
        .await;
        match joined {
            Ok(Ok(av)) => av,
            Ok(Err(e)) => {
                state
                    .loading_game
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                return Err(e.to_string());
            }
            Err(e) => {
                state
                    .loading_game
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                return Err(e.to_string());
            }
        }
    };

    // Persiste o schema que o core declarou (repopula todo load). Antes,
    // garante a linha em `installed_cores` — `core_options_schema.core_id` tem
    // FK pra ela, e a descoberta de cores (varredura de disco) não registra
    // nada sozinha.
    if let Some(pool) = state.db.clone() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let cores = db::InstalledCoresRepo::new(pool.clone());
        if matches!(cores.get(&core_id).await, Ok(None)) {
            let version = emu_session::discover_cores(&state.cores_dir)
                .into_iter()
                .find(|c| c.core_id == core_id)
                .map(|c| c.library_version)
                .unwrap_or_default();
            if let Err(e) = cores
                .register(&domain::core_loader::InstalledCore {
                    core_id: core_id.clone(),
                    version,
                    installed_at: now,
                    render_requirements: None,
                })
                .await
            {
                log::warn!("registrando core em installed_cores: {e}");
            }
        }

        let (schema, _) = state.session.core_options();
        if let Err(e) = db::CoreOptionsRepo::new(pool)
            .replace_schema(&core_id, &schema)
            .await
        {
            log::warn!("salvando schema de core options: {e}");
        }
    }

    // "Continuar jogando" — marca a hora do load.
    if let (Some(pool), Some(rid)) = (state.db.clone(), rom_id.as_deref()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let _ = db::RomsRepo::new(pool).mark_played(rid, now).await;
    }

    // Shader atribuído (rom → sistema → default, ou `plain`); depois a
    // decoração — pulada se o shader já desenha a própria moldura.
    let shader_has_bezel = apply_resolved_shader(&state, rom_id.as_deref()).await;
    apply_resolved_decoration(&state, rom_id.as_deref(), shader_has_bezel).await;

    let display_aspect = {
        let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        guard
            .as_ref()
            .and_then(|fp| fp.decoration_aspect())
            .unwrap_or(av.geometry.aspect_ratio)
    };

    // Guarda o `rom_id` (do DB) pro QuickSave/QuickLoad; `None` se o jogo veio
    // de fora da biblioteca.
    *state.current_rom.lock().unwrap_or_else(|p| p.into_inner()) = rom_id;
    // Load concluído: o pump pode voltar a apresentar a subsurface (agora com
    // frames do jogo NOVO — `latest_frame` foi zerado no `Command::Load`).
    state
        .loading_game
        .store(false, std::sync::atomic::Ordering::Relaxed);
    Ok(LoadedGame {
        base_width: av.geometry.base_width,
        base_height: av.geometry.base_height,
        fps: av.timing.fps,
        sample_rate: av.timing.sample_rate,
        aspect_ratio: display_aspect,
    })
}

/// Traduz `scope` ("default" | "system" | "rom") + alvos num
/// `(AssignmentScope, system_id, rom_id)` pros repos de shader. Pro escopo
/// `"system"` aceita `system_id` direto ou deriva de `rom_id`.
pub(super) async fn shader_scope_args(
    pool: &db::Db,
    scope: &str,
    system_id: Option<&str>,
    rom_id: Option<&str>,
) -> Result<(AssignmentScope, Option<String>, Option<String>), String> {
    match scope {
        "default" => Ok((AssignmentScope::Default, None, None)),
        "system" => {
            let sys = match system_id {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    let rid = rom_id.ok_or("scope 'system' precisa de system_id ou rom_id")?;
                    let s = system_of(pool, Some(rid)).await;
                    if s.is_empty() {
                        return Err("não consegui achar o sistema do jogo".into());
                    }
                    s
                }
            };
            Ok((AssignmentScope::System, Some(sys), None))
        }
        "rom" => Ok((
            AssignmentScope::Rom,
            None,
            Some(rom_id.ok_or("scope 'rom' precisa de rom_id")?.to_string()),
        )),
        other => Err(format!("scope desconhecido '{other}'")),
    }
}

/// `system_id` de uma rom (ou `""` sem rom).
async fn system_of(pool: &db::Db, rom_id: Option<&str>) -> String {
    match rom_id {
        Some(rid) => db::RomsRepo::new(pool.clone())
            .get(rid)
            .await
            .ok()
            .flatten()
            .map(|r| r.system_id)
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// Resolve o shader do jogo e aplica no `FrameProcessor`. Devolve se o preset
/// ativo desenha a própria moldura (`includes_bezel`) — pra exclusão mútua.
async fn apply_resolved_shader(state: &AppState, rom_id: Option<&str>) -> bool {
    apply_resolved_shader_ex(state, rom_id, false).await
}

/// Resolve o shader do jogo (cascata rom→sistema→default), aplica o preset e os
/// overrides de parâmetro no processador GPU. `force` recarrega o preset mesmo
/// se já for o ativo (usado ao "restaurar padrões" — o `set_preset` zera os
/// params antes dos overrides). Devolve `includes_bezel` (exclusão mútua).
pub(super) async fn apply_resolved_shader_ex(
    state: &AppState,
    rom_id: Option<&str>,
    force: bool,
) -> bool {
    use domain::shader_chain::ShaderChainResolver;
    let Some(pool) = state.db.clone() else {
        return false;
    };
    let system = system_of(&pool, rom_id).await;
    let sc = db::ShaderChainRepo::new(pool);
    let (target, has_bezel, overrides) = match sc.resolve(&system, rom_id).await {
        Ok(Some(a)) => {
            let (src, bezel) = sc
                .list_presets()
                .await
                .ok()
                .and_then(|ps| ps.into_iter().find(|p| p.id == a.preset_id))
                .map(|p| (p.source_path, p.includes_bezel))
                .unwrap_or_else(|| ("plain".into(), false));
            (src, bezel, a.parameter_overrides)
        }
        _ => ("plain".into(), false, Default::default()),
    };
    let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(fp) = guard.as_mut() {
        if force || fp.preset_source() != target {
            if let Err(e) = fp.set_preset(&target) {
                log::warn!("shader do jogo: {e}");
            }
        }
        for (k, v) in &overrides {
            if let Ok(val) = v.parse::<f32>() {
                fp.set_shader_param(k, val);
            }
        }
    }
    has_bezel
}

/// Resolve a decoração do jogo (`DecorationRepo`) e aplica no `FrameProcessor`.
/// Pulada quando `shader_has_bezel` (exclusão mútua Mega Bezel × decoração).
async fn apply_resolved_decoration(state: &AppState, rom_id: Option<&str>, shader_has_bezel: bool) {
    // Override de teste: REEMU_BEZEL=/caminho/bezel.png
    let want = if let Some(p) = std::env::var_os("REEMU_BEZEL") {
        let path = std::path::PathBuf::from(p);
        crate::decoration::decode_png(&path)
            .inspect_err(|e| log::warn!("REEMU_BEZEL: {e}"))
            .ok()
            .map(|(rgba, w, h)| {
                let vp = deco_viewport(&path, &rgba, w, h);
                log::info!("decoração: REEMU_BEZEL {} ({w}x{h})", path.display());
                (rgba, w, h, vp)
            })
    } else if shader_has_bezel {
        log::info!("decoração: pulada (shader ativo já desenha moldura)");
        None
    } else {
        resolve_decoration(state, rom_id).await
    };
    if want.is_none() {
        log::info!("decoração: nenhuma pra este jogo");
    }
    let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(fp) = guard.as_mut() {
        fp.set_decoration(want);
    } else {
        log::warn!("decoração: sem GPU");
    }
}

async fn resolve_decoration(
    state: &AppState,
    rom_id: Option<&str>,
) -> Option<(Vec<u8>, u32, u32, Option<crate::gpu::DecoViewport>)> {
    use domain::decoration::DecorationResolver;
    let pool = state.db.clone()?;
    let system = system_of(&pool, rom_id).await;
    let a = match db::DecorationRepo::new(pool).resolve(&system, rom_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            log::info!("decoração: sem atribuição (sistema '{system}', rom {rom_id:?})");
            return None;
        }
        Err(e) => {
            log::warn!("decoração: erro ao resolver: {e}");
            return None;
        }
    };
    log::info!("decoração: atribuição {:?} → {}", a.scope, a.asset_path);
    let path = std::path::Path::new(&a.asset_path);
    let (rgba, w, h) = crate::decoration::decode_png(path)
        .inspect_err(|e| log::warn!("decoração {}: {e}", a.asset_path))
        .ok()?;
    let vp = deco_viewport(path, &rgba, w, h);
    log::info!("decoração: {} ({w}x{h})", a.asset_path);
    Some((rgba, w, h, vp))
}

/// Retângulo do jogo dentro da moldura: usa o `custom_viewport_*` do `.cfg`
/// irmão se houver; senão descobre pela janela transparente da própria arte
/// (The Bezel Project não põe viewport nos `.cfg` dos packs "games"). Loga qual
/// fonte venceu — sem nenhuma das duas, o compositor centraliza com a AR do
/// core (só serve pra 4:3 de altura cheia).
fn deco_viewport(
    path: &std::path::Path,
    rgba: &[u8],
    w: u32,
    h: u32,
) -> Option<crate::gpu::DecoViewport> {
    if let Some(v) = library_scan::viewport_for_image(path) {
        log::info!(
            "decoração: viewport do .cfg ({},{} {}×{})",
            v.x,
            v.y,
            v.w,
            v.h
        );
        return Some(crate::gpu::DecoViewport {
            x: v.x as f32,
            y: v.y as f32,
            w: v.w as f32,
            h: v.h as f32,
        });
    }
    match crate::decoration::transparent_bbox(rgba, w, h) {
        Some(v) => {
            log::info!(
                "decoração: viewport pela transparência ({},{} {}×{})",
                v.x,
                v.y,
                v.w,
                v.h
            );
            Some(v)
        }
        None => {
            log::info!("decoração: sem viewport (fallback: centralizado, AR do core)");
            None
        }
    }
}

#[tauri::command]
pub async fn import_decoration_pack(
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, String> {
    let pool = pool(&state)?;
    crate::decoration::import_pack(&pool, std::path::Path::new(&path)).await
}

/// Remove todas as decorações (packs + atribuições) e tira a moldura ativa.
#[tauri::command]
pub async fn clear_decorations(state: State<'_, AppState>) -> Result<(), String> {
    use domain::decoration::DecorationStore;
    if let Some(pool) = state.db.clone() {
        db::DecorationRepo::new(pool)
            .clear_all()
            .await
            .map_err(|e| e.to_string())?;
    }
    let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(fp) = guard.as_mut() {
        fp.set_decoration(None);
    }
    Ok(())
}

/// Catálogo de bezels do The Bezel Project + quais já estão baixados.
#[tauri::command]
pub fn bezel_catalog(state: State<'_, AppState>) -> Vec<crate::bezel_pack::CatalogItem> {
    crate::bezel_pack::catalog(&state.decorations_dir)
}

/// Baixa o pack de bezels de `system_id` (The Bezel Project) e re-importa a
/// árvore. Devolve o total de atribuições gravadas.
#[tauri::command]
pub async fn download_bezel_pack(
    state: State<'_, AppState>,
    system_id: String,
    on_progress: tauri::ipc::Channel<crate::bezel_pack::DownloadProgress>,
) -> Result<usize, String> {
    let pool = pool(&state)?;
    let dir = state.decorations_dir.clone();
    crate::bezel_pack::download(&dir, &pool, &system_id, on_progress).await
}

#[tauri::command]
pub async fn unload_game(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let session = Arc::clone(&state.session);
    *state.current_rom.lock().unwrap_or_else(|p| p.into_inner()) = None;
    *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()) = VideoMenu::Playing;
    *state.pause_bg.lock().unwrap_or_else(|p| p.into_inner()) = None;
    // A subsurface é escondida pelo `reemu-video-pump` assim que a sessão fica
    // `Idle` (que `Command::Unload` marca antes do teardown bloqueante). Tocar
    // na conexão Wayland fora da thread do pump corrompe o `wl_display`
    // (o driver Vulkan da NVIDIA também escreve nela no present) → app fecha.
    let _ = state.session.take_latest_frame();
    tauri::async_runtime::spawn_blocking(move || session.unload())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// `true` quando o vídeo do jogo sai numa surface nativa atrás da webview
/// (subsurface anexada — padrão no Wayland) — a `PlayScreen` fica
/// transparente e não roda o loop do canvas.
#[tauri::command]
pub fn native_video_active(state: State<'_, AppState>) -> bool {
    state
        .video
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .is_some()
}

/// Tamanho do cabeçalho do `poll_frame`: `[w u32][h u32][deco_gen u32]
/// [retângulo do jogo na moldura: cx, cy, meia_l, meia_a em f32 NDC]`, tudo LE.
/// `deco_gen == 0` = sem moldura (o retângulo não vale).
const FRAME_HEADER: usize = 28;

fn frame_header(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER + w as usize * h as usize * 4);
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for v in [0.0f32, 0.0, 1.0, 1.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Frame mais recente do core como RGBA8, com o cabeçalho de
/// `FRAME_HEADER` bytes na frente. Corpo vazio = sem frame novo. A
/// `PlayScreen` consome num loop e pinta no canvas. Com moldura, o quadro é
/// SÓ o jogo: a moldura vem uma vez por `decoration_image` e o WebView
/// empilha as duas (ver `FrameProcessor::split_decoration`).
#[tauri::command]
pub fn poll_frame(state: State<'_, AppState>) -> tauri::ipc::Response {
    use domain::frame_source::{rotate_rgba, to_rgba8, to_rgba8_slice, FrameOrigin};
    let Some(frame) = state.session.take_latest_frame() else {
        return tauri::ipc::Response::new(Vec::new());
    };
    let rot = frame.metadata.rotation_degrees;

    // Caminho GPU (etapa 04 — shader chain). A rotação (`SET_ROTATION`) já é
    // aplicada dentro da chain (antes da moldura). Cai no CPU em qualquer falha.
    {
        let mut gpu = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(fp) = gpu.as_mut() {
            if let Some(packed) = fp.process_packed_split(&frame) {
                drop(gpu);
                // a GPU já copiou o quadro: o buffer volta pro pool da sessão
                state.session.recycle_frame(frame);
                let w = u32::from_le_bytes(packed[0..4].try_into().unwrap_or_default());
                let h = u32::from_le_bytes(packed[4..8].try_into().unwrap_or_default());
                cache_thumb_frame(&state, w, h, &packed[FRAME_HEADER..]);
                return tauri::ipc::Response::new(packed);
            }
        }
    }

    let FrameOrigin::SoftwareRawBuffer {
        ref data,
        pitch,
        format,
    } = frame.origin
    else {
        return tauri::ipc::Response::new(Vec::new());
    };
    let (w, h) = (frame.metadata.native_width, frame.metadata.native_height);
    if !matches!(rot, 90 | 180 | 270) {
        // Sem rotação (quase sempre): converte direto no `Vec` da resposta,
        // depois do cabeçalho — sem o RGBA intermediário nem a cópia do
        // `pack_frame`.
        let n = w as usize * h as usize * 4;
        let mut out = frame_header(w, h);
        out.resize(FRAME_HEADER + n, 0);
        to_rgba8_slice(&mut out[FRAME_HEADER..], data, w, h, pitch, format);
        state.session.recycle_frame(frame);
        cache_thumb_frame(&state, w, h, &out[FRAME_HEADER..]);
        return tauri::ipc::Response::new(out);
    }
    let rgba = to_rgba8(data, w, h, pitch, format);
    state.session.recycle_frame(frame);
    let (rgba, w, h) = rotate_rgba(rgba, w, h, rot);
    cache_thumb_frame(&state, w, h, &rgba);
    let mut out = frame_header(w, h);
    out.extend_from_slice(&rgba);
    tauri::ipc::Response::new(out)
}

/// Imagem da moldura ativa pro modo canvas: `[deco_gen u32][w u32][h u32]`
/// + RGBA8. Corpo vazio = sem moldura. Buscada quando o `deco_gen` do
/// `poll_frame` muda — não a cada quadro.
#[tauri::command]
pub fn decoration_image(state: State<'_, AppState>) -> tauri::ipc::Response {
    let gpu = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    let Some((gen, rgba, w, h)) = gpu.as_ref().and_then(|fp| fp.decoration_image()) else {
        return tauri::ipc::Response::new(Vec::new());
    };
    let mut out = Vec::with_capacity(12 + rgba.len());
    out.extend_from_slice(&gen.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(rgba);
    tauri::ipc::Response::new(out)
}

/// Guarda uma cópia do frame pra thumbnail do save state — no máximo 1×/500ms
/// (o clone de um frame grande custa; a frescura do thumb não precisa de 60fps).
fn cache_thumb_frame(state: &AppState, w: u32, h: u32, rgba: &[u8]) {
    let mut slot = state.last_frame.lock().unwrap_or_else(|p| p.into_inner());
    let stale = slot
        .as_ref()
        .map_or(true, |c| c.at.elapsed().as_millis() >= 500);
    if stale {
        *slot = Some(CachedFrame {
            w,
            h,
            rgba: rgba.to_vec(),
            at: std::time::Instant::now(),
        });
    }
}

/// PNG do frame que estava na tela quando o menu de pausa abriu (vídeo nativo).
/// A `PlayScreen` usa de fundo do menu. Corpo vazio se não há.
#[tauri::command]
pub fn pause_background(state: State<'_, AppState>) -> tauri::ipc::Response {
    let bg = state.pause_bg.lock().unwrap_or_else(|p| p.into_inner());
    let png = bg
        .as_ref()
        .and_then(|(w, h, rgba)| thumbnail_png(*w, *h, rgba, 960));
    tauri::ipc::Response::new(png.unwrap_or_default())
}

/// Fecha o app. O flush final da save RAM acontece no handler de
/// `ExitRequested` (`lib.rs`), que cobre também o X da janela / Alt+F4.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Desliga a máquina (`systemctl poweroff`) — menu de energia do rail,
/// estilo "modo XBOX". Só dispara o comando (`spawn`, não espera): a
/// própria queda do sistema encerra o ReEmu, não precisa de outro passo
/// aqui. Depende de o usuário ter permissão via polkit/systemd-logind
/// (padrão em qualquer desktop Linux moderno, sem precisar de senha).
#[tauri::command]
pub fn shutdown_system() -> Result<(), String> {
    std::process::Command::new("systemctl")
        .arg("poweroff")
        .spawn()
        .map_err(|e| format!("não consegui desligar: {e}"))?;
    Ok(())
}

/// Reinicia a máquina (`systemctl reboot`). Ver `shutdown_system`.
#[tauri::command]
pub fn restart_system() -> Result<(), String> {
    std::process::Command::new("systemctl")
        .arg("reboot")
        .spawn()
        .map_err(|e| format!("não consegui reiniciar: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn is_fullscreen(window: tauri::WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

#[tauri::command]
pub fn set_fullscreen(window: tauri::WebviewWindow, value: bool) -> Result<(), String> {
    window.set_fullscreen(value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn session_state(state: State<'_, AppState>) -> &'static str {
    match state.session.state() {
        SessionState::Idle => "Idle",
        SessionState::Running => "Running",
        SessionState::Paused => "Paused",
    }
}
