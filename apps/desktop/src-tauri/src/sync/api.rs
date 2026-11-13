//! Cliente REST das rotas de save do Alvanista.
//!
//! Duas famílias de rota, com dois tipos de credencial que nunca se misturam:
//! - `/api/partner/v1/save-devices`: token de acesso do parceiro (o par do
//!   "Entrar com Alvanista"), só para criar e listar aparelhos;
//! - `/api/v1/saves/*`: o token do aparelho (`asv_...`), sempre em `Bearer`.
//!
//! O middleware do parceiro responde `{"error":"invalid_token"}` (texto
//! solto) e os handlers `{"error":{"code":"..."}}` (objeto); os dois formatos
//! são lidos aqui. Nenhum token entra em log nem em mensagem de erro.

use super::device::HardwarePayload;
use serde::Deserialize;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// 401: token inválido, expirado ou revogado.
    Unauthorized,
    /// 403 `insufficient_scope`: a sessão não tem o escopo `saves`.
    InsufficientScope,
    /// 423: o aparelho está pausado no site.
    Paused,
    /// 402: o plano não libera o sync.
    TierInsufficient {
        upgrade_to: String,
    },
    /// 507: acabou a cota de armazenamento.
    Quota,
    /// 413: o arquivo passa do limite.
    TooLarge,
    NotFound,
    /// 429, 5xx ou rede: vale tentar depois.
    Transient(String),
    /// Outro 4xx; leva o `code` do servidor (ou o status).
    Rejected(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized => f.write_str("não autorizado"),
            Self::InsufficientScope => f.write_str("escopo insuficiente"),
            Self::Paused => f.write_str("aparelho pausado"),
            Self::TierInsufficient { .. } => f.write_str("plano sem sync de saves"),
            Self::Quota => f.write_str("cota esgotada"),
            Self::TooLarge => f.write_str("arquivo grande demais"),
            Self::NotFound => f.write_str("não encontrado"),
            Self::Transient(e) => write!(f, "Alvanista indisponível: {e}"),
            Self::Rejected(c) => write!(f, "o Alvanista recusou o pedido: {c}"),
        }
    }
}

/// Resposta do POST que cria o aparelho. O `token` só existe aqui.
#[derive(Clone, Deserialize)]
pub struct MintedDevice {
    pub id: String,
    pub token: String,
    pub label: String,
}

impl std::fmt::Debug for MintedDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MintedDevice")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("token", &"..")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RemoteDevice {
    pub id: String,
    pub label: String,
    pub client_kind: String,
    pub created_at: String,
    #[serde(default)]
    pub last_used_at: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<String>,
    #[serde(default)]
    pub paused_at: Option<String>,
    #[serde(default)]
    pub receive_platforms: Option<Vec<String>>,
    #[serde(default)]
    pub hardware: Option<HardwarePayload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Quota {
    pub used_bytes: i64,
    pub limit_bytes: i64,
}

#[derive(Deserialize)]
struct DeviceList {
    #[serde(default)]
    devices: Vec<RemoteDevice>,
}

/// Uma versão de um arquivo da nuvem (`VersionSummary` do GameKave).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RemoteVersion {
    pub version_id: String,
    /// SHA-256 (hex) dos bytes: o mesmo cálculo de `local::sha256_file`.
    pub content_hash: String,
    pub size_bytes: i64,
    /// RFC 3339.
    pub server_created_at: String,
    #[serde(default)]
    pub client_mtime: Option<String>,
    /// Quem mandou (`retroarch`, `web`, `reemu`...).
    #[serde(default)]
    pub source_client: Option<String>,
    /// Quem ESCREVEU esta versão. Serve só para o nome da máquina na tela.
    #[serde(default)]
    pub device_token_id: Option<String>,
    /// O Alvanista pode preencher o nome do aparelho.
    #[serde(default)]
    pub device_label: Option<String>,
}

