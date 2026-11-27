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
//! A GameKave é a última opção, por título e plataforma, com revisão manual.
//!
//! Ids de plataforma do ScreenScraper e do TheGamesDB: tabelas do ES-DE (EmulationStation
//! Desktop Edition, GPL — `es-app/src/scrapers/ScreenScraper.cpp` e
//! `GamesDBJSONScraper.cpp`), não de memória; os 15 ids do ScreenScraper que
//! o ReEmu já tinha batem com os de lá.
//!
//! Crate própria desde a Fase 6 (item 12 da análise de 2026-11-12): o app
//! carrega a configuração (com a senha do chaveiro, `credentials.rs`) e passa
//! o cache de capas como [`CoverCache`].

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use domain::library::{Rom, RomRepository};
use domain::metadata::{
    GameDetails, GameMetadata, MatchStatus, MetadataConfig, MetadataRepository, RomHash,
    ScrapeCandidate, ScrapeQuery,
};
use serde_json::Value;

mod gamekave;
pub mod rom_name;

/// Cache de capas do app: descarta a capa guardada de um jogo que ganhou
/// uma `cover_url` nova, pra próxima leitura buscar a capa escrapeada.
pub trait CoverCache: Send + Sync {
    fn invalidate(&self, rom_id: &str);
}

/// Pausa entre requisições ao ScreenScraper — anônimo é bem limitado.
const REQUEST_DELAY: std::time::Duration = std::time::Duration::from_millis(1200);
/// Pausa depois de consultar o TheGamesDB — a chave tem cota MENSAL
/// (`remaining_monthly_allowance`), então sem pressa.
const TGDB_DELAY: std::time::Duration = std::time::Duration::from_millis(1000);
/// Pausa depois de um erro de quota ou de rede, antes da próxima ROM.
const ERROR_PAUSE: std::time::Duration = std::time::Duration::from_secs(5);

/// Onde ficam os provedores e quanto esperar entre perguntas. O app usa
/// [`Providers::live`]; os testes apontam pra um servidor local, sem pausa.
struct Providers {
    /// Base do `jeuInfos.php` (sem a barra do fim).
    screenscraper: String,
    /// Base da API v1 do TheGamesDB (sem a barra do fim).
    thegamesdb: String,
    /// Busca de jogos da GameKave (a URL inteira do `/games/search`).
    gamekave: String,
    request_delay: std::time::Duration,
    tgdb_delay: std::time::Duration,
    error_pause: std::time::Duration,
}

impl Providers {
    fn live() -> Self {
        Providers {
            screenscraper: "https://api.screenscraper.fr/api2".into(),
            thegamesdb: "https://api.thegamesdb.net/v1".into(),
            gamekave: gamekave::SEARCH_URL.into(),
            request_delay: REQUEST_DELAY,
            tgdb_delay: TGDB_DELAY,
            error_pause: ERROR_PAUSE,
        }
    }
}

/// Texto de um erro do `reqwest` sem a URL: a do ScreenScraper leva a senha
/// de desenvolvedor do ReEmu e o login e a senha da pessoa, a do TheGamesDB
/// leva a chave, e as duas o nome e os hashes do jogo. O texto vai pro log e
/// pra interface.
fn net_error(e: reqwest::Error) -> String {
    e.without_url().to_string()
}

/// Credenciais de DESENVOLVEDOR do ScreenScraper (as do software, não as do
/// usuário). A API responde 403 "Erreur de login : Vérifier vos identifiants
/// développeur !" sem elas — api.screenscraper.fr/webapi2.php: o dev pede as
/// suas "via le forum". Vêm do build (segredo do CI), nunca do repositório.
const SS_DEVID: Option<&str> = option_env!("REEMU_SS_DEVID");
const SS_DEVPASSWORD: Option<&str> = option_env!("REEMU_SS_DEVPASSWORD");

/// Código de erro da leva (a UI traduz): o ScreenScraper recusou as
/// credenciais de desenvolvedor do ReEmu.
pub const ERR_SS_DEV_LOGIN: &str = "ss_dev_login";

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
        "atarist" => 42,
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
        "zx81" => 77,
        "x68000" => 79,
        "amstradcpc" => 65,
        "arcade" => 75,
        "laserdisc" => 49, // Daphne no catálogo ScreenScraper
        "arduboy" => 263,
        "megaduck" => 90,
        "uzebox" => 216,
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
        "atarist" => "4937",
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
        "zx81" => "5010",
        "x68000" => "4931",
        "amstradcpc" => "4914",
        "megaduck" => "4948",
        "dos" | "scummvm" => "1",
        _ => return None,
    })
}

use rom_name::search_title;

/// Escolhe em `data.games` do `/v1/Games/ByGameName` o jogo cujo título casa
/// canonicamente com `rom_title` (caindo para o 1º item quando a API devolve
/// uma grafia regional diferente, mantendo o comportamento de revisão manual).
#[cfg(test)]
fn parse_tgdb_game(v: &Value) -> Option<ScrapeCandidate> {
    parse_tgdb_game_for(v, None)
}

fn parse_tgdb_game_for(v: &Value, rom_title: Option<&str>) -> Option<ScrapeCandidate> {
    let games = v.get("data")?.get("games")?.as_array()?;
    let game = rom_title
        .filter(|t| !t.trim().is_empty())
        .and_then(|target| {
            games
                .iter()
                .filter_map(|g| {
                    let name = g.get("game_title").and_then(Value::as_str)?.trim();
                    let tier = rom_name::title_match_tier(target, name)?;
                    Some((tier, g))
                })
                .max_by_key(|(tier, _)| *tier)
                .map(|(_, g)| g)
        })
        .or_else(|| games.first())?;
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
        // `players` (inteiro), `rating` (classificação, ex.: "E - Everyone") e
        // `coop` ("Yes"/"No") do spec.yaml. Publicadora/desenvolvedora vêm só
        // como ids (outra chamada por jogo pra ter o nome) — ficam de fora.
        details: GameDetails {
            players: game
                .get("players")
                .and_then(Value::as_u64)
                .filter(|n| *n > 0)
                .map(|n| n.to_string()),
            age_rating: text("rating"),
            modes: (text("coop").as_deref() == Some("Yes")).then(|| "Co-op".to_string()),
            ..Default::default()
        },
    })
}

