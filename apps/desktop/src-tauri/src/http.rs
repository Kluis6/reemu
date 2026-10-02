//! Cliente HTTP dos downloads de servidores de terceiros: buildbot da
//! libretro (cores, shaders, assets do PPSSPP), GitHub (molduras) e capas.
//!
//! Sempre com um User-Agent que identifica o ReEmu e a página do projeto,
//! pra quem mantém o servidor saber de onde vem o tráfego e ter como
//! entrar em contato. Antes os downloads saíam sem User-Agent nenhum.

use std::sync::OnceLock;

pub const USER_AGENT: &str = concat!(
    "ReEmu/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Kluis6/reemu)"
);

/// Cliente compartilhado (reaproveita conexões entre downloads).
pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}
