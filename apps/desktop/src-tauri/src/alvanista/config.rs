//! URLs do Alvanista e montagem da URL de autorização.

use super::pkce::Pkce;

const WEB: &str = "https://www.alvanista.com";
const API: &str = "https://api.alvanista.com";

pub const CLIENT_ID: &str = "reemu";
pub const SCOPE: &str = "profile:read";

/// `(site, api)`. Em release são sempre as duas URLs de produção. Só em
/// build de debug `REEMU_ALVANISTA_BASE` troca: `"<site>,<api>"` ou uma URL
/// só, usada para os dois.
pub fn base_urls() -> (String, String) {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("REEMU_ALVANISTA_BASE") {
        if let Some(urls) = parse_override(&v) {
            return urls;
        }
    }
    (WEB.to_owned(), API.to_owned())
}

#[cfg(any(debug_assertions, test))]
fn parse_override(v: &str) -> Option<(String, String)> {
    let clean = |s: &str| s.trim().trim_end_matches('/').to_owned();
    let mut parts = v.split(',').map(clean).filter(|s| !s.is_empty());
    let web = parts.next()?;
    let api = parts.next().unwrap_or_else(|| web.clone());
    Some((web, api))
}

pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}/callback")
}

/// Rótulo do aparelho que o Alvanista mostra na tela de consentimento.
pub fn device_label() -> &'static str {
    if cfg!(windows) {
        "ReEmu (Windows)"
    } else {
        "ReEmu (Linux)"
    }
}

/// URL da página de consentimento, para abrir no navegador do sistema.
pub fn authorize_url(web: &str, port: u16, pkce: &Pkce) -> String {
    let params = [
        ("client_id", CLIENT_ID),
        ("redirect_uri", &redirect_uri(port)),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("state", &pkce.state),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("device_label", device_label()),
    ];
    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{k}={}", percent_encode(v)))
        .collect();
    format!("{web}/conectar?{}", query.join("&"))
}

/// Tudo que não é "não reservado" (RFC 3986) vira `%XX`; espaço é `%20`.
/// Os parênteses ficam como estão (válidos na query).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'(' | b')') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_accepts_one_url_or_a_pair() {
        assert_eq!(
            parse_override("http://localhost:3000/"),
            Some((
                "http://localhost:3000".into(),
                "http://localhost:3000".into()
            ))
        );
        assert_eq!(
            parse_override("http://a, http://b/"),
            Some(("http://a".into(), "http://b".into()))
        );
        assert_eq!(parse_override("  "), None);
    }

    #[test]
    fn authorize_url_carries_every_parameter() {
        let pkce = Pkce {
            verifier: "v".into(),
            challenge: "chal".into(),
            state: "st".into(),
        };
        let url = authorize_url("https://www.alvanista.com", 4242, &pkce);
        assert!(url.starts_with("https://www.alvanista.com/conectar?client_id=reemu&"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A4242%2Fcallback"));
        assert!(url.contains("&response_type=code&scope=profile%3Aread&state=st&"));
        assert!(url.contains("code_challenge=chal&code_challenge_method=S256"));
        assert!(
            url.ends_with("&device_label=ReEmu%20(Windows)")
                || url.ends_with("&device_label=ReEmu%20(Linux)")
        );
        assert!(!url.contains("verifier"));
    }
}
