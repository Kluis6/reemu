//! PKCE (RFC 7636), método S256, e o `state` anti-CSRF.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub struct Pkce {
    /// 43 caracteres `[A-Za-z0-9-_]`; nunca sai do processo, só vai no
    /// `POST /token`.
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

/// `n` bytes do gerador do SO, em base64url sem padding.
pub fn random_token(n: usize) -> String {
    let mut buf = vec![0u8; n];
    OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn new() -> Pkce {
    let verifier = random_token(32);
    Pkce {
        challenge: challenge_for(&verifier),
        verifier,
        state: random_token(16),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        // RFC 7636, Apêndice B.
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn new_pkce_is_well_formed_and_fresh() {
        let (a, b) = (new(), new());
        assert_eq!(a.verifier.len(), 43);
        assert!(a
            .verifier
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
        assert_eq!(a.challenge, challenge_for(&a.verifier));
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
    }
}
