//! Cliente HTTP dos downloads de servidores de terceiros: buildbot da
//! libretro (cores, shaders, assets do PPSSPP), GitHub (molduras) e capas.
//!
//! Sempre com um User-Agent que identifica o ReEmu e a página do projeto,
//! pra quem mantém o servidor saber de onde vem o tráfego e ter como
//! entrar em contato. Antes os downloads saíam sem User-Agent nenhum.

use std::sync::OnceLock;
use std::time::Duration;

pub const USER_AGENT: &str = concat!(
    "ReEmu/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Kluis6/reemu)"
);

// Sem limites o reqwest espera para sempre, e a tela de download ficava girando
// sem rede. O limite é por leitura, não total: core grande em rede lenta segue baixando.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

fn build(connect: Duration, read: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(connect)
        .read_timeout(read)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Cliente compartilhado (reaproveita conexões entre downloads).
pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| build(CONNECT_TIMEOUT, READ_TIMEOUT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::{AppError, ErrorCode};

    /// Conecta e nunca responde: é o que uma rede caída no meio do caminho faz.
    #[tokio::test]
    async fn servidor_parado_vira_erro_de_rede_em_vez_de_esperar_para_sempre() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/core.zip", listener.local_addr().unwrap());
        let client = build(Duration::from_secs(1), Duration::from_millis(200));

        let result = tokio::time::timeout(Duration::from_secs(5), client.get(&url).send())
            .await
            .expect("o cliente deveria desistir sozinho");

        let error = AppError::from_http("download", result.unwrap_err());
        assert_eq!(error.code, Some(ErrorCode::Offline));
        drop(listener);
    }
}
