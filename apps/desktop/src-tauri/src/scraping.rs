//! Scraping de metadata (etapa 09). Provider: **ScreenScraper** (`screenscraper.fr`)
//! por hash (CRC32) — hash exato OU nome de arquivo batendo exato com o
//! `romfilename` que o próprio ScreenScraper devolve = auto; qualquer outra
//! coisa vai pra `pending_review` (decisão Abordagem B revisada, ver
//! docs/ai-context/09 e `domain::metadata::ScrapeCandidate::auto_matches`).
//!
//! Credenciais do usuário (conta grátis no screenscraper.fr) são opcionais mas
//! melhoram muito o limite de requisições — anônimo é bem restrito.
//!
//! Reserva: **TheGamesDB** (`thegamesdb.net`, precisa de chave de API do
//! usuário) quando o ScreenScraper não acha o jogo. É busca por NOME (o
//! TheGamesDB não tem hash), então o resultado vai SEMPRE pra revisão — nunca
//! auto-aplicado. IGDB ficou de fora: a lista de ids de plataforma dele só sai
//! da API autenticada, sem fonte pública confiável pra conferir.
//!
//! Ids de plataforma dos dois provedores: tabelas do ES-DE (EmulationStation
//! Desktop Edition, GPL — `es-app/src/scrapers/ScreenScraper.cpp` e
//! `GamesDBJSONScraper.cpp`), não de memória; os 15 ids do ScreenScraper que
//! o ReEmu já tinha batem com os de lá.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use domain::library::RomRepository;
use domain::metadata::{
    GameMetadata, MatchStatus, MetadataConfig, MetadataRepository, ScrapeCandidate, ScrapeQuery,
};
use serde_json::Value;

/// Pausa entre requisições ao ScreenScraper — anônimo é bem limitado.
const REQUEST_DELAY: std::time::Duration = std::time::Duration::from_millis(1200);
/// Pausa depois de consultar o TheGamesDB — a chave tem cota MENSAL
/// (`remaining_monthly_allowance`), então sem pressa.
const TGDB_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);

/// `system_id` canônico do ReEmu → `systemeid` do ScreenScraper (tabela do
/// ES-DE, ver comentário do módulo). `disc` fica de fora: sem saber o sistema,
/// não há o que perguntar.
fn screenscraper_system_id(system_id: &str) -> Option<u32> {
    Some(match system_id {
        "nes" => 3,
        "snes" => 4,
        "n64" => 14,
        "gb" => 9,
        "gbc" => 10,
        "gba" => 12,
        "vb" => 11,
        "nds" => 15,
        "pokemini" => 211,
        "megadrive" => 1,
        "mastersystem" => 2,
        "gamegear" => 21,
        "sega32x" => 19,
        "segacd" => 20,
        "saturn" => 22,
        "dreamcast" => 23,
        "sg1000" => 109,
        "naomi" => 56,
        "atomiswave" => 53,
        "psx" => 57,
        "ps2" => 58,
        "psp" => 61,
        "pcengine" => 31,
        "pcenginecd" => 114,
        "supergrafx" => 105,
        "pcfx" => 72,
        "atari2600" => 26,
        "atari5200" => 40,
        "atari7800" => 41,
        "atari8bit" => 43,
        "jaguar" => 27,
        "lynx" => 28,
        "wonderswan" => 45,
        "ngp" => 25,
        "neogeocd" => 70,
        "coleco" => 48,
        "intellivision" => 115,
        "vectrex" => 102,
        "odyssey2" => 104,
        "supervision" => 207,
        "3do" => 29,
        "cdi" => 133,
        "msx" => 113,
        "c64" => 66,
        "amiga" => 64,
        "zxspectrum" => 76,
        "amstradcpc" => 65,
        "arcade" => 75,
        "dos" => 135,
        "scummvm" => 123,
        _ => return None,
    })
}

