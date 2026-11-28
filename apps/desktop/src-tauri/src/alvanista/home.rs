//! Novidades curadas do Alvanista para o `HeroCarousel` da tela inicial
//! (`GET /api/partner/v1/home?lang=<lang>`).
//!
//! Regras:
//! - Toda busca de rede acontece aqui no Rust, nunca no webview e nunca
//!   durante um jogo aberto ou carregando.
//! - O cache em `alvanista_home_feed` vale por 4 horas (`14400` s) por idioma;
//!   sem rede (ou durante o jogo), vale a cópia local.
//! - Entram somente `article` e `review` não patrocinados (`!sponsored`), com
//!   URL do próprio site do Alvanista (`is_site_url`) e dentro da janela de
//!   validade (`starts_at <= agora` quando presente, `ends_at` ausente ou `> agora`).
//! - A interface recebe no máximo 2 itens, sem URL crua, e pede a abertura
//!   pelo `id` (`alvanista_open_home_item`), que revalida tudo no SQLite antes
//!   de abrir o navegador do sistema.

use super::client::HomeFeedItemWire;
use super::loopback::Lang;
use super::{config, is_site_url_against, open_in_browser, store, Alvanista};
use crate::app_error::{AppError, ErrorCode};
use db::{AlvanistaHomeFeedRow, AlvanistaRepo};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// Validade da cópia local por idioma: 4 horas.
pub const HOME_CACHE_TTL_SECS: i64 = 4 * 60 * 60;
/// Limite de novidades entregues ao carrossel da tela inicial.
pub const MAX_HOME_ITEMS: usize = 2;
/// Máximo de linhas válidas guardadas por idioma no SQLite (permite que o 3º
/// assuma se o 1º vencer antes da próxima atualização).
const MAX_STORED_PER_LANG: usize = 16;
/// Evento disparado quando a abertura do app atualiza as novidades em segundo plano.
pub const HOME_EVENT: &str = "alvanista-home";
const LAST_LANG_KEY: &str = "home.lang";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlvanistaHomeItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlvanistaHomeView {
    pub enabled: bool,
    pub items: Vec<AlvanistaHomeItem>,
}

pub fn normalize_lang(raw: Option<&str>) -> &'static str {
    Lang::from_tag(raw.unwrap_or("pt-BR")).as_tag()
}

fn is_allowed_kind(kind: &str) -> bool {
    matches!(kind, "article" | "review")
}

fn parse_rfc3339_secs(raw: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(raw.trim())
        .ok()
        .map(|dt| dt.timestamp())
}

/// `true` se o item já começou (quando `starts_at` é um RFC3339 válido) e
/// ainda não venceu (`ends_at` vazio/nulo ou RFC3339 válido no futuro).
pub(crate) fn is_active_at(starts_at: Option<&str>, ends_at: Option<&str>, now_secs: i64) -> bool {
    if let Some(raw_start) = starts_at.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(start_ts) = parse_rfc3339_secs(raw_start) {
            if start_ts > now_secs {
                return false;
            }
        }
    }
    is_not_expired(ends_at, now_secs)
}

fn is_not_expired(ends_at: Option<&str>, now_secs: i64) -> bool {
    let Some(raw_end) = ends_at.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    let Some(end_ts) = parse_rfc3339_secs(raw_end) else {
        return false;
    };
    end_ts > now_secs
}

fn clean_image_url(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(trimmed).ok()?;
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.host_str().is_none() {
        return None;
    }
    let allowed = if cfg!(any(debug_assertions, test)) {
        matches!(parsed.scheme(), "https" | "http")
    } else {
        parsed.scheme() == "https"
    };
    allowed.then(|| trimmed.to_owned())
}

