use domain::metadata::{GameDetails, GameKaveData, ScrapeCandidate, ScrapeQuery};
use serde_json::Value;

use super::rom_name;

pub(super) const SEARCH_URL: &str = "https://api.gamekave.net/games/search";

/// Slugs do catálogo GameKave (/games/filters/platforms).
fn platforms(system: &str) -> &[&str] {
    match system {
        "nes" => &["nintendo-entertainment-system", "family-computer"],
        "snes" => &["super-nintendo-entertainment-system", "super-famicom"],
        "n64" => &["nintendo-64"],
        "gamecube" => &["nintendo-gamecube"],
        "gb" => &["game-boy"],
        "gbc" => &["game-boy-color"],
        "gba" => &["game-boy-advance"],
        "vb" => &["virtual-boy"],
        "nds" => &["nintendo-ds", "nintendo-dsi"],
        "pokemini" => &["pokemon-mini"],
        "megadrive" => &["sega-mega-drive-genesis"],
        "mastersystem" => &["sega-master-system-mark-iii"],
        "gamegear" => &["sega-game-gear"],
        "sega32x" => &["sega-32x"],
        "segacd" => &["sega-cd"],
        "saturn" => &["sega-saturn"],
        "dreamcast" => &["dreamcast"],
        "psx" => &["playstation"],
        "ps2" => &["playstation-2"],
        "psp" => &["playstation-portable", "playstation-minis"],
        "pcengine" => &["turbografx-16-pc-engine"],
        "pcenginecd" => &["turbografx-16-pc-engine-cd", "pc-engine-cd-rom"],
        "supergrafx" => &["pc-engine-supergrafx"],
        "pcfx" => &["pc-fx"],
        "atari2600" => &["atari-2600"],
        "atari5200" => &["atari-5200"],
        "atari7800" => &["atari-7800"],
        "atari8bit" => &["atari-8-bit"],
        "atarist" => &["atari-st-ste"],
        "jaguar" => &["atari-jaguar"],
        "lynx" => &["atari-lynx"],
        "wonderswan" => &["wonderswan", "wonderswan-color"],
        "ngp" => &["neo-geo-pocket", "neo-geo-pocket-color"],
        "neogeocd" => &["neo-geo-cd"],
        "sg1000" => &["sg-1000"],
        "naomi" => &["naomi", "naomi-gd-rom"],
        "atomiswave" => &["atomiswave"],
        "coleco" => &["colecovision"],
        "intellivision" => &["intellivision"],
        "vectrex" => &["vectrex"],
        "odyssey2" => &["odyssey-2-videopac-g7000"],
        "supervision" => &["watara-supervision", "watara-quickshot-supervision"],
        "megaduck" => &["mega-duck-cougar-boy"],
        "3do" => &["3do-interactive-multiplayer"],
        "cdi" => &["philips-cd-i"],
        "msx" => &["msx", "msx2", "msx-turbo-r"],
        "c64" => &["commodore-c64-128-max"],
        "amiga" => &["amiga", "amiga-aga"],
        "zxspectrum" => &["zx-spectrum"],
        "zx81" => &["sinclair-zx81"],
        "x68000" => &["sharp-x68000"],
        "amstradcpc" => &["amstrad-cpc"],
        "arcade" => &["arcade"],
        "dos" => &["dos"],
        "scummvm" => &["dos", "pc-microsoft-windows"],
        _ => &[],
    }
}

