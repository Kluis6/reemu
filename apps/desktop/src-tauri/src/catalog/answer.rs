//! Formato da resposta do catálogo (D136) e tradução para `GameMetadata` e
//! `ScrapeCandidate`.

use domain::metadata::{GameDetails, GameKaveData, GameMetadata, ScrapeCandidate};
use net::http::https_url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Media {
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub regional_covers: BTreeMap<String, String>,
    #[serde(default)]
    pub hero: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub title_screen: Option<String>,
    #[serde(default)]
    pub video: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameAnswer {
    pub game_id: String,
    pub title: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub developer: Option<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub players: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_lang: Option<String>,
    #[serde(default)]
    pub media: Media,
    #[serde(default)]
    pub media_pending: bool,
}

fn clean_opt(s: Option<&str>) -> Option<String> {
    let trimmed = s?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
pub fn metadata_from_answer(rom_id: &str, a: &GameAnswer) -> GameMetadata {
    metadata_from_answer_for_rom(rom_id, a, None, None)
}

pub fn metadata_from_answer_for_rom(
    rom_id: &str,
    a: &GameAnswer,
    rom_region: Option<&str>,
    rom_revision: Option<&str>,
) -> GameMetadata {
    let description = clean_opt(a.description.as_deref());
    let release_date = clean_opt(a.release_date.as_deref());
    let genres: Vec<&str> = a
        .genres
        .iter()
        .map(|g| g.trim())
        .filter(|g| !g.is_empty())
        .collect();
    let genre = (!genres.is_empty()).then(|| genres.join(", "));

    let default_cover = a.media.cover.as_deref().and_then(https_url);
    let mut regional_covers = BTreeMap::new();
    for (region, raw_url) in &a.media.regional_covers {
        let Some(url) = https_url(raw_url) else {
            continue;
        };
        if scraping::rom_name::is_non_front_cover_url(&url) {
            continue;
        }
        let key = scraping::rom_name::normalize_region_code(region)
            .map(str::to_owned)
            .unwrap_or_else(|| region.trim().to_ascii_uppercase());
        if !key.is_empty() {
            regional_covers.insert(key, url);
        }
    }
    let normalized_region = rom_region.and_then(|r| {
        scraping::rom_name::normalize_region_code(r)
            .map(str::to_owned)
            .or_else(|| clean_opt(Some(r)).map(|s| s.to_ascii_uppercase()))
    });
    let cover_url = scraping::rom_name::pick_regional_cover(
        &regional_covers,
        normalized_region.as_deref(),
        default_cover.as_deref(),
    )
    .map(str::to_owned);
    let fanart_url = a.media.hero.as_deref().and_then(https_url);
    let logo_url = a.media.logo.as_deref().and_then(https_url);
    let title_screen_url = a.media.title_screen.as_deref().and_then(https_url);
    let video_url = a.media.video.as_deref().and_then(https_url);
    let screenshots: Vec<String> = a
        .media
        .screenshots
        .iter()
        .filter_map(|u| https_url(u))
        .collect();
    let screenshot_url = screenshots.first().cloned();

    let mut descriptions = BTreeMap::new();
    if let (Some(lang), Some(desc)) = (clean_opt(a.description_lang.as_deref()), &description) {
        descriptions.insert(lang.to_ascii_lowercase(), desc.clone());
    }

    let mut covers: Vec<String> = Vec::new();
    for c in cover_url.iter().chain(default_cover.iter()) {
        if !covers.contains(c) {
            covers.push(c.clone());
        }
    }
    for c in regional_covers.values() {
        if !covers.contains(c) {
            covers.push(c.clone());
        }
    }
    covers.sort_by_key(|c| {
        let is_selected = cover_url.as_deref() == Some(c.as_str());
        scraping::rom_name::cover_url_rank(c, a.platform.as_deref(), is_selected, None)
    });

    let gamekave = GameKaveData {
        descriptions,
        regional_covers,
        rom_region: normalized_region,
        rom_revision: clean_opt(rom_revision),
        covers,
        heroes: fanart_url.iter().cloned().collect(),
        screenshots,
        trailers: video_url.iter().cloned().collect(),
        ..Default::default()
    };

    GameMetadata {
        rom_id: rom_id.to_owned(),
        title: a.title.trim().to_owned(),
        description,
        cover_url,
        release_date,
        genre,
        provider_source: Some("alvanista".into()),
        game_uuid: clean_opt(Some(&a.game_id)),
        details: GameDetails {
            developer: clean_opt(a.developer.as_deref()),
            publisher: clean_opt(a.publisher.as_deref()),
            players: clean_opt(a.players.as_deref()),
            rating: None,
            age_rating: None,
            modes: None,
            screenshot_url,
            title_screen_url,
            fanart_url,
            logo_url,
            video_url,
            gamekave: Some(gamekave),
        },
    }
}

pub fn candidate_from_answer(meta: &GameMetadata) -> ScrapeCandidate {
    ScrapeCandidate {
        provider: "alvanista".into(),
        external_id: meta.game_uuid.clone().unwrap_or_default(),
        title: meta.title.clone(),
        description: meta.description.clone(),
        cover_url: meta.cover_url.clone(),
        release_date: meta.release_date.clone(),
        genre: meta.genre.clone(),
        exact_hash_match: true,
        exact_filename_match: false,
        details: meta.details.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_from_answer_maps_every_field() {
        let answer = GameAnswer {
            game_id: "01950f10-aaaa-7bbb-8ccc-ddddeeeeffff".into(),
            title: "Chrono Trigger".into(),
            platform: Some("snes".into()),
            release_date: Some("1995-03-11".into()),
            genres: vec!["RPG".into(), "Aventura".into()],
            developer: Some("Square".into()),
            publisher: Some("Square".into()),
            players: Some("1".into()),
            description: Some("Viagem no tempo.".into()),
            description_lang: Some("pt-BR".into()),
            media: Media {
                cover: Some("https://cdn.alvanista.com/ct/cover.webp".into()),
                regional_covers: BTreeMap::new(),
                hero: Some("https://cdn.alvanista.com/ct/hero.webp".into()),
                logo: Some("https://cdn.alvanista.com/ct/logo.png".into()),
                title_screen: Some("https://cdn.alvanista.com/ct/title.png".into()),
                video: Some("https://cdn.alvanista.com/ct/trailer.mp4".into()),
                screenshots: vec![
                    "https://cdn.alvanista.com/ct/s0.webp".into(),
                    "http://insecure.example/s1.png".into(),
                    "https://cdn.alvanista.com/ct/s2.webp".into(),
                ],
            },
            media_pending: false,
        };

        let meta = metadata_from_answer("rom-ct", &answer);
        assert_eq!(meta.rom_id, "rom-ct");
        assert_eq!(meta.title, "Chrono Trigger");
        assert_eq!(meta.description.as_deref(), Some("Viagem no tempo."));
        assert_eq!(
            meta.cover_url.as_deref(),
            Some("https://cdn.alvanista.com/ct/cover.webp")
        );
        assert_eq!(meta.release_date.as_deref(), Some("1995-03-11"));
        assert_eq!(meta.genre.as_deref(), Some("RPG, Aventura"));
        assert_eq!(meta.provider_source.as_deref(), Some("alvanista"));
        assert_eq!(
            meta.game_uuid.as_deref(),
            Some("01950f10-aaaa-7bbb-8ccc-ddddeeeeffff")
        );
        assert_eq!(meta.details.developer.as_deref(), Some("Square"));
        assert_eq!(meta.details.publisher.as_deref(), Some("Square"));
        assert_eq!(meta.details.players.as_deref(), Some("1"));
        assert_eq!(
            meta.details.screenshot_url.as_deref(),
            Some("https://cdn.alvanista.com/ct/s0.webp")
        );
        assert_eq!(
            meta.details.title_screen_url.as_deref(),
            Some("https://cdn.alvanista.com/ct/title.png")
        );
        assert_eq!(
            meta.details.fanart_url.as_deref(),
            Some("https://cdn.alvanista.com/ct/hero.webp")
        );
        assert_eq!(
            meta.details.logo_url.as_deref(),
            Some("https://cdn.alvanista.com/ct/logo.png")
        );
        assert_eq!(
            meta.details.video_url.as_deref(),
            Some("https://cdn.alvanista.com/ct/trailer.mp4")
        );
        let gk = meta.details.gamekave.unwrap();
        assert_eq!(
            gk.screenshots,
            vec![
                "https://cdn.alvanista.com/ct/s0.webp",
                "https://cdn.alvanista.com/ct/s2.webp"
            ]
        );
        assert_eq!(
            gk.descriptions.get("pt-br").map(String::as_str),
            Some("Viagem no tempo.")
        );
    }

    #[test]
    fn answer_with_media_pending_keeps_cover_none() {
        let answer = GameAnswer {
            game_id: "01950f10-1111-7222-8333-444455556666".into(),
            title: "Super Metroid".into(),
            media: Media::default(),
            media_pending: true,
            ..Default::default()
        };
        let meta = metadata_from_answer("rom-sm", &answer);
        assert_eq!(meta.cover_url, None);
        assert_eq!(meta.details.fanart_url, None);
        assert_eq!(meta.details.logo_url, None);
        assert_eq!(meta.details.screenshot_url, None);
        assert_eq!(
            meta.game_uuid.as_deref(),
            Some("01950f10-1111-7222-8333-444455556666")
        );
    }

    #[test]
    fn metadata_from_answer_for_rom_picks_regional_cover_matching_rom_region() {
        let mut regional_covers = BTreeMap::new();
        regional_covers.insert("JP".into(), "https://cdn.gamekave.net/ct/jp.webp".into());
        regional_covers.insert("EU".into(), "https://cdn.gamekave.net/ct/eu.webp".into());
        regional_covers.insert("KR".into(), "http://insecure.example/kr.webp".into());
        let answer = GameAnswer {
            game_id: "01950f10-aaaa-7bbb-8ccc-ddddeeeeffff".into(),
            title: "Chrono Trigger".into(),
            media: Media {
                cover: Some("https://cdn.gamekave.net/ct/na.webp".into()),
                regional_covers,
                ..Default::default()
            },
            ..Default::default()
        };

        let jp = metadata_from_answer_for_rom("rom-jp", &answer, Some("JP"), Some("Rev 1"));
        assert_eq!(
            jp.cover_url.as_deref(),
            Some("https://cdn.gamekave.net/ct/jp.webp")
        );
        let gk_jp = jp.details.gamekave.unwrap();
        assert_eq!(gk_jp.rom_region.as_deref(), Some("JP"));
        assert_eq!(gk_jp.rom_revision.as_deref(), Some("Rev 1"));
        assert!(!gk_jp.regional_covers.contains_key("KR"));

        let au = metadata_from_answer_for_rom("rom-au", &answer, Some("Australia"), None);
        assert_eq!(
            au.cover_url.as_deref(),
            Some("https://cdn.gamekave.net/ct/eu.webp")
        );

        let na = metadata_from_answer_for_rom("rom-na", &answer, Some("USA"), None);
        assert_eq!(
            na.cover_url.as_deref(),
            Some("https://cdn.gamekave.net/ct/na.webp")
        );
    }
}