fn sanitize_wire_items(
    lang: &str,
    items: Vec<HomeFeedItemWire>,
    now_secs: i64,
    site_base: &str,
) -> Vec<AlvanistaHomeFeedRow> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for wire in items {
        let id = wire.id.trim().to_owned();
        let kind = wire.kind.trim().to_owned();
        let title = wire.title.trim().to_owned();
        let url = wire.url.trim().to_owned();
        if id.is_empty() || title.is_empty() || !seen.insert(id.clone()) {
            continue;
        }
        if wire.sponsored || !is_allowed_kind(&kind) {
            continue;
        }
        if !is_site_url_against(&url, site_base) {
            continue;
        }
        if !is_not_expired(wire.ends_at.as_deref(), now_secs) {
            continue;
        }
        let position = out.len() as i64;
        out.push(AlvanistaHomeFeedRow {
            id,
            lang: lang.to_owned(),
            kind,
            title,
            summary: wire.summary.trim().to_owned(),
            image_url: clean_image_url(wire.image_url.as_deref()),
            url,
            starts_at: wire
                .starts_at
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
            ends_at: wire
                .ends_at
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
            sponsored: false,
            position,
            fetched_at: now_secs,
        });
        if out.len() >= MAX_STORED_PER_LANG {
            break;
        }
    }
    out
}

fn filter_visible_rows(
    rows: Vec<AlvanistaHomeFeedRow>,
    now_secs: i64,
    site_base: &str,
) -> Vec<AlvanistaHomeItem> {
    rows.into_iter()
        .filter(|r| {
            !r.id.trim().is_empty()
                && !r.title.trim().is_empty()
                && !r.sponsored
                && is_allowed_kind(r.kind.trim())
                && is_site_url_against(r.url.trim(), site_base)
                && is_active_at(r.starts_at.as_deref(), r.ends_at.as_deref(), now_secs)
        })
        .take(MAX_HOME_ITEMS)
        .map(|r| AlvanistaHomeItem {
            id: r.id,
            kind: r.kind,
            title: r.title,
            summary: r.summary,
            image_url: clean_image_url(r.image_url.as_deref()),
        })
        .collect()
}

pub(crate) fn is_game_running(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<crate::commands::AppState>() else {
        return false;
    };
    state.session.loaded_core().is_some()
        || state
            .loading_game
            .load(std::sync::atomic::Ordering::Relaxed)
        || state
            .current_rom
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
}

impl Alvanista {
    fn home_repo(&self) -> AlvanistaRepo {
        AlvanistaRepo::new(self.store().db().clone())
    }

    pub async fn set_home_highlights_enabled(&self, enabled: bool) -> Result<(), AppError> {
        self.home_repo()
            .set_home_highlights_enabled(enabled)
            .await
            .map_err(AppError::from)
    }

    pub async fn home_highlights(
        &self,
        lang: Option<&str>,
        is_playing: bool,
    ) -> Result<AlvanistaHomeView, AppError> {
        self.home_highlights_at(lang, is_playing, store::now_secs(), &config::base_urls().0)
            .await
            .map(|(view, _)| view)
    }

    pub(crate) async fn home_highlights_at(
        &self,
        lang: Option<&str>,
        is_playing: bool,
        now_secs: i64,
        site_base: &str,
    ) -> Result<(AlvanistaHomeView, bool), AppError> {
        let repo = self.home_repo();
        if !repo.home_highlights_enabled().await? {
            return Ok((
                AlvanistaHomeView {
                    enabled: false,
                    items: Vec::new(),
                },
                false,
            ));
        }
        let lang = normalize_lang(lang);
        let _ = repo.state_set(LAST_LANG_KEY, lang).await;

        let mut refreshed = false;
        if !is_playing && is_cache_stale(&repo, lang, now_secs).await? {
            let _guard = self.home_lock.lock().await;
            if !repo.home_highlights_enabled().await? {
                return Ok((
                    AlvanistaHomeView {
                        enabled: false,
                        items: Vec::new(),
                    },
                    false,
                ));
            }
            if is_cache_stale(&repo, lang, now_secs).await? {
                match self.client.get_home_feed(lang).await {
                    Ok(wire_items) => {
                        let clean = sanitize_wire_items(lang, wire_items, now_secs, site_base);
                        repo.replace_home_feed(lang, &clean, now_secs).await?;
                        refreshed = true;
                    }
                    Err(e) => {
                        log::info!(
                            "alvanista: sem resposta em /home ({lang}), usando cache local: {e}"
                        );
                    }
                }
            }
        }

        let rows = repo.list_home_feed(lang).await?;
        let items = filter_visible_rows(rows, now_secs, site_base);
        Ok((
            AlvanistaHomeView {
                enabled: true,
                items,
            },
            refreshed,
        ))
    }