/// `system_id` → `filter[platform]` do TheGamesDB (tabela do ES-DE). Mais de
/// um id quando o TheGamesDB separa o que o ReEmu junta (Genesis/Mega Drive,
/// WonderSwan/Color, NGP/Color, Atari 8-bit/XE).
fn thegamesdb_platforms(system_id: &str) -> Option<&'static str> {
    Some(match system_id {
        "nes" => "7",
        "snes" => "6",
        "n64" => "3",
        "gb" => "4",
        "gbc" => "41",
        "gba" => "5",
        "vb" => "4918",
        "nds" => "8",
        "pokemini" => "4957",
        "megadrive" => "18,36",
        "mastersystem" => "35",
        "gamegear" => "20",
        "sega32x" => "33",
        "segacd" => "21",
        "saturn" => "17",
        "dreamcast" => "16",
        "sg1000" => "4949",
        "naomi" | "atomiswave" | "arcade" => "23",
        "psx" => "10",
        "ps2" => "11",
        "psp" => "13",
        "pcengine" | "supergrafx" => "34",
        "pcenginecd" => "4955",
        "pcfx" => "4930",
        "atari2600" => "22",
        "atari5200" => "26",
        "atari7800" => "27",
        "atari8bit" => "4943,30",
        "jaguar" => "28",
        "lynx" => "4924",
        "wonderswan" => "4925,4926",
        "ngp" => "4922,4923",
        "neogeocd" => "4956",
        "coleco" => "31",
        "intellivision" => "32",
        "vectrex" => "4939",
        "odyssey2" => "4927",
        "supervision" => "4959",
        "3do" => "25",
        "cdi" => "4917",
        "msx" => "4929",
        "c64" => "40",
        "amiga" => "4911",
        "zxspectrum" => "4913",
        "amstradcpc" => "4914",
        "dos" | "scummvm" => "1",
        _ => return None,
    })
}

