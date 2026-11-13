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
const HW_FINGERPRINT_KEY: &str = "sync.hw_fingerprint";
const HW_SENT_AT_KEY: &str = "sync.hw_sent_at";

/// O corpo `hardware` das rotas de aparelho. Campo ausente na resposta vira
/// vazio: o servidor omite o que não tem.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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

/// Converte o que o app já sabe da máquina: o que não se descobriu vira
/// texto vazio, a memória vai em MiB. `now` em segundos Unix.
pub fn hardware_payload(hw: &HardwareInfo, app_version: &str, now: i64) -> HardwarePayload {
    HardwarePayload {
        os: hw.os.clone().unwrap_or_default(),
        arch: hw.arch.clone(),
        cpu: hw.cpu.clone().unwrap_or_default(),
        cpu_cores: hw.cpu_cores.map_or(0, |n| n as u32),
        cpu_threads: hw.cpu_threads as u32,
        memory_mb: (hw.memory_bytes / 1_048_576) as i64,
        gpu: hw.gpu.clone().unwrap_or_default(),
        gpu_backend: hw.gpu_backend.clone().unwrap_or_default(),
        gpu_driver: hw.gpu_driver.clone().unwrap_or_default(),
        app_version: app_version.to_owned(),
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

/// O nome da máquina; sem ele, o rótulo padrão do Alvanista.
pub fn default_label() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| crate::alvanista::config::device_label().to_owned())
}

/// Segundos Unix em RFC 3339 UTC (`2026-11-13T10:00:00Z`).
fn rfc3339(secs: i64) -> String {
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

pub struct DeviceStore {
    repo: AlvanistaRepo,
    secrets: SharedSecrets,
}

impl DeviceStore {
    pub fn new(db: db::Db, secrets: SharedSecrets) -> Self {
        Self {
            repo: AlvanistaRepo::new(db),
            secrets,
        }
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

    /// Esquece o aparelho: limpa o SQLite e depois o chaveiro. Se o chaveiro
    /// recusar apagar, o erro volta, mas os marcadores já saíram.
    pub async fn forget(&self) -> Result<(), String> {
        for key in [
            DEVICE_ID_KEY,
            LABEL_KEY,
            ENABLED_KEY,
            PAUSED_KEY,
            HW_FINGERPRINT_KEY,
            HW_SENT_AT_KEY,
        ] {
            self.repo
                .state_delete(key)
                .await
                .map_err(|e| e.to_string())?;
        }
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

    async fn set_flag(&self, key: &str, on: bool) -> Result<(), String> {
        if on {
            self.set(key, "1").await
        } else {
            self.repo.state_delete(key).await.map_err(|e| e.to_string())
        }
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
    fn the_default_label_is_never_empty() {
        assert!(!default_label().trim().is_empty());
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