/// Um arquivo da nuvem (`FileSummary` do GameKave), com os campos que o sync
/// usa.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RemoteFile {
    pub save_id: String,
    pub filename: String,
    pub kind: String,
    pub platform: String,
    #[serde(default)]
    pub core: Option<String>,
    /// `None` numa linha sem versão.
    #[serde(default)]
    pub current: Option<RemoteVersion>,
    #[serde(default)]
    pub version_count: i64,
    /// RFC 3339; presente numa linha apagada.
    #[serde(default)]
    pub deleted_at: Option<String>,
    /// O DONO da linha, `None` para save da conta. Nunca é quem escreveu a
    /// última versão (esse é `current.device_token_id`). O `device_label` que
    /// o Alvanista pode pôr ao lado é o nome desse dono, não de quem escreveu:
    /// o nome da máquina vem da versão.
    #[serde(default)]
    pub device_token_id: Option<String>,
}

/// Os filtros de `GET /api/v1/saves`.
#[derive(Debug, Clone, Default)]
pub struct FileFilter<'a> {
    pub kind: &'a str,
    pub filename: Option<&'a str>,
    pub platform: Option<&'a str>,
    /// Igualdade com `COALESCE(core, '')`: `Some("")` pede as linhas sem core.
    pub core: Option<&'a str>,
    pub device_token_id: Option<&'a str>,
    /// Inclui as linhas apagadas no site: sem elas, uma remoção pareceria
    /// "não existe" e o arquivo subiria de novo.
    pub include_deleted: bool,
}

/// Os campos do envio. O servidor acha o arquivo lógico por
/// `platform + kind + filename + core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadMeta<'a> {
    pub platform: &'a str,
    pub kind: &'a str,
    pub filename: &'a str,
    pub core: Option<&'a str>,
    pub core_version: Option<&'a str>,
    /// RFC 3339.
    pub client_mtime: Option<String>,
}

/// Resposta do envio (sempre 201, mesmo quando os bytes já existiam).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Uploaded {
    pub save_id: String,
    pub version_id: String,
    #[serde(default)]
    pub deduplicated: bool,
}

pub struct SavesApi {
    http: reqwest::Client,
    api: String,
}

impl SavesApi {
    pub fn new(api: String) -> Self {
        let builder = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("ReEmu/", env!("CARGO_PKG_VERSION")));
        // Os testes falam com um servidor local: sem proxy do ambiente.
        #[cfg(test)]
        let builder = builder.no_proxy();
        Self {
            http: builder
                .build()
                .expect("cliente HTTP do sync (reqwest) não pôde ser criado"),
            api: api.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn partner_mint_device(
        &self,
        partner_access: &str,
        label: &str,
        hw: &HardwarePayload,
    ) -> Result<MintedDevice, ApiError> {
        let req = self
            .http
            .post(format!("{}/api/partner/v1/save-devices", self.api))
            .bearer_auth(partner_access)
            .json(&serde_json::json!({
                "label": label,
                "client_kind": "reemu",
                "hardware": hw,
            }));
        read_json(req).await
    }

    pub async fn partner_list_devices(
        &self,
        partner_access: &str,
    ) -> Result<Vec<RemoteDevice>, ApiError> {
        let req = self
            .http
            .get(format!("{}/api/partner/v1/save-devices", self.api))
            .bearer_auth(partner_access);
        Ok(read_json::<DeviceList>(req).await?.devices)
    }

    pub async fn patch_own_device(
        &self,
        device_token: &str,
        label: Option<&str>,
        hw: Option<&HardwarePayload>,
    ) -> Result<RemoteDevice, ApiError> {
        let mut body = serde_json::Map::new();
        if let Some(label) = label {
            body.insert("label".into(), label.into());
        }
        if let Some(hw) = hw {
            body.insert(
                "hardware".into(),
                serde_json::to_value(hw).map_err(|e| ApiError::Rejected(e.to_string()))?,
            );
        }
        let req = self
            .http
            .patch(format!("{}/api/v1/saves/device", self.api))
            .bearer_auth(device_token)
            .json(&body);
        read_json(req).await
    }

    pub async fn revoke_own_device(&self, device_token: &str) -> Result<(), ApiError> {
        let req = self
            .http
            .delete(format!("{}/api/v1/saves/device", self.api))
            .bearer_auth(device_token);
        send(req).await.map(|_| ())
    }

    pub async fn quota(&self, device_token: &str) -> Result<Quota, ApiError> {
        let req = self
            .http
            .get(format!("{}/api/v1/saves/quota", self.api))
            .bearer_auth(device_token);
        read_json(req).await
    }

    /// `GET /api/v1/saves?...`: os arquivos da conta que passam no filtro.
    pub async fn list_files(
        &self,
        device_token: &str,
        f: &FileFilter<'_>,
    ) -> Result<Vec<RemoteFile>, ApiError> {
        let mut query: Vec<(&str, &str)> = vec![("kind", f.kind)];
        for (name, value) in [
            ("filename", f.filename),
            ("platform", f.platform),
            ("core", f.core),
            ("device_token_id", f.device_token_id),
        ] {
            if let Some(v) = value {
                query.push((name, v));
            }
        }
        if f.include_deleted {
            query.push(("include_deleted", "true"));
        }
        let req = self
            .http
            .get(format!("{}/api/v1/saves", self.api))
            .bearer_auth(device_token)
            .query(&query);
        read_json(req).await
    }

    /// `POST /api/v1/saves` (multipart): os campos de `meta` e a parte `file`.
    pub async fn upload(
        &self,
        device_token: &str,
        meta: &UploadMeta<'_>,
        bytes: Vec<u8>,
    ) -> Result<Uploaded, ApiError> {
        let file = reqwest::multipart::Part::bytes(bytes)
            .file_name(meta.filename.to_owned())
            .mime_str("application/octet-stream")
            .map_err(|e| ApiError::Rejected(e.without_url().to_string()))?;
        let mut form = reqwest::multipart::Form::new()
            .text("platform", meta.platform.to_owned())
            .text("kind", meta.kind.to_owned())
            .text("filename", meta.filename.to_owned());
        if let Some(core) = meta.core {
            form = form.text("core", core.to_owned());
        }
        if let Some(version) = meta.core_version {
            form = form.text("core_version", version.to_owned());
        }
        if let Some(mtime) = &meta.client_mtime {
            form = form.text("client_mtime", mtime.clone());
        }
        let req = self
            .http
            .post(format!("{}/api/v1/saves", self.api))
            .bearer_auth(device_token)
            .multipart(form.part("file", file));
        read_json(req).await
    }

    /// Os bytes de uma versão.
    pub async fn download(
        &self,
        device_token: &str,
        save_id: &str,
        version_id: &str,
    ) -> Result<Vec<u8>, ApiError> {
        let req = self
            .http
            .get(format!(
                "{}/api/v1/saves/{}/versions/{}/content",
                self.api,
                segment(save_id),
                segment(version_id)
            ))
            .bearer_auth(device_token);
        let resp = send(req).await?;
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ApiError::Transient(format!("download cortado: {}", e.without_url())))
    }

