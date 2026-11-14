//! O aparelho do cloud save: o que a máquina diz de si (hardware) e onde
//! ficam o token e os marcadores.
//!
//! O token `asv_...` mora só no chaveiro do SO (conta `alvanista-saves`).
//! No SQLite (`alvanista_state`) ficam o id, o rótulo e os marcadores, nunca
//! o token.

use crate::alvanista::store::SharedSecrets;
use crate::commands::hardware::HardwareInfo;
use db::AlvanistaRepo;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TOKEN_ACCOUNT: &str = "alvanista-saves";
const DEVICE_ID_KEY: &str = "sync.device_id";
const LABEL_KEY: &str = "sync.label";
const ENABLED_KEY: &str = "sync.enabled";
const PAUSED_KEY: &str = "sync.paused";
/// O interruptor dos game saves: ausente vale ligado (só o `0` desliga).
const SAVES_ENABLED_KEY: &str = "sync.saves_enabled";
/// O interruptor dos save states: ausente vale ligado (só o `0` desliga).
const STATES_ENABLED_KEY: &str = "sync.states_enabled";
const HW_FINGERPRINT_KEY: &str = "sync.hw_fingerprint";
const HW_SENT_AT_KEY: &str = "sync.hw_sent_at";
/// Os últimos GPU, backend e driver enviados (JSON), para não apagá-los
/// quando a GPU não abre.
const HW_GPU_KEY: &str = "sync.hw_gpu";
/// O que a lista de aparelhos do site disse da última vez (D128): serve
/// quando a lista não responde.
const SITE_PAUSED_KEY: &str = "sync.site_paused";
const REVOKED_KEY: &str = "sync.revoked";
/// A conta (AlvaID) dona da base do cloud save (`sync_base`, `sync_conflicts`).
/// Fica ao desconectar, como a base; só outra conta a troca (e limpa a base).
const ACCOUNT_KEY: &str = "sync.account";
/// Quando "Enviar as desta máquina" mandou as configurações (segundos Unix).
const SETTINGS_SENT_AT_KEY: &str = "sync.settings_sent_at";
/// O hash do documento de cada grupo enviado (JSON `{grupo: sha256}`): o que
/// diz se um grupo mudou desde o último envio.
const SETTINGS_SENT_KEY: &str = "sync.settings_sent";
/// O backup da última cópia de configurações (JSON com o arquivo de cada
/// grupo), que o "Desfazer" volta. É desta máquina, não do aparelho: fica ao
/// desligar o sync. Gravado e apagado na transação da cópia e do "Desfazer".
pub(super) const LAST_COPY_BACKUP_KEY: &str = "sync.last_copy_backup";
/// O backup do estado de antes do último "Desfazer" (R39): o registro dos
/// arquivos, que nada oferece de volta.
pub(super) const LAST_UNDO_BACKUP_KEY: &str = "sync.last_undo_backup";

/// O limite do Alvanista para cada texto do hardware.
const TEXT_MAX_CHARS: usize = 120;
/// O limite do Alvanista para o rótulo do aparelho (acima disso, 400).
pub const LABEL_MAX_CHARS: usize = 80;

/// O corpo `hardware` das rotas de aparelho. Campo ausente na resposta vira
/// vazio: o servidor omite o que não tem.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct HardwarePayload {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub memory_mb: i64,
    pub gpu: String,
    pub gpu_backend: String,
    pub gpu_driver: String,
    pub app_version: String,
    /// RFC 3339, UTC.
    pub captured_at: String,
}

/// Caracteres de formatação Unicode (categoria Cf), que o Alvanista recusa.
fn is_format_char(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{0600}'..='\u{0605}' | '\u{061C}' | '\u{06DD}' | '\u{070F}'
        | '\u{08E2}' | '\u{180E}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}' | '\u{FEFF}'
        | '\u{FFF9}'..='\u{FFFB}' | '\u{110BD}' | '\u{1D173}'..='\u{1D17A}'
        | '\u{E0001}' | '\u{E0020}'..='\u{E007F}')
}

/// Texto que o servidor aceita: sem caracteres de controle nem de formatação,
/// sem espaços nas pontas, com no máximo 120 caracteres. Um valor fora disso
/// leva 400 e travaria a criação do aparelho para sempre.
pub fn clean_text(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|c| !c.is_control() && !is_format_char(*c))
        .collect();
    let clipped: String = kept.trim().chars().take(TEXT_MAX_CHARS).collect();
    clipped.trim_end().to_owned()
}