    pub async fn open_home_item(&self, id: &str) -> Result<(), AppError> {
        self.open_home_item_with(
            id,
            store::now_secs(),
            &config::base_urls().0,
            open_in_browser,
        )
        .await
    }

    pub(crate) async fn open_home_item_with<F>(
        &self,
        id: &str,
        now_secs: i64,
        site_base: &str,
        open: F,
    ) -> Result<(), AppError>
    where
        F: FnOnce(&str) -> Result<(), String>,
    {
        let clean_id = id.trim();
        if clean_id.is_empty() {
            return Err(AppError::new(ErrorCode::NotFound, "destaque inválido"));
        }
        let row = self
            .home_repo()
            .home_feed_item_by_id(clean_id)
            .await?
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "destaque não encontrado"))?;
        if row.sponsored || !is_allowed_kind(row.kind.trim()) {
            return Err(AppError::from("destaque indisponível"));
        }
        if !is_active_at(row.starts_at.as_deref(), row.ends_at.as_deref(), now_secs) {
            return Err(AppError::from("destaque expirado"));
        }
        let url = row.url.trim();
        if !is_site_url_against(url, site_base) {
            return Err(AppError::from("URL fora do site do Alvanista"));
        }
        open(url).map_err(|e| {
            log::warn!("alvanista: não abriu o destaque {clean_id}: {e}");
            AppError::from(e)
        })
    }
}

async fn is_cache_stale(repo: &AlvanistaRepo, lang: &str, now_secs: i64) -> Result<bool, AppError> {
    let Some(fetched_at) = repo.home_feed_fetched_at(lang).await? else {
        return Ok(true);
    };
    Ok(now_secs.saturating_sub(fetched_at) >= HOME_CACHE_TTL_SECS)
}