/// Capa (boxart frontal) do `/v1/Games/Images`: `data.base_url.large` +
/// o `filename` do item `type: boxart, side: front` do jogo `game_id`.
/// Capa frontal do `include.boxart` da própria busca (`base_url.large` +
/// `filename` do item `side: front` do jogo), como no spec.yaml.
fn parse_tgdb_included_boxart(v: &Value, game_id: &str) -> Option<String> {
    let bx = v.get("include")?.get("boxart")?;
    let base = bx.get("base_url")?.get("large")?.as_str()?;
    let front = bx
        .get("data")?
        .get(game_id)?
        .as_array()?
        .iter()
        .find(|i| i.get("side").and_then(Value::as_str) == Some("front"))?;
    Some(format!("{base}{}", front.get("filename")?.as_str()?))
}

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
    base: &str,
) -> Result<Option<ScrapeCandidate>, String> {
    let Some(platforms) = thegamesdb_platforms(q.system_id) else {
        return Ok(None);
    };
    let title = search_title(q.file_stem);
    if title.is_empty() {
        return Ok(None);
    }
    let get = |path: &str, params: &[(&str, &str)]| {
        let mut url = reqwest::Url::parse(&format!("{base}/{path}")).expect("URL fixa válida");
        url.query_pairs_mut()
            .append_pair("apikey", api_key)
            .extend_pairs(params);
        client.get(url).send()
    };
    let read = |resp: reqwest::Response| async move {
        let status = resp.status();
        let body = resp.text().await.map_err(net_error)?;
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
                // Campos e `include` do `/v1/Games/ByGameName` no spec.yaml
                // oficial (api.thegamesdb.net/spec.yaml): a capa vem na mesma
                // resposta (`include.boxart`), sem a 2ª chamada — a cota é de
                // 1000 requisições/mês por chave.
                ("fields", "overview,players,rating,coop"),
                ("include", "boxart"),
            ],
        )
        .await
        .map_err(|e| format!("rede: {}", net_error(e)))?,
    )
    .await?;
    let Some(mut c) = parse_tgdb_game_for(&found, Some(q.file_stem)) else {
        return Ok(None);
    };
    c.cover_url = parse_tgdb_included_boxart(&found, &c.external_id);
    if c.cover_url.is_some() {
        return Ok(Some(c));
    }
    // Sem capa no `include`: segunda chamada. Falhar aqui não perde o resto.
    match get("Games/Images", &[("games_id", c.external_id.as_str())]).await {
        Ok(resp) => match read(resp).await {
            Ok(v) => c.cover_url = parse_tgdb_boxart(&v, &c.external_id),
            Err(e) => log::warn!("metadata: capa do TheGamesDB: {e}"),
        },
        Err(e) => log::warn!("metadata: capa do TheGamesDB: rede: {}", net_error(e)),
    }
    Ok(Some(c))
}

/// Estado visível mesmo quando a leva termina sem processar nenhuma ROM.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScrapeState {
    #[default]
    Idle,
    Preparing,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl ScrapeState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Preparing | Self::Running | Self::Cancelling)
    }
}

/// Progresso de uma leva de scraping (consultável via comando).
#[derive(Default)]
pub struct ScrapeProgress {
    state: std::sync::Mutex<ScrapeState>,
    done: AtomicUsize,
    total: AtomicUsize,
    auto: AtomicUsize,
    pending: AtomicUsize,
    failed: AtomicUsize,
    error: std::sync::Mutex<Option<String>>,
}

impl ScrapeProgress {
    fn state(&self) -> std::sync::MutexGuard<'_, ScrapeState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn set_error(&self, code: &str) {
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = Some(code.to_string());
    }

    pub fn snapshot(
        &self,
    ) -> (
        ScrapeState,
        usize,
        usize,
        usize,
        usize,
        usize,
        Option<String>,
    ) {
        let state = self.state();
        (
            *state,
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
            self.auto.load(Ordering::Relaxed),
            self.pending.load(Ordering::Relaxed),
            self.failed.load(Ordering::Relaxed),
            self.error.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        )
    }

    pub fn begin(&self) -> bool {
        let mut state = self.state();
        if state.is_active() {
            return false;
        }
        self.done.store(0, Ordering::Relaxed);
        self.total.store(0, Ordering::Relaxed);
        self.auto.store(0, Ordering::Relaxed);
        self.pending.store(0, Ordering::Relaxed);
        self.failed.store(0, Ordering::Relaxed);
        *self.error.lock().unwrap_or_else(|p| p.into_inner()) = None;
        *state = ScrapeState::Preparing;
        true
    }

    pub fn cancel(&self) {
        let mut state = self.state();
        if state.is_active() {
            *state = ScrapeState::Cancelling;
        }
    }

    fn is_cancelled(&self) -> bool {
        *self.state() == ScrapeState::Cancelling
    }

    fn start_processing(&self, total: usize) {
        let mut state = self.state();
        self.total.store(total, Ordering::Relaxed);
        if *state == ScrapeState::Preparing {
            *state = ScrapeState::Running;
        }
    }

    pub fn finish(&self, failed: bool) {
        let mut state = self.state();
        *state = match *state {
            ScrapeState::Cancelling => ScrapeState::Cancelled,
            _ if failed => ScrapeState::Failed,
            _ => ScrapeState::Completed,
        };
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
        details: parse_details(jeu),
    })
}

/// Texto de um campo do `jeu` que pode vir como string, como `{ "text": .. }`
/// ou como lista por idioma/região (`[{ "langue"/"region", "text" }]`).
fn field_text(v: &Value) -> Option<String> {
    let t = match v {
        Value::String(s) => Some(s.as_str()),
        Value::Object(o) => o.get("text").and_then(Value::as_str),
        Value::Array(_) => first_text(v, &["pt", "en", "wor", "us", "eu", "ss", "jp"]),
        _ => None,
    }?;
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Nomes de um grupo (`modes`, `genres`…): cada item com `noms` por idioma.
fn group_names(v: &Value) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for x in v.as_array()? {
        if let Some(n) = x
            .get("noms")
            .and_then(|n| first_text(n, &["pt", "en", "wor"]))
        {
            let n = n.trim().to_string();
            if !n.is_empty() && !names.contains(&n) {
                names.push(n);
            }
        }
    }
    (!names.is_empty()).then(|| names.join(", "))
}

/// Ficha técnica do `jeuInfos.php` (doc da API v2 do ScreenScraper):
/// `developpeur`, `editeur`, `joueurs`, `note` (de 0 a 20), `classifications`
/// (um item por órgão — `type` + `text`) e `modes`.
fn parse_details(jeu: &Value) -> GameDetails {
    let get = |k: &str| jeu.get(k).and_then(field_text);
    let rating = get("note")
        .and_then(|n| n.replace(',', ".").parse::<f32>().ok())
        .filter(|n| (0.0..=20.0).contains(n))
        .map(|n| (n * 5.0).round() as u8);
    // PEGI primeiro (o público do app é BR/EU), depois ESRB, depois o 1º.
    let age_rating = jeu
        .get("classifications")
        .and_then(Value::as_array)
        .and_then(|arr| {
            let fmt = |x: &Value| {
                let org = x.get("type").and_then(Value::as_str)?.trim();
                let val = x.get("text").and_then(Value::as_str)?.trim();
                (!val.is_empty()).then(|| format!("{org} {val}").trim().to_string())
            };
            ["PEGI", "ESRB"]
                .iter()
                .find_map(|org| {
                    arr.iter()
                        .find(|x| x.get("type").and_then(Value::as_str) == Some(*org))
                        .and_then(fmt)
                })
                .or_else(|| arr.iter().find_map(fmt))
        });
    let media = |types: &[&str]| media_url(jeu, types);
    GameDetails {
        developer: get("developpeur"),
        publisher: get("editeur"),
        players: get("joueurs"),
        rating,
        age_rating,
        modes: jeu.get("modes").and_then(group_names),
        // Tipos de mídia do ScreenScraper (os mesmos do `mediaJeu.php` na doc
        // da API v2): `ss` = captura, `sstitle` = tela de título, `fanart`,
        // `wheel` = logo, `video-normalized` (tamanho padronizado) ou `video`.
        screenshot_url: media(&["ss"]),
        title_screen_url: media(&["sstitle"]),
        fanart_url: media(&["fanart"]),
        logo_url: media(&["wheel-hd", "wheel"]),
        video_url: media(&["video-normalized", "video"]),
        ..Default::default()
    }
}

