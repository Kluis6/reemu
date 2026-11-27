//! Peça 2 da parceria com o Alvanista (`friends:read` e `achievements:read`):
//!
//! - `GET /api/partner/v1/friends` lista os Gamers conectados da conta;
//! - `GET /api/partner/v1/achievements` traz o resumo multiplataforma e as
//!   últimas conquistas e badges desbloqueadas.

use crate::alvanista::client::{partner_error_code, ClientError};
use crate::alvanista::Alvanista;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerFriendPlayingDto {
    pub game_id: Option<String>,
    pub title: Option<String>,
    pub platform: Option<String>,
    pub custom_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerFriendDto {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub profile_url: String,
    pub online: bool,
    pub playing_now: Option<PartnerFriendPlayingDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerPlatformStatDto {
    pub platform: String,
    pub total: i64,
    pub unlocked: i64,
    pub completion_percentage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerAchievementStatsDto {
    pub total: i64,
    pub unlocked: i64,
    pub total_points: i64,
    pub completion_percentage: f64,
    pub by_platform: Vec<PartnerPlatformStatDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerPlatformSummaryDto {
    pub platform: String,
    pub status: String,
    pub last_synced: Option<String>,
    pub total_games: i64,
    pub total_achievements: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerAchievementItemDto {
    pub source: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub game_or_app: Option<String>,
    pub game_id: Option<String>,
    pub unlocked_at: Option<String>,
    pub points: i64,
    pub names: HashMap<String, String>,
    pub descriptions: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerAchievementsDto {
    pub stats: PartnerAchievementStatsDto,
    pub platforms: Vec<PartnerPlatformSummaryDto>,
    pub recent: Vec<PartnerAchievementItemDto>,
}

#[derive(Deserialize)]
struct FriendsWire {
    #[serde(default)]
    friends: Vec<FriendWire>,
}

#[derive(Deserialize)]
struct FriendWire {
    id: String,
    username: String,
    #[serde(default)]
    avatar_url: Option<String>,
    profile_url: String,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    playing_now: Option<FriendPlayingWire>,
}

#[derive(Deserialize)]
struct FriendPlayingWire {
    #[serde(default)]
    game_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    custom_message: Option<String>,
}

#[derive(Deserialize)]
struct AchievementsWire {
    #[serde(default)]
    stats: StatsWire,
    #[serde(default)]
    platforms: Vec<PlatformSummaryWire>,
    #[serde(default)]
    recent: Vec<AchievementItemWire>,
}

#[derive(Default, Deserialize)]
struct StatsWire {
    #[serde(default)]
    total: i64,
    #[serde(default)]
    unlocked: i64,
    #[serde(default)]
    total_points: i64,
    #[serde(default)]
    completion_percentage: f64,
    #[serde(default)]
    by_platform: Vec<PlatformStatWire>,
}

#[derive(Deserialize)]
struct PlatformStatWire {
    platform: String,
    #[serde(default)]
    total: i64,
    #[serde(default)]
    unlocked: i64,
    #[serde(default)]
    completion_percentage: f64,
}

#[derive(Deserialize)]
struct PlatformSummaryWire {
    platform: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    last_synced: Option<String>,
    #[serde(default)]
    total_games: i64,
    #[serde(default)]
    total_achievements: i64,
}

#[derive(Deserialize)]
struct AchievementItemWire {
    source: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    game_or_app: Option<String>,
    #[serde(default)]
    game_id: Option<String>,
    #[serde(default)]
    unlocked_at: Option<String>,
    #[serde(default)]
    points: i64,
    #[serde(default)]
    names: HashMap<String, String>,
    #[serde(default)]
    descriptions: HashMap<String, String>,
}

fn map_client_error(err: ClientError) -> String {
    match err {
        ClientError::AuthEnded => "no_account".to_owned(),
        ClientError::Transient(_) => "offline".to_owned(),
        ClientError::Rejected(code) => {
            if code == "insufficient_scope" {
                "needs_consent".to_owned()
            } else {
                "failed".to_owned()
            }
        }
    }
}

fn map_non_success(status: u16, body: &serde_json::Value) -> String {
    match partner_error_code(body) {
        Some("insufficient_scope") => "needs_consent".to_owned(),
        _ if status == 403 => "needs_consent".to_owned(),
        _ => "failed".to_owned(),
    }
}

pub async fn fetch_friends(alva: &Alvanista) -> Result<Vec<PartnerFriendDto>, String> {
    let (status, val) = alva
        .partner_json(reqwest::Method::GET, "/api/partner/v1/friends", None)
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: FriendsWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    Ok(parsed
        .friends
        .into_iter()
        .map(|f| PartnerFriendDto {
            id: f.id,
            username: f.username,
            avatar_url: f.avatar_url,
            profile_url: f.profile_url,
            online: f.online,
            playing_now: f.playing_now.map(|p| PartnerFriendPlayingDto {
                game_id: p.game_id,
                title: p.title,
                platform: p.platform,
                custom_message: p.custom_message,
            }),
        })
        .collect())
}

pub async fn fetch_achievements(alva: &Alvanista) -> Result<PartnerAchievementsDto, String> {
    let (status, val) = alva
        .partner_json(reqwest::Method::GET, "/api/partner/v1/achievements", None)
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: AchievementsWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    Ok(PartnerAchievementsDto {
        stats: PartnerAchievementStatsDto {
            total: parsed.stats.total,
            unlocked: parsed.stats.unlocked,
            total_points: parsed.stats.total_points,
            completion_percentage: parsed.stats.completion_percentage,
            by_platform: parsed
                .stats
                .by_platform
                .into_iter()
                .map(|p| PartnerPlatformStatDto {
                    platform: p.platform,
                    total: p.total,
                    unlocked: p.unlocked,
                    completion_percentage: p.completion_percentage,
                })
                .collect(),
        },
        platforms: parsed
            .platforms
            .into_iter()
            .map(|p| PartnerPlatformSummaryDto {
                platform: p.platform,
                status: p.status,
                last_synced: p.last_synced,
                total_games: p.total_games,
                total_achievements: p.total_achievements,
            })
            .collect(),
        recent: parsed
            .recent
            .into_iter()
            .map(|r| PartnerAchievementItemDto {
                source: r.source,
                name: r.name,
                description: r.description,
                icon_url: r.icon_url,
                game_or_app: r.game_or_app,
                game_id: r.game_id,
                unlocked_at: r.unlocked_at,
                points: r.points,
                names: r.names,
                descriptions: r.descriptions,
            })
            .collect(),
    })
}

fn alva_state(app: &AppHandle) -> Result<tauri::State<'_, Alvanista>, String> {
    app.try_state::<Alvanista>()
        .ok_or_else(|| "failed".to_owned())
}

#[tauri::command]
pub async fn alvanista_friends(app: AppHandle) -> Result<Vec<PartnerFriendDto>, String> {
    let alva = alva_state(&app)?;
    fetch_friends(&alva).await
}

#[tauri::command]
pub async fn alvanista_achievements(app: AppHandle) -> Result<PartnerAchievementsDto, String> {
    let alva = alva_state(&app)?;
    fetch_achievements(&alva).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::Client;
    use crate::alvanista::store::test_support::{store_with, tokens, MemSecrets, SERIAL};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    fn spawn_fake_partner(reply: impl Fn(&str) -> (u16, String) + Send + 'static) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(15);
            while std::time::Instant::now() < until {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let _ = stream.set_nonblocking(false);
                let mut buf = vec![0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).into_owned();
                let first_line = raw.lines().next().unwrap_or_default().to_owned();
                let (code, resp_body) = reply(&first_line);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                    resp_body.len()
                );
            }
        });
        base
    }

    #[tokio::test]
    async fn fetch_friends_and_achievements_parse_and_handle_insufficient_scope() {
        let _serial = SERIAL.lock().await;
        let api = spawn_fake_partner(|line| {
            if line.starts_with("GET /api/partner/v1/friends ") {
                (
                    200,
                    r#"{"friends":[{"id":"alva_1","username":"kluis","avatar_url":null,"profile_url":"https://www.alvanista.com/users/kluis","online":true,"playing_now":{"game_id":null,"title":"Chrono Trigger","platform":"SNES"}}]}"#.into(),
                )
            } else if line.starts_with("GET /api/partner/v1/achievements ") {
                (
                    200,
                    r#"{"stats":{"total":50,"unlocked":25,"total_points":300,"completion_percentage":50.0,"by_platform":[{"platform":"steam","total":50,"unlocked":25,"completion_percentage":50.0}]},"platforms":[{"platform":"steam","status":"active","total_games":5,"total_achievements":25}],"recent":[{"source":"steam","name":"Hero","points":10}]}"#.into(),
                )
            } else {
                (403, r#"{"error":"insufficient_scope"}"#.into())
            }
        });
        let (store, _) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(
                &tokens("soc1"),
                &["profile:read", "friends:read", "achievements:read"],
            )
            .await
            .unwrap();

        let friends = fetch_friends(&alva).await.unwrap();
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].username, "kluis");
        assert!(friends[0].online);
        assert_eq!(
            friends[0]
                .playing_now
                .as_ref()
                .and_then(|p| p.title.as_deref()),
            Some("Chrono Trigger")
        );

        let ach = fetch_achievements(&alva).await.unwrap();
        assert_eq!(ach.stats.unlocked, 25);
        assert_eq!(ach.platforms.len(), 1);
        assert_eq!(ach.recent.len(), 1);
        assert_eq!(ach.recent[0].name, "Hero");
    }
}
