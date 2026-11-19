use domain::metadata::{GameDetails, GameKaveData, ScrapeCandidate, ScrapeQuery};
use serde_json::Value;

use super::search_title;

const SEARCH_URL: &str = "https://api.gamekave.net/games/search";

/// Slugs do catálogo GameKave (/games/filters/platforms).
fn platforms(system: &str) -> &[&str] {
    match system {
        "nes" => &["nintendo-entertainment-system", "family-computer"],
        "snes" => &["super-nintendo-entertainment-system", "super-famicom"],
        "n64" => &["nintendo-64"],
        "gb" => &["game-boy"],
        "gbc" => &["game-boy-color"],
        "gba" => &["game-boy-advance"],
        "vb" => &["virtual-boy"],
        "nds" => &["nintendo-ds", "nintendo-dsi"],
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
        "3do" => &["3do-interactive-multiplayer"],
        "cdi" => &["philips-cd-i"],
        "msx" => &["msx", "msx2", "msx-turbo-r"],
        "c64" => &["commodore-c64-128-max"],
        "amiga" => &["amiga", "amiga-aga"],
        "zxspectrum" => &["zx-spectrum"],
        "amstradcpc" => &["amstrad-cpc"],
        "arcade" => &["arcade"],
        "dos" => &["dos"],
        _ => &[],
    }
}

pub(super) async fn query(
    client: &reqwest::Client,
    query: &ScrapeQuery<'_>,
) -> Result<Option<ScrapeCandidate>, String> {
    let platforms = platforms(query.system_id);
    let title = search_title(query.file_stem);
    if platforms.is_empty() || title.is_empty() {
        return Ok(None);
    }
    let response = client
        .get(SEARCH_URL)
        .query(&[("q", &title)])
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
    let Some(mut candidate) = select_candidate(games, &title, platforms) else {
        return Ok(None);
    };
    // A ficha da busca continua utilizável se a consulta extra falhar.
    if let Some(assets) = fetch_assets(client, &candidate.external_id).await {
        let details = candidate
            .details
            .gamekave
            .get_or_insert_with(Default::default);
        for (list, kind) in [
            (&mut details.covers, "covers"),
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
        candidate.cover_url = candidate
            .cover_url
            .or_else(|| details.covers.first().cloned());
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

async fn fetch_assets(client: &reqwest::Client, game_id: &str) -> Option<Value> {
    let mut url = reqwest::Url::parse(SEARCH_URL).ok()?;
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

/// A busca usa título (não hash). Só propõe um candidato quando há um único
/// título exato na plataforma, e a confirmação continua no fluxo de revisão.
fn select_candidate(games: &[Value], title: &str, platforms: &[&str]) -> Option<ScrapeCandidate> {
    let title = title.to_lowercase();
    let mut matches = games.iter().filter(|game| {
        text(&game["title"]).is_some_and(|name| name.to_lowercase() == title)
            && game["platforms"].as_array().is_some_and(|entries| {
                entries
                    .iter()
                    .any(|entry| text(&entry["slug"]).is_some_and(|slug| platforms.contains(&slug)))
            })
    });
    let game = matches.next()?;
    if matches.next().is_some() {
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
}
