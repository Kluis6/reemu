//! Biblioteca (crates/library-scan).

use super::*;
use crate::app_error::AppError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RomEntry"))]
pub struct RomDto {
    pub id: String,
    pub title: String,
    pub system_id: String,
    pub file_path: String,
    /// `cover://localhost/<id>` (`http://cover.localhost/<id>` no Windows,
    /// ver `covers::cover_url`) — protocolo custom (`covers.rs`) que serve
    /// do cache em disco ou baixa da libretro e cacheia na 1ª vez; o
    /// `<img>` cai num placeholder de iniciais se vier 404 (sem cobertura
    /// ou sem rede na 1ª tentativa).
    pub boxart: Option<String>,
    /// Unix (s) do último load — pra "Continuar jogando". `None` = nunca.
    pub last_played_at: Option<i64>,
    /// Unix (s) de quando entrou na biblioteca — pra "Adicionados recentemente".
    pub added_at: i64,
    /// Aba "Favoritos" da biblioteca.
    pub is_favorite: bool,
    /// Tempo de jogo acumulado (s): o perfil soma e ordena sem uma chamada por ROM.
    pub play_time_secs: u64,
    /// Extensão interna usada na escolha do core para jogos compactados.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub content_extension: Option<String>,
    /// Nome escolhido pela pessoa (`roms.user_title`), quando definido; tem
    /// precedência sobre o título do catálogo na grade e na página do jogo.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub user_title: Option<String>,
    /// Chave canônica para agrupar múltiplas ROMs regionais do mesmo jogo na
    /// mesma plataforma.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub group_key: Option<String>,
    /// Título limpo extraído do arquivo quando a ROM ainda não tem título do
    /// catálogo nem `user_title` (usado ao exibir um grupo com múltiplas dumps).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub clean_title: Option<String>,
    /// Código canônico de região da ROM (`NA`, `EU`, `JP`, `BR`, `KR`, `AU`, `CN`, `WORLD`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub region: Option<String>,
    /// Qualificador de disco/revisão/edição (`Disc 1`, `Chip 2`, `Rev 1`, `v1.02`...).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub version_qualifier: Option<String>,
}

/// Título de exibição de uma ROM: o que o usuário renomeou, senão o nome do
/// arquivo sem extensão. Mesma regra usada pro `list_roms` e pro cache de
/// capas (`covers.rs`) resolverem o mesmo jogo pro mesmo nome.
pub(crate) fn rom_title(r: &domain::library::Rom) -> String {
    r.user_title.clone().unwrap_or_else(|| {
        std::path::Path::new(&r.file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&r.file_path)
            .to_string()
    })
}

#[tauri::command]
pub async fn list_roms(state: State<'_, AppState>) -> Result<Vec<RomDto>, AppError> {
    let pool = pool(&state)?;
    list_roms_from_pool(&pool).await
}

