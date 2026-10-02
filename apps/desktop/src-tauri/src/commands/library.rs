//! Biblioteca (crates/library-scan).

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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
pub async fn list_roms(state: State<'_, AppState>) -> Result<Vec<RomDto>, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let roms = repo.list().await.map_err(|e| e.to_string())?;
    Ok(roms
        .into_iter()
        .map(|r| {
            let title = rom_title(&r);
            // Sem cobertura conhecida (`libretro_boxart_url` → `None`, ex.:
            // arcade) nem tenta o protocolo — cai direto nas iniciais, igual
            // hoje. Com cobertura, aponta pro protocolo `cover://` (ver
            // `covers.rs`): serve do cache em disco se já baixou antes, ou
            // baixa na hora e grava — funciona offline depois da 1ª vez.
            let boxart = library_scan::libretro_boxart_url(&r.system_id, &title)
                .map(|_| crate::covers::cover_url(&r.id));
            RomDto {
                boxart,
                title,
                id: r.id,
                system_id: r.system_id,
                file_path: r.file_path,
                last_played_at: r.last_played_at,
                added_at: r.added_at,
                is_favorite: r.is_favorite,
            }
        })
        .collect())
}

/// Edição manual dos dados de uma ROM. `title` vazio limpa (volta pro nome do
/// arquivo); `system_id` vazio/`None` não mexe na plataforma.
#[tauri::command]
pub async fn set_rom_metadata(
    state: State<'_, AppState>,
    rom_id: String,
    title: Option<String>,
    system_id: Option<String>,
) -> Result<(), String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.set_metadata(&rom_id, title.as_deref(), system_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Remove a ROM da biblioteca (só o registro no banco — o arquivo em disco
/// fica; um novo scan a readiciona). Save states / metadata caem em cascata.
#[tauri::command]
pub async fn remove_rom(state: State<'_, AppState>, rom_id: String) -> Result<(), String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.remove(&rom_id).await.map_err(|e| e.to_string())
}

/// Liga/desliga o favorito de uma ROM (aba "Favoritos" da biblioteca).
#[tauri::command]
pub async fn set_rom_favorite(
    state: State<'_, AppState>,
    rom_id: String,
    favorite: bool,
) -> Result<(), String> {
    use domain::library::RomRepository;
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.set_favorite(&rom_id, favorite)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomSourceDto {
    /// Pasta raiz (dois níveis acima do arquivo, ex.: `.../RetroBat/roms`).
    pub path: String,
    pub count: usize,
}

/// Agrupa as ROMs por pasta de origem (a "biblioteca" — dois níveis acima do
/// arquivo, o que casa com `roms/<sistema>/<jogo>`), pra oferecer remoção em
/// bloco. Ordenado por contagem desc.
#[tauri::command]
pub async fn list_rom_sources(state: State<'_, AppState>) -> Result<Vec<RomSourceDto>, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let roms = repo.list().await.map_err(|e| e.to_string())?;
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
) -> Result<u64, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo
        .remove_by_system(&system_id)
        .await
        .map_err(|e| e.to_string())?;
    log::info!("biblioteca: {n} ROM(s) removida(s) do sistema {system_id}");
    Ok(n)
}

/// Tempo total de jogo da ROM, em segundos (banco + o que o relógio ainda
/// não gravou).
#[tauri::command]
pub async fn rom_play_time(state: State<'_, AppState>, rom_id: String) -> Result<u64, String> {
    Ok(crate::play_clock::total(&state, &rom_id).await.unwrap_or(0))
}

/// Core preferido por plataforma → `{ system_id: core_id }`.
#[tauri::command]
pub async fn list_system_cores(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    use domain::core_loader::SystemCoreRepository;
    let repo = db::SystemCoreRepo::new(pool(&state)?);
    Ok(repo
        .all()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect())
}

/// Define (ou limpa, se `core_id` vier vazio) o core preferido de uma plataforma.
#[tauri::command]
pub async fn set_system_core(
    state: State<'_, AppState>,
    system_id: String,
    core_id: String,
) -> Result<(), String> {
    use domain::core_loader::SystemCoreRepository;
    let repo = db::SystemCoreRepo::new(pool(&state)?);
    if core_id.is_empty() {
        repo.clear(&system_id).await.map_err(|e| e.to_string())
    } else {
        repo.set(&system_id, &core_id)
            .await
            .map_err(|e| e.to_string())
    }
}

/// Remove todas as ROMs sob `path` (uma biblioteca inteira). Devolve a contagem.
#[tauri::command]
pub async fn remove_rom_source(state: State<'_, AppState>, path: String) -> Result<u64, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo
        .remove_under_dir(&path)
        .await
        .map_err(|e| e.to_string())?;
    // Esquece a pasta, senão "Atualizar biblioteca" traria tudo de volta.
    db::LibraryFoldersRepo::new(pool(&state)?)
        .remove_under(&path)
        .await
        .map_err(|e| e.to_string())?;
    log::info!("biblioteca: {n} ROM(s) removida(s) de {path}");
    Ok(n)
}

/// Esvazia a biblioteca inteira (todos os sistemas). Devolve a contagem.
#[tauri::command]
pub async fn clear_library(state: State<'_, AppState>) -> Result<u64, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo.remove_all().await.map_err(|e| e.to_string())?;
    db::LibraryFoldersRepo::new(pool(&state)?)
        .clear()
        .await
        .map_err(|e| e.to_string())?;
    log::info!("biblioteca: limpa ({n} ROM(s) removida(s))");
    Ok(n)
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanReportDto {
    pub found: usize,
    pub added: usize,
    pub skipped_known: usize,
    pub skipped_unrecognized: usize,
    pub errors: usize,
    /// Já catalogadas, com o sistema corrigido pela regra atual.
    pub reclassified: usize,
    /// Saíram da biblioteca: não são jogo que o core rode (ex.: `.chd` de
    /// set do MAME, cópia do Redump em pasta de NAOMI).
    pub removed: usize,
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
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
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
) -> Result<ScanReportDto, String> {
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
        .map_err(|e| e.to_string())?;
        log::info!("biblioteca: varredura de {dir}: {r:?}");
        out.add(&r);
        done += n;
    }
    Ok(out)
}

#[tauri::command]
pub async fn scan_library(
    state: State<'_, AppState>,
    path: String,
    on_progress: tauri::ipc::Channel<ScanProgressDto>,
) -> Result<ScanReportDto, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let r = scan_dirs(&repo, std::slice::from_ref(&path), &on_progress).await?;
    db::LibraryFoldersRepo::new(pool(&state)?)
        .add(&path, now_unix())
        .await
        .map_err(|e| e.to_string())?;
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
    state: State<'_, AppState>,
    on_progress: tauri::ipc::Channel<ScanProgressDto>,
) -> Result<ScanReportDto, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let folders = db::LibraryFoldersRepo::new(pool(&state)?);
    let mut dirs = folders.list().await.map_err(|e| e.to_string())?;
    if dirs.is_empty() {
        let roms = repo.list().await.map_err(|e| e.to_string())?;
        for d in folders_from_roms(&roms) {
            folders
                .add(&d, now_unix())
                .await
                .map_err(|e| e.to_string())?;
        }
        dirs = folders.list().await.map_err(|e| e.to_string())?;
        log::info!("biblioteca: pastas deduzidas das ROMs: {dirs:?}");
    }
    let (present, missing): (Vec<String>, Vec<String>) = dirs
        .into_iter()
        .partition(|d| std::path::Path::new(d).is_dir());
    if !missing.is_empty() {
        log::warn!("biblioteca: pastas não encontradas, puladas: {missing:?}");
    }
    scan_dirs(&repo, &present, &on_progress).await
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
}
