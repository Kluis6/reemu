//! Erro que os comandos devolvem à interface: o texto técnico de sempre e, quando
//! a causa é conhecida, um código que a interface traduz (`lib/errors.ts`) sem
//! precisar adivinhar pelo texto. Sem código, a interface usa as regras de texto.

use domain::core_loader::CoreLoadError;
use emu_session::SessionError;
use serde::Serialize;
use std::io;

/// Cada código é uma regra de `errors.rules` em `lib/errors.ts`; o TypeScript
/// confere isso ao compilar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum ErrorCode {
    NoCore,
    CoreIncompatible,
    GpuRejected,
    NotFound,
    NotFoundRemote,
    Offline,
    RateLimited,
    DiskFull,
    NoPermission,
    Corrupt,
    SaveStateIncompatible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AppError {
    pub code: Option<ErrorCode>,
    pub message: String,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code: Some(code),
            message: message.into(),
        }
    }

    /// Erro de rede ou de HTTP; `context` vai na frente do texto técnico.
    pub fn from_http(context: &str, e: reqwest::Error) -> Self {
        let code = match e.status().map(|s| s.as_u16()) {
            Some(404) => Some(ErrorCode::NotFoundRemote),
            Some(429) => Some(ErrorCode::RateLimited),
            Some(_) => None,
            None if e.is_connect() || e.is_timeout() || e.is_request() || e.is_body() => {
                Some(ErrorCode::Offline)
            }
            None => None,
        };
        Self {
            code,
            message: format!("{context}: {e}"),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Causa ainda sem código: a interface segue as regras de texto.
impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self {
            code: None,
            message,
        }
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        let code = match e.kind() {
            io::ErrorKind::StorageFull => Some(ErrorCode::DiskFull),
            io::ErrorKind::PermissionDenied => Some(ErrorCode::NoPermission),
            io::ErrorKind::NotFound => Some(ErrorCode::NotFound),
            _ => None,
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

/// Erro do banco: sem código (a interface mostra o texto). Disco cheio e
/// permissão chegam como texto do SQLite, não como `io::Error`.
impl From<db::DbError> for AppError {
    fn from(e: db::DbError) -> Self {
        e.to_string().into()
    }
}

/// JSON de configuração ou de cache que não parseia.
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(ErrorCode::Corrupt, e.to_string())
    }
}

/// Rede sem contexto: o código vem do status/da causa (`from_http`).
impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::from_http("rede", e)
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        e.to_string().into()
    }
}

/// Repositório: `Corrupt` é dado que o domínio não interpreta (schema ou JSON).
impl From<domain::error::RepoError> for AppError {
    fn from(e: domain::error::RepoError) -> Self {
        let code = match &e {
            domain::error::RepoError::Corrupt(_) => Some(ErrorCode::Corrupt),
            domain::error::RepoError::Backend(_) => None,
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

impl From<library_scan::ScanError> for AppError {
    fn from(e: library_scan::ScanError) -> Self {
        match e {
            library_scan::ScanError::Repo(r) => r.into(),
        }
    }
}

impl From<crate::save_state::SaveError> for AppError {
    fn from(e: crate::save_state::SaveError) -> Self {
        use crate::save_state::SaveError;
        match e {
            SaveError::Io(io) => io.into(),
            SaveError::Repo(r) => r.into(),
            SaveError::NotFound => Self::new(ErrorCode::NotFound, e.to_string()),
            SaveError::CoreMismatch { .. } => {
                Self::new(ErrorCode::SaveStateIncompatible, e.to_string())
            }
            SaveError::NoCore => Self::new(ErrorCode::NoCore, e.to_string()),
        }
    }
}

impl From<SessionError> for AppError {
    fn from(e: SessionError) -> Self {
        // `LoadFailed` junta causas diferentes (core travou, ROM recusada, BIOS):
        // fica com as regras de texto.
        let code = match &e {
            SessionError::Load(CoreLoadError::NotFound(_)) => Some(ErrorCode::NoCore),
            SessionError::Load(CoreLoadError::RomNotFound(_)) => Some(ErrorCode::NotFound),
            SessionError::Load(CoreLoadError::IncompatiblePlatform(_)) => {
                Some(ErrorCode::CoreIncompatible)
            }
            SessionError::Load(CoreLoadError::HwRenderUnsupported(_)) => {
                Some(ErrorCode::GpuRejected)
            }
            SessionError::Load(CoreLoadError::LoadFailed(_)) | SessionError::ThreadDown => None,
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vai_para_a_interface_com_codigo_e_texto() {
        let e = AppError::new(ErrorCode::NoCore, "core não encontrado: x");
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"code":"noCore","message":"core não encontrado: x"}"#
        );
        let e = AppError::from("outra coisa");
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"code":null,"message":"outra coisa"}"#
        );
    }

    #[test]
    fn classifica_erro_do_sistema_pelo_tipo() {
        let code = |k| AppError::from(io::Error::from(k)).code;
        assert_eq!(code(io::ErrorKind::StorageFull), Some(ErrorCode::DiskFull));
        assert_eq!(
            code(io::ErrorKind::PermissionDenied),
            Some(ErrorCode::NoPermission)
        );
        assert_eq!(code(io::ErrorKind::NotFound), Some(ErrorCode::NotFound));
        assert_eq!(code(io::ErrorKind::Other), None);
    }

    #[test]
    fn classifica_erro_de_carregar_pelo_tipo() {
        let code = |e| AppError::from(SessionError::Load(e)).code;
        assert_eq!(
            code(CoreLoadError::NotFound("x".into())),
            Some(ErrorCode::NoCore)
        );
        assert_eq!(
            code(CoreLoadError::RomNotFound("x".into())),
            Some(ErrorCode::NotFound)
        );
        assert_eq!(
            code(CoreLoadError::IncompatiblePlatform("x".into())),
            Some(ErrorCode::CoreIncompatible)
        );
        assert_eq!(
            code(CoreLoadError::HwRenderUnsupported("x".into())),
            Some(ErrorCode::GpuRejected)
        );
        assert_eq!(code(CoreLoadError::LoadFailed("x".into())), None);
    }
}