/// Nome do arquivo → título pra busca por nome: tira as tags No-Intro/TOSEC
/// (`(USA)`, `(Rev 1)`, `[!]`…) e troca `_` por espaço.
fn search_title(file_stem: &str) -> String {
    let mut out = String::with_capacity(file_stem.len());
    let mut depth = 0i32;
    for c in file_stem.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => out.push(if c == '_' { ' ' } else { c }),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 1º jogo de `data.games` do `/v1/Games/ByGameName` → candidato (sem capa;
/// a capa vem de outra chamada, `parse_tgdb_boxart`).
fn parse_tgdb_game(v: &Value) -> Option<ScrapeCandidate> {
    let game = v.get("data")?.get("games")?.as_array()?.first()?;
    let id = game.get("id")?;
    let external_id = id
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| id.to_string());
    let text = |k: &str| {
        game.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(ScrapeCandidate {
        provider: "thegamesdb".into(),
        external_id,
        title: text("game_title")?,
        description: text("overview").map(|d| d.replace('\r', "")),
        cover_url: None,
        release_date: text("release_date"),
        // Os gêneros vêm só como ids numéricos (a tabela exige outra chamada
        // com a chave) — melhor sem gênero que com um chute.
        genre: None,
        // Busca por nome: nunca auto-aplica (ver comentário do módulo).
        exact_hash_match: false,
        exact_filename_match: false,
    })
}

/// Capa (boxart frontal) do `/v1/Games/Images`: `data.base_url.large` +
/// o `filename` do item `type: boxart, side: front` do jogo `game_id`.
fn parse_tgdb_boxart(v: &Value, game_id: &str) -> Option<String> {
    let data = v.get("data")?;
    let base = data.get("base_url")?.get("large")?.as_str()?;
    let images = data.get("images")?.get(game_id)?.as_array()?;
    let front = images.iter().find(|i| {
        i.get("type").and_then(Value::as_str) == Some("boxart")
            && i.get("side").and_then(Value::as_str) == Some("front")
    })?;
    Some(format!("{base}{}", front.get("filename")?.as_str()?))
}

/// Consulta o TheGamesDB por nome + plataforma. `Ok(None)` = nada achado ou
/// sistema que ele não cobre.
async fn query_thegamesdb(
    client: &reqwest::Client,
    api_key: &str,
    q: &ScrapeQuery<'_>,
) -> Result<Option<ScrapeCandidate>, String> {
    let Some(platforms) = thegamesdb_platforms(q.system_id) else {
        return Ok(None);
    };
    let title = search_title(q.file_stem);
    if title.is_empty() {
        return Ok(None);
    }
    let get = |path: &str, params: &[(&str, &str)]| {
        let mut url = reqwest::Url::parse(&format!("https://api.thegamesdb.net/v1/{path}"))
            .expect("URL fixa válida");
        url.query_pairs_mut()
            .append_pair("apikey", api_key)
            .extend_pairs(params);
        client.get(url).send()
    };
    let read = |resp: reqwest::Response| async move {
        let status = resp.status();
        let body = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            // 403 = chave inválida ou cota do mês esgotada
            return Err(format!(
                "TheGamesDB HTTP {status}: {}",
                body.chars().take(160).collect::<String>().trim()
            ));
        }
        serde_json::from_str::<Value>(&body).map_err(|e| format!("TheGamesDB: JSON inválido ({e})"))
    };

    let found = read(
        get(
            "Games/ByGameName",
            &[
                ("name", title.as_str()),
                ("filter[platform]", platforms),
                ("fields", "overview"),
            ],
        )
        .await
        .map_err(|e| format!("rede: {e}"))?,
    )
    .await?;
    let Some(mut c) = parse_tgdb_game(&found) else {
        return Ok(None);
    };
    // Capa: segunda chamada. Falhar aqui não perde o resto da metadata.
    match get("Games/Images", &[("games_id", c.external_id.as_str())]).await {
        Ok(resp) => match read(resp).await {
            Ok(v) => c.cover_url = parse_tgdb_boxart(&v, &c.external_id),
            Err(e) => log::warn!("metadata: capa do TheGamesDB: {e}"),
        },
        Err(e) => log::warn!("metadata: capa do TheGamesDB: rede: {e}"),
    }
    Ok(Some(c))
}

/// Progresso de uma leva de scraping (consultável via comando).
#[derive(Default)]
pub struct ScrapeProgress {
    pub running: AtomicBool,
    pub done: AtomicUsize,
    pub total: AtomicUsize,
    pub auto: AtomicUsize,
    pub pending: AtomicUsize,
    pub failed: AtomicUsize,
}

impl ScrapeProgress {
    pub fn snapshot(&self) -> (bool, usize, usize, usize, usize, usize) {
        (
            self.running.load(Ordering::Relaxed),
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
            self.auto.load(Ordering::Relaxed),
            self.pending.load(Ordering::Relaxed),
            self.failed.load(Ordering::Relaxed),
        )
    }
    fn reset(&self, total: usize) {
        self.done.store(0, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        self.auto.store(0, Ordering::Relaxed);
        self.pending.store(0, Ordering::Relaxed);
        self.failed.store(0, Ordering::Relaxed);
        self.running.store(true, Ordering::Relaxed);
    }
}

fn first_text<'a>(arr: &'a Value, prefer_lang: &[&str]) -> Option<&'a str> {
    let items = arr.as_array()?;
    for lang in prefer_lang {
        for it in items {
            let matches = it.get("langue").and_then(Value::as_str) == Some(*lang)
                || it.get("region").and_then(Value::as_str) == Some(*lang);
            if matches {
                if let Some(t) = it.get("text").and_then(Value::as_str) {
                    return Some(t);
                }
            }
        }
    }
    items.first()?.get("text").and_then(Value::as_str)
}

/// Nome de arquivo "canônico" que o ScreenScraper reconhece pro candidato
/// (campo `rom.romfilename` do `jeuInfos.php`), sem extensão.
fn candidate_filename_stem(jeu: &Value) -> Option<&str> {
    let name = jeu.get("rom")?.get("romfilename")?.as_str()?;
    // Tira a extensão à mão (evita puxar `std::path::Path` só pra isto e
    // funciona igual pra nomes com pontos no meio, ex. "Sonic 3 & Knuckles").
    Some(name.rsplit_once('.').map_or(name, |(stem, _ext)| stem))
}