/// Converte o que o app já sabe da máquina: o que não se descobriu vira
/// texto vazio, a memória vai em MiB. `now` em segundos Unix.
pub fn hardware_payload(hw: &HardwareInfo, app_version: &str, now: i64) -> HardwarePayload {
    HardwarePayload {
        os: clean_text(&hw.os.clone().unwrap_or_default()),
        arch: clean_text(&hw.arch),
        cpu: clean_text(&hw.cpu.clone().unwrap_or_default()),
        cpu_cores: hw.cpu_cores.map_or(0, |n| n as u32),
        cpu_threads: hw.cpu_threads as u32,
        memory_mb: (hw.memory_bytes / 1_048_576) as i64,
        gpu: clean_text(&hw.gpu.clone().unwrap_or_default()),
        gpu_backend: clean_text(&hw.gpu_backend.clone().unwrap_or_default()),
        gpu_driver: clean_text(&hw.gpu_driver.clone().unwrap_or_default()),
        app_version: clean_text(app_version),
        captured_at: rfc3339(now),
    }
}

/// SHA-256 (hex) de tudo, menos `captured_at` e `app_version`: muda quando a
/// máquina muda, não quando o relógio anda nem o app atualiza.
pub fn hardware_fingerprint(hw: &HardwarePayload) -> String {
    let mut h = Sha256::new();
    let parts = [
        hw.os.as_str(),
        hw.arch.as_str(),
        hw.cpu.as_str(),
        &hw.cpu_cores.to_string(),
        &hw.cpu_threads.to_string(),
        &hw.memory_mb.to_string(),
        hw.gpu.as_str(),
        hw.gpu_backend.as_str(),
        hw.gpu_driver.as_str(),
    ];
    for part in parts {
        h.update(part.as_bytes());
        // Separador: ("ab", "c") e ("a", "bc") não podem dar o mesmo hash.
        h.update([0u8]);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// O rótulo como o servidor aceita: `clean_text`, cortado em 80 caracteres.
pub fn clean_label(s: &str) -> String {
    let clipped: String = clean_text(s).chars().take(LABEL_MAX_CHARS).collect();
    clipped.trim_end().to_owned()
}

/// O nome da máquina; sem ele, o rótulo padrão do Alvanista.
pub fn default_label() -> String {
    label_from_host(sysinfo::System::host_name())
}

fn label_from_host(host: Option<String>) -> String {
    host.map(|h| clean_label(&h))
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| crate::alvanista::config::device_label().to_owned())
}

/// Segundos Unix em RFC 3339 UTC (`2026-11-13T10:00:00Z`).
pub(super) fn rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Dias desde 1970-01-01 em data civil (Howard Hinnant, `civil_from_days`).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub label: String,
}

#[derive(Clone)]
pub struct DeviceStore {
    repo: AlvanistaRepo,
    secrets: SharedSecrets,
    db: db::Db,
}

impl DeviceStore {
    pub fn new(db: db::Db, secrets: SharedSecrets) -> Self {
        Self {
            repo: AlvanistaRepo::new(db.clone()),
            secrets,
            db,
        }
    }

    /// O banco, para a base do cloud save (`SyncRepo`) no mesmo lugar.
    pub(super) fn db(&self) -> db::Db {
        self.db.clone()
    }

    /// A conta dona da base do cloud save; `None` antes do primeiro sync.
    pub async fn base_account(&self) -> Result<Option<String>, String> {
        self.get(ACCOUNT_KEY).await
    }

    pub async fn set_base_account(&self, alva_id: &str) -> Result<(), String> {
        self.set(ACCOUNT_KEY, alva_id).await
    }

    async fn get(&self, key: &str) -> Result<Option<String>, String> {
        self.repo.state_get(key).await.map_err(|e| e.to_string())
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.repo
            .state_set(key, value)
            .await
            .map_err(|e| e.to_string())
    }

    /// O aparelho guardado, só pelo SQLite (não toca o chaveiro).
    pub async fn device(&self) -> Result<Option<Device>, String> {
        let Some(id) = self.get(DEVICE_ID_KEY).await? else {
            return Ok(None);
        };
        let label = self.get(LABEL_KEY).await?.unwrap_or_default();
        Ok(Some(Device { id, label }))
    }

    /// O token `asv_...`. Erro de leitura do chaveiro é erro, não "sem token".
    pub fn token(&self) -> Result<Option<String>, String> {
        self.secrets.get(TOKEN_ACCOUNT)
    }