async fn list_roms_from_pool(pool: &db::Db) -> Result<Vec<RomDto>, AppError> {
    let repo = db::RomsRepo::new(pool.clone());
    let meta_repo = db::MetadataRepo::new(pool.clone());
    let roms = repo.list().await.map_err(AppError::from)?;
    let play_times = repo.play_times().await.map_err(AppError::from)?;
    let cover_overrides = meta_repo.cover_overrides().await.map_err(AppError::from)?;
    let meta_summaries = meta_repo
        .library_summaries()
        .await
        .map_err(AppError::from)?;

    // Se uma ROM do grupo já tem `game_uuid` do catálogo e outra cópia regional
    // ainda não terminou de resolver, associa `(system_id, canonical_title)` ao
    // mesmo `game_uuid` somente se o título identificar um único jogo na plataforma.
    let mut title_to_uuid: std::collections::HashMap<(String, String), Option<String>> =
        std::collections::HashMap::new();
    for r in &roms {
        if r.user_title.is_some() {
            continue;
        }
        if let Some(m) = meta_summaries.get(&r.id) {
            if let Some(uuid) = &m.game_uuid {
                let canon = scraping::rom_name::canonical_key(&m.title);
                if !canon.is_empty() {
                    title_to_uuid
                        .entry((r.system_id.clone(), canon))
                        .and_modify(|known| {
                            if known.as_ref() != Some(uuid) {
                                *known = None;
                            }
                        })
                        .or_insert_with(|| Some(uuid.clone()));
                }
            }
        }
    }

    Ok(roms
        .into_iter()
        .map(|r| {
            let fallback_title = rom_title(&r);
            let meta = meta_summaries.get(&r.id);
            let cat_title = meta
                .map(|m| m.title.trim().to_owned())
                .filter(|t| !t.is_empty());
            let cleaned_stem = scraping::rom_name::clean_rom_title(&fallback_title);
            let clean_title = (r.user_title.is_none()
                && cat_title.is_none()
                && !cleaned_stem.is_empty()
                && cleaned_stem != fallback_title)
                .then(|| cleaned_stem.clone());

            let title = r
                .user_title
                .clone()
                .or_else(|| cat_title.clone())
                .unwrap_or_else(|| fallback_title.clone());

            let group_key = if let Some(ut) = &r.user_title {
                let k = scraping::rom_name::canonical_key(ut);
                Some(if k.is_empty() {
                    format!("rom:{}", r.id)
                } else {
                    format!("user:{k}")
                })
            } else if let Some(uuid) = meta.and_then(|m| m.game_uuid.as_ref()) {
                Some(format!("game:{uuid}"))
            } else {
                let base_for_canon = cat_title.as_deref().unwrap_or(if cleaned_stem.is_empty() {
                    &fallback_title
                } else {
                    &cleaned_stem
                });
                let canon = scraping::rom_name::canonical_key(base_for_canon);
                if canon.is_empty() {
                    Some(format!("rom:{}", r.id))
                } else {
                    Some(
                        match title_to_uuid.get(&(r.system_id.clone(), canon.clone())) {
                            Some(Some(uuid)) => format!("game:{uuid}"),
                            Some(None) => format!("rom:{}", r.id),
                            None => format!("title:{canon}"),
                        },
                    )
                }
            };

            let region = meta
                .and_then(|m| m.rom_region.as_deref())
                .and_then(|reg| {
                    scraping::rom_name::normalize_region_code(reg)
                        .map(str::to_owned)
                        .or_else(|| Some(reg.trim().to_ascii_uppercase()))
                })
                .or_else(|| {
                    scraping::rom_name::detect_rom_region(
                        &r.file_path,
                        r.ident_name.as_deref(),
                        r.serial.as_deref(),
                    )
                    .map(str::to_owned)
                });
            let version_qualifier = scraping::rom_name::extract_rom_qualifier(
                &r.file_path,
                r.ident_name.as_deref(),
                meta.and_then(|m| m.rom_revision.as_deref()),
            );

            let has_meta_cover = meta.is_some_and(|m| m.has_cover);
            let override_cover = cover_overrides.get(&r.id);
            let boxart = (override_cover.is_some()
                || has_meta_cover
                || library_scan::libretro_boxart_url(&r.system_id, &fallback_title).is_some())
            .then(|| {
                let seed = if let Some(ov) = override_cover {
                    Some(format!("ov:{ov}"))
                } else if has_meta_cover {
                    let stored_cover = meta.and_then(|m| m.cover_url.as_deref()).unwrap_or("");
                    let heal_tag = if scraping::rom_name::is_non_front_cover_url(stored_cover) {
                        ":front-v2"
                    } else {
                        ""
                    };
                    Some(format!(
                        "meta:{}:{}:{}{heal_tag}",
                        meta.and_then(|m| m.game_uuid.as_deref()).unwrap_or(""),
                        stored_cover,
                        region.as_deref().unwrap_or(""),
                    ))
                } else {
                    None
                };
                crate::covers::cover_url_versioned(&r.id, seed.as_deref())
            });
            let content_extension = std::path::Path::new(&r.file_path)
                .extension()
                .and_then(|ext| ext.to_str())
                .filter(|ext| library_scan::is_supported_archive(ext))
                .and(r.ident_name.as_deref())
                .and_then(|name| std::path::Path::new(name).extension()?.to_str())
                .map(str::to_ascii_lowercase)
                .filter(|ext| match r.system_id.as_str() {
                    "snes" => matches!(ext.as_str(), "bs" | "st"),
                    "atarist" => matches!(ext.as_str(), "st" | "msa" | "stx"),
                    "zx81" => matches!(ext.as_str(), "p" | "t81" | "tzx"),
                    "zxspectrum" => ext == "tzx",
                    "uzebox" => ext == "uze",
                    "vaporspec" => ext == "vaporbin",
                    "gametank" => ext == "gtr",
                    "megaduck" => matches!(ext.as_str(), "duck" | "bin"),
                    "handheld_electronic" => ext == "mgw",
                    _ => false,
                });
            RomDto {
                play_time_secs: play_times.get(&r.id).copied().unwrap_or(0),
                boxart,
                title,
                user_title: r.user_title,
                group_key,
                clean_title,
                region,
                version_qualifier,
                id: r.id,
                system_id: r.system_id,
                file_path: r.file_path,
                content_extension,
                last_played_at: r.last_played_at,
                added_at: r.added_at,
                is_favorite: r.is_favorite,
            }
        })
        .collect())
}