/// URL da 1ª mídia de `jeu.medias` de um dos `types` (na ordem dada),
/// preferindo a região mundial/americana/europeia, como a capa.
fn media_url(jeu: &Value, types: &[&str]) -> Option<String> {
    let arr = jeu.get("medias")?.as_array()?;
    let region_rank = |x: &Value| {
        let r = x.get("region").and_then(Value::as_str).unwrap_or("");
        ["wor", "us", "eu", "ss", "br", "jp"]
            .iter()
            .position(|p| *p == r)
            .unwrap_or(99)
    };
    for t in types {
        let best = arr
            .iter()
            .filter(|x| x.get("type").and_then(Value::as_str) == Some(*t))
            .min_by_key(|x| region_rank(x));
        if let Some(u) = best.and_then(|x| x.get("url")).and_then(Value::as_str) {
            if !u.is_empty() {
                return Some(u.to_string());
            }
        }
    }
    None
}

/// A consulta de uma ROM da biblioteca: os hashes, o sistema, o nome do
/// arquivo e, pro disco sem hash, o serial, o nome e o tamanho do que o
/// identifica. `hash` e `stem` vêm de [`hash_and_stem`].
fn rom_query<'a>(rom: &'a Rom, hash: &'a RomHash, stem: &'a str) -> ScrapeQuery<'a> {
    ScrapeQuery {
        hash,
        system_id: &rom.system_id,
        file_stem: stem,
        serial: rom.serial.as_deref(),
        ident_name: rom.ident_name.as_deref(),
        ident_size: rom.ident_size,
    }
}

/// Os hashes da ROM e o nome do arquivo dela, sem a extensão.
fn hash_and_stem(rom: &Rom) -> (RomHash, String) {
    let stem = std::path::Path::new(&rom.file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let hash = RomHash {
        crc32: rom.crc32.clone(),
        md5: rom.md5.clone(),
    };
    (hash, stem)
}

/// Contêineres comprimidos: o tamanho do arquivo não é o de nada que o
/// ScreenScraper indexe (ele conhece a trilha ou a imagem descomprimida).
const COMPRESSED_CONTAINERS: &[&str] = &["chd", "cso"];

/// `name` é um `.chd`/`.cso` (pela extensão, sem diferença de caixa).
fn is_compressed_container(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| {
        COMPRESSED_CONTAINERS
            .iter()
            .any(|c| ext.eq_ignore_ascii_case(c))
    })
}

/// URL do `jeuInfos.php` pra `q`; `None` pra sistema que o ScreenScraper não
/// cobre (ex: 'disc'). Com hash, pergunta pelo CRC32 e pelo MD5. Sem hash
/// (CSO, CHD, ou ISO e imagem de disco acima de 1,5 GiB), pelo serial
/// (`serialnum`) e pelo nome com a extensão (`romnom`), que a API também
/// aceita (doc da API v2, `jeuInfos.php`), e pelo tamanho (`romtaille`) só
/// quando ele é o de algo que o ScreenScraper conhece: nunca o de um `.chd`
/// ou `.cso`, que é o do arquivo comprimido.
fn screenscraper_url(
    cfg: &MetadataConfig,
    q: &ScrapeQuery<'_>,
    base: &str,
) -> Option<reqwest::Url> {
    let sys = screenscraper_system_id(q.system_id)?;
    let mut url = reqwest::Url::parse(&format!("{base}/jeuInfos.php")).ok()?;
    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("output", "json")
            .append_pair("softname", "reemu")
            .append_pair("systemeid", &sys.to_string());
        if q.hash.crc32.is_empty() {
            if let Some(serial) = q.serial {
                qp.append_pair("serialnum", serial);
            }
            let name = q
                .ident_name
                .map_or_else(|| format!("{}.zip", q.file_stem), str::to_string);
            qp.append_pair("romnom", &name);
            if let Some(size) = q.ident_size.filter(|_| !is_compressed_container(&name)) {
                qp.append_pair("romtaille", &size.to_string());
            }
        } else {
            qp.append_pair("crc", &q.hash.crc32.to_uppercase())
                .append_pair("romnom", &format!("{}.zip", q.file_stem));
        }
        // MD5 junto: o ScreenScraper identifica por qualquer um dos hashes
        // (o ES-DE consulta por MD5) — ajuda quando o CRC não está catalogado.
        if !q.hash.md5.is_empty() {
            qp.append_pair("md5", &q.hash.md5.to_lowercase());
        }
        // segredo ausente no CI chega como string vazia
        if let (Some(id), Some(pw)) = (SS_DEVID, SS_DEVPASSWORD) {
            if !id.is_empty() {
                qp.append_pair("devid", id).append_pair("devpassword", pw);
            }
        }
        if let (Some(u), Some(p)) = (&cfg.screenscraper_user, &cfg.screenscraper_password) {
            if !u.is_empty() {
                qp.append_pair("ssid", u).append_pair("sspassword", p);
            }
        }
    }
    Some(url)
}

/// Uma consulta ao ScreenScraper. `Ok(None)` = não catalogado (404), ou
/// pedido sem hash recusado (400).
async fn query_screenscraper(
    client: &reqwest::Client,
    cfg: &MetadataConfig,
    q: &ScrapeQuery<'_>,
    base: &str,
) -> Result<Option<ScrapeCandidate>, String> {
    let Some(url) = screenscraper_url(cfg, q, base) else {
        return Ok(None); // sistema que o ScreenScraper não cobre (ex: 'disc')
    };

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("rede: {}", net_error(e)))?;
    let status = resp.status();
    let body = resp.text().await.map_err(net_error)?;

    if status.as_u16() == 404 || body.contains("Erreur : Rom/Iso/Dossier non trouv") {
        return Ok(None);
    }
    // Sem hash (CSO, CHD, disco acima de 1,5 GiB) o pedido vai só com serial, nome
    // e tamanho, e a doc da API pede um hash: o 400 que vem disso ("Il manque
    // des champs obligatoires dans l'url") conta como não achado. Assim o
    // TheGamesDB tenta pelo nome e a ROM não volta pra fila a cada leva.
    if status.as_u16() == 400 && q.hash.crc32.is_empty() {
        return Ok(None);
    }
    if status.as_u16() == 403 {
        // a doc da API: 403 = credenciais de desenvolvedor erradas/ausentes
        return Err(ERR_SS_DEV_LOGIN.to_string());
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
/// Hash vazio (disco sem hash) nunca bate.
fn rom_hash_matches(jeu: &Value, q: &ScrapeQuery<'_>) -> bool {
    let rom = jeu.get("rom");
    let field = |k: &str| rom.and_then(|r| r.get(k)).and_then(Value::as_str);
    let crc = !q.hash.crc32.is_empty()
        && field("romcrc").is_some_and(|c| c.eq_ignore_ascii_case(&q.hash.crc32));
    let md5 = !q.hash.md5.is_empty()
        && field("rommd5").is_some_and(|m| m.eq_ignore_ascii_case(&q.hash.md5));
    crc || md5
}

/// O que aconteceu com uma ROM depois de consultar os provedores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Correspondência exata: metadado aplicado.
    Auto,
    /// Correspondência incerta: foi pra revisão (Configurações › Metadados).
    Pending,
    /// Nenhum provedor achou o jogo.
    NoMatch,
}

