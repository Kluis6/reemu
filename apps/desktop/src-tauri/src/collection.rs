//! Vínculo da biblioteca com os três domínios do Alvanista (Coleção, Jornada e
//! Lista de Desejos) pelo `game_uuid` resolvido no catálogo (D160 a D165).
//!
//! - `POST /api/partner/v1/collection/status` lê até 200 jogos de uma vez;
//! - `PUT /api/partner/v1/collection/{game_id}` altera apenas os eixos enviados.

use crate::alvanista::client::{partner_error_code, ClientError};
use crate::alvanista::Alvanista;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const MAX_BATCH_IDS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItemDto {
    pub game_id: String,
    pub in_collection: bool,
    pub possessed: bool,
    pub ownership: String,
    pub in_wishlist: bool,
    pub interest: String,
    pub status: String,
    pub playtime_minutes: i64,
    pub last_played_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPatchDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_collection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_wishlist: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playtime_minutes: Option<i64>,
}

#[derive(Deserialize)]
struct StatusBatchWire {
    #[serde(default)]
    items: Vec<ItemWire>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UpdateItemWire {
    Wrapped { item: ItemWire },
    Direct(ItemWire),
}

#[derive(Deserialize)]
struct ItemWire {
    game_id: String,
    #[serde(default)]
    in_collection: bool,
    #[serde(default)]
    possessed: bool,
    #[serde(default)]
    ownership: String,
    #[serde(default)]
    in_wishlist: bool,
    #[serde(default)]
    interest: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    playtime_minutes: i64,
    #[serde(default)]
    last_played_at: Option<String>,
}

impl From<ItemWire> for CollectionItemDto {
    fn from(w: ItemWire) -> Self {
        Self {
            game_id: w.game_id,
            in_collection: w.in_collection,
            possessed: w.possessed,
            ownership: if w.ownership.is_empty() {
                "none".to_owned()
            } else {
                w.ownership
            },
            in_wishlist: w.in_wishlist,
            interest: if w.interest.is_empty() {
                "none".to_owned()
            } else {
                w.interest
            },
            status: match w.status.as_str() {
                "" | "wishlist" => "no_status".to_owned(),
                other => other.to_owned(),
            },
            playtime_minutes: w.playtime_minutes,
            last_played_at: w.last_played_at,
        }
    }
}

fn is_valid_uuid(raw: &str) -> bool {
    let s = raw.trim();
    if s.len() != 36 {
        return false;
    }
    for (idx, b) in s.bytes().enumerate() {
        let hyphen = matches!(idx, 8 | 13 | 18 | 23);
        if hyphen {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
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
        Some("game_not_found") => "game_not_found".to_owned(),
        Some("invalid_request") => "invalid_request".to_owned(),
        _ if status == 403 => "needs_consent".to_owned(),
        _ if status == 404 => "game_not_found".to_owned(),
        _ => "failed".to_owned(),
    }
}

pub async fn fetch_status(
    alva: &Alvanista,
    game_ids: &[String],
) -> Result<Vec<CollectionItemDto>, String> {
    if game_ids.is_empty() {
        return Ok(Vec::new());
    }
    if game_ids.len() > MAX_BATCH_IDS {
        return Err("invalid_request".to_owned());
    }
    let mut cleaned = Vec::with_capacity(game_ids.len());
    for id in game_ids {
        let trimmed = id.trim();
        if !is_valid_uuid(trimmed) {
            return Err("invalid_request".to_owned());
        }
        cleaned.push(trimmed.to_ascii_lowercase());
    }
    let payload = serde_json::json!({ "game_ids": cleaned });
    let (status, val) = alva
        .partner_json(
            reqwest::Method::POST,
            "/api/partner/v1/collection/status",
            Some(&payload),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: StatusBatchWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    Ok(parsed.items.into_iter().map(Into::into).collect())
}

pub async fn update_item(
    alva: &Alvanista,
    game_id: &str,
    patch: &CollectionPatchDto,
) -> Result<CollectionItemDto, String> {
    let clean_id = game_id.trim().to_ascii_lowercase();
    if !is_valid_uuid(&clean_id) {
        return Err("invalid_request".to_owned());
    }
    if patch.in_collection.is_none()
        && patch.in_wishlist.is_none()
        && patch.status.is_none()
        && patch.playtime_minutes.is_none()
    {
        return Err("invalid_request".to_owned());
    }
    let mut body = serde_json::Map::new();
    if let Some(sys) = patch
        .system
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        body.insert("system".into(), serde_json::Value::String(sys.to_owned()));
    }
    if let Some(v) = patch.in_collection {
        body.insert("in_collection".into(), serde_json::Value::Bool(v));
    }
    if let Some(v) = patch.in_wishlist {
        body.insert("wishlist".into(), serde_json::Value::Bool(v));
    }
    if let Some(st) = patch
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let canonical = match st {
            "completed_100" => "completed",
            "dropped" => "abandoned",
            other => other,
        };
        body.insert(
            "status".into(),
            serde_json::Value::String(canonical.to_owned()),
        );
    }
    if let Some(mins) = patch.playtime_minutes {
        if mins < 0 {
            return Err("invalid_request".to_owned());
        }
        body.insert("playtime_minutes".into(), serde_json::json!(mins));
    }
    let path = format!("/api/partner/v1/collection/{clean_id}");
    let (status, val) = alva
        .partner_json(
            reqwest::Method::PUT,
            &path,
            Some(&serde_json::Value::Object(body)),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: UpdateItemWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    let item = match parsed {
        UpdateItemWire::Wrapped { item } | UpdateItemWire::Direct(item) => item,
    };
    Ok(item.into())
}

fn alva_state(app: &AppHandle) -> Result<tauri::State<'_, Alvanista>, String> {
    app.try_state::<Alvanista>()
        .ok_or_else(|| "failed".to_owned())
}

#[tauri::command]
pub async fn alvanista_collection_status(
    app: AppHandle,
    game_ids: Vec<String>,
) -> Result<Vec<CollectionItemDto>, String> {
    let alva = alva_state(&app)?;
    fetch_status(&alva, &game_ids).await
}

#[tauri::command]
pub async fn alvanista_collection_update(
    app: AppHandle,
    game_id: String,
    patch: CollectionPatchDto,
) -> Result<CollectionItemDto, String> {
    let alva = alva_state(&app)?;
    update_item(&alva, &game_id, &patch).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::Client;
    use crate::alvanista::store::test_support::{store_with, tokens, MemSecrets, SERIAL};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const GAME_A: &str = "11111111-2222-3333-4444-555555555555";

    type SeenRequests = Arc<Mutex<Vec<(String, String)>>>;

    fn spawn_fake_partner(
        reply: impl Fn(&str, &str) -> (u16, String) + Send + 'static,
    ) -> (String, SeenRequests) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(15);
            while std::time::Instant::now() < until {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let _ = stream.set_nonblocking(false);
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).into_owned();
                let first_line = raw.lines().next().unwrap_or_default().to_owned();
                let body = raw
                    .split("\r\n\r\n")
                    .nth(1)
                    .unwrap_or_default()
                    .trim_matches(char::from(0))
                    .to_owned();
                seen2
                    .lock()
                    .unwrap()
                    .push((first_line.clone(), body.clone()));
                let (code, resp_body) = reply(&first_line, &body);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp_body}",
                    resp_body.len()
                );
            }
        });
        (base, seen)
    }

    #[tokio::test]
    async fn fetch_status_parses_three_domains_and_rejects_bad_uuids() {
        let _serial = SERIAL.lock().await;
        let (api, seen) = spawn_fake_partner(|line, _body| {
            assert!(line.starts_with("POST /api/partner/v1/collection/status "));
            (
                200,
                format!(
                    r#"{{"items":[{{"game_id":"{GAME_A}","in_collection":true,"possessed":true,"ownership":"owned","in_wishlist":false,"interest":"none","status":"playing","playtime_minutes":95,"last_played_at":"2026-11-26T20:00:00Z"}}]}}"#
                ),
            )
        });
        let (store, _) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(&tokens("col1"), &["profile:read", "collection"])
            .await
            .unwrap();

        assert_eq!(
            fetch_status(&alva, &["not-a-uuid".to_owned()]).await,
            Err("invalid_request".to_owned())
        );
        assert!(seen.lock().unwrap().is_empty());

        let items = fetch_status(&alva, &[GAME_A.to_owned()]).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].game_id, GAME_A);
        assert!(items[0].in_collection);
        assert!(items[0].possessed);
        assert!(!items[0].in_wishlist);
        assert_eq!(items[0].status, "playing");
        assert_eq!(items[0].playtime_minutes, 95);
    }

    #[tokio::test]
    async fn insufficient_scope_maps_to_needs_consent() {
        let _serial = SERIAL.lock().await;
        let (api, _) = spawn_fake_partner(|_, _| (403, r#"{"error":"insufficient_scope"}"#.into()));
        let (store, _) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(&tokens("col2"), &["profile:read"])
            .await
            .unwrap();

        let res = fetch_status(&alva, &[GAME_A.to_owned()]).await;
        assert_eq!(res, Err("needs_consent".to_owned()));
    }

    #[tokio::test]
    async fn update_item_sends_only_patched_axes_and_reads_flat_response() {
        let _serial = SERIAL.lock().await;
        let (api, seen) = spawn_fake_partner(|line, _body| {
            assert!(line.starts_with(&format!("PUT /api/partner/v1/collection/{GAME_A} ")));
            (
                200,
                format!(
                    r#"{{"game_id":"{GAME_A}","in_collection":false,"possessed":false,"ownership":"none","in_wishlist":true,"interest":"medium","status":"finished","playtime_minutes":240}}"#
                ),
            )
        });
        let (store, _) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(&tokens("col3"), &["profile:read", "collection"])
            .await
            .unwrap();

        let updated = update_item(
            &alva,
            GAME_A,
            &CollectionPatchDto {
                system: Some("n64".into()),
                in_collection: None,
                in_wishlist: Some(true),
                status: Some("finished".into()),
                playtime_minutes: None,
            },
        )
        .await
        .unwrap();
        assert!(updated.in_wishlist);
        assert_eq!(updated.interest, "medium");
        assert_eq!(updated.status, "finished");
        assert_eq!(updated.playtime_minutes, 240);

        let sent = seen.lock().unwrap();
        assert_eq!(sent.len(), 1);
        let payload: serde_json::Value = serde_json::from_str(&sent[0].1).unwrap();
        assert_eq!(payload["system"], "n64");
        assert_eq!(payload["wishlist"], true);
        assert!(payload.get("in_wishlist").is_none());
        assert!(payload.get("in_collection").is_none());
        assert_eq!(payload["status"], "finished");
        assert!(payload.get("playtime_minutes").is_none());
    }
}