/// Aquece o cache das novidades ao abrir o app quando a preferência está
/// ligada, nenhum jogo está rodando e a cópia do último idioma venceu.
pub(crate) async fn refresh_home_on_start(app: &AppHandle) {
    if is_game_running(app) {
        return;
    }
    let Some(alva) = app.try_state::<Alvanista>() else {
        return;
    };
    let saved_lang = alva
        .home_repo()
        .state_get(LAST_LANG_KEY)
        .await
        .ok()
        .flatten();
    match alva
        .home_highlights_at(
            saved_lang.as_deref(),
            is_game_running(app),
            store::now_secs(),
            &config::base_urls().0,
        )
        .await
    {
        Ok((view, true)) => {
            if let Err(e) = app.emit(HOME_EVENT, &view) {
                log::warn!("alvanista: não avisou a interface das novidades: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => log::warn!("alvanista: falha ao ler novidades na abertura: {e}"),
    }
}

fn alva_for_cmd(app: &AppHandle) -> Result<tauri::State<'_, Alvanista>, AppError> {
    app.try_state::<Alvanista>()
        .ok_or_else(|| AppError::from("integração do Alvanista indisponível"))
}

#[tauri::command]
pub async fn alvanista_home_highlights(
    app: AppHandle,
    lang: Option<String>,
) -> Result<AlvanistaHomeView, AppError> {
    let alva = alva_for_cmd(&app)?;
    alva.home_highlights(lang.as_deref(), is_game_running(&app))
        .await
}

#[tauri::command]
pub async fn alvanista_set_home_highlights_enabled(
    app: AppHandle,
    enabled: bool,
) -> Result<(), AppError> {
    let alva = alva_for_cmd(&app)?;
    alva.set_home_highlights_enabled(enabled).await
}

#[tauri::command]
pub async fn alvanista_open_home_item(app: AppHandle, id: String) -> Result<(), AppError> {
    let alva = alva_for_cmd(&app)?;
    alva.open_home_item(&id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::Client;
    use crate::alvanista::store::test_support::{store_and_db, store_with, MemSecrets};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    const SITE_PROD: &str = "https://www.alvanista.com";

    fn spawn_home_server(
        reply: impl Fn(&str) -> (u16, String) + Send + 'static,
    ) -> (String, Arc<AtomicUsize>, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(Mutex::new(Vec::new()));
        let (hits2, paths2) = (hits.clone(), paths.clone());
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(15);
            while std::time::Instant::now() < until {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                hits2.fetch_add(1, Ordering::SeqCst);
                let _ = stream.set_nonblocking(false);
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);
                let first_line = raw.lines().next().unwrap_or_default();
                let req_path = first_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_owned();
                paths2.lock().unwrap().push(req_path.clone());
                let (status, body) = reply(&req_path);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (base, hits, paths)
    }

    #[test]
    fn site_url_accepts_alvanista_https_and_debug_localhost_and_rejects_external() {
        assert!(is_site_url_against(
            "https://www.alvanista.com/articles/snes-jrpgs",
            SITE_PROD
        ));
        assert!(is_site_url_against(
            "https://alvanista.com/reviews/chrono-trigger",
            SITE_PROD
        ));
        for bad in [
            "http://www.alvanista.com/articles/1",
            "http://alvanista.com/articles/1",
            "https://evil.example/articles/1",
            "https://www.alvanista.com.evil.example/articles/1",
            "https://sub.alvanista.com/articles/1",
            "https://user@www.alvanista.com/articles/1",
            "https://www.alvanista.com:8443/articles/1",
            "javascript:alert(1)",
            "file:///etc/passwd",
        ] {
            assert!(!is_site_url_against(bad, SITE_PROD), "aceitou {bad}");
        }

        // Em debug/test com override localhost, só a mesma origem e porta valem.
        let local_site = "http://localhost:3123";
        assert!(is_site_url_against(
            "http://localhost:3123/articles/teste",
            local_site
        ));
        assert!(!is_site_url_against(
            "http://localhost:9999/articles/teste",
            local_site
        ));
        assert!(!is_site_url_against(
            "http://127.0.0.1:3123/articles/teste",
            local_site
        ));
    }

    #[tokio::test]
    async fn filters_sponsored_invalid_kinds_external_urls_expired_items_and_caps_at_two() {
        let now = 1_760_000_000; // 2025-10-09T08:53:20Z
        let payload = serde_json::json!({
            "items": [
                {
                    "id": "sponsored-1",
                    "kind": "article",
                    "title": "Patrocinado",
                    "summary": "Nao entra",
                    "image_url": "https://cdn.alvanista.com/s.jpg",
                    "url": "https://www.alvanista.com/articles/s",
                    "starts_at": "2025-10-01T00:00:00Z",
                    "ends_at": null,
                    "sponsored": true
                },
                {
                    "id": "wrong-kind",
                    "kind": "promo",
                    "title": "Promo",
                    "summary": "Nao entra",
                    "image_url": "https://cdn.alvanista.com/p.jpg",
                    "url": "https://www.alvanista.com/promos/p",
                    "starts_at": "2025-10-01T00:00:00Z",
                    "ends_at": null,
                    "sponsored": false
                },
                {
                    "id": "bad-url",
                    "kind": "article",
                    "title": "Fora do site",
                    "summary": "Nao entra",
                    "image_url": "https://cdn.alvanista.com/e.jpg",
                    "url": "https://parceiro-externo.example/post",
                    "starts_at": "2025-10-01T00:00:00Z",
                    "ends_at": null,
                    "sponsored": false
                },
                {
                    "id": "expired-1",
                    "kind": "review",
                    "title": "Ja venceu",
                    "summary": "Nao entra",
                    "image_url": "https://cdn.alvanista.com/ex.jpg",
                    "url": "https://www.alvanista.com/reviews/ex",
                    "starts_at": "2025-09-01T00:00:00Z",
                    "ends_at": "2025-10-01T00:00:00Z",
                    "sponsored": false
                },
                {
                    "id": "art-1",
                    "kind": "article",
                    "title": "Historia do Super Nintendo",
                    "summary": "Os bastidores de 16 bits.",
                    "image_url": "https://cdn.alvanista.com/art1.jpg",
                    "url": "https://www.alvanista.com/articles/snes",
                    "starts_at": "2025-10-01T00:00:00Z",
                    "ends_at": "2025-10-20T00:00:00Z",
                    "sponsored": false
                },
                {
                    "id": "rev-1",
                    "kind": "review",
                    "title": "Review: Chrono Trigger",
                    "summary": "Um classico atemporal.",
                    "image_url": "https://cdn.alvanista.com/rev1.jpg",
                    "url": "https://alvanista.com/reviews/chrono-trigger",
                    "starts_at": "2025-10-01T00:00:00Z",
                    "ends_at": null,
                    "sponsored": false
                },
                {
                    "id": "art-3",
                    "kind": "article",
                    "title": "Terceiro item valido",
                    "summary": "Fica de reserva no cache, fora do teto de 2.",
                    "image_url": "https://cdn.alvanista.com/art3.jpg",
                    "url": "https://www.alvanista.com/articles/terceiro",
                    "starts_at": "2025-10-01T00:00:00Z",
                    "ends_at": null,
                    "sponsored": false
                }
            ]
        })
        .to_string();

        let (api, hits, paths) = spawn_home_server(move |_| (200, payload.clone()));
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));

        let (view, refreshed) = alva
            .home_highlights_at(Some("pt-PT"), false, now, SITE_PROD)
            .await
            .unwrap();
        assert!(refreshed);
        assert!(view.enabled);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(
            secrets.calls.load(Ordering::SeqCst),
            0,
            "endpoint publico nao toca no chaveiro"
        );
        assert_eq!(
            paths.lock().unwrap().as_slice(),
            &["/api/partner/v1/home?lang=pt-BR"]
        );
        assert_eq!(view.items.len(), 2);
        assert_eq!(view.items[0].id, "art-1");
        assert_eq!(view.items[0].kind, "article");
        assert_eq!(view.items[1].id, "rev-1");
        assert_eq!(view.items[1].kind, "review");
    }

    #[tokio::test]
    async fn respects_four_hour_ttl_never_fetches_during_gameplay_and_hides_expired_offline() {
        let t0 = 1_760_000_000; // 2025-10-09T08:53:20Z
                                // art-short vence em +2h (t0 + 7200), antes mesmo de o TTL de 4h acabar!
        let short_end = "2025-10-09T10:53:20Z";
        let payload = serde_json::json!({
            "items": [
                {
                    "id": "art-short",
                    "kind": "article",
                    "title": "Curta duracao",
                    "summary": "Vence em 2h",
                    "image_url": "https://cdn.alvanista.com/short.jpg",
                    "url": "https://www.alvanista.com/articles/short",
                    "starts_at": "2025-10-09T08:00:00Z",
                    "ends_at": short_end,
                    "sponsored": false
                },
                {
                    "id": "rev-long",
                    "kind": "review",
                    "title": "Sem data de fim",
                    "summary": "Continua valendo",
                    "image_url": "https://cdn.alvanista.com/long.jpg",
                    "url": "https://www.alvanista.com/reviews/long",
                    "starts_at": "2025-10-09T08:00:00Z",
                    "ends_at": null,
                    "sponsored": false
                }
            ]
        })
        .to_string();

        let (api, hits, _) = spawn_home_server(move |_| (200, payload.clone()));
        let (store, _) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));

        // 1) Em jogo antes de qualquer cache: NUNCA chama a rede!
        let (while_playing, refreshed) = alva
            .home_highlights_at(Some("en"), true, t0, SITE_PROD)
            .await
            .unwrap();
        assert!(!refreshed);
        assert!(while_playing.items.is_empty());
        assert_eq!(hits.load(Ordering::SeqCst), 0);

        // 2) Fora do jogo: busca na rede e grava no SQLite.
        let (initial, refreshed) = alva
            .home_highlights_at(Some("en"), false, t0, SITE_PROD)
            .await
            .unwrap();
        assert!(refreshed);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(initial.items.len(), 2);

        // 3) 3h depois (< 4h): lê só do SQLite, sem nova chamada HTTP, e o item
        //    que venceu em +2h (`art-short`) já desapareceu!
        let t_3h = t0 + 3 * 3600;
        let (after_3h, refreshed) = alva
            .home_highlights_at(Some("en"), false, t_3h, SITE_PROD)
            .await
            .unwrap();
        assert!(!refreshed);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(after_3h.items.len(), 1);
        assert_eq!(after_3h.items[0].id, "rev-long");

        // 4) 5h depois (>= 14400s), mas COM jogo aberto: continua sem tocar na rede!
        let t_5h = t0 + 5 * 3600;
        let (playing_stale, refreshed) = alva
            .home_highlights_at(Some("en"), true, t_5h, SITE_PROD)
            .await
            .unwrap();
        assert!(!refreshed);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(playing_stale.items.len(), 1);

        // 5) 5h depois e fora do jogo: agora renova na rede!
        let (_, refreshed) = alva
            .home_highlights_at(Some("en"), false, t_5h, SITE_PROD)
            .await
            .unwrap();
        assert!(refreshed);
        assert_eq!(hits.load(Ordering::SeqCst), 2);

        // 6) Chave desligada pelo usuário: devolve vazio e nunca chama a rede.
        alva.set_home_highlights_enabled(false).await.unwrap();
        let (disabled, refreshed) = alva
            .home_highlights_at(Some("en"), false, t_5h + 20_000, SITE_PROD)
            .await
            .unwrap();
        assert!(!disabled.enabled);
        assert!(disabled.items.is_empty());
        assert!(!refreshed);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn open_home_item_revalidates_url_sponsorship_and_expiry_from_sqlite() {
        let now = 1_760_000_000;
        let (store, _, db) = store_and_db(MemSecrets::default()).await;
        let repo = AlvanistaRepo::new(db);
        repo.replace_home_feed(
            "pt-BR",
            &[
                AlvanistaHomeFeedRow {
                    id: "ok".into(),
                    lang: "pt-BR".into(),
                    kind: "article".into(),
                    title: "Valido".into(),
                    summary: String::new(),
                    image_url: None,
                    url: "https://www.alvanista.com/articles/valido".into(),
                    starts_at: Some("2025-10-01T00:00:00Z".into()),
                    ends_at: Some("2025-10-20T00:00:00Z".into()),
                    sponsored: false,
                    position: 0,
                    fetched_at: now,
                },
                AlvanistaHomeFeedRow {
                    id: "expired".into(),
                    lang: "pt-BR".into(),
                    kind: "review".into(),
                    title: "Vencido".into(),
                    summary: String::new(),
                    image_url: None,
                    url: "https://www.alvanista.com/reviews/vencido".into(),
                    starts_at: None,
                    ends_at: Some("2025-10-01T00:00:00Z".into()),
                    sponsored: false,
                    position: 1,
                    fetched_at: now,
                },
                AlvanistaHomeFeedRow {
                    id: "evil".into(),
                    lang: "pt-BR".into(),
                    kind: "article".into(),
                    title: "Adulterado".into(),
                    summary: String::new(),
                    image_url: None,
                    url: "https://evil.example/phish".into(),
                    starts_at: None,
                    ends_at: None,
                    sponsored: false,
                    position: 2,
                    fetched_at: now,
                },
                AlvanistaHomeFeedRow {
                    id: "sponsored".into(),
                    lang: "pt-BR".into(),
                    kind: "article".into(),
                    title: "Pago".into(),
                    summary: String::new(),
                    image_url: None,
                    url: "https://www.alvanista.com/articles/pago".into(),
                    starts_at: None,
                    ends_at: None,
                    sponsored: true,
                    position: 3,
                    fetched_at: now,
                },
            ],
            now,
        )
        .await
        .unwrap();

        let alva = Alvanista::new(Client::with_api("http://127.0.0.1:9".into(), store));
        let opened = Arc::new(Mutex::new(Vec::<String>::new()));

        let opened_ok = opened.clone();
        alva.open_home_item_with("ok", now, SITE_PROD, move |u| {
            opened_ok.lock().unwrap().push(u.to_owned());
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(
            opened.lock().unwrap().as_slice(),
            &["https://www.alvanista.com/articles/valido"]
        );

        for bad_id in ["missing", "expired", "evil", "sponsored", "  "] {
            let opened_bad = opened.clone();
            let res = alva
                .open_home_item_with(bad_id, now, SITE_PROD, move |u| {
                    opened_bad.lock().unwrap().push(u.to_owned());
                    Ok(())
                })
                .await;
            assert!(res.is_err(), "deveria recusar {bad_id}");
        }
        assert_eq!(opened.lock().unwrap().len(), 1);
    }
}