    /// Guarda o aparelho: o token primeiro (se o chaveiro recusar, nada
    /// aponta para um aparelho sem token), depois id e rótulo.
    pub async fn save(&self, d: &Device, token: &str) -> Result<(), String> {
        self.secrets.set(TOKEN_ACCOUNT, token)?;
        self.set(DEVICE_ID_KEY, &d.id).await?;
        self.set(LABEL_KEY, &d.label).await
    }

    /// Esquece o aparelho: tenta o SQLite e o chaveiro, mesmo que um falhe
    /// (o token não pode ficar no chaveiro por causa de um erro do banco), e
    /// devolve o primeiro erro.
    pub async fn forget(&self) -> Result<(), String> {
        let mut first: Option<String> = None;
        for key in [
            DEVICE_ID_KEY,
            LABEL_KEY,
            ENABLED_KEY,
            PAUSED_KEY,
            SAVES_ENABLED_KEY,
            STATES_ENABLED_KEY,
            HW_FINGERPRINT_KEY,
            HW_SENT_AT_KEY,
            HW_GPU_KEY,
            SITE_PAUSED_KEY,
            REVOKED_KEY,
            // as linhas de configuração eram do aparelho que sai
            SETTINGS_SENT_AT_KEY,
            SETTINGS_SENT_KEY,
        ] {
            if let Err(e) = self.repo.state_delete(key).await {
                first.get_or_insert(e.to_string());
            }
        }
        if let Err(e) = self.secrets.delete(TOKEN_ACCOUNT) {
            first.get_or_insert(e);
        }
        first.map_or(Ok(()), Err)
    }

    /// Tira só o token do chaveiro: o id e o nome ficam para "Ligar de novo"
    /// criar o aparelho novo com o nome que a pessoa escolheu.
    pub fn drop_token(&self) -> Result<(), String> {
        self.secrets.delete(TOKEN_ACCOUNT)
    }

    pub async fn set_label(&self, label: &str) -> Result<(), String> {
        self.set(LABEL_KEY, label).await
    }

    pub async fn enabled(&self) -> Result<bool, String> {
        Ok(self.get(ENABLED_KEY).await?.as_deref() == Some("1"))
    }

    pub async fn set_enabled(&self, on: bool) -> Result<(), String> {
        self.set_flag(ENABLED_KEY, on).await
    }

    pub async fn paused(&self) -> Result<bool, String> {
        Ok(self.get(PAUSED_KEY).await?.as_deref() == Some("1"))
    }

    pub async fn set_paused(&self, on: bool) -> Result<(), String> {
        self.set_flag(PAUSED_KEY, on).await
    }

    /// Os game saves sincronizam? Ausente vale sim.
    pub async fn saves_enabled(&self) -> Result<bool, String> {
        Ok(self.get(SAVES_ENABLED_KEY).await?.as_deref() != Some("0"))
    }

    pub async fn set_saves_enabled(&self, on: bool) -> Result<(), String> {
        self.set_default_on(SAVES_ENABLED_KEY, on).await
    }

    /// Os save states sobem? Ausente vale sim.
    pub async fn states_enabled(&self) -> Result<bool, String> {
        Ok(self.get(STATES_ENABLED_KEY).await?.as_deref() != Some("0"))
    }

    pub async fn set_states_enabled(&self, on: bool) -> Result<(), String> {
        self.set_default_on(STATES_ENABLED_KEY, on).await
    }

    /// Um interruptor que vale ligado quando ausente: ligar apaga a chave,
    /// desligar grava `0`.
    async fn set_default_on(&self, key: &str, on: bool) -> Result<(), String> {
        if on {
            self.repo.state_delete(key).await.map_err(|e| e.to_string())
        } else {
            self.set(key, "0").await
        }
    }

    async fn set_flag(&self, key: &str, on: bool) -> Result<(), String> {
        if on {
            self.set(key, "1").await
        } else {
            self.repo.state_delete(key).await.map_err(|e| e.to_string())
        }
    }

    /// O que o site disse por último: `(pausado no site, revogado)`.
    pub async fn site_state(&self) -> Result<(bool, bool), String> {
        let paused = self.get(SITE_PAUSED_KEY).await?.as_deref() == Some("1");
        let revoked = self.get(REVOKED_KEY).await?.as_deref() == Some("1");
        Ok((paused, revoked))
    }