/// `search_url`: o [`SEARCH_URL`] no app; um servidor local nos testes.
pub(super) async fn query(
    client: &reqwest::Client,
    query: &ScrapeQuery<'_>,
    search_url: &str,
) -> Result<Option<ScrapeCandidate>, String> {
    let platforms = platforms(query.system_id);
    let queries = rom_name::search_queries(query.file_stem);
    if platforms.is_empty() || queries.is_empty() {
        return Ok(None);
    }
    let clean_title = rom_name::clean_rom_title(query.file_stem);
    let serial = query
        .serial
        .and_then(rom_name::normalize_serial)
        .or_else(|| rom_name::extract_disc_serial(query.file_stem));

    let mut matched = None;
    for q_str in &queries {
        let mut req = client
            .get(search_url)
            .query(&[("q", q_str.as_str()), ("limit", "40")]);
        if platforms.len() == 1 {
            req = req.query(&[("platform", platforms[0])]);
        }
        let response = req
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| format!("GameKave: {}", e.without_url()))?;
        let games: Value = response
            .json()
            .await
            .map_err(|e| format!("GameKave: {}", e.without_url()))?;
        let games = games["games"]
            .as_array()
            .ok_or("GameKave: resposta sem lista de jogos")?;
        if let Some(candidate) = select_candidate_for_query(
            games,
            if clean_title.is_empty() {
                q_str
            } else {
                &clean_title
            },
            serial.as_deref(),
            platforms,
        ) {
            matched = Some(candidate);
            break;
        }
    }

    let Some(mut candidate) = matched else {
        return Ok(None);
    };
    let rom_region = rom_name::detect_rom_region(
        query.ident_name.unwrap_or(query.file_stem),
        None,
        query.serial,
    );
    if let Some(gk) = candidate.details.gamekave.as_mut() {
        gk.rom_region = rom_region.map(str::to_owned);
    }
    // A ficha da busca continua utilizável se a consulta extra falhar.
    if let Some(assets) = fetch_assets(client, search_url, &candidate.external_id).await {
        let details = candidate
            .details
            .gamekave
            .get_or_insert_with(Default::default);
        for (region, val) in assets["regional_covers"].as_object().into_iter().flatten() {
            if let Some(url) = cover_media(val).filter(|u| !rom_name::is_non_front_cover_url(u)) {
                let key = rom_name::normalize_region_code(region)
                    .map(str::to_owned)
                    .unwrap_or_else(|| region.trim().to_ascii_uppercase());
                if !key.is_empty() {
                    details.regional_covers.insert(key, url);
                }
            }
        }
        let default_asset_cover = cover_media(&assets["covers"]["default"]["url"]);
        let mut ranked_covers: Vec<((u8, u8, u8), usize, String)> =
            entries(&assets["covers"]["list"])
                .enumerate()
                .filter_map(|(idx, item)| {
                    if item["hidden"].as_bool() == Some(true) {
                        return None;
                    }
                    let url = cover_media(&item["url"])?;
                    let is_def = item["is_default"].as_bool() == Some(true)
                        || default_asset_cover.as_deref() == Some(url.as_str());
                    let dims = match (count(&item["width"]), count(&item["height"])) {
                        (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
                        _ => None,
                    };
                    let rank = rom_name::cover_url_rank(&url, Some(query.system_id), is_def, dims);
                    Some((rank, idx, url))
                })
                .collect();
        ranked_covers.sort_by_key(|(rank, idx, _)| (*rank, *idx));
        let mut merged_covers = Vec::new();
        append_urls(
            &mut merged_covers,
            ranked_covers
                .into_iter()
                .map(|(_, _, url)| url)
                .chain(details.covers.drain(..)),
        );
        merged_covers.sort_by_key(|url| {
            let is_def = default_asset_cover.as_deref() == Some(url.as_str());
            rom_name::cover_url_rank(url, Some(query.system_id), is_def, None)
        });
        details.covers = merged_covers;

        for (list, kind) in [
            (&mut details.heroes, "heroes"),
            (&mut details.screenshots, "screenshots"),
            (&mut details.trailers, "trailers"),
        ] {
            append_urls(
                list,
                entries(&assets[kind]["list"]).filter_map(|item| {
                    if item["hidden"].as_bool() == Some(true) {
                        return None;
                    }
                    media(&item["url"])
                }),
            );
        }
        details.achievement_count = count(&assets["achievements"]["count"]);
        details.soundtrack_count = count(&assets["soundtrack_files"]["count"]);
        let best_cover_pool: Vec<&str> = candidate
            .cover_url
            .as_deref()
            .into_iter()
            .chain(default_asset_cover.as_deref())
            .chain(details.covers.iter().map(String::as_str))
            .collect();
        let fallback_cover = rom_name::pick_best_cover_url(
            best_cover_pool,
            Some(query.system_id),
            default_asset_cover
                .as_deref()
                .or(candidate.cover_url.as_deref()),
        )
        .map(str::to_owned);
        candidate.cover_url = rom_name::pick_regional_cover(
            &details.regional_covers,
            rom_region,
            fallback_cover.as_deref(),
        )
        .map(str::to_owned);
        candidate.details.fanart_url = candidate
            .details
            .fanart_url
            .or_else(|| details.heroes.first().cloned());
        candidate.details.screenshot_url = candidate
            .details
            .screenshot_url
            .or_else(|| details.screenshots.first().cloned());
        append_urls(
            &mut details.trailers,
            entries(&assets["videos"]["list"]).filter_map(|item| media(&item["url"])),
        );
        candidate.details.video_url = candidate
            .details
            .video_url
            .or_else(|| details.trailers.first().cloned());
    } else if let Some(details) = candidate.details.gamekave.as_ref() {
        let fallback_cover = candidate.cover_url.clone();
        candidate.cover_url = rom_name::pick_regional_cover(
            &details.regional_covers,
            rom_region,
            fallback_cover.as_deref(),
        )
        .map(str::to_owned);
    }
    Ok(Some(candidate))
}

