//! Formato da resposta do catálogo (D136) e tradução para `GameMetadata` e
//! `ScrapeCandidate`.

use domain::metadata::{GameDetails, GameKaveData, GameMetadata, ScrapeCandidate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Media {
    #[serde(default)]
    pub cover: Option<String>,
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

/// Só URLs `https://` sem usuário/senha entram nos metadados ou no cache.
pub fn https_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let parsed = reqwest::Url::parse(trimmed).ok()?;
    (parsed.scheme() == "https"
        && parsed.host_str().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none())
    .then(|| trimmed.to_owned())
}

fn clean_opt(s: Option<&str>) -> Option<String> {
    let trimmed = s?.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub fn metadata_from_answer(rom_id: &str, a: &GameAnswer) -> GameMetadata {
    let description = clean_opt(a.description.as_deref());
    let release_date = clean_opt(a.release_date.as_deref());
    let genres: Vec<&str> = a
        .genres
        .iter()
        .map(|g| g.trim())
        .filter(|g| !g.is_empty())
        .collect();
    let genre = (!genres.is_empty()).then(|| genres.join(", "));

    let cover_url = a.media.cover.as_deref().and_then(https_url);
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

    let gamekave = GameKaveData {
        descriptions,
        covers: cover_url.iter().cloned().collect(),
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
}