    pub async fn set_site_state(&self, paused: bool, revoked: bool) -> Result<(), String> {
        self.set_flag(SITE_PAUSED_KEY, paused).await?;
        self.set_flag(REVOKED_KEY, revoked).await
    }

    /// Os últimos `[gpu, backend, driver]` enviados.
    pub async fn hw_gpu(&self) -> Result<Option<[String; 3]>, String> {
        Ok(self
            .get(HW_GPU_KEY)
            .await?
            .and_then(|v| serde_json::from_str(&v).ok()))
    }

    pub async fn set_hw_gpu(&self, gpu: &[String; 3]) -> Result<(), String> {
        let json = serde_json::to_string(gpu).map_err(|e| e.to_string())?;
        self.set(HW_GPU_KEY, &json).await
    }

    pub async fn hw_fingerprint(&self) -> Result<Option<String>, String> {
        self.get(HW_FINGERPRINT_KEY).await
    }

    pub async fn set_hw_fingerprint(&self, fp: &str) -> Result<(), String> {
        self.set(HW_FINGERPRINT_KEY, fp).await
    }

    /// Quando o hardware foi enviado pela última vez (segundos Unix).
    pub async fn hw_sent_at(&self) -> Result<Option<i64>, String> {
        Ok(self.get(HW_SENT_AT_KEY).await?.and_then(|v| v.parse().ok()))
    }

    pub async fn set_hw_sent_at(&self, secs: i64) -> Result<(), String> {
        self.set(HW_SENT_AT_KEY, &secs.to_string()).await
    }

    /// O último envio das configurações: quando, e o hash de cada grupo que
    /// subiu.
    pub async fn settings_sent(
        &self,
    ) -> Result<(Option<i64>, std::collections::BTreeMap<String, String>), String> {
        let at = self
            .get(SETTINGS_SENT_AT_KEY)
            .await?
            .and_then(|v| v.parse().ok());
        let hashes = self
            .get(SETTINGS_SENT_KEY)
            .await?
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        Ok((at, hashes))
    }

    pub async fn set_settings_sent(
        &self,
        at: i64,
        hashes: &std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        let json = serde_json::to_string(hashes).map_err(|e| e.to_string())?;
        self.set(SETTINGS_SENT_KEY, &json).await?;
        self.set(SETTINGS_SENT_AT_KEY, &at.to_string()).await
    }

