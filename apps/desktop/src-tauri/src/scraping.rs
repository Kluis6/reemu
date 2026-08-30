//! Scraping de metadata (etapa 09). Provider: **ScreenScraper** (`screenscraper.fr`)
//! por hash (CRC32) — match exato = auto; qualquer resultado por nome vai pra
//! `pending_review` (decisão Abordagem B, ver docs/ai-context/09).
//!
//! Credenciais do usuário (conta grátis no screenscraper.fr) são opcionais mas
//! melhoram muito o limite de requisições — anônimo é bem restrito.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use domain::library::RomRepository;
use domain::metadata::{
    GameMetadata, MatchStatus, MetadataConfig, MetadataRepository, ScrapeCandidate, ScrapeQuery,
};
use serde_json::Value;

/// Pausa entre requisições — o ScreenScraper anônimo é bem limitado.
const REQUEST_DELAY: std::time::Duration = std::time::Duration::from_millis(1200);

/// `system_id` canônico do ReEmu → `systemeid` do ScreenScraper.
fn screenscraper_system_id(system_id: &str) -> Option<u32> {
    Some(match system_id {
        "nes" => 3,
        "snes" => 4,
        "n64" => 14,
        "gb" => 9,
        "gbc" => 10,
        "gba" => 12,
        "megadrive" => 1,
        "mastersystem" => 2,
        "gamegear" => 21,
        "sega32x" => 19,
        "pcengine" => 31,
        "atari2600" => 26,
        "lynx" => 28,
        "wonderswan" => 45,
        "ngp" => 25,
        _ => return None,
    })
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

/// Extrai um `ScrapeCandidate` do JSON do `jeuInfos.php`.
fn parse_jeu(jeu: &Value, exact: bool) -> Option<ScrapeCandidate> {
    let title = first_text(jeu.get("noms")?, &["wor", "ss", "us", "eu", "jp"])?.to_string();
    let external_id = jeu
        .get("id")
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default();
    let description = jeu
        .get("synopsis")
        .and_then(|s| first_text(s, &["en", "wor"]))
        .map(str::to_string);
    let release_date = jeu
        .get("dates")
        .and_then(|d| first_text(d, &["wor", "us", "eu", "jp"]))
        .map(str::to_string);
    let genre = jeu.get("genres").and_then(|g| {
        g.as_array()?
            .first()?
            .get("noms")
            .and_then(|n| first_text(n, &["en", "wor"]))
            .map(str::to_string)
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

    Some(ScrapeCandidate {
        provider: "screenscraper".into(),
        external_id,
        title,
        description,
        cover_url,
        release_date,
        genre,
        exact_hash_match: exact,
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

    // O CRC bateu? o ScreenScraper devolve `romcrc` / `rom` com o hash usado.
    let exact = jeu
        .get("rom")
        .and_then(|r| r.get("romcrc"))
        .and_then(Value::as_str)
        .map(|c| c.eq_ignore_ascii_case(&q.hash.crc32))
        .unwrap_or(false);

    Ok(parse_jeu(jeu, exact))
}

/// Roda uma leva de scraping sobre as ROMs sem match. Bloqueante (chamar de
/// `spawn_blocking`/task). `stop` permite cancelar.
pub async fn scrape_pending(
    pool: db::Db,
    progress: Arc<ScrapeProgress>,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let repo = db::MetadataRepo::new(pool.clone());
    let roms_repo = db::RomsRepo::new(pool.clone());
    let cfg = repo.get_config().await.map_err(|e| e.to_string())?;

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

        match query_screenscraper(&client, &cfg, &q).await {
            Ok(Some(c)) => {
                let status = if c.exact_hash_match {
                    MatchStatus::AutoMatched
                } else {
                    MatchStatus::PendingReview
                };
                if let Err(e) = repo.record_match(&rom_id, &c, status).await {
                    log::warn!("metadata: gravar match de {stem}: {e}");
                    progress.failed.fetch_add(1, Ordering::Relaxed);
                } else if c.exact_hash_match {
                    let _ = repo
                        .upsert_metadata(&GameMetadata {
                            rom_id: rom_id.clone(),
                            title: c.title.clone(),
                            description: c.description.clone(),
                            cover_url: c.cover_url.clone(),
                            release_date: c.release_date.clone(),
                            genre: c.genre.clone(),
                            provider_source: Some(c.provider.clone()),
                        })
                        .await;
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
