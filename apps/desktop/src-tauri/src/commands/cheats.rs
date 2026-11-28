//! Cheats por código e importação `.cht` (docs/ai-context/17-cheats-rewind.md,
//! fases 1, 2 e 5): a lista de cada jogo fica no banco (`cheats`) e vai pro
//! core ao carregar o jogo e a cada mudança no jogo aberto
//! (`EmuSession::set_cheats` -> `retro_cheat_set`). Em sessões Hardcore do
//! RetroAchievements, o core sempre recebe uma lista vazia (`retro_cheat_reset`).

use super::*;
use crate::app_error::{AppError, ErrorCode};
use domain::cheats::{deduplicate_parsed_cheats, parse_cht, ParsedCheat};
use domain::library::{Rom, RomRepository};
use domain::metadata::{GameMetadata, MetadataRepository};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "Cheat"))]
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

/// Manda a lista do jogo pro core. Em modo Hardcore envia sempre lista vazia
/// (`retro_cheat_reset`) para garantir que nenhum cheat fique ativo na sessão.
pub(super) async fn apply_cheats(state: &AppState, hardcore_active: bool, rom_id: Option<&str>) {
    if hardcore_active {
        log::info!("cheats: modo Hardcore ativo; limpando cheats no core");
        let session = Arc::clone(&state.session);
        let _ = tauri::async_runtime::spawn_blocking(move || session.set_cheats(Vec::new())).await;
        return;
    }
    let (Some(pool), Some(rom_id)) = (state.db.clone(), rom_id) else {
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

/// Reaplica se a mudança foi no jogo atualmente aberto (respeitando o bloqueio Hardcore).
async fn reapply_if_current(state: &AppState, hardcore_active: bool, rom_id: &str) {
    let current = state
        .current_rom
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if current.as_deref() == Some(rom_id) {
        apply_cheats(state, hardcore_active, Some(rom_id)).await;
    }
}

/// Código sem espaços nas pontas; vazio é recusado. Descrição vazia vira o
/// próprio código.
fn clean(description: &str, code: &str) -> Result<(String, String), AppError> {
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

async fn insert_unique_cheats(
    repo: &db::CheatsRepo,
    rom_id: &str,
    parsed: Vec<ParsedCheat>,
) -> Result<usize, AppError> {
    if parsed.is_empty() {
        return Ok(0);
    }
    let existing = repo.list(rom_id).await.map_err(AppError::from)?;
    let unique = deduplicate_parsed_cheats(&existing, parsed);
    let mut imported = 0usize;
    for entry in unique {
        repo.add(rom_id, &entry.description, &entry.code, entry.enabled)
            .await
            .map_err(AppError::from)?;
        imported += 1;
    }
    Ok(imported)
}

#[tauri::command]
pub async fn list_cheats(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<Vec<CheatDto>, AppError> {
    let repo = db::CheatsRepo::new(pool(&state)?);
    let list = repo.list(&rom_id).await.map_err(AppError::from)?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn add_cheat(
    state: State<'_, AppState>,
    ra: State<'_, crate::ra::RaState>,
    rom_id: String,
    description: String,
    code: String,
) -> Result<CheatDto, AppError> {
    let hardcore = ra.is_hardcore_active();
    let (description, code) = clean(&description, &code)?;
    let repo = db::CheatsRepo::new(pool(&state)?);
    // Fora do Hardcore entra ligado; em Hardcore entra desligado para não ativar.
    let cheat = repo
        .add(&rom_id, &description, &code, !hardcore)
        .await
        .map_err(AppError::from)?;
    reapply_if_current(&state, hardcore, &rom_id).await;
    Ok(cheat.into())
}

#[tauri::command]
pub async fn update_cheat(
    state: State<'_, AppState>,
    ra: State<'_, crate::ra::RaState>,
    id: i64,
    description: String,
    code: String,
    enabled: bool,
) -> Result<CheatDto, AppError> {
    let hardcore = ra.is_hardcore_active();
    if hardcore && enabled {
        return Err(
            "Modo Hardcore ativo: cheats ficam desativados durante partidas Hardcore.".into(),
        );
    }
    let (description, code) = clean(&description, &code)?;
    let repo = db::CheatsRepo::new(pool(&state)?);
    let cheat = repo
        .update(id, &description, &code, enabled)
        .await
        .map_err(AppError::from)?
        .ok_or("cheat não encontrado")?;
    reapply_if_current(&state, hardcore, &cheat.rom_id).await;
    Ok(cheat.into())
}

#[tauri::command]
pub async fn delete_cheat(
    state: State<'_, AppState>,
    ra: State<'_, crate::ra::RaState>,
    id: i64,
) -> Result<(), AppError> {
    let repo = db::CheatsRepo::new(pool(&state)?);
    if let Some(rom_id) = repo.delete(id).await.map_err(AppError::from)? {
        reapply_if_current(&state, ra.is_hardcore_active(), &rom_id).await;
    }
    Ok(())
}

/// Importa um arquivo `.cht` do disco sem duplicar entradas já existentes em
/// `rom_id` e devolve quantos cheats novos foram adicionados.
#[tauri::command]
pub async fn import_cheats_cht(
    state: State<'_, AppState>,
    ra: State<'_, crate::ra::RaState>,
    rom_id: String,
    path: String,
) -> Result<usize, AppError> {
    let raw_bytes = std::fs::read(Path::new(&path)).map_err(AppError::from)?;
    let text = String::from_utf8_lossy(&raw_bytes);
    let mut parsed = parse_cht(&text);
    let hardcore = ra.is_hardcore_active();
    if hardcore {
        for c in &mut parsed {
            c.enabled = false;
        }
    }
    let repo = db::CheatsRepo::new(pool(&state)?);
    let count = insert_unique_cheats(&repo, &rom_id, parsed).await?;
    if count > 0 {
        reapply_if_current(&state, hardcore, &rom_id).await;
    }
    Ok(count)
}

/// Baixa sob demanda o `.cht` correspondente a `rom_id` no `libretro-database`
/// (licença CC-BY-SA 4.0), importa sem duplicatas e devolve quantos cheats
/// novos entraram.
#[tauri::command]
pub async fn download_libretro_cheats(
    state: State<'_, AppState>,
    ra: State<'_, crate::ra::RaState>,
    rom_id: String,
) -> Result<usize, AppError> {
    let db_pool = pool(&state)?;
    let rom = db::RomsRepo::new(db_pool.clone())
        .get(&rom_id)
        .await
        .map_err(AppError::from)?
        .ok_or("ROM não encontrada na biblioteca")?;
    let meta = db::MetadataRepo::new(db_pool.clone())
        .get_metadata(&rom_id)
        .await
        .ok()
        .flatten();

    let ext = rom
        .ident_name
        .as_deref()
        .and_then(|n| Path::new(n).extension())
        .or_else(|| Path::new(&rom.file_path).extension())
        .and_then(|e| e.to_str())
        .map(str::to_owned);
    let systems = library_scan::libretro_cht_systems(&rom.system_id, ext.as_deref());
    if systems.is_empty() {
        return Ok(0);
    }

    let candidates = build_cht_candidate_stems(&rom, meta.as_ref());
    let Some(cht_text) =
        fetch_matching_libretro_cht(systems, &candidates, &rom, meta.as_ref()).await?
    else {
        return Ok(0);
    };

    let mut parsed = parse_cht(&cht_text);
    let hardcore = ra.is_hardcore_active();
    if hardcore {
        for c in &mut parsed {
            c.enabled = false;
        }
    }
    let repo = db::CheatsRepo::new(db_pool);
    let count = insert_unique_cheats(&repo, &rom_id, parsed).await?;
    if count > 0 {
        reapply_if_current(&state, hardcore, &rom_id).await;
    }
    Ok(count)
}

fn sanitize_nointro_stem(raw: &str) -> String {
    let replaced = raw.replace(": ", " - ").replace(':', " - ");
    let cleaned: String = replaced
        .chars()
        .map(|c| match c {
            '/' | '\\' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_unique_candidate(list: &mut Vec<String>, s: impl Into<String>) {
    let val = s.into();
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return;
    }
    if !list.iter().any(|existing| existing == trimmed) {
        list.push(trimmed.to_string());
    }
}

fn strip_archive_double_ext(stem: &str) -> &str {
    let Some((base, ext)) = stem.rsplit_once('.') else {
        return stem;
    };
    if !base.is_empty() && library_scan::system_for_extension(ext).is_some() {
        base
    } else {
        stem
    }
}

fn raw_rom_stem(rom: &Rom) -> &str {
    let source = rom.ident_name.as_deref().unwrap_or(&rom.file_path);
    Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(strip_archive_double_ext)
        .unwrap_or(source)
}

fn region_nointro_label(code: &str) -> Option<&'static str> {
    match code.to_ascii_uppercase().as_str() {
        "USA" | "US" => Some("USA"),
        "EUR" | "EU" | "PAL" => Some("Europe"),
        "JPN" | "JP" => Some("Japan"),
        "BRA" | "BR" => Some("Brazil"),
        "WORLD" => Some("World"),
        "KOR" | "KR" => Some("Korea"),
        _ => None,
    }
}

/// Monta os nomes candidatos `<Jogo>.cht` em ordem de especificidade:
/// 1. stem exato de `ident_name` e de `file_path` (com e sem sanitização No-Intro);
/// 2. título limpo / metadados combinados com a região detectada da ROM e as
///    regiões No-Intro mais frequentes (`USA`, `World`, `Europe`, `Japan`).
fn build_cht_candidate_stems(rom: &Rom, meta: Option<&GameMetadata>) -> Vec<String> {
    let mut out = Vec::new();

    for raw_path in [rom.ident_name.as_deref(), Some(rom.file_path.as_str())]
        .into_iter()
        .flatten()
    {
        if let Some(stem) = Path::new(raw_path).file_stem().and_then(|s| s.to_str()) {
            let stem = strip_archive_double_ext(stem);
            push_unique_candidate(&mut out, stem);
            let sanitized = sanitize_nointro_stem(stem);
            push_unique_candidate(&mut out, &sanitized);
            if sanitized.contains('&') {
                push_unique_candidate(&mut out, sanitized.replace('&', "_"));
            }
        }
    }

    let detected_reg = scraping::rom_name::detect_rom_region(
        &rom.file_path,
        rom.ident_name.as_deref(),
        rom.serial.as_deref(),
    )
    .and_then(region_nointro_label);

    let mut base_titles = Vec::new();
    if let Some(m) = meta {
        push_unique_candidate(&mut base_titles, sanitize_nointro_stem(&m.title));
    }
    if let Some(ut) = rom.user_title.as_deref() {
        push_unique_candidate(&mut base_titles, sanitize_nointro_stem(ut));
    }
    let cleaned = scraping::rom_name::clean_rom_title(raw_rom_stem(rom));
    if !cleaned.is_empty() {
        push_unique_candidate(&mut base_titles, sanitize_nointro_stem(&cleaned));
    }

    let mut regions = Vec::new();
    if let Some(r) = detected_reg {
        regions.push(r);
    }
    for fallback in ["USA", "World", "USA, Europe", "Europe", "Japan"] {
        if !regions.contains(&fallback) {
            regions.push(fallback);
        }
    }

    for base in &base_titles {
        if !base.contains('(') {
            for reg in &regions {
                push_unique_candidate(&mut out, format!("{base} ({reg})"));
            }
        }
        push_unique_candidate(&mut out, base);
    }

    out.truncate(12);
    out
}

#[derive(Deserialize)]
struct GithubTreeResponse {
    #[serde(default)]
    tree: Vec<GithubTreeItem>,
}

#[derive(Deserialize)]
struct GithubTreeItem {
    path: String,
    #[serde(rename = "type", default)]
    kind: String,
}

static CHT_TREE_CACHE: OnceLock<tokio::sync::Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();

fn encode_url_segment(s: &str) -> String {
    s.bytes()
        .flat_map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
                vec![b as char]
            } else {
                format!("%{b:02X}").chars().collect()
            }
        })
        .collect()
}

async fn list_github_cht_stems(system_folder: &str) -> Option<Vec<String>> {
    let cache = CHT_TREE_CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().await.get(system_folder).cloned() {
        return Some(hit);
    }

    let spec = format!("master:cht/{system_folder}");
    let url = format!(
        "https://api.github.com/repos/libretro/libretro-database/git/trees/{}",
        encode_url_segment(&spec),
    );
    let resp = net::http::client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let parsed = resp.json::<GithubTreeResponse>().await.ok()?;
    let stems: Vec<String> = parsed
        .tree
        .into_iter()
        .filter(|it| it.kind == "blob" || it.kind.is_empty())
        .filter_map(|it| it.path.strip_suffix(".cht").map(str::to_owned))
        .collect();
    cache
        .lock()
        .await
        .insert(system_folder.to_string(), stems.clone());
    Some(stems)
}

fn normalize_lookup_key(s: &str) -> String {
    // Tira tags `(USA)`, `[!]`, etc., e deixa só alfanuméricos ASCII minúsculos.
    let mut bare = String::with_capacity(s.len());
    let mut depth = 0u32;
    for ch in s.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => bare.push(ch),
            _ => {}
        }
    }
    let replaced = bare.replace('&', "and");
    replaced
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn pick_best_stem_from_tree(
    stems: &[String],
    target_titles: &[String],
    preferred_region: Option<&str>,
) -> Option<String> {
    let target_keys: Vec<String> = target_titles
        .iter()
        .map(|t| normalize_lookup_key(t))
        .filter(|k| !k.is_empty())
        .collect();
    if target_keys.is_empty() {
        return None;
    }

    let mut best: Option<((u8, u8, usize), &String)> = None;
    for stem in stems {
        let stem_key = normalize_lookup_key(stem);
        let Some(title_rank) = target_keys.iter().position(|k| k == &stem_key) else {
            continue;
        };
        let lower = stem.to_ascii_lowercase();
        let is_prerelease = lower.contains("(beta")
            || lower.contains("(proto")
            || lower.contains("(demo")
            || lower.contains("(sample")
            || lower.contains("(pirate")
            || lower.contains("(unl)");
        let reg_rank = if let Some(pref) = preferred_region {
            if stem.contains(&format!("({pref}")) {
                0
            } else if stem.contains("(World") || stem.contains("(USA") {
                1
            } else if stem.contains("(Europe") {
                2
            } else {
                3
            }
        } else if stem.contains("(World") || stem.contains("(USA") {
            0
        } else if stem.contains("(Europe") {
            1
        } else {
            2
        };
        let score = (
            u8::from(is_prerelease),
            (title_rank as u8)
                .saturating_mul(4)
                .saturating_add(reg_rank),
            stem.len(),
        );
        let better = match best {
            None => true,
            Some((b_score, _)) => score < b_score,
        };
        if better {
            best = Some((score, stem));
        }
    }
    best.map(|(_, s)| s.clone())
}

async fn fetch_cht_url(url: &str) -> Result<Option<String>, reqwest::Error> {
    let resp = net::http::client()
        .get(url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Ok(None);
    }
    let bytes = resp.bytes().await?;
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

async fn fetch_matching_libretro_cht(
    systems: &[&str],
    candidates: &[String],
    rom: &Rom,
    meta: Option<&GameMetadata>,
) -> Result<Option<String>, AppError> {
    let mut had_http_reply = false;
    let mut last_net_err: Option<String> = None;

    for sys in systems {
        for stem in candidates {
            let url = library_scan::libretro_cht_url(sys, stem);
            match fetch_cht_url(&url).await {
                Ok(Some(body)) if !parse_cht(&body).is_empty() => return Ok(Some(body)),
                Ok(_) => had_http_reply = true,
                Err(e) => last_net_err = Some(e.to_string()),
            }
        }
    }

    if !had_http_reply {
        if let Some(err) = last_net_err {
            return Err(AppError::new(ErrorCode::Offline, err));
        }
    }

    // Fallback: consulta a árvore da pasta `cht/<Sistema>` no GitHub quando o
    // nome do arquivo local usa outra revisão/região No-Intro.
    let mut target_titles = Vec::new();
    if let Some(m) = meta {
        push_unique_candidate(&mut target_titles, &m.title);
    }
    if let Some(ut) = rom.user_title.as_deref() {
        push_unique_candidate(&mut target_titles, ut);
    }
    let cleaned = scraping::rom_name::clean_rom_title(raw_rom_stem(rom));
    push_unique_candidate(&mut target_titles, cleaned);

    let preferred_reg = scraping::rom_name::detect_rom_region(
        &rom.file_path,
        rom.ident_name.as_deref(),
        rom.serial.as_deref(),
    )
    .and_then(region_nointro_label);

    for sys in systems {
        let Some(tree_stems) = list_github_cht_stems(sys).await else {
            continue;
        };
        let Some(matched_stem) =
            pick_best_stem_from_tree(&tree_stems, &target_titles, preferred_reg)
        else {
            continue;
        };
        if candidates.iter().any(|c| c == &matched_stem) {
            continue;
        }
        let url = library_scan::libretro_cht_url(sys, &matched_stem);
        if let Ok(Some(body)) = fetch_cht_url(&url).await {
            if !parse_cht(&body).is_empty() {
                return Ok(Some(body));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn candidate_builder_prioritizes_exact_stem_and_regional_nointro_names() {
        let rom = Rom {
            id: "r1".into(),
            file_path: "/roms/snes/Super Mario World (U) [!].zip".into(),
            ident_name: Some("Super Mario World (USA).sfc".into()),
            system_id: "snes".into(),
            ..Default::default()
        };
        let cands = build_cht_candidate_stems(&rom, None);
        assert_eq!(cands[0], "Super Mario World (USA)");
    }

    #[test]
    fn tree_fallback_picks_matching_region_and_skips_betas() {
        let tree = vec![
            "Chrono Trigger (USA) (Beta)".to_string(),
            "Chrono Trigger (Japan)".to_string(),
            "Chrono Trigger (USA)".to_string(),
        ];
        let picked =
            pick_best_stem_from_tree(&tree, &["Chrono Trigger".to_string()], Some("USA")).unwrap();
        assert_eq!(picked, "Chrono Trigger (USA)");
    }

    #[tokio::test]
    async fn insert_unique_cheats_deduplicates_on_repeated_imports() {
        let pool = db::connect_in_memory().await.unwrap();
        db::RomsRepo::new(pool.clone())
            .add(&Rom {
                id: "rom-cht".into(),
                file_path: "/roms/snes/game.sfc".into(),
                system_id: "snes".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let repo = db::CheatsRepo::new(pool);

        let cht = "cheats = 2\n\
                   cheat0_desc = \"Infinite Lives\"\n\
                   cheat0_code = \"7E0DBF09\"\n\
                   cheat0_enable = false\n\
                   cheat1_desc = \"Invincibility\"\n\
                   cheat1_code = \"7E1497FF\"\n\
                   cheat1_enable = false\n";
        let n1 = insert_unique_cheats(&repo, "rom-cht", parse_cht(cht))
            .await
            .unwrap();
        assert_eq!(n1, 2);

        // Reimportar o mesmo `.cht` não duplica nada.
        let n2 = insert_unique_cheats(&repo, "rom-cht", parse_cht(cht))
            .await
            .unwrap();
        assert_eq!(n2, 0);

        let all = repo.list("rom-cht").await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(!all[0].enabled);
        assert!(!all[1].enabled);
    }
}