/// Extrai um `ScrapeCandidate` do JSON do `jeuInfos.php`. `file_stem` é o
/// nome do arquivo local (sem extensão) — comparado, exato e
/// case-insensitive, contra `candidate_filename_stem` pro segundo critério
/// de auto-match (ver comentário do módulo).
fn parse_jeu(jeu: &Value, exact_hash: bool, file_stem: &str) -> Option<ScrapeCandidate> {
    let title = first_text(jeu.get("noms")?, &["wor", "ss", "us", "eu", "jp"])?.to_string();
    let external_id = jeu
        .get("id")
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default();
    // Idiomas pelo `nomcourt` da `languesListe.php` (doc da API v2 do
    // ScreenScraper): português primeiro, depois inglês e o texto "mundial".
    let description = jeu
        .get("synopsis")
        .and_then(|s| first_text(s, &["pt", "en", "wor"]))
        .map(str::to_string);
    let release_date = jeu
        .get("dates")
        .and_then(|d| first_text(d, &["wor", "us", "eu", "jp"]))
        .map(str::to_string);
    // Todos os gêneros, o principal (`principale: "1"` na doc) primeiro,
    // separados por ", " — a gaveta de informações mostra cada um como etiqueta.
    let genre = jeu.get("genres").and_then(|g| {
        let arr = g.as_array()?;
        let is_main = |x: &Value| {
            matches!(x.get("principale"), Some(v) if v.as_str() == Some("1") || v.as_i64() == Some(1))
        };
        let mut ordered: Vec<&Value> = arr.iter().filter(|x| is_main(x)).collect();
        ordered.extend(arr.iter().filter(|x| !is_main(x)));
        let mut names: Vec<String> = Vec::new();
        for x in ordered {
            if let Some(n) = x.get("noms").and_then(|n| first_text(n, &["pt", "en", "wor"])) {
                let n = n.trim().to_string();
                if !n.is_empty() && !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        (!names.is_empty()).then(|| names.join(", "))
    });
    let cover_url = jeu.get("medias").and_then(|m| {
        let arr = m.as_array()?;
        arr.iter()
            .find(|x| {
                matches!(
                    x.get("type").and_then(Value::as_str),
                    Some("box-2D") | Some("box-2d")
                )
            })
            .or_else(|| arr.first())
            .and_then(|x| x.get("url").and_then(Value::as_str))
            .map(str::to_string)
    });

    let exact_filename_match = candidate_filename_stem(jeu)
        .is_some_and(|candidate_stem| candidate_stem.eq_ignore_ascii_case(file_stem));

    Some(ScrapeCandidate {
        provider: "screenscraper".into(),
        external_id,
        title,
        description,
        cover_url,
        release_date,
        genre,
        exact_hash_match: exact_hash,
        exact_filename_match,
    })
}

/// Uma consulta ao ScreenScraper. `Ok(None)` = não catalogado (404).
async fn query_screenscraper(
    client: &reqwest::Client,
    cfg: &MetadataConfig,
    q: &ScrapeQuery<'_>,
) -> Result<Option<ScrapeCandidate>, String> {
    let Some(sys) = screenscraper_system_id(q.system_id) else {
        return Ok(None); // sistema que o ScreenScraper não cobre (ex: 'disc')
    };
    let mut url = reqwest::Url::parse("https://api.screenscraper.fr/api2/jeuInfos.php")
        .map_err(|e| e.to_string())?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("output", "json")
            .append_pair("softname", "reemu")
            .append_pair("systemeid", &sys.to_string())
            .append_pair("crc", &q.hash.crc32.to_uppercase())
            .append_pair("romnom", &format!("{}.zip", q.file_stem));
        // MD5 junto: o ScreenScraper identifica por qualquer um dos hashes
        // (o ES-DE consulta por MD5) — ajuda quando o CRC não está catalogado.
        if !q.hash.md5.is_empty() {
            qp.append_pair("md5", &q.hash.md5.to_lowercase());
        }
        if let (Some(u), Some(p)) = (&cfg.screenscraper_user, &cfg.screenscraper_password) {
            if !u.is_empty() {
                qp.append_pair("ssid", u).append_pair("sspassword", p);
            }
        }
    }

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("rede: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;

    if status.as_u16() == 404 || body.contains("Erreur : Rom/Iso/Dossier non trouv") {
        return Ok(None);
    }
    if !status.is_success() {
        // 429/430/431 = quota; 400 = credencial ruim; etc.
        return Err(format!(
            "ScreenScraper HTTP {}: {}",
            status,
            body.chars().take(160).collect::<String>().trim()
        ));
    }

    let v: Value = serde_json::from_str(&body).map_err(|e| {
        format!(
            "resposta não-JSON ({e}): {}",
            body.chars().take(120).collect::<String>()
        )
    })?;
    let jeu = v
        .get("response")
        .and_then(|r| r.get("jeu"))
        .ok_or("resposta sem 'jeu'")?;

    Ok(parse_jeu(jeu, rom_hash_matches(jeu, q), q.file_stem))
}

/// O hash bateu? O ScreenScraper devolve em `rom` o CRC (`romcrc`) e o MD5
/// (`rommd5`) da ROM que ele achou — qualquer um igual ao nosso é exato.
fn rom_hash_matches(jeu: &Value, q: &ScrapeQuery<'_>) -> bool {
    let rom = jeu.get("rom");
    let field = |k: &str| rom.and_then(|r| r.get(k)).and_then(Value::as_str);
    let crc = field("romcrc").is_some_and(|c| c.eq_ignore_ascii_case(&q.hash.crc32));
    let md5 = !q.hash.md5.is_empty()
        && field("rommd5").is_some_and(|m| m.eq_ignore_ascii_case(&q.hash.md5));
    crc || md5
}

/// Roda uma leva de scraping sobre as ROMs sem match. Bloqueante (chamar de
/// `spawn_blocking`/task). `stop` permite cancelar. `covers_dir`: descarta o
/// cache de capa (`covers.rs`) de qualquer rom que ganhar uma `cover_url`
/// nova aqui, pra próxima leitura buscar a capa escrapeada.
pub async fn scrape_pending(
    pool: db::Db,
    progress: Arc<ScrapeProgress>,
    stop: Arc<AtomicBool>,
    covers_dir: std::path::PathBuf,
) -> Result<(), String> {
    let repo = db::MetadataRepo::new(pool.clone());
    let roms_repo = db::RomsRepo::new(pool.clone());
    let cfg = crate::credentials::load_config(&repo, &crate::credentials::OsKeyring).await?;

    let ids = repo
        .rom_ids_without_match()
        .await
        .map_err(|e| e.to_string())?;
    progress.reset(ids.len());
    log::info!("metadata: {} ROM(s) na fila de scraping", ids.len());

    let client = reqwest::Client::builder()
        .user_agent("reemu/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    for rom_id in ids {
        if stop.load(Ordering::Relaxed) {
            log::info!("metadata: scraping cancelado");
            break;
        }
        let rom = match roms_repo.get(&rom_id).await {
            Ok(Some(r)) => r,
            _ => {
                progress.done.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        let stem = std::path::Path::new(&rom.file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let hash = domain::metadata::RomHash {
            crc32: rom.crc32.clone(),
            md5: rom.md5.clone(),
        };
        let q = ScrapeQuery {
            hash: &hash,
            system_id: &rom.system_id,
            file_stem: &stem,
        };

        // Cascata: ScreenScraper (hash) primeiro; se ele não achar e houver
        // chave do TheGamesDB, tenta por nome lá — sempre pra revisão.
        let tgdb_key = cfg.thegamesdb_api_key.as_deref().filter(|k| !k.is_empty());
        let mut result = query_screenscraper(&client, &cfg, &q).await;
        if let (Ok(None), Some(key)) = (&result, tgdb_key) {
            tokio::time::sleep(REQUEST_DELAY).await;
            result = query_thegamesdb(&client, key, &q).await;
            tokio::time::sleep(TGDB_DELAY).await;
        }
        match result {
            Ok(Some(c)) => {
                let auto = c.auto_matches();
                let status = if auto {
                    MatchStatus::AutoMatched
                } else {
                    MatchStatus::PendingReview
                };
                if let Err(e) = repo.record_match(&rom_id, &c, status).await {
                    log::warn!("metadata: gravar match de {stem}: {e}");
                    progress.failed.fetch_add(1, Ordering::Relaxed);
                } else if auto {
                    let ok = repo
                        .upsert_metadata(&GameMetadata {
                            rom_id: rom_id.clone(),
                            title: c.title.clone(),
                            description: c.description.clone(),
                            cover_url: c.cover_url.clone(),
                            release_date: c.release_date.clone(),
                            genre: c.genre.clone(),
                            provider_source: Some(c.provider.clone()),
                        })
                        .await
                        .is_ok();
                    if ok {
                        crate::covers::invalidate(&covers_dir, &rom_id);
                    }
                    progress.auto.fetch_add(1, Ordering::Relaxed);
                } else {
                    progress.pending.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(None) => {
                let _ = repo
                    .record_match(
                        &rom_id,
                        &ScrapeCandidate {
                            provider: "screenscraper".into(),
                            external_id: String::new(),
                            title: stem.clone(),
                            description: None,
                            cover_url: None,
                            release_date: None,
                            genre: None,
                            exact_hash_match: false,
                            exact_filename_match: false,
                        },
                        MatchStatus::NoMatch,
                    )
                    .await;
            }
            Err(e) => {
                log::warn!("metadata: {stem}: {e}");
                progress.failed.fetch_add(1, Ordering::Relaxed);
                // erro de quota/rede: espera mais antes de continuar
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }

        progress.done.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(REQUEST_DELAY).await;
    }

    progress.running.store(false, Ordering::Relaxed);
    let (_, done, total, auto, pending, failed) = progress.snapshot();
    log::info!(
        "metadata: leva concluída — {done}/{total} ({auto} auto, {pending} p/ revisão, {failed} falha)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::metadata::RomHash;
    use serde_json::json;

    /// `system_id`s que o scan produz — lidos do próprio `systems.rs` (tabela
    /// canônica), pra sistema novo lá sem id aqui quebrar o teste.
    fn scan_system_ids() -> Vec<String> {
        let src = include_str!("../../../../crates/library-scan/src/systems.rs");
        let mut ids: Vec<String> = src
            .split("=> \"")
            .skip(1)
            .filter_map(|rest| {
                let id = rest.split('"').next()?;
                id.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                    .then(|| id.to_string())
            })
            .filter(|id| id != "disc")
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    #[test]
    fn every_scanned_system_has_ids_in_both_providers() {
        let ids = scan_system_ids();
        assert!(
            ids.len() > 40,
            "o parse ainda acha a tabela ({})",
            ids.len()
        );
        let no_ss: Vec<_> = ids
            .iter()
            .filter(|i| screenscraper_system_id(i).is_none())
            .collect();
        let no_tgdb: Vec<_> = ids
            .iter()
            .filter(|i| thegamesdb_platforms(i).is_none())
            .collect();
        assert!(no_ss.is_empty(), "sem id no ScreenScraper: {no_ss:?}");
        assert!(no_tgdb.is_empty(), "sem id no TheGamesDB: {no_tgdb:?}");
    }

    #[test]
    fn parse_jeu_prefers_portuguese_and_main_genre() {
        // Formato do `jeuInfos.php` (doc da API v2): textos por `langue`,
        // gêneros com `principale` e nomes por idioma.
        let jeu = serde_json::json!({
            "id": "3",
            "noms": [{"region": "wor", "text": "Sonic The Hedgehog"}],
            "synopsis": [
                {"langue": "en", "text": "A blue hedgehog."},
                {"langue": "pt", "text": "Um ouriço azul."}
            ],
            "dates": [{"region": "us", "text": "1991-06-23"}],
            "genres": [
                {"id": "2", "principale": "0",
                 "noms": [{"langue": "en", "text": "Action"}, {"langue": "pt", "text": "Ação"}]},
                {"id": "7", "principale": "1",
                 "noms": [{"langue": "en", "text": "Platform"}, {"langue": "pt", "text": "Plataforma"}]}
            ]
        });
        let c = parse_jeu(&jeu, true, "Sonic").expect("candidato");
        assert_eq!(c.description.as_deref(), Some("Um ouriço azul."));
        assert_eq!(c.genre.as_deref(), Some("Plataforma, Ação"));
        assert_eq!(c.release_date.as_deref(), Some("1991-06-23"));
    }

    #[test]
    fn search_title_drops_dump_tags() {
        assert_eq!(search_title("Super Mario World (USA)"), "Super Mario World");
        assert_eq!(
            search_title("Chrono_Trigger (USA) (Rev 1) [!]"),
            "Chrono Trigger"
        );
        assert_eq!(search_title("Sonic 3 & Knuckles"), "Sonic 3 & Knuckles");
        assert_eq!(search_title("(Beta)"), "");
    }

    #[test]
    fn exact_hash_by_crc_or_md5() {
        let hash = RomHash {
            crc32: "b19ed489".into(),
            md5: "cdd3c8c37322978ca8669b34bc89c804".into(),
        };
        let q = ScrapeQuery {
            hash: &hash,
            system_id: "snes",
            file_stem: "x",
        };
        let crc = json!({"rom": {"romcrc": "B19ED489"}});
        let md5 =
            json!({"rom": {"romcrc": "00000000", "rommd5": "CDD3C8C37322978CA8669B34BC89C804"}});
        let none = json!({"rom": {"romcrc": "00000000", "rommd5": "ffff"}});
        assert!(rom_hash_matches(&crc, &q));
        assert!(rom_hash_matches(&md5, &q), "MD5 igual também é exato");
        assert!(!rom_hash_matches(&none, &q));
        assert!(!rom_hash_matches(&json!({}), &q));
    }

    // JSON no formato que o ES-DE lê do TheGamesDB (`GamesDBJSONScraper.cpp`:
    // `data.games[].{id, game_title, overview, release_date}` e
    // `data.base_url.large` + `data.images[<id>][].{type, side, filename}`).
    // Montado a partir desses campos — não é uma resposta capturada ao vivo.

    #[test]
    fn tgdb_game_goes_to_review_never_auto() {
        let v = json!({"data": {"games": [
            {"id": 1018, "game_title": "Chrono Trigger", "release_date": "1995-08-11",
             "platform": 6, "overview": "Linha 1.\r\nLinha 2.\r\n"},
            {"id": 9, "game_title": "Outro"}
        ]}});
        let c = parse_tgdb_game(&v).expect("candidato");
        assert_eq!(c.provider, "thegamesdb");
        assert_eq!(c.external_id, "1018");
        assert_eq!(c.title, "Chrono Trigger");
        assert_eq!(c.release_date.as_deref(), Some("1995-08-11"));
        assert_eq!(c.description.as_deref(), Some("Linha 1.\nLinha 2."));
        assert!(!c.auto_matches(), "busca por nome vai sempre pra revisão");
        assert!(parse_tgdb_game(&json!({"data": {"games": []}})).is_none());
        assert!(parse_tgdb_game(&json!({"code": 403})).is_none());
    }

    #[test]
    fn tgdb_boxart_front_with_base_url() {
        let v = json!({"data": {
            "base_url": {"large": "https://cdn.thegamesdb.net/images/large/"},
            "images": {"1018": [
                {"type": "boxart", "side": "back", "filename": "boxart/back/1018-1.jpg"},
                {"type": "boxart", "side": "front", "filename": "boxart/front/1018-1.jpg"}
            ]}
        }});
        assert_eq!(
            parse_tgdb_boxart(&v, "1018").as_deref(),
            Some("https://cdn.thegamesdb.net/images/large/boxart/front/1018-1.jpg")
        );
        assert_eq!(parse_tgdb_boxart(&v, "999"), None);
    }
}
