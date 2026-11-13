//! Diagnóstico gerado só sob pedido: junta o relatório do frontend ao fim do log,
//! sem a pasta do usuário nem segredos, e grava num arquivo local. Nada é enviado.

use crate::app_error::AppError;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Linhas finais do log que entram no relatório.
const LOG_LINES: usize = 300;

/// Nomes de campo cujo valor nunca deve sair no relatório.
const SECRET_FIELDS: [&str; 6] = ["password", "passwd", "apikey", "api_key", "token", "secret"];

/// Prefixos dos tokens do Alvanista: o do aparelho do cloud save e o par do
/// "Entrar com Alvanista" (acesso e renovação). Onde quer que apareçam, o que
/// vem depois do prefixo não sai no relatório.
const TOKEN_PREFIXES: [&str; 3] = ["asv_", "alp_", "alr_"];

/// Tira do texto a pasta do usuário, os valores de campos sensíveis
/// (`campo=valor`) e os tokens do Alvanista.
fn redact(text: &str, home: Option<&str>) -> String {
    mask_tokens(&redact_fields(text, home))
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// `asv_...`, `alp_...` e `alr_...` viram `asv_<oculto>` (o prefixo fica, para
/// o relatório ainda dizer de que token se tratava).
fn mask_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((at, len)) = TOKEN_PREFIXES
        .iter()
        .filter_map(|p| rest.find(p).map(|i| (i, p.len())))
        .min_by_key(|(i, _)| *i)
    {
        let after = &rest[at + len..];
        let end = after
            .find(|c: char| !is_token_char(c))
            .unwrap_or(after.len());
        out.push_str(&rest[..at + len]);
        if end > 0 {
            out.push_str("<oculto>");
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

fn redact_fields(text: &str, home: Option<&str>) -> String {
    let text = match home.filter(|h| !h.is_empty()) {
        Some(home) => text.replace(home, "<usuario>"),
        None => text.to_string(),
    };
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    'scan: while !rest.is_empty() {
        let lower = rest.to_ascii_lowercase();
        let hit = SECRET_FIELDS
            .iter()
            .filter_map(|f| lower.find(&format!("{f}=")).map(|i| (i, f.len() + 1)))
            .min_by_key(|(i, _)| *i);
        let Some((at, len)) = hit else {
            out.push_str(rest);
            break 'scan;
        };
        out.push_str(&rest[..at + len]);
        out.push_str("<oculto>");
        let after = &rest[at + len..];
        let end = after
            .find(|c: char| c.is_whitespace() || c == '&' || c == '"' || c == '\'')
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out
}

/// O log mais recente da pasta, por data de modificação.
fn newest_log(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(at, _)| *at)
        .map(|(_, path)| path)
}

fn log_tail(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    Some(lines[lines.len().saturating_sub(LOG_LINES)..].join("\n"))
}

/// Grava o relatório e devolve o caminho do arquivo.
#[tauri::command]
pub async fn export_diagnostics(app: AppHandle, report: String) -> Result<String, AppError> {
    let log = app
        .path()
        .app_log_dir()
        .ok()
        .and_then(|dir| newest_log(&dir))
        .and_then(|path| log_tail(&path))
        .unwrap_or_else(|| "(sem arquivo de log)".into());
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok();
    let body = redact(&format!("{report}\n\n## Log\n{log}\n"), home.as_deref());

    let dir = crate::data_dir(&app).join("diagnostics");
    std::fs::create_dir_all(&dir).map_err(AppError::from)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("reemu-diagnostico-{stamp}.txt"));
    std::fs::write(&path, body).map_err(AppError::from)?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tira_a_pasta_do_usuario() {
        let text = redact(r"abriu C:\Users\Ana\jogo.sfc", Some(r"C:\Users\Ana"));
        assert_eq!(text, r"abriu <usuario>\jogo.sfc");
    }

    #[test]
    fn tira_o_valor_de_campos_sensiveis() {
        let text = redact("url?ssid=x&sspassword=abc&devid=1 apikey=zzz fim", None);
        assert_eq!(
            text,
            "url?ssid=x&sspassword=<oculto>&devid=1 apikey=<oculto> fim"
        );
    }

    #[test]
    fn tira_os_tokens_do_alvanista() {
        let text = redact(
            "Bearer asv_AbC-12_x ok; {\"access_token\":\"alp_k9\",\"refresh_token\":\"alr_Z-1\"} fim alp_",
            None,
        );
        assert_eq!(
            text,
            "Bearer asv_<oculto> ok; {\"access_token\":\"alp_<oculto>\",\"refresh_token\":\"alr_<oculto>\"} fim alp_"
        );
        for token in ["asv_", "alp_", "alr_"] {
            let secret = format!("{token}segredo");
            assert!(!redact(&format!("x={secret}"), None).contains("segredo"));
            assert!(!redact(&format!("linha {secret}\n"), None).contains("segredo"));
        }
    }

    #[test]
    fn texto_comum_nao_muda() {
        assert_eq!(redact("perf core 1s: 60 fps", None), "perf core 1s: 60 fps");
    }
}