    /// O backup da última cópia de configurações, como a cópia o gravou.
    pub async fn last_copy_backup(&self) -> Result<Option<String>, String> {
        self.get(LAST_COPY_BACKUP_KEY).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::store::test_support::MemSecrets;
    use crate::sync::test_support::all_state_values;
    use std::sync::Arc;

    fn info() -> HardwareInfo {
        HardwareInfo {
            os: Some("Windows 11 Pro 26300".into()),
            arch: "x86_64".into(),
            cpu: Some("Ryzen".into()),
            cpu_cores: Some(8),
            cpu_threads: 16,
            memory_bytes: 16 * 1_073_741_824,
            gpu: Some("RTX".into()),
            gpu_backend: Some("Vulkan".into()),
            gpu_driver: Some("555 NVIDIA".into()),
        }
    }

    #[test]
    fn hardware_payload_converts_units_and_blanks_unknowns() {
        let p = hardware_payload(&info(), "0.4.0", 1_794_564_000);
        assert_eq!(p.memory_mb, 16_384);
        assert_eq!((p.cpu_cores, p.cpu_threads), (8, 16));
        assert_eq!(p.app_version, "0.4.0");
        assert_eq!(p.captured_at, "2026-11-13T10:00:00Z");
        let blank = hardware_payload(
            &HardwareInfo {
                os: None,
                arch: "aarch64".into(),
                cpu: None,
                cpu_cores: None,
                cpu_threads: 4,
                memory_bytes: 0,
                gpu: None,
                gpu_backend: None,
                gpu_driver: None,
            },
            "0.4.0",
            0,
        );
        assert_eq!(
            (
                blank.os.as_str(),
                blank.cpu.as_str(),
                blank.gpu.as_str(),
                blank.gpu_backend.as_str(),
                blank.gpu_driver.as_str()
            ),
            ("", "", "", "", "")
        );
        assert_eq!((blank.cpu_cores, blank.memory_mb), (0, 0));
        assert_eq!(blank.captured_at, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn rfc3339_handles_leap_days_and_the_far_side_of_the_year() {
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_709_251_199), "2024-02-29T23:59:59Z");
        assert_eq!(rfc3339(1_735_689_599), "2024-12-31T23:59:59Z");
    }

    #[test]
    fn hardware_fingerprint_ignores_capture_time_and_version() {
        let a = hardware_payload(&info(), "0.4.0", 1_000);
        let b = hardware_payload(&info(), "0.5.0", 999_999);
        assert_eq!(hardware_fingerprint(&a), hardware_fingerprint(&b));
        assert_eq!(hardware_fingerprint(&a).len(), 64);
        let mut other = a.clone();
        other.gpu_driver = "556 NVIDIA".into();
        assert_ne!(hardware_fingerprint(&a), hardware_fingerprint(&other));
        // O separador impede que campos vizinhos se confundam.
        let (mut x, mut y) = (a.clone(), a.clone());
        (x.os, x.arch) = ("ab".into(), "c".into());
        (y.os, y.arch) = ("a".into(), "bc".into());
        assert_ne!(hardware_fingerprint(&x), hardware_fingerprint(&y));
    }

    #[test]
    fn hardware_payload_clamps_and_cleans_every_text() {
        let long = "x".repeat(300);
        let mut i = info();
        i.cpu = Some(format!("  Ry\u{200B}zen\u{0007} {long}"));
        i.gpu = Some("\u{202E}RTX\n".into());
        i.os = Some("\u{FEFF}".into());
        let p = hardware_payload(&i, "0.4.0\u{0000}", 0);
        assert_eq!(p.cpu.chars().count(), 120);
        assert!(p.cpu.starts_with("Ryzen xxx"));
        assert_eq!(p.gpu, "RTX");
        assert_eq!(p.os, "");
        assert_eq!(p.app_version, "0.4.0");
        // O corte é por caractere, não por byte, e emoji comum fica.
        assert_eq!(clean_text(&"é".repeat(200)).chars().count(), 120);
        assert_eq!(clean_text("Sala \u{1F3AE}"), "Sala \u{1F3AE}");
    }

    #[test]
    fn the_default_label_is_never_empty() {
        assert!(!default_label().trim().is_empty());
    }

    #[test]
    fn the_label_is_clipped_to_the_server_limit_and_hardware_text_is_not() {
        let long = format!("PC-{}", "x".repeat(150));
        assert_eq!(label_from_host(Some(long.clone())).chars().count(), 80);
        assert_eq!(clean_label(&"é".repeat(200)).chars().count(), 80);
        // O corte não deixa espaço no fim.
        assert_eq!(
            clean_label(&format!("{} y", "a".repeat(79))),
            "a".repeat(79)
        );
        assert_eq!(clean_text(&long).chars().count(), 120);
        assert_eq!(
            label_from_host(Some("\u{200B} ".into())),
            crate::alvanista::config::device_label()
        );
    }

    async fn store(secrets: MemSecrets) -> (DeviceStore, Arc<MemSecrets>, db::Db) {
        let secrets = Arc::new(secrets);
        let db = db::connect_in_memory().await.unwrap();
        (DeviceStore::new(db.clone(), secrets.clone()), secrets, db)
    }

    #[tokio::test]
    async fn device_store_keeps_token_only_in_keyring() {
        let (store, secrets, db) = store(MemSecrets::default()).await;
        assert_eq!(store.device().await.unwrap(), None);
        let d = Device {
            id: "dev1".into(),
            label: "Sala".into(),
        };
        store.save(&d, "asv_secret").await.unwrap();
        assert_eq!(store.device().await.unwrap(), Some(d));
        assert_eq!(store.token().unwrap().as_deref(), Some("asv_secret"));
        assert_eq!(
            secrets.secrets.lock().unwrap().get("alvanista-saves"),
            Some(&"asv_secret".to_owned())
        );
        let repo = AlvanistaRepo::new(db.clone());
        assert_eq!(
            repo.state_get("sync.device_id").await.unwrap().as_deref(),
            Some("dev1")
        );
        assert_eq!(
            repo.state_get("sync.label").await.unwrap().as_deref(),
            Some("Sala")
        );
        for v in all_state_values(&db).await {
            assert!(!v.contains("asv_"), "o token vazou para o SQLite: {v}");
        }
    }

    #[tokio::test]
    async fn a_refusing_keyring_leaves_no_device_behind() {
        let (store, _, _) = store(MemSecrets {
            readonly: true.into(),
            ..Default::default()
        })
        .await;
        let d = Device {
            id: "dev1".into(),
            label: "Sala".into(),
        };
        assert!(store.save(&d, "asv_secret").await.is_err());
        assert_eq!(store.device().await.unwrap(), None);
    }

    #[tokio::test]
    async fn forget_still_clears_the_keyring_when_the_database_fails() {
        let (store, secrets, db) = store(MemSecrets::default()).await;
        let d = Device {
            id: "dev1".into(),
            label: "Sala".into(),
        };
        store.save(&d, "asv_secret").await.unwrap();
        db.close().await;
        assert!(store.forget().await.is_err());
        assert!(secrets.secrets.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_label_and_the_last_gpu_round_trip() {
        let (store, _, _) = store(MemSecrets::default()).await;
        store
            .save(
                &Device {
                    id: "dev1".into(),
                    label: "Sala".into(),
                },
                "asv_secret",
            )
            .await
            .unwrap();
        store.set_label("Quarto").await.unwrap();
        assert_eq!(store.device().await.unwrap().unwrap().label, "Quarto");
        assert_eq!(store.hw_gpu().await.unwrap(), None);
        let gpu = ["RTX".to_owned(), "Vulkan".to_owned(), "555".to_owned()];
        store.set_hw_gpu(&gpu).await.unwrap();
        assert_eq!(store.hw_gpu().await.unwrap(), Some(gpu));
        store.forget().await.unwrap();
        assert_eq!(store.hw_gpu().await.unwrap(), None);
    }

    #[tokio::test]
    async fn the_saves_switch_defaults_on_and_forget_puts_it_back() {
        let (store, _, _) = store(MemSecrets::default()).await;
        assert!(store.saves_enabled().await.unwrap());
        store.set_saves_enabled(false).await.unwrap();
        assert!(!store.saves_enabled().await.unwrap());
        store.set_saves_enabled(true).await.unwrap();
        assert!(store.saves_enabled().await.unwrap());
        store.set_saves_enabled(false).await.unwrap();
        store.forget().await.unwrap();
        assert!(store.saves_enabled().await.unwrap());
    }

    #[tokio::test]
    async fn the_states_switch_defaults_on_is_its_own_key_and_forget_puts_it_back() {
        let (store, _, db) = store(MemSecrets::default()).await;
        assert!(store.states_enabled().await.unwrap());
        store.set_states_enabled(false).await.unwrap();
        assert!(!store.states_enabled().await.unwrap());
        // Um interruptor não mexe no outro.
        assert!(store.saves_enabled().await.unwrap());
        assert_eq!(
            AlvanistaRepo::new(db.clone())
                .state_get("sync.states_enabled")
                .await
                .unwrap()
                .as_deref(),
            Some("0")
        );
        store.set_states_enabled(true).await.unwrap();
        assert!(store.states_enabled().await.unwrap());
        assert!(all_state_values(&db)
            .await
            .iter()
            .all(|v| v != "sync.states_enabled"));
        store.set_states_enabled(false).await.unwrap();
        store.forget().await.unwrap();
        assert!(store.states_enabled().await.unwrap());
    }

    #[tokio::test]
    async fn forget_clears_the_keyring_and_every_marker() {
        let (store, secrets, db) = store(MemSecrets::default()).await;
        let d = Device {
            id: "dev1".into(),
            label: "Sala".into(),
        };
        store.save(&d, "asv_secret").await.unwrap();
        store.set_enabled(true).await.unwrap();
        store.set_paused(true).await.unwrap();
        store.set_hw_fingerprint("abc").await.unwrap();
        store.set_hw_sent_at(42).await.unwrap();
        assert!(store.enabled().await.unwrap() && store.paused().await.unwrap());
        assert_eq!(store.hw_sent_at().await.unwrap(), Some(42));
        assert_eq!(
            store.hw_fingerprint().await.unwrap().as_deref(),
            Some("abc")
        );
        store.forget().await.unwrap();
        assert_eq!(store.device().await.unwrap(), None);
        assert_eq!(store.token().unwrap(), None);
        assert!(secrets.secrets.lock().unwrap().is_empty());
        assert!(!store.enabled().await.unwrap() && !store.paused().await.unwrap());
        assert_eq!(store.hw_sent_at().await.unwrap(), None);
        assert!(all_state_values(&db)
            .await
            .iter()
            .all(|v| !v.starts_with("sync.")));
    }
}