    /// As versões de um arquivo, da mais nova para a mais velha.
    pub async fn versions(
        &self,
        device_token: &str,
        save_id: &str,
    ) -> Result<Vec<RemoteVersion>, ApiError> {
        let req = self
            .http
            .get(format!(
                "{}/api/v1/saves/{}/versions",
                self.api,
                segment(save_id)
            ))
            .bearer_auth(device_token);
        read_json(req).await
    }
}

/// Um id do servidor como segmento de caminho: só o que um UUID tem passa;
/// o resto vira `_` (um id estranho nunca muda a rota pedida).
fn segment(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

async fn send(req: reqwest::RequestBuilder) -> Result<reqwest::Response, ApiError> {
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::Transient(e.without_url().to_string()))?;
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.bytes().await.unwrap_or_default();
    Err(classify(status.as_u16(), &body))
}

async fn read_json<T: serde::de::DeserializeOwned>(
    req: reqwest::RequestBuilder,
) -> Result<T, ApiError> {
    let resp = send(req).await?;
    resp.json::<T>()
        .await
        .map_err(|e| ApiError::Transient(format!("resposta ilegível: {}", e.without_url())))
}

/// O `code` do corpo de erro, nos dois formatos: `{"error":"x"}` do
/// middleware e `{"error":{"code":"x"}}` dos handlers.
fn error_code(body: &serde_json::Value) -> Option<String> {
    match body.get("error")? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(o) => o.get("code")?.as_str().map(str::to_owned),
        _ => None,
    }
}