/// Grava o resultado de uma consulta: aplica o metadado (exato), manda pra
/// revisão (incerto) ou marca "sem correspondência". Comum à leva
/// (`scrape_pending`) e ao "buscar de novo" de um jogo (`scrape_one`).
async fn record_result(
    repo: &db::MetadataRepo,
    rom_id: &str,
    stem: &str,
    candidate: Option<ScrapeCandidate>,
    covers: &dyn CoverCache,
) -> Result<Outcome, String> {
    let Some(c) = candidate else {
        let none = ScrapeCandidate {
            provider: "screenscraper".into(),
            external_id: String::new(),
            title: stem.to_string(),
            description: None,
            cover_url: None,
            release_date: None,
            genre: None,
            exact_hash_match: false,
            exact_filename_match: false,
            details: Default::default(),
        };
        let _ = repo.record_match(rom_id, &none, MatchStatus::NoMatch).await;
        return Ok(Outcome::NoMatch);
    };
    let auto = c.auto_matches();
    let status = if auto {
        MatchStatus::AutoMatched
    } else {
        MatchStatus::PendingReview
    };
    repo.record_match(rom_id, &c, status)
        .await
        .map_err(|e| e.to_string())?;
    if !auto {
        return Ok(Outcome::Pending);
    }
    let ok = repo
        .upsert_metadata(&GameMetadata {
            rom_id: rom_id.to_string(),
            title: c.title.clone(),
            description: c.description.clone(),
            cover_url: c.cover_url.clone(),
            release_date: c.release_date.clone(),
            genre: c.genre.clone(),
            provider_source: Some(c.provider.clone()),
            game_uuid: (c.provider == "gamekave" && !c.external_id.is_empty())
                .then(|| c.external_id.clone()),
            details: c.details.clone(),
        })
        .await
        .is_ok();
    if ok {
        covers.invalidate(rom_id);
    }
    Ok(Outcome::Auto)
}

/// "Buscar de novo" de UM jogo (gaveta de informações): consulta os
/// provedores na mesma cascata da leva — ScreenScraper pelo hash, depois o
/// TheGamesDB pelo nome se houver chave e GameKave por título e plataforma. Erro de
/// credencial do ScreenScraper volta como `ERR_SS_DEV_LOGIN` (a UI traduz).
/// `cfg`: a configuração de metadados já com as credenciais.
pub async fn scrape_one(
    pool: db::Db,
    cfg: MetadataConfig,
    covers: &dyn CoverCache,
    rom_id: &str,
) -> Result<Outcome, String> {
    scrape_one_with(pool, cfg, covers, rom_id, &Providers::live()).await
}

async fn scrape_one_with(
    pool: db::Db,
    cfg: MetadataConfig,
    covers: &dyn CoverCache,
    rom_id: &str,
    providers: &Providers,
) -> Result<Outcome, String> {
    let skip = db::RomResolutionRepo::new(pool.clone())
        .skip_own_scraper_ids()
        .await
        .map_err(|e| e.to_string())?;
    if skip.contains(rom_id) {
        return Ok(Outcome::NoMatch);
    }
    let rom = load_scrape_rom(&pool, rom_id).await?;
    let (stem, candidate) = lookup_rom(&rom, cfg, providers).await?;
    record_result(
        &db::MetadataRepo::new(pool),
        rom_id,
        &stem,
        candidate,
        covers,
    )
    .await
}

pub async fn preview_rom(
    pool: &db::Db,
    cfg: MetadataConfig,
    rom_id: &str,
) -> Result<Option<ScrapeCandidate>, String> {
    let rom = load_scrape_rom(pool, rom_id).await?;
    lookup_rom(&rom, cfg, &Providers::live())
        .await
        .map(|(_, candidate)| candidate)
}

async fn load_scrape_rom(pool: &db::Db, rom_id: &str) -> Result<domain::library::Rom, String> {
    db::RomsRepo::new(pool.clone())
        .get(rom_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "jogo não encontrado na biblioteca".into())
}

fn has_screenscraper_credentials(cfg: &MetadataConfig) -> bool {
    cfg.screenscraper_user
        .as_deref()
        .is_some_and(|u| !u.trim().is_empty())
        && cfg
            .screenscraper_password
            .as_deref()
            .is_some_and(|p| !p.trim().is_empty())
}