pub(super) fn validate_metadata_edits(
    details: Option<&domain::metadata::GameMetadataEdits>,
) -> Result<(), AppError> {
    if let Some(details) = details {
        let valid_url = |value: &str| {
            let value = value.trim();
            value.is_empty()
                || (value.starts_with("data:") && crate::covers::uploaded_artwork(value).is_some())
                || net::http::https_url(value).is_some()
        };
        if [
            &details.cover_url,
            &details.fanart_url,
            &details.screenshot_url,
            &details.logo_url,
        ]
        .into_iter()
        .flatten()
        .any(|value| !valid_url(value))
        {
            return Err(AppError::new(
                crate::app_error::ErrorCode::Corrupt,
                "image must be an HTTPS URL or supported local artwork",
            ));
        }
        if details.rating.as_deref().is_some_and(|rating| {
            !rating.trim().is_empty()
                && !rating.trim().parse::<u8>().is_ok_and(|value| value <= 100)
        }) {
            return Err(AppError::new(
                crate::app_error::ErrorCode::Corrupt,
                "rating must be between 0 and 100",
            ));
        }
    }
    Ok(())
}

/// Edição manual dos dados de uma ROM. `title` vazio limpa (volta pro nome do
/// arquivo); `system_id` vazio/`None` não mexe na plataforma.
#[tauri::command]
pub async fn set_rom_metadata(
    state: State<'_, AppState>,
    rom_id: String,
    title: Option<String>,
    system_id: Option<String>,
    details: Option<domain::metadata::GameMetadataEdits>,
) -> Result<(), AppError> {
    validate_metadata_edits(details.as_ref())?;
    let cover_changed = details
        .as_ref()
        .is_some_and(|value| value.cover_url.is_some())
        || title.is_some()
        || system_id.is_some();
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.set_metadata_and_details(
        &rom_id,
        title.as_deref(),
        system_id.as_deref(),
        details.as_ref(),
    )
    .await
    .map_err(AppError::from)?;
    if cover_changed {
        crate::covers::invalidate(&state.covers_dir, &rom_id);
    }
    Ok(())
}

/// Remove a ROM da biblioteca (só o registro no banco — o arquivo em disco
/// fica; um novo scan a readiciona). Save states / metadata caem em cascata.
#[tauri::command]
pub async fn remove_rom(state: State<'_, AppState>, rom_id: String) -> Result<(), AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.remove(&rom_id).await.map_err(AppError::from)
}