fn classify(status: u16, body: &[u8]) -> ApiError {
    let json: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    let code = error_code(&json);
    match status {
        401 => ApiError::Unauthorized,
        402 => ApiError::TierInsufficient {
            upgrade_to: json
                .pointer("/error/upgrade_to")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned(),
        },
        403 if code.as_deref() == Some("insufficient_scope") => ApiError::InsufficientScope,
        404 => ApiError::NotFound,
        413 => ApiError::TooLarge,
        423 => ApiError::Paused,
        507 => ApiError::Quota,
        429 | 500..=599 => ApiError::Transient(format!("HTTP {status}")),
        _ => ApiError::Rejected(code.unwrap_or_else(|| format!("http_{status}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn hw() -> HardwarePayload {
        HardwarePayload {
            os: "Windows 11".into(),
            arch: "x86_64".into(),
            cpu: "CPU".into(),
            cpu_cores: 8,
            cpu_threads: 16,
            memory_mb: 16384,
            gpu: "GPU".into(),
            gpu_backend: "Vulkan".into(),
            gpu_driver: "1.0".into(),
            app_version: "0.1.0".into(),
            captured_at: "2026-11-13T10:00:00Z".into(),
        }
    }

    fn api(server: &MockServer) -> SavesApi {
        SavesApi::new(server.uri())
    }

    #[tokio::test]
    async fn mint_sends_bearer_body_and_user_agent_and_parses_the_token() {
        let server = MockServer::start().await;
        let expected = serde_json::json!({
            "label": "Sala",
            "client_kind": "reemu",
            "hardware": hw(),
        });
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .and(header("authorization", "Bearer alp_x"))
            .and(header(
                "user-agent",
                concat!("ReEmu/", env!("CARGO_PKG_VERSION")),
            ))
            .and(body_json(expected))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "id": "dev1", "label": "Sala", "client_kind": "reemu",
                "token": "asv_secret", "token_prefix": "asv_se",
                "created_at": "2026-11-13T10:00:00Z", "hardware": hw(),
            })))
            .expect(1)
            .mount(&server)
            .await;
        let got = api(&server)
            .partner_mint_device("alp_x", "Sala", &hw())
            .await
            .unwrap();
        assert_eq!(
            (got.id.as_str(), got.token.as_str()),
            ("dev1", "asv_secret")
        );
        assert!(!format!("{got:?}").contains("asv_"));
    }

    async fn status_of(code: u16, body: serde_json::Value) -> ApiError {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/quota"))
            .respond_with(ResponseTemplate::new(code).set_body_json(body))
            .mount(&server)
            .await;
        api(&server).quota("asv_t").await.unwrap_err()
    }

    #[tokio::test]
    async fn errors_are_read_from_both_body_shapes() {
        use serde_json::json;
        assert_eq!(
            status_of(401, json!({"error": "invalid_token"})).await,
            ApiError::Unauthorized
        );
        assert_eq!(
            status_of(403, json!({"error": "insufficient_scope"})).await,
            ApiError::InsufficientScope
        );
        assert_eq!(
            status_of(403, json!({"error": {"code": "insufficient_scope"}})).await,
            ApiError::InsufficientScope
        );
        assert_eq!(
            status_of(403, json!({"error": {"code": "device_token_required"}})).await,
            ApiError::Rejected("device_token_required".into())
        );
        assert_eq!(
            status_of(
                402,
                json!({"error": {"code": "tier_insufficient", "current_tier": "free",
                    "upgrade_to": "collector", "required_quota": "saves", "required_value": 1}})
            )
            .await,
            ApiError::TierInsufficient {
                upgrade_to: "collector".into()
            }
        );
        assert_eq!(status_of(423, json!({})).await, ApiError::Paused);
        assert_eq!(status_of(507, json!({})).await, ApiError::Quota);
        assert_eq!(status_of(413, json!({})).await, ApiError::TooLarge);
        assert_eq!(status_of(404, json!({})).await, ApiError::NotFound);
        assert!(matches!(
            status_of(429, json!({})).await,
            ApiError::Transient(_)
        ));
        assert!(matches!(
            status_of(503, json!({})).await,
            ApiError::Transient(_)
        ));
        assert_eq!(
            status_of(400, json!({"error": {"code": "bad_label"}})).await,
            ApiError::Rejected("bad_label".into())
        );
        assert_eq!(
            status_of(400, json!("not an object")).await,
            ApiError::Rejected("http_400".into())
        );
    }

    #[tokio::test]
    async fn a_server_that_hangs_up_is_transient() {
        // Aceita a conexão e a derruba: nada de porta "morta" que outro teste
        // paralelo poderia reaproveitar.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                drop(socket);
            }
        });
        assert!(matches!(
            SavesApi::new(url).quota("asv_t").await,
            Err(ApiError::Transient(_))
        ));
    }

    #[tokio::test]
    async fn list_devices_reads_optional_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .and(header("authorization", "Bearer alp_x"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "devices": [
                    {"id": "a", "label": "A", "client_kind": "reemu",
                     "token_prefix": "asv_aa", "created_at": "2026-11-01T00:00:00Z",
                     "receive_platforms": ["switch"], "hardware": {"os": "Linux"}},
                    {"id": "b", "label": "B", "client_kind": "web",
                     "token_prefix": "asv_bb", "created_at": "2026-11-02T00:00:00Z",
                     "last_used_at": "2026-11-03T00:00:00Z",
                     "revoked_at": "2026-11-04T00:00:00Z",
                     "paused_at": "2026-11-05T00:00:00Z"}
                ]
            })))
            .mount(&server)
            .await;
        let list = api(&server).partner_list_devices("alp_x").await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].receive_platforms, Some(vec!["switch".to_owned()]));
        assert_eq!(list[0].hardware.as_ref().unwrap().os, "Linux");
        assert_eq!(list[0].last_used_at, None);
        assert_eq!(list[1].revoked_at.as_deref(), Some("2026-11-04T00:00:00Z"));
        assert_eq!(list[1].paused_at.as_deref(), Some("2026-11-05T00:00:00Z"));
        assert!(list[1].hardware.is_none());
    }

    #[tokio::test]
    async fn list_files_sends_the_filters_and_reads_owner_writer_and_tombstones() {
        use wiremock::matchers::query_param;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves"))
            .and(header("authorization", "Bearer asv_t"))
            .and(query_param("kind", "file"))
            .and(query_param("filename", "Zelda.srm"))
            .and(query_param("core", ""))
            .and(query_param("include_deleted", "true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"save_id": "s1", "filename": "Zelda.srm", "slug": null, "kind": "file",
                 "platform": "gba", "game_uuid": null, "core": null, "core_version": null,
                 "visibility": "private",
                 "current": {"version_id": "v1", "content_hash": "abc", "size_bytes": 8,
                     "server_created_at": "2026-11-13T10:00:00Z", "client_mtime": null,
                     "last_seen_at": "2026-11-13T10:00:00Z", "source_client": "retroarch",
                     "origin_media": "upload", "device_token_id": "dev2"},
                 "version_count": 2, "deleted_at": null, "device_token_id": null},
                {"save_id": "s2", "filename": "Zelda.srm", "slug": null, "kind": "file",
                 "platform": "other", "game_uuid": null, "core": "", "core_version": null,
                 "visibility": "private", "current": null, "version_count": 0,
                 "deleted_at": "2026-11-13T11:00:00Z", "device_token_id": null}
            ])))
            .expect(1)
            .mount(&server)
            .await;
        let got = api(&server)
            .list_files(
                "asv_t",
                &FileFilter {
                    kind: "file",
                    filename: Some("Zelda.srm"),
                    core: Some(""),
                    include_deleted: true,
                    ..FileFilter::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(got.len(), 2);
        // O dono da linha e quem escreveu a versão são campos diferentes.
        assert_eq!(got[0].device_token_id, None);
        let current = got[0].current.as_ref().unwrap();
        assert_eq!(current.device_token_id.as_deref(), Some("dev2"));
        assert_eq!(current.content_hash, "abc");
        assert_eq!(got[0].version_count, 2);
        assert!(got[1].current.is_none());
        assert_eq!(got[1].deleted_at.as_deref(), Some("2026-11-13T11:00:00Z"));
        assert_eq!(got[1].core.as_deref(), Some(""));
    }

    /// O valor de um campo de um corpo multipart.
    fn form_field(body: &[u8], name: &str) -> Option<String> {
        let text = String::from_utf8_lossy(body);
        let at = text.find(&format!("name=\"{name}\""))?;
        let rest = &text[at..];
        let start = rest.find("\r\n\r\n")? + 4;
        let end = rest[start..].find("\r\n--")? + start;
        Some(rest[start..end].to_owned())
    }

    #[tokio::test]
    async fn upload_is_multipart_with_the_fields_and_the_file_part() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/saves"))
            .and(header("authorization", "Bearer asv_t"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "save_id": "s9", "version_id": "v2", "deduplicated": true
            })))
            .expect(1)
            .mount(&server)
            .await;
        let up = api(&server)
            .upload(
                "asv_t",
                &UploadMeta {
                    platform: "snes",
                    kind: "file",
                    filename: "Chrono.srm",
                    core: Some("Snes9x"),
                    core_version: None,
                    client_mtime: Some("2026-11-13T10:00:00Z".into()),
                },
                b"sram".to_vec(),
            )
            .await
            .unwrap();
        assert_eq!(
            up,
            Uploaded {
                save_id: "s9".into(),
                version_id: "v2".into(),
                deduplicated: true
            }
        );
        let req = &server.received_requests().await.unwrap()[0];
        let ctype = req.headers.get("content-type").unwrap().to_str().unwrap();
        assert!(ctype.starts_with("multipart/form-data"), "{ctype}");
        let body = &req.body;
        assert_eq!(form_field(body, "platform").as_deref(), Some("snes"));
        assert_eq!(form_field(body, "kind").as_deref(), Some("file"));
        assert_eq!(form_field(body, "filename").as_deref(), Some("Chrono.srm"));
        assert_eq!(form_field(body, "core").as_deref(), Some("Snes9x"));
        assert_eq!(form_field(body, "core_version"), None);
        assert_eq!(
            form_field(body, "client_mtime").as_deref(),
            Some("2026-11-13T10:00:00Z")
        );
        assert_eq!(form_field(body, "file").as_deref(), Some("sram"));
        assert!(String::from_utf8_lossy(body).contains("filename=\"Chrono.srm\""));
    }

    #[tokio::test]
    async fn download_and_versions_use_the_device_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/s1/versions/v1/content"))
            .and(header("authorization", "Bearer asv_t"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bytes".to_vec()))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/s1/versions"))
            .and(header("authorization", "Bearer asv_t"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"version_id": "v2", "content_hash": "h2", "size_bytes": 5,
                 "server_created_at": "2026-11-13T10:00:00Z", "client_mtime": null,
                 "last_seen_at": "2026-11-13T10:00:00Z", "source_client": "reemu",
                 "origin_media": "upload", "device_token_id": "dev1"},
                {"version_id": "v1", "content_hash": "h1", "size_bytes": 4,
                 "server_created_at": "2026-11-12T10:00:00Z", "client_mtime": null,
                 "last_seen_at": "2026-11-12T10:00:00Z", "source_client": "retroarch",
                 "origin_media": "upload", "device_token_id": null}
            ])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/s1/versions/gone/content"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let api = api(&server);
        assert_eq!(api.download("asv_t", "s1", "v1").await.unwrap(), b"bytes");
        let versions = api.versions("asv_t", "s1").await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].device_token_id.as_deref(), Some("dev1"));
        assert_eq!(versions[1].source_client.as_deref(), Some("retroarch"));
        assert_eq!(
            api.download("asv_t", "s1", "gone").await,
            Err(ApiError::NotFound)
        );
    }

    #[test]
    fn an_odd_id_never_changes_the_route() {
        assert_eq!(segment("6f1c-4e2a"), "6f1c-4e2a");
        assert_eq!(segment("../x y?z"), "___x_y_z");
    }

    #[tokio::test]
    async fn patch_revoke_and_quota_use_the_device_token() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_t"))
            .and(body_json(serde_json::json!({"hardware": hw()})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "d", "label": "L", "client_kind": "reemu",
                "token_prefix": "asv_t", "created_at": "2026-11-13T10:00:00Z"
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_t"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/quota"))
            .and(header("authorization", "Bearer asv_t"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "used_bytes": 10, "limit_bytes": 100, "file_versions": 3,
                "state_retention": 5
            })))
            .expect(1)
            .mount(&server)
            .await;
        let api = api(&server);
        assert_eq!(
            api.patch_own_device("asv_t", None, Some(&hw()))
                .await
                .unwrap()
                .id,
            "d"
        );
        api.revoke_own_device("asv_t").await.unwrap();
        assert_eq!(
            api.quota("asv_t").await.unwrap(),
            Quota {
                used_bytes: 10,
                limit_bytes: 100
            }
        );
    }
}