async fn lookup_rom(
    rom: &domain::library::Rom,
    cfg: MetadataConfig,
    providers: &Providers,
) -> Result<(String, Option<ScrapeCandidate>), String> {
    let client = reqwest::Client::builder()
        .user_agent("reemu/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(net_error)?;
    let (hash, stem) = hash_and_stem(rom);
    let q_ss = rom_query(rom, &hash, &stem);
    let effective_stem = rom_name::effective_rom_stem(rom, &stem);
    let q_name = rom_query(rom, &hash, &effective_stem);
    let tgdb_key = cfg.thegamesdb_api_key.as_deref().filter(|k| !k.is_empty());
    let mut result = if has_screenscraper_credentials(&cfg) {
        query_screenscraper(&client, &cfg, &q_ss, &providers.screenscraper).await
    } else {
        Ok(None)
    };
    let ss_refused = matches!(&result, Err(e) if e == ERR_SS_DEV_LOGIN);
    if let Some(key) = tgdb_key.filter(|_| ss_refused || matches!(&result, Ok(None))) {
        result = query_thegamesdb(&client, key, &q_name, &providers.thegamesdb).await;
    }
    let candidate = match result {
        Ok(Some(candidate)) => Some(candidate),
        fallback => match gamekave::query(&client, &q_name, &providers.gamekave).await {
            Ok(Some(candidate)) => Some(candidate),
            Ok(None) if ss_refused && matches!(fallback, Ok(None)) => {
                return Err(ERR_SS_DEV_LOGIN.to_string());
            }
            Ok(None) => fallback?,
            Err(error) => return Err(fallback.err().unwrap_or(error)),
        },
    };
    Ok((stem, candidate))
}

/// Roda uma leva de scraping sobre as ROMs sem match. Bloqueante (chamar de
/// `spawn_blocking`/task). `cfg`: a configuração de
/// metadados já com as credenciais. `covers`: descarta o cache de capa de
/// qualquer rom que ganhar uma `cover_url` nova aqui.
pub async fn scrape_pending(
    pool: db::Db,
    progress: Arc<ScrapeProgress>,
    cfg: MetadataConfig,
    covers: &dyn CoverCache,
) -> Result<(), String> {
    scrape_pending_with(pool, progress, cfg, covers, &Providers::live()).await
}

async fn scrape_pending_with(
    pool: db::Db,
    progress: Arc<ScrapeProgress>,
    cfg: MetadataConfig,
    covers: &dyn CoverCache,
    providers: &Providers,
) -> Result<(), String> {
    let repo = db::MetadataRepo::new(pool.clone());
    let roms_repo = db::RomsRepo::new(pool.clone());

    let skip = db::RomResolutionRepo::new(pool.clone())
        .skip_own_scraper_ids()
        .await
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = repo
        .rom_ids_without_match()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|id| !skip.contains(id))
        .collect();
    progress.start_processing(ids.len());
    log::info!("metadata: {} ROM(s) na fila de scraping", ids.len());

    let client = reqwest::Client::builder()
        .user_agent("reemu/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(net_error)?;

    // ScreenScraper só entra na cascata quando o usuário configurou conta
    // própria; se recusar as credenciais de desenvolvedor, para de perguntar
    // nas próximas ROMs e segue pelos provedores alternativos.
    let ss_configured = has_screenscraper_credentials(&cfg);
    let mut ss_ok = true;

    for rom_id in ids {
        if progress.is_cancelled() {
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
        let (hash, stem) = hash_and_stem(&rom);
        let q_ss = rom_query(&rom, &hash, &stem);
        let effective_stem = rom_name::effective_rom_stem(&rom, &stem);
        let q_name = rom_query(&rom, &hash, &effective_stem);

        // Cascata: ScreenScraper (hash) quando há credenciais; se não achar e
        // houver chave do TheGamesDB, tenta por nome lá. A GameKave fica por último.
        let tgdb_key = cfg.thegamesdb_api_key.as_deref().filter(|k| !k.is_empty());
        let mut result = if ss_configured && ss_ok {
            query_screenscraper(&client, &cfg, &q_ss, &providers.screenscraper).await
        } else {
            Ok(None)
        };
        if matches!(&result, Err(e) if e == ERR_SS_DEV_LOGIN) {
            log::warn!("metadata: ScreenScraper recusou as credenciais de desenvolvedor do ReEmu");
            ss_ok = false;
            progress.set_error(ERR_SS_DEV_LOGIN);
            result = Ok(None);
        }
        if let (Ok(None), Some(key)) = (&result, tgdb_key) {
            tokio::time::sleep(providers.request_delay).await;
            result = query_thegamesdb(&client, key, &q_name, &providers.thegamesdb).await;
            tokio::time::sleep(providers.tgdb_delay).await;
        }
        if !matches!(&result, Ok(Some(_))) && !progress.is_cancelled() {
            match gamekave::query(&client, &q_name, &providers.gamekave).await {
                Ok(Some(candidate)) => result = Ok(Some(candidate)),
                Ok(None) => {}
                Err(error) if result.is_ok() => result = Err(error),
                Err(_) => {}
            }
        }
        if progress.is_cancelled() {
            break;
        }
        match result {
            // A GameKave pode não cobrir a ROM. Deixa o jogo na fila se o
            // ScreenScraper esteve indisponível e não havia chave do TheGamesDB.
            Ok(None) if !ss_ok && tgdb_key.is_none() => {}
            Ok(c) => match record_result(&repo, &rom_id, &stem, c, covers).await {
                Ok(Outcome::Auto) => {
                    progress.auto.fetch_add(1, Ordering::Relaxed);
                }
                Ok(Outcome::Pending) => {
                    progress.pending.fetch_add(1, Ordering::Relaxed);
                }
                Ok(Outcome::NoMatch) => {}
                Err(e) => {
                    log::warn!("metadata: gravar o resultado de uma ROM: {e}");
                    progress.failed.fetch_add(1, Ordering::Relaxed);
                }
            },
            Err(e) => {
                log::warn!("metadata: consulta de uma ROM: {e}");
                progress.failed.fetch_add(1, Ordering::Relaxed);
                // erro de quota/rede: espera mais antes de continuar
                tokio::time::sleep(providers.error_pause).await;
            }
        }

        progress.done.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(providers.request_delay).await;
    }

    let (_, done, total, auto, pending, failed, _) = progress.snapshot();
    log::info!(
        "metadata: leva concluída — {done}/{total} ({auto} auto, {pending} p/ revisão, {failed} falha)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `system_id`s que o scan produz — lidos do próprio `systems.rs` (tabela
    /// canônica), pra sistema novo lá exigir conferência nos provedores.
    fn scan_system_ids() -> Vec<String> {
        let src = include_str!("../../library-scan/src/systems.rs");
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
    fn scan_transitions_include_empty_completion_and_cancellation() {
        let progress = ScrapeProgress::default();
        assert_eq!(progress.snapshot().0, ScrapeState::Idle);
        assert!(progress.begin());
        assert_eq!(progress.snapshot().0, ScrapeState::Preparing);
        assert!(!progress.begin());
        progress.start_processing(0);
        progress.finish(false);
        assert_eq!(progress.snapshot().0, ScrapeState::Completed);

        assert!(progress.begin());
        progress.cancel();
        progress.start_processing(2);
        assert_eq!(progress.snapshot().0, ScrapeState::Cancelling);
        progress.finish(false);
        assert_eq!(progress.snapshot().0, ScrapeState::Cancelled);

        assert!(progress.begin());
        progress.finish(true);
        assert_eq!(progress.snapshot().0, ScrapeState::Failed);
    }

    #[test]
    fn scanned_systems_have_ids_or_documented_provider_gaps() {
        let ids = scan_system_ids();
        assert!(
            ids.len() > 40,
            "o parse ainda acha a tabela ({})",
            ids.len()
        );
        let no_ss: Vec<_> = ids
            .iter()
            .filter(|i| screenscraper_system_id(i).is_none())
            .map(String::as_str)
            .collect();
        let no_tgdb: Vec<_> = ids
            .iter()
            .filter(|i| thegamesdb_platforms(i).is_none())
            .map(String::as_str)
            .collect();
        // Catálogos oficiais:
        // https://www.screenscraper.fr/systemesinfos.php
        // https://thegamesdb.net/list_platforms.php
        assert_eq!(no_ss, ["chip8", "gametank", "vaporspec"]);
        // O catálogo do TGDB tem Pioneer LaserActive, que é outra plataforma.
        assert_eq!(
            no_tgdb,
            [
                "arduboy",
                "chip8",
                "gametank",
                "laserdisc",
                "uzebox",
                "vaporspec"
            ]
        );
    }

    #[test]
    fn parse_details_reads_the_jeu_fields() {
        // Campos do `jeuInfos.php` (doc da API v2): texto direto ou
        // `{ "text": .. }`, nota de 0 a 20, classificações por órgão.
        let jeu = serde_json::json!({
            "developpeur": {"id": "3", "text": "Sonic Team"},
            "editeur": {"id": "1", "text": "Sega"},
            "joueurs": {"text": "1-2"},
            "note": {"text": "17"},
            "classifications": [
                {"type": "ESRB", "text": "E"},
                {"type": "PEGI", "text": "3"}
            ],
            "modes": [
                {"id": "1", "noms": [{"langue": "en", "text": "1 player"}]},
                {"id": "2", "noms": [{"langue": "en", "text": "Co-op"}]}
            ]
        });
        let d = parse_details(&jeu);
        assert_eq!(d.developer.as_deref(), Some("Sonic Team"));
        assert_eq!(d.publisher.as_deref(), Some("Sega"));
        assert_eq!(d.players.as_deref(), Some("1-2"));
        assert_eq!(d.rating, Some(85));
        assert_eq!(d.age_rating.as_deref(), Some("PEGI 3"));
        assert_eq!(d.modes.as_deref(), Some("1 player, Co-op"));
        // mídias: tipo pedido, região preferida (wor antes de jp)
        let jeu = serde_json::json!({"medias": [
            {"type": "ss", "region": "jp", "url": "https://x/ss-jp.png"},
            {"type": "ss", "region": "wor", "url": "https://x/ss-wor.png"},
            {"type": "wheel", "region": "wor", "url": "https://x/logo.png"},
            {"type": "video", "url": "https://x/v.mp4"},
            {"type": "video-normalized", "url": "https://x/vn.mp4"}
        ]});
        let m = parse_details(&jeu);
        assert_eq!(m.screenshot_url.as_deref(), Some("https://x/ss-wor.png"));
        assert_eq!(m.logo_url.as_deref(), Some("https://x/logo.png"));
        assert_eq!(m.video_url.as_deref(), Some("https://x/vn.mp4"));
        assert_eq!(m.title_screen_url, None);
        // nada disso no `jeu` → ficha vazia, sem erro
        assert_eq!(
            parse_details(&serde_json::json!({})),
            GameDetails::default()
        );
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

    /// Disco que a varredura guardou sem hash (aqui um ISO de DVD, acima de
    /// 1,5 GiB): o pedido ao ScreenScraper vai pelo serial, pelo nome com a
    /// extensão e pelo tamanho, sem `crc`/`md5` vazios, e um hash vazio na
    /// resposta não conta como correspondência exata.
    #[test]
    fn disc_without_hashes_is_asked_by_serial_name_and_size() {
        let params = |rom: &Rom| -> std::collections::HashMap<String, String> {
            let (hash, stem) = hash_and_stem(rom);
            let q = rom_query(rom, &hash, &stem);
            screenscraper_url(&MetadataConfig::default(), &q, "https://ss.invalid/api2")
                .expect("sistema coberto")
                .query_pairs()
                .into_owned()
                .collect()
        };
        let get = |p: &std::collections::HashMap<String, String>, k: &str| p.get(k).cloned();
        let disc = Rom {
            id: "r".into(),
            file_path: "/biblioteca/ps2/Jogo (USA).iso".into(),
            system_id: "ps2".into(),
            serial: Some("SLUS-20062".into()),
            ident_name: Some("Jogo (USA).iso".into()),
            ident_size: Some(4_380_000_000),
            ..Default::default()
        };
        let p = params(&disc);
        assert_eq!(get(&p, "serialnum").as_deref(), Some("SLUS-20062"));
        assert_eq!(get(&p, "romnom").as_deref(), Some("Jogo (USA).iso"));
        assert_eq!(get(&p, "romtaille").as_deref(), Some("4380000000"));
        assert!(!p.contains_key("crc") && !p.contains_key("md5"), "{p:?}");

        // Serial que não se leu: o nome e o tamanho.
        let p = params(&Rom {
            serial: None,
            ..disc.clone()
        });
        assert!(!p.contains_key("serialnum"), "{p:?}");
        assert_eq!(get(&p, "romtaille").as_deref(), Some("4380000000"));

        // Controle: cartucho vai pelos hashes, como antes.
        let cart = Rom {
            file_path: "/biblioteca/snes/Jogo (USA).sfc".into(),
            system_id: "snes".into(),
            crc32: "b19ed489".into(),
            md5: "CDD3C8C37322978CA8669B34BC89C804".into(),
            ident_name: Some("Jogo (USA).sfc".into()),
            ident_size: Some(1024),
            ..Default::default()
        };
        let p = params(&cart);
        assert_eq!(get(&p, "crc").as_deref(), Some("B19ED489"));
        assert_eq!(
            get(&p, "md5").as_deref(),
            Some("cdd3c8c37322978ca8669b34bc89c804")
        );
        assert_eq!(get(&p, "romnom").as_deref(), Some("Jogo (USA).zip"));
        assert!(!p.contains_key("serialnum") && !p.contains_key("romtaille"));

        let (hash, stem) = hash_and_stem(&disc);
        let q = rom_query(&disc, &hash, &stem);
        let empty = json!({"rom": {"romcrc": "", "rommd5": ""}});
        assert!(!rom_hash_matches(&empty, &q), "hash vazio não é exato");
    }

    /// `.chd` e `.cso` sem hash vão pelo serial e pelo nome, sem `romtaille`
    /// (o tamanho é o do arquivo comprimido, que não casa com nada). Trilha
    /// `.bin` e `.iso` grandes continuam mandando o tamanho.
    #[test]
    fn compressed_container_sends_no_size() {
        let ask = |name: &str, serial: Option<&str>| {
            let rom = Rom {
                id: "r".into(),
                file_path: format!("/biblioteca/psx/{name}"),
                system_id: "psx".into(),
                serial: serial.map(str::to_string),
                ident_name: Some(name.to_string()),
                ident_size: Some(2_000_000_000),
                ..Default::default()
            };
            let (hash, stem) = hash_and_stem(&rom);
            let q = rom_query(&rom, &hash, &stem);
            let url = screenscraper_url(&MetadataConfig::default(), &q, "https://ss.invalid/api2")
                .expect("sistema coberto");
            url.query_pairs()
                .into_owned()
                .collect::<std::collections::HashMap<String, String>>()
        };
        for name in ["Jogo (USA).chd", "Jogo (USA).cso", "JOGO (USA).CHD"] {
            let p = ask(name, Some("SCUS-94900"));
            assert!(!p.contains_key("romtaille"), "{name}: {p:?}");
            assert_eq!(p.get("romnom").map(String::as_str), Some(name));
            assert_eq!(p.get("serialnum").map(String::as_str), Some("SCUS-94900"));
            // Sem serial lido: só o nome.
            let p = ask(name, None);
            assert!(!p.contains_key("romtaille") && !p.contains_key("serialnum"));
        }
        for name in ["Jogo (USA) (Track 1).bin", "Jogo (USA).iso"] {
            let p = ask(name, Some("SCUS-94900"));
            assert_eq!(
                p.get("romtaille").map(String::as_str),
                Some("2000000000"),
                "{name}"
            );
        }
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
            serial: None,
            ident_name: None,
            ident_size: None,
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

    /// Servidor HTTP local mínimo: responde cada pedido com `answer(alvo)`
    /// (caminho com a query) e guarda os alvos na ordem.
    fn mock(answer: fn(&str) -> (u16, String)) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                let head = String::from_utf8_lossy(&head);
                let target = head
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                let (status, body) = answer(&target);
                log.lock().unwrap().push(target);
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (base, seen)
    }

    fn local(base: &str) -> Providers {
        Providers {
            screenscraper: format!("{base}/api2"),
            thegamesdb: format!("{base}/v1"),
            gamekave: format!("{base}/games/search"),
            request_delay: std::time::Duration::ZERO,
            tgdb_delay: std::time::Duration::ZERO,
            error_pause: std::time::Duration::ZERO,
        }
    }

    struct NoCovers;
    impl CoverCache for NoCovers {
        fn invalidate(&self, _: &str) {}
    }

    /// Disco sem hash, como a varredura grava um `.chd`.
    fn hashless_disc() -> Rom {
        Rom {
            id: "disco".into(),
            file_path: "/biblioteca/psx/Crash Bandicoot (USA).chd".into(),
            system_id: "psx".into(),
            serial: Some("SCUS-94900".into()),
            ident_name: Some("Crash Bandicoot (USA).chd".into()),
            ident_size: Some(541_249_456),
            ..Default::default()
        }
    }

    /// O ScreenScraper recusa com 400 o pedido sem hash: conta como não
    /// achado, o TheGamesDB tenta pelo nome, a correspondência é gravada e a
    /// leva seguinte não pergunta de novo. Um cartucho (com hash) recusado
    /// com 400 continua sendo erro e volta na leva seguinte.
    #[tokio::test]
    async fn hashless_disc_refused_by_screenscraper_falls_to_thegamesdb_once() {
        let (base, seen) = mock(|target| {
            if target.starts_with("/api2/jeuInfos.php") {
                (
                    400,
                    "Erreur : Il manque des champs obligatoires dans l'url".into(),
                )
            } else if target.starts_with("/v1/Games/ByGameName") {
                let body = json!({
                    "data": {"games": [{"id": 77, "game_title": "Crash Bandicoot"}]},
                    "include": {"boxart": {
                        "base_url": {"large": "https://cdn.invalid/"},
                        "data": {"77": [{"side": "front", "filename": "c.jpg"}]}
                    }}
                });
                (200, body.to_string())
            } else if target.starts_with("/games/search") {
                // A GameKave (último da cascata) não acha nada.
                (200, r#"{"games": []}"#.into())
            } else {
                (404, String::new())
            }
        });
        let pool = db::connect_in_memory().await.unwrap();
        let roms = db::RomsRepo::new(pool.clone());
        roms.add(&hashless_disc()).await.unwrap();
        roms.add(&Rom {
            id: "cartucho".into(),
            file_path: "/biblioteca/snes/Jogo (USA).sfc".into(),
            system_id: "snes".into(),
            crc32: "B19ED489".into(),
            md5: "cdd3c8c37322978ca8669b34bc89c804".into(),
            ..Default::default()
        })
        .await
        .unwrap();
        let cfg = MetadataConfig {
            screenscraper_user: Some("usuario".into()),
            screenscraper_password: Some("senha".into()),
            thegamesdb_api_key: Some("chave".into()),
            ..Default::default()
        };
        let providers = local(&base);

        let mut failed = Vec::new();
        for _ in 0..2 {
            let progress = Arc::new(ScrapeProgress::default());
            scrape_pending_with(
                pool.clone(),
                progress.clone(),
                cfg.clone(),
                &NoCovers,
                &providers,
            )
            .await
            .unwrap();
            failed.push(progress.failed.load(Ordering::Relaxed));
        }

        let seen = seen.lock().unwrap().clone();
        let asked = |path: &str, needle: &str| {
            seen.iter()
                .filter(|t| t.starts_with(path) && t.contains(needle))
                .count()
        };
        assert_eq!(
            asked("/api2/jeuInfos.php", "serialnum=SCUS-94900"),
            1,
            "{seen:?}"
        );
        assert_eq!(
            asked("/v1/Games/ByGameName", "Crash+Bandicoot"),
            1,
            "{seen:?}"
        );
        assert_eq!(asked("/api2/jeuInfos.php", "crc=B19ED489"), 2, "{seen:?}");
        assert_eq!(failed, [1, 1], "só o cartucho falha, nas duas levas");
        let pending = db::MetadataRepo::new(pool.clone())
            .list_pending()
            .await
            .unwrap();
        let pending: Vec<_> = pending
            .iter()
            .map(|p| (p.rom_id.as_str(), p.provider.as_str()))
            .collect();
        assert_eq!(pending, [("disco", "thegamesdb")]);

        // "Buscar de novo" do disco: a mesma cascata, sem erro pra interface.
        let one = scrape_one_with(pool, cfg, &NoCovers, "disco", &providers).await;
        assert_eq!(one, Ok(Outcome::Pending));
    }

    /// Pedido que falha na rede: o texto do erro (que a leva grava no log e
    /// o "buscar de novo" mostra na interface) não leva a URL, nem a senha da
    /// pessoa, nem a de desenvolvedor, nem a chave do TheGamesDB.
    #[tokio::test]
    async fn failed_request_error_has_no_url_nor_password() {
        // Porta fechada: a conexão é recusada.
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let providers = local(&format!("http://127.0.0.1:{port}"));
        let pool = db::connect_in_memory().await.unwrap();
        let disc = hashless_disc();
        db::RomsRepo::new(pool.clone()).add(&disc).await.unwrap();
        let cfg = MetadataConfig {
            screenscraper_user: Some("pessoa-login".into()),
            screenscraper_password: Some("senha-da-pessoa".into()),
            thegamesdb_api_key: Some("chave-tgdb".into()),
            ..Default::default()
        };

        let ui = scrape_one_with(pool, cfg.clone(), &NoCovers, "disco", &providers)
            .await
            .unwrap_err();
        let client = reqwest::Client::new();
        let (hash, stem) = hash_and_stem(&disc);
        let q = rom_query(&disc, &hash, &stem);
        let ss = query_screenscraper(&client, &cfg, &q_ss, &providers.screenscraper)
            .await
            .unwrap_err();
        let tgdb = query_thegamesdb(&client, "chave-tgdb", &q, &providers.thegamesdb)
            .await
            .unwrap_err();

        let mut secrets = vec![
            "senha-da-pessoa",
            "pessoa-login",
            "chave-tgdb",
            "sspassword",
            "apikey",
            "SCUS-94900",
            "Crash",
            "127.0.0.1",
            "jeuInfos",
            "Games/",
        ];
        if let Some(pw) = SS_DEVPASSWORD.filter(|p| !p.is_empty()) {
            secrets.push(pw);
        }
        for text in [&ui, &ss, &tgdb] {
            assert!(text.starts_with("rede: "), "{text}");
            for secret in &secrets {
                assert!(!text.contains(secret), "{secret} em {text:?}");
            }
        }
    }

    /// Servidor que promete 100 bytes de corpo, manda 10 e fecha: a falha
    /// vem na leitura do corpo, não no envio.
    fn truncated_mock() -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&buf[..n]),
                    }
                }
                let _ = s.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n0123456789",
                );
            }
        });
        base
    }

    /// Corpo que não chega inteiro: o erro de leitura do corpo, nos dois
    /// provedores, também sai sem a URL e sem as senhas. No reqwest 0.12 esse
    /// erro já vem sem a URL (o do envio é que traz); o teste prende o
    /// comportamento pra uma troca de versão.
    #[tokio::test]
    async fn body_error_has_no_url_nor_password() {
        let providers = local(&truncated_mock());
        let disc = hashless_disc();
        let cfg = MetadataConfig {
            screenscraper_user: Some("pessoa-login".into()),
            screenscraper_password: Some("senha-da-pessoa".into()),
            ..Default::default()
        };
        let client = reqwest::Client::new();
        let (hash, stem) = hash_and_stem(&disc);
        let q = rom_query(&disc, &hash, &stem);
        let ss = query_screenscraper(&client, &cfg, &q_ss, &providers.screenscraper)
            .await
            .unwrap_err();
        let tgdb = query_thegamesdb(&client, "chave-tgdb", &q, &providers.thegamesdb)
            .await
            .unwrap_err();
        for text in [&ss, &tgdb] {
            for secret in [
                "senha-da-pessoa",
                "pessoa-login",
                "chave-tgdb",
                "sspassword",
                "apikey",
                "SCUS-94900",
                "Crash",
                "127.0.0.1",
            ] {
                assert!(!text.contains(secret), "{secret} em {text:?}");
            }
        }
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

    // Formato do exemplo do `/v1/Games/ByGameName` no spec.yaml oficial
    // (api.thegamesdb.net/spec.yaml), com `fields` e `include=boxart`.
    #[test]
    fn tgdb_fields_and_included_boxart() {
        let v = json!({
            "data": {"games": [{"id": 53, "game_title": "Sonic the Hedgehog",
                "release_date": "1991-06-23", "platform": 18, "players": 1,
                "rating": "E - Everyone", "coop": "No"}]},
            "include": {"boxart": {
                "base_url": {"large": "https://cdn.thegamesdb.net/images/large/"},
                "data": {"53": [
                    {"id": 1, "type": "boxart", "side": "back", "filename": "boxart/back/53-1.jpg"},
                    {"id": 2, "type": "boxart", "side": "front", "filename": "boxart/front/53-1.jpg"}
                ]}
            }}
        });
        let c = parse_tgdb_game(&v).expect("candidato");
        assert_eq!(c.details.players.as_deref(), Some("1"));
        assert_eq!(c.details.age_rating.as_deref(), Some("E - Everyone"));
        assert_eq!(c.details.modes, None, "coop No não vira modo");
        assert_eq!(
            parse_tgdb_included_boxart(&v, "53").as_deref(),
            Some("https://cdn.thegamesdb.net/images/large/boxart/front/53-1.jpg")
        );
        assert_eq!(parse_tgdb_included_boxart(&v, "99"), None);
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

    #[test]
    fn tgdb_prefers_canonical_title_over_first_array_entry() {
        let v = json!({"data": {"games": [
            {"id": 1, "game_title": "Castlevania: Symphony of the Night - Demo"},
            {"id": 2, "game_title": "Castlevania: Symphony of the Night"}
        ]}});
        let c = parse_tgdb_game_for(&v, Some("Castlevania - Symphony of the Night (USA)"))
            .expect("candidato canônico");
        assert_eq!(c.external_id, "2");
        assert_eq!(c.title, "Castlevania: Symphony of the Night");
    }

    #[tokio::test]
    async fn own_scraper_skips_pending_and_found() {
        use domain::metadata::{ResolutionState, RomResolution};

        let (base, seen) = mock(|target| {
            if target.starts_with("/api2/jeuInfos.php") {
                (404, String::new())
            } else if target.starts_with("/games/search") {
                (200, r#"{"games": []}"#.into())
            } else {
                (404, String::new())
            }
        });
        let pool = db::connect_in_memory().await.unwrap();
        let roms = db::RomsRepo::new(pool.clone());
        let res_repo = db::RomResolutionRepo::new(pool.clone());

        for (id, name, state) in [
            ("r_found", "FoundGame", ResolutionState::Found),
            ("r_pending", "PendingGame", ResolutionState::Pending),
            ("r_manual", "ManualGame", ResolutionState::Manual),
            ("r_unknown", "UnknownGame", ResolutionState::Unknown),
        ] {
            roms.add(&Rom {
                id: id.into(),
                file_path: format!("/biblioteca/nes/{name}.nes"),
                system_id: "nes".into(),
                crc32: "11223344".into(),
                md5: "0123456789abcdef0123456789abcdef".into(),
                ..Default::default()
            })
            .await
            .unwrap();
            res_repo
                .put(&RomResolution {
                    rom_id: id.into(),
                    state,
                    game_uuid: None,
                    matched_by: None,
                    reason: None,
                    retry_after: Some(9_999_999),
                    media_pending: false,
                    answer_json: None,
                    resolved_at: 100,
                })
                .await
                .unwrap();
        }

        let providers = local(&base);
        let progress = Arc::new(ScrapeProgress::default());
        scrape_pending_with(
            pool.clone(),
            progress,
            MetadataConfig::default(),
            &NoCovers,
            &providers,
        )
        .await
        .unwrap();

        let targets = seen.lock().unwrap().clone();
        assert!(
            targets.iter().any(|t| t.contains("UnknownGame")),
            "unknown deveria consultar os provedores locais: {targets:?}"
        );
        for skipped in ["FoundGame", "PendingGame", "ManualGame"] {
            assert!(
                !targets.iter().any(|t| t.contains(skipped)),
                "{skipped} não deveria consultar os provedores locais: {targets:?}"
            );
        }
    }

    #[tokio::test]
    async fn without_screenscraper_credentials_skips_screenscraper_and_sets_no_warning() {
        let (base, seen) = mock(|target| {
            if target.starts_with("/api2/jeuInfos.php") {
                (
                    403,
                    "Erreur de login : Vérifier vos identifiants développeur !".into(),
                )
            } else if target.starts_with("/games/search") {
                (200, r#"{"games": []}"#.into())
            } else {
                (404, String::new())
            }
        });
        let pool = db::connect_in_memory().await.unwrap();
        db::RomsRepo::new(pool.clone())
            .add(&Rom {
                id: "r1".into(),
                file_path: "/biblioteca/snes/Super Mario World.sfc".into(),
                system_id: "snes".into(),
                crc32: "B19ED489".into(),
                md5: "cdd3c8c37322978ca8669b34bc89c804".into(),
                ..Default::default()
            })
            .await
            .unwrap();

        let providers = local(&base);
        let progress = Arc::new(ScrapeProgress::default());
        scrape_pending_with(
            pool.clone(),
            progress.clone(),
            MetadataConfig::default(),
            &NoCovers,
            &providers,
        )
        .await
        .unwrap();

        let targets = seen.lock().unwrap().clone();
        assert!(
            !targets.iter().any(|t| t.starts_with("/api2/jeuInfos.php")),
            "sem credenciais do ScreenScraper não deve consultar a API: {targets:?}"
        );
        assert_eq!(progress.snapshot().6, None);

        let one =
            scrape_one_with(pool, MetadataConfig::default(), &NoCovers, "r1", &providers).await;
        assert_eq!(one, Ok(Outcome::NoMatch));
    }
}