fn text(value: &Value) -> Option<&str> {
    value.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn media(value: &Value) -> Option<String> {
    let url = reqwest::Url::parse(text(value)?).ok()?;
    (url.scheme() == "https" && url.host_str() == Some("cdn.gamekave.net")).then(|| url.to_string())
}

fn cover_media(value: &Value) -> Option<String> {
    let url = reqwest::Url::parse(text(value)?).ok()?;
    let host_ok = matches!(url.host_str(), Some("cdn.gamekave.net" | "images.igdb.com"));
    (url.scheme() == "https" && host_ok).then(|| url.to_string())
}

async fn fetch_assets(client: &reqwest::Client, search_url: &str, game_id: &str) -> Option<Value> {
    let mut url = reqwest::Url::parse(search_url).ok()?;
    url.path_segments_mut()
        .ok()?
        .pop()
        .push(game_id)
        .push("assets");
    client
        .get(url)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()
}

fn entries(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn count(value: &Value) -> Option<u32> {
    u32::try_from(value.as_u64()?).ok()
}

fn append_urls(target: &mut Vec<String>, urls: impl Iterator<Item = String>) {
    for url in urls {
        if target.len() >= 24 {
            break;
        }
        if !target.contains(&url) {
            target.push(url);
        }
    }
}

fn catalog_media<'a>(metadata: &'a Value, kind: &'a str) -> impl Iterator<Item = String> + 'a {
    entries(&metadata["medias"])
        .filter(move |item| text(&item["type"]) == Some(kind))
        .filter_map(|item| media(&item["url"]))
}

fn catalog_details(game: &Value) -> GameKaveData {
    let metadata = &game["metadata"];
    let mut details = GameKaveData::default();
    for (lang, value) in metadata["descriptions"].as_object().into_iter().flatten() {
        if let Some(description) = text(value) {
            details
                .descriptions
                .insert(lang.to_lowercase(), description.into());
        }
    }
    for item in entries(&metadata["synopsis"]) {
        if let (Some(lang), Some(description)) = (text(&item["langue"]), text(&item["text"])) {
            details
                .descriptions
                .entry(lang.to_lowercase())
                .or_insert_with(|| description.into());
        }
    }
    for item in entries(&metadata["dates"]) {
        if let (Some(region), Some(date)) = (text(&item["region"]), text(&item["text"])) {
            details
                .release_dates
                .entry(region.to_uppercase())
                .or_insert_with(|| date.into());
        }
    }
    details.time_to_beat_minutes = ["hastily_seconds", "normally_seconds", "completely_seconds"]
        .map(|key| {
            count(&game["time_to_beat"][key])
                .filter(|&seconds| seconds > 0)
                .map(|seconds| seconds.div_ceil(60))
        });
    append_urls(
        &mut details.covers,
        [media(&game["cover_url"]), media(&metadata["cover_url"])]
            .into_iter()
            .flatten(),
    );
    append_urls(
        &mut details.heroes,
        media(&metadata["hero_url"]).into_iter(),
    );
    append_urls(
        &mut details.screenshots,
        entries(&metadata["screenshots"]).filter_map(media),
    );
    append_urls(&mut details.covers, catalog_media(metadata, "box-2D"));
    for item in entries(&metadata["medias"]) {
        let is_box = text(&item["type"]).is_some_and(|t| t.eq_ignore_ascii_case("box-2D"));
        if !is_box {
            continue;
        }
        if let (Some(reg), Some(url)) = (
            text(&item["region"]).and_then(rom_name::normalize_region_code),
            cover_media(&item["url"]),
        ) {
            details.regional_covers.entry(reg.to_owned()).or_insert(url);
        }
    }
    append_urls(&mut details.heroes, catalog_media(metadata, "fanart"));
    append_urls(&mut details.screenshots, catalog_media(metadata, "ss"));
    append_urls(
        &mut details.trailers,
        catalog_media(metadata, "video-normalized"),
    );
    details
}

fn named_text(value: &Value) -> Option<&str> {
    text(value).or_else(|| text(&value["text"]))
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| text(value)?.parse().ok())
}

fn has_matching_platform(game: &Value, platforms: &[&str]) -> bool {
    game["platforms"].as_array().is_some_and(|entries| {
        entries.iter().any(|entry| {
            text(&entry["slug"])
                .or_else(|| text(entry))
                .is_some_and(|slug| platforms.contains(&slug))
        })
    })
}

fn is_official_game(game: &Value) -> bool {
    let is_addon = game["game_type"]["is_addon"].as_bool() == Some(true)
        || game["type_is_addon"].as_bool() == Some(true);
    let counts_as_game = game["game_type"]["counts_as_game"]
        .as_bool()
        .or_else(|| game["type_counts_as_game"].as_bool())
        .unwrap_or(true);
    !is_addon && counts_as_game
}

fn alternate_titles(game: &Value) -> Vec<&str> {
    let metadata = &game["metadata"];
    let mut out = Vec::new();
    if let Some(igdb_name) = text(&metadata["igdb_name"]) {
        out.push(igdb_name);
    }
    for item in entries(&metadata["legacy_titles"]) {
        if let Some(t) = text(item) {
            out.push(t);
        }
    }
    for item in entries(&metadata["alternative_names"]) {
        if let Some(t) = text(&item["name"]).or_else(|| text(item)) {
            out.push(t);
        }
    }
    for item in entries(&metadata["noms"]) {
        if let Some(t) = text(&item["text"]) {
            out.push(t);
        }
    }
    out
}

fn match_tier(game: &Value, rom_title: &str, serial: Option<&str>) -> Option<u8> {
    if let Some(target_serial) = serial {
        let serial_hit = entries(&game["psn_serials"])
            .filter_map(text)
            .filter_map(rom_name::normalize_serial)
            .any(|s| s == target_serial);
        if serial_hit {
            return Some(4);
        }
    }
    if let Some(name) = text(&game["title"]) {
        if let Some(tier) = rom_name::title_match_tier(rom_title, name) {
            return Some(tier);
        }
    }
    if alternate_titles(game)
        .into_iter()
        .any(|alt| rom_name::title_match_tier(rom_title, alt).is_some())
    {
        return Some(1);
    }
    None
}

#[cfg(test)]
fn select_candidate(games: &[Value], title: &str, platforms: &[&str]) -> Option<ScrapeCandidate> {
    select_candidate_for_query(games, title, None, platforms)
}

/// A busca usa título canônico (ou serial quando disponível). Prioriza jogos
/// oficiais (`is_addon = false`) sobre hacks/mods e só propõe candidato quando
/// não há empate entre jogos de mesmo nível na plataforma; a confirmação
/// continua no fluxo de revisão.
fn select_candidate_for_query(
    games: &[Value],
    rom_title: &str,
    serial: Option<&str>,
    platforms: &[&str],
) -> Option<ScrapeCandidate> {
    let scored: Vec<(&Value, (u8, bool))> = games
        .iter()
        .filter(|game| has_matching_platform(game, platforms))
        .filter_map(|game| {
            let tier = match_tier(game, rom_title, serial)?;
            Some((game, (tier, is_official_game(game))))
        })
        .collect();

    let best_rank = scored.iter().map(|(_, rank)| *rank).max()?;
    let mut best_games = scored.into_iter().filter(|(_, rank)| *rank == best_rank);
    let (game, _) = best_games.next()?;
    let first_id = text(&game["id"]);
    if best_games.any(|(other, _)| text(&other["id"]) != first_id) {
        return None;
    }

    let metadata = &game["metadata"];
    let catalog = catalog_details(game);
    let rating = number(&game["rating"])
        .or_else(|| number(&metadata["note"]["text"]).map(|rating| rating * 5.0))
        .filter(|rating| (0.0..=100.0).contains(rating))
        .map(|rating| rating.round() as u8);
    let age_rating = entries(&metadata["classifications"])
        .filter_map(|item| Some(format!("{} {}", text(&item["type"])?, text(&item["text"])?)))
        .collect::<Vec<_>>()
        .join(" · ");
    let genres = entries(&game["genres"])
        .filter_map(text)
        .collect::<Vec<_>>();
    let genres = if genres.is_empty() {
        entries(&metadata["genres"])
            .filter_map(|genre| {
                let mut names = entries(&genre["noms"]);
                names
                    .find(|name| text(&name["langue"]) == Some("en"))
                    .and_then(|name| text(&name["text"]))
                    .or_else(|| entries(&genre["noms"]).find_map(|name| text(&name["text"])))
            })
            .collect::<Vec<_>>()
    } else {
        genres
    };
    Some(ScrapeCandidate {
        provider: "gamekave".into(),
        external_id: text(&game["id"])?.into(),
        title: text(&game["title"])?.into(),
        description: text(&game["description"]).map(str::to_owned),
        cover_url: catalog.covers.first().cloned(),
        release_date: text(&game["release_date"])
            .map(|date| date.split('T').next().unwrap_or(date).to_owned()),
        genre: (!genres.is_empty()).then(|| genres.join(", ")),
        exact_hash_match: false,
        exact_filename_match: false,
        details: GameDetails {
            developer: named_text(&game["developer"])
                .or_else(|| named_text(&metadata["developpeur"]))
                .map(str::to_owned),
            publisher: named_text(&game["publisher"])
                .or_else(|| named_text(&metadata["editeur"]))
                .map(str::to_owned),
            players: named_text(&metadata["joueurs"]).map(str::to_owned),
            rating,
            age_rating: (!age_rating.is_empty()).then_some(age_rating),
            screenshot_url: catalog.screenshots.first().cloned(),
            title_screen_url: catalog_media(metadata, "sstitle").next(),
            fanart_url: catalog.heroes.first().cloned(),
            logo_url: media(&metadata["logo_url"])
                .or_else(|| catalog_media(metadata, "wheel-hd").next()),
            video_url: catalog.trailers.first().cloned(),
            gamekave: Some(catalog),
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalog_keeps_localized_synopses_and_regional_facts() {
        let game = json!({
            "id": "sample",
            "title": "Super Mario World",
            "platforms": [{"slug": "super-famicom"}],
            "metadata": {
                "synopsis": [{"langue": "pt", "text": "Em português"}],
                "dates": [{"region": "jp", "text": "1990-11-21"}],
                "joueurs": {"text": "1-2"},
                "note": {"text": "17"},
                "classifications": [{"type": "USK", "text": "0"}]
            },
            "time_to_beat": {"normally_seconds": 3601}
        });
        let candidate = select_candidate(&[game], "Super Mario World", platforms("snes")).unwrap();
        assert_eq!(candidate.details.players.as_deref(), Some("1-2"));
        assert_eq!(candidate.details.rating, Some(85));
        assert_eq!(candidate.details.age_rating.as_deref(), Some("USK 0"));
        let catalog = candidate.details.gamekave.unwrap();
        assert_eq!(catalog.descriptions["pt"], "Em português");
        assert_eq!(catalog.release_dates["JP"], "1990-11-21");
        assert_eq!(catalog.time_to_beat_minutes, [None, Some(61), None]);
    }

    #[test]
    fn platform_disambiguates_identical_titles_and_match_needs_review() {
        let games = [
            json!({"title": "Super Mario World", "id": "famicom",
                   "platforms": [{"slug": "family-computer"}]}),
            json!({"title": "Super Mario World", "id": "snes",
                   "platforms": [{"slug": "super-famicom"}],
                   "cover_url": "https://cdn.gamekave.net/box.png"}),
        ];
        let candidate = select_candidate(&games, "super mario world", platforms("snes")).unwrap();
        assert_eq!(candidate.external_id, "snes");
        assert_eq!(
            candidate.cover_url.as_deref(),
            Some("https://cdn.gamekave.net/box.png")
        );
        assert!(!candidate.auto_matches());
        assert!(select_candidate(&games, "super mario", platforms("snes")).is_none());
        assert!(select_candidate(&games, "super mario world", platforms("gba")).is_none());
        let same_platform = [
            games[1].clone(),
            json!({
                "title": "Super Mario World", "id": "other-snes",
                "platforms": [{"slug": "super-nintendo-entertainment-system"}]
            }),
        ];
        assert!(select_candidate(&same_platform, "super mario world", platforms("snes")).is_none());
    }

    #[test]
    fn matches_colon_subtitles_inverted_articles_diacritics_and_prefers_official_over_mods() {
        let psx = [json!({
            "id": "sotn",
            "title": "Castlevania: Symphony of the Night",
            "platforms": [{"slug": "playstation"}]
        })];
        let hit = select_candidate(
            &psx,
            "Castlevania - Symphony of the Night (USA)",
            platforms("psx"),
        )
        .expect("subtítulo com hífen casa com dois-pontos");
        assert_eq!(hit.external_id, "sotn");

        let snes = [json!({
            "id": "alttp",
            "title": "The Legend of Zelda: A Link to the Past",
            "platforms": [{"slug": "super-nintendo-entertainment-system"}]
        })];
        let hit = select_candidate(
            &snes,
            "Legend of Zelda, The - A Link to the Past (USA)",
            platforms("snes"),
        )
        .expect("artigo invertido No-Intro casa com título oficial");
        assert_eq!(hit.external_id, "alttp");

        let gba = [
            json!({
                "id": "emerald-hack",
                "title": "Pokémon Emerald Version",
                "platforms": [{"slug": "game-boy-advance"}],
                "game_type": {"slug": "mod", "is_addon": true, "counts_as_game": false}
            }),
            json!({
                "id": "emerald-official",
                "title": "Pokémon Emerald Version",
                "platforms": [{"slug": "game-boy-advance"}],
                "game_type": {"slug": "main_game", "is_addon": false, "counts_as_game": true}
            }),
        ];
        let hit = select_candidate(
            &gba,
            "0425 - Pokemon - Emerald Version (USA, Europe)",
            platforms("gba"),
        )
        .expect("jogo oficial vence mod homônimo e ignora acento");
        assert_eq!(hit.external_id, "emerald-official");
    }

    #[test]
    fn matches_by_psn_serial_and_alternate_localized_titles() {
        let ps2 = [
            json!({
                "id": "gta3",
                "title": "Grand Theft Auto III",
                "platforms": [{"slug": "playstation-2"}],
                "psn_serials": ["SLUS-20062"]
            }),
            json!({
                "id": "gta3-alt",
                "title": "Grand Theft Auto III",
                "platforms": [{"slug": "playstation-2"}]
            }),
        ];
        let hit = select_candidate_for_query(
            &ps2,
            "Grand Theft Auto III",
            Some("SLUS-20062"),
            platforms("ps2"),
        )
        .expect("serial desempata a edição exata");
        assert_eq!(hit.external_id, "gta3");

        let snes_jp = [json!({
            "id": "rockman-x",
            "title": "Mega Man X",
            "platforms": [{"slug": "super-famicom"}],
            "metadata": {
                "noms": [{"region": "jp", "text": "Rockman X"}]
            }
        })];
        let hit = select_candidate(&snes_jp, "Rockman X (Japan)", platforms("snes"))
            .expect("nome regional no metadata casa");
        assert_eq!(hit.external_id, "rockman-x");
    }
}