/// Liga/desliga o favorito de uma ROM (aba "Favoritos" da biblioteca).
#[tauri::command]
pub async fn set_rom_favorite(
    state: State<'_, AppState>,
    rom_id: String,
    favorite: bool,
) -> Result<(), AppError> {
    use domain::library::RomRepository;
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.set_favorite(&rom_id, favorite)
        .await
        .map_err(AppError::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RomSource"))]
pub struct RomSourceDto {
    /// Pasta raiz (dois níveis acima do arquivo, ex.: `.../RetroBat/roms`).
    pub path: String,
    pub count: usize,
}

/// Agrupa as ROMs por pasta de origem (a "biblioteca" — dois níveis acima do
/// arquivo, o que casa com `roms/<sistema>/<jogo>`), pra oferecer remoção em
/// bloco. Ordenado por contagem desc.
#[tauri::command]
pub async fn list_rom_sources(state: State<'_, AppState>) -> Result<Vec<RomSourceDto>, AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let roms = repo.list().await.map_err(AppError::from)?;
    let mut by_dir: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for r in &roms {
        let p = std::path::Path::new(&r.file_path);
        let root = p
            .parent()
            .and_then(|d| d.parent())
            .or_else(|| p.parent())
            .unwrap_or(p);
        *by_dir
            .entry(root.to_string_lossy().into_owned())
            .or_default() += 1;
    }
    let mut out: Vec<RomSourceDto> = by_dir
        .into_iter()
        .map(|(path, count)| RomSourceDto { path, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    Ok(out)
}

/// Remove todas as ROMs de um sistema (snes, nes, …). Devolve a contagem.
#[tauri::command]
pub async fn remove_rom_system(
    state: State<'_, AppState>,
    system_id: String,
) -> Result<u64, AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo
        .remove_by_system(&system_id)
        .await
        .map_err(AppError::from)?;
    log::info!("biblioteca: {n} ROM(s) removida(s) do sistema {system_id}");
    Ok(n)
}

/// Tempo total de jogo da ROM, em segundos (banco + o que o relógio ainda
/// não gravou).
#[tauri::command]
pub async fn rom_play_time(state: State<'_, AppState>, rom_id: String) -> Result<u64, AppError> {
    Ok(crate::play_clock::total(&state, &rom_id).await.unwrap_or(0))
}

/// Core preferido por plataforma → `{ system_id: core_id }`.
#[tauri::command]
pub async fn list_system_cores(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, AppError> {
    use domain::core_loader::SystemCoreRepository;
    let repo = db::SystemCoreRepo::new(pool(&state)?);
    Ok(repo
        .all()
        .await
        .map_err(AppError::from)?
        .into_iter()
        .collect())
}

/// Define (ou limpa, se `core_id` vier vazio) o core preferido de uma plataforma.
#[tauri::command]
pub async fn set_system_core(
    state: State<'_, AppState>,
    system_id: String,
    core_id: String,
) -> Result<(), AppError> {
    use domain::core_loader::SystemCoreRepository;
    let repo = db::SystemCoreRepo::new(pool(&state)?);
    if core_id.is_empty() {
        repo.clear(&system_id).await.map_err(AppError::from)
    } else {
        repo.set(&system_id, &core_id).await.map_err(AppError::from)
    }
}

/// Remove todas as ROMs sob `path` (uma biblioteca inteira). Devolve a contagem.
#[tauri::command]
pub async fn remove_rom_source(state: State<'_, AppState>, path: String) -> Result<u64, AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo.remove_under_dir(&path).await.map_err(AppError::from)?;
    // Esquece a pasta, senão "Atualizar biblioteca" traria tudo de volta.
    db::LibraryFoldersRepo::new(pool(&state)?)
        .remove_under(&path)
        .await
        .map_err(AppError::from)?;
    log::info!("biblioteca: {n} ROM(s) removida(s) de uma pasta");
    Ok(n)
}

/// Esvazia a biblioteca inteira (todos os sistemas). Devolve a contagem.
#[tauri::command]
pub async fn clear_library(state: State<'_, AppState>) -> Result<u64, AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo.remove_all().await.map_err(AppError::from)?;
    db::LibraryFoldersRepo::new(pool(&state)?)
        .clear()
        .await
        .map_err(AppError::from)?;
    log::info!("biblioteca: limpa ({n} ROM(s) removida(s))");
    Ok(n)
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "ScanReport"))]
pub struct ScanReportDto {
    pub found: usize,
    pub added: usize,
    pub skipped_known: usize,
    pub skipped_unrecognized: usize,
    pub errors: usize,
    /// Já catalogadas, com o sistema corrigido pela regra atual.
    pub reclassified: usize,
    /// Saíram da biblioteca: não são jogo que o core rode (ex.: `.chd` de
    /// set do MAME, cópia do Redump em pasta de NAOMI, BIOS de arcade).
    pub removed: usize,
    /// `.zip`/`.7z` sem jogo reconhecido fora de pasta de arcade.
    pub unrecognized_archives: usize,
}

impl ScanReportDto {
    fn add(&mut self, r: &library_scan::ScanReport) {
        self.found += r.found;
        self.added += r.added;
        self.skipped_known += r.skipped_known;
        self.skipped_unrecognized += r.skipped_unrecognized;
        self.errors += r.errors;
        self.reclassified += r.reclassified;
        self.removed += r.removed;
        self.unrecognized_archives += r.unrecognized_archives;
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "ScanProgress"))]
pub struct ScanProgressDto {
    pub current: usize,
    pub total: usize,
    pub file: String,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Varre `dirs` em sequência, com o progresso somado numa barra só.
async fn scan_dirs(
    repo: &db::RomsRepo,
    dirs: &[String],
    on_progress: &tauri::ipc::Channel<ScanProgressDto>,
) -> Result<ScanReportDto, AppError> {
    let now = now_unix();
    let totals: Vec<usize> = dirs
        .iter()
        .map(|d| library_scan::count_roms(std::path::Path::new(d)))
        .collect();
    let total: usize = totals.iter().sum();
    let mut done = 0;
    let mut out = ScanReportDto::default();
    for (dir, n) in dirs.iter().zip(&totals) {
        let r = library_scan::scan_into(repo, std::path::Path::new(dir), now, |p| {
            let _ = on_progress.send(ScanProgressDto {
                current: done + p.current,
                total,
                file: p.file,
            });
        })
        .await
        .map_err(AppError::from)?;
        log::info!("biblioteca: varredura de uma pasta: {r:?}");
        out.add(&r);
        done += n;
    }
    Ok(out)
}

#[tauri::command]
pub async fn scan_library(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    on_progress: tauri::ipc::Channel<ScanProgressDto>,
) -> Result<ScanReportDto, AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let r = scan_dirs(&repo, std::slice::from_ref(&path), &on_progress).await?;
    db::LibraryFoldersRepo::new(pool(&state)?)
        .add(&path, now_unix())
        .await
        .map_err(AppError::from)?;
    if r.added + r.reclassified > 0 {
        crate::catalog::trigger_in_background(&app, crate::catalog::PassScope::All);
    }
    Ok(r)
}

/// Pastas de origem deduzidas das ROMs já catalogadas, pra bibliotecas de
/// antes de o app guardar as pastas: a de cada ROM (dois níveis acima, como
/// em `list_rom_sources`), sem as que estão dentro de outra.
fn folders_from_roms(roms: &[domain::library::Rom]) -> Vec<String> {
    let mut dirs: Vec<String> = roms
        .iter()
        .filter_map(|r| {
            let p = std::path::Path::new(&r.file_path);
            p.parent()
                .and_then(|d| d.parent())
                .or_else(|| p.parent())
                .map(|d| d.to_string_lossy().into_owned())
        })
        .collect();
    dirs.sort();
    dirs.dedup();
    let outer: Vec<String> = dirs
        .iter()
        .filter(|d| !dirs.iter().any(|o| o != *d && db::is_under(d, o)))
        .cloned()
        .collect();
    outer
}

/// "Atualizar biblioteca": varre de novo todas as pastas adicionadas. Acha
/// jogos novos e aplica a regra de identificação atual aos que já estavam
/// (sistema corrigido, o que não é jogo sai). Pasta que não existe mais
/// (disco externo desligado) é pulada, sem mexer no que veio dela.
#[tauri::command]
pub async fn rescan_library(
    app: AppHandle,
    state: State<'_, AppState>,
    on_progress: tauri::ipc::Channel<ScanProgressDto>,
) -> Result<ScanReportDto, AppError> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let folders = db::LibraryFoldersRepo::new(pool(&state)?);
    let mut dirs = folders.list().await.map_err(AppError::from)?;
    if dirs.is_empty() {
        let roms = repo.list().await.map_err(AppError::from)?;
        for d in folders_from_roms(&roms) {
            folders.add(&d, now_unix()).await.map_err(AppError::from)?;
        }
        dirs = folders.list().await.map_err(AppError::from)?;
        log::info!("biblioteca: {} pasta(s) deduzida(s) das ROMs", dirs.len());
    }
    let (present, missing): (Vec<String>, Vec<String>) = dirs
        .into_iter()
        .partition(|d| std::path::Path::new(d).is_dir());
    if !missing.is_empty() {
        log::warn!(
            "biblioteca: {} pasta(s) não encontrada(s), pulada(s)",
            missing.len()
        );
    }
    let r = scan_dirs(&repo, &present, &on_progress).await?;
    if r.added + r.reclassified > 0 {
        crate::catalog::trigger_in_background(&app, crate::catalog::PassScope::All);
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom(path: &str) -> domain::library::Rom {
        domain::library::Rom {
            id: path.into(),
            file_path: path.into(),
            crc32: String::new(),
            md5: String::new(),
            system_id: String::new(),
            added_at: 0,
            last_played_at: None,
            is_favorite: false,
            user_title: None,
            ..Default::default()
        }
    }

    /// Layout do RetroBat: `roms/<sistema>/<jogo>` e `roms/<sistema>/<pasta
    /// do jogo>/<arquivo>` viram uma pasta só, `roms`.
    #[test]
    fn folders_from_roms_keeps_the_outermost() {
        let roms = [
            rom("/r/roms/cps1/1941.zip"),
            rom("/r/roms/naomi/azumanga/gdl-0018.chd"),
            rom("/r/roms/dreamcast/jsr/jsr.cue"),
            rom("/d/jogos/snes/x.sfc"),
        ];
        assert_eq!(folders_from_roms(&roms), ["/d/jogos", "/r/roms"]);
    }

    #[tokio::test]
    async fn list_roms_prefers_user_title_then_catalogue_title() {
        use domain::library::RomRepository;
        use domain::metadata::{GameMetadata, MetadataRepository};

        let pool = db::connect_in_memory().await.unwrap();
        let roms_repo = db::RomsRepo::new(pool.clone());
        let meta_repo = db::MetadataRepo::new(pool.clone());

        roms_repo
            .add(&domain::library::Rom {
                id: "r_stem".into(),
                file_path: "/roms/snes/smw_dump.sfc".into(),
                system_id: "snes".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        roms_repo
            .add(&domain::library::Rom {
                id: "r_cat".into(),
                file_path: "/roms/snes/ct_dump.sfc".into(),
                system_id: "snes".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        roms_repo
            .add(&domain::library::Rom {
                id: "r_user".into(),
                file_path: "/roms/snes/eb_dump.sfc".into(),
                system_id: "snes".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        roms_repo
            .set_metadata("r_user", Some("Meu EarthBound"), None)
            .await
            .unwrap();

        for (id, title) in [("r_cat", "Chrono Trigger"), ("r_user", "EarthBound")] {
            meta_repo
                .upsert_metadata(&GameMetadata {
                    rom_id: id.into(),
                    title: title.into(),
                    provider_source: Some("alvanista".into()),
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        let got = list_roms_from_pool(&pool).await.unwrap();
        let by_id = |id: &str| got.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id("r_stem").title, "smw_dump");
        assert_eq!(by_id("r_cat").title, "Chrono Trigger");
        assert_eq!(by_id("r_user").title, "Meu EarthBound");
        assert_eq!(
            by_id("r_user").user_title.as_deref(),
            Some("Meu EarthBound")
        );
    }

    #[tokio::test]
    async fn list_roms_uses_cached_cover_when_metadata_has_one() {
        use domain::library::RomRepository;
        use domain::metadata::{GameMetadata, MetadataRepository};

        let pool = db::connect_in_memory().await.unwrap();
        let roms_repo = db::RomsRepo::new(pool.clone());
        let meta_repo = db::MetadataRepo::new(pool.clone());

        // `disc` não tem cobertura na libretro: `boxart` só aparece quando
        // `game_metadata.cover_url` existe.
        roms_repo
            .add(&domain::library::Rom {
                id: "r_no_cover".into(),
                file_path: "/roms/disc/a.iso".into(),
                system_id: "disc".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        roms_repo
            .add(&domain::library::Rom {
                id: "r_with_cover".into(),
                file_path: "/roms/disc/b.iso".into(),
                system_id: "disc".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        meta_repo
            .upsert_metadata(&GameMetadata {
                rom_id: "r_with_cover".into(),
                title: "Jogo com Capa".into(),
                cover_url: Some("https://cdn.alvanista.com/b/cover.webp".into()),
                provider_source: Some("alvanista".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        let got = list_roms_from_pool(&pool).await.unwrap();
        let by_id = |id: &str| got.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id("r_no_cover").boxart, None);
        let first_boxart = by_id("r_with_cover").boxart.clone().expect("boxart");
        assert!(
            first_boxart.starts_with(&format!("{}?v=", crate::covers::cover_url("r_with_cover"))),
            "unexpected boxart url: {first_boxart}"
        );

        meta_repo
            .upsert_metadata(&GameMetadata {
                rom_id: "r_with_cover".into(),
                title: "Jogo Outra Capa".into(),
                cover_url: Some("https://cdn.alvanista.com/b/cover-v2.webp".into()),
                provider_source: Some("alvanista".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        let got_after = list_roms_from_pool(&pool).await.unwrap();
        let next_boxart = got_after
            .iter()
            .find(|r| r.id == "r_with_cover")
            .unwrap()
            .boxart
            .clone()
            .expect("boxart");
        assert_ne!(first_boxart, next_boxart);
    }

    #[tokio::test]
    async fn list_roms_assigns_shared_group_key_and_regions_across_regional_dumps() {
        use domain::library::RomRepository;

        let pool = db::connect_in_memory().await.unwrap();
        let roms_repo = db::RomsRepo::new(pool.clone());

        for (id, path) in [
            ("ct_us", "/roms/snes/Chrono Trigger (USA).sfc"),
            ("ct_jp", "/roms/snes/Chrono Trigger (Japan).sfc"),
            ("ct_eu", "/roms/snes/Chrono Trigger (Europe) (Rev 1).sfc"),
        ] {
            roms_repo
                .add(&domain::library::Rom {
                    id: id.into(),
                    file_path: path.into(),
                    system_id: "snes".into(),
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        let got = list_roms_from_pool(&pool).await.unwrap();
        let by_id = |id: &str| got.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id("ct_us").region.as_deref(), Some("NA"));
        assert_eq!(by_id("ct_jp").region.as_deref(), Some("JP"));
        assert_eq!(by_id("ct_eu").region.as_deref(), Some("EU"));
        assert_eq!(by_id("ct_eu").version_qualifier.as_deref(), Some("Rev 1"));
        assert_eq!(by_id("ct_us").group_key, by_id("ct_jp").group_key);
        assert_eq!(by_id("ct_us").group_key, by_id("ct_eu").group_key);
        assert_eq!(
            by_id("ct_us").clean_title.as_deref(),
            Some("Chrono Trigger")
        );

        use domain::metadata::{GameMetadata, MetadataRepository};

        let metadata = db::MetadataRepo::new(pool.clone());
        for (rom_id, game_uuid) in [("ct_us", "game-a"), ("ct_jp", "game-b")] {
            metadata
                .upsert_metadata(&GameMetadata {
                    rom_id: rom_id.into(),
                    title: "Chrono Trigger".into(),
                    game_uuid: Some(game_uuid.into()),
                    ..Default::default()
                })
                .await
                .unwrap();
        }

        let got = list_roms_from_pool(&pool).await.unwrap();
        let key = |id: &str| {
            got.iter()
                .find(|rom| rom.id == id)
                .and_then(|rom| rom.group_key.as_deref())
        };
        assert_eq!(key("ct_us"), Some("game:game-a"));
        assert_eq!(key("ct_jp"), Some("game:game-b"));
        assert_eq!(key("ct_eu"), Some("rom:ct_eu"));
    }
}
