//! Versão de um pacote baixado de um repositório do GitHub (molduras do The
//! Bezel Project, pacote `slang-shaders`): o commit que veio no download e a
//! comparação com o último commit do `master`, pra avisar que há versão nova.
//! A API do GitHub sem login aceita 60 consultas por hora por IP: quem usa
//! guarda o resultado e consulta de tempos em tempos.

use std::path::Path;

/// Último commit do `master` de um repositório do GitHub.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Commit {
    pub sha: String,
    #[serde(default)]
    /// ISO 8601 (data do commit), pra comparar com pacotes baixados antes de o
    /// ReEmu registrar o SHA.
    pub date: String,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// O HEAD na resposta do `ls-refs` (pkt-line: `0032<oid> HEAD\n0000`).
pub fn parse_ls_refs(body: &str) -> Option<String> {
    let oid = body.get(4..)?.split_whitespace().next()?;
    (oid.len() == 40 && oid.chars().all(|c| c.is_ascii_hexdigit())).then(|| oid.to_string())
}

/// `ls-refs` do protocolo Git v2 (gitprotocol-v2 e gitprotocol-http): pede só o
/// HEAD, ~50 bytes de resposta mesmo num repositório como o do MAME. Não passa
/// pela API REST, então não gasta o limite de 60 consultas por hora. `repo` =
/// `dono/nome`.
pub async fn remote_head(repo: &str) -> Option<String> {
    let body = crate::http::client()
        .post(format!("https://github.com/{repo}.git/git-upload-pack"))
        .header("Content-Type", "application/x-git-upload-pack-request")
        .header("Git-Protocol", "version=2")
        .body("0014command=ls-refs\n00010014ref-prefix HEAD\n0000")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    parse_ls_refs(&body)
}

/// O commit atual do `master` pelo `ls-refs` (sem a data, que só a API traz).
pub async fn head_commit(owner: &str, repo: &str) -> Option<Commit> {
    remote_head(&format!("{owner}/{repo}"))
        .await
        .map(|sha| Commit {
            sha,
            date: String::new(),
        })
}

/// O "último commit" a guardar depois de consultar o repositório, ou `None`
/// se a consulta falhou (fica o guardado). O HEAD vem pelo `ls-refs`, sem
/// gastar a API. A API REST só entra pro pacote baixado antes de o ReEmu
/// registrar o commit (`installed` vazio), que precisa da data do commit, e só
/// quando o HEAD mudou desde a última consulta.
pub async fn refresh_latest(
    owner: &str,
    repo: &str,
    installed: Option<&Commit>,
    cached: Option<&Commit>,
) -> Option<Commit> {
    let head = remote_head(&format!("{owner}/{repo}")).await?;
    if let Some(c) = cached.filter(|c| c.sha == head && (installed.is_some() || !c.date.is_empty()))
    {
        return Some(c.clone());
    }
    if installed.is_some() {
        return Some(Commit {
            sha: head,
            date: String::new(),
        });
    }
    match latest_commit(owner, repo).await {
        Ok(c) => Some(c),
        Err(e) => {
            log::warn!("versão: data do commit de {owner}/{repo} não consultada: {e}");
            None
        }
    }
}

/// Último commit pela API REST (com a data). Gasta o limite de 60 por hora:
/// só pro caso de [`refresh_latest`] que precisa da data.
pub async fn latest_commit(owner: &str, repo: &str) -> Result<Commit, String> {
    #[derive(serde::Deserialize)]
    struct Api {
        sha: String,
        commit: ApiCommit,
    }
    #[derive(serde::Deserialize)]
    struct ApiCommit {
        committer: ApiWho,
    }
    #[derive(serde::Deserialize)]
    struct ApiWho {
        date: String,
    }
    let url = format!("https://api.github.com/repos/{owner}/{repo}/commits/master");
    let api: Api = crate::http::client()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(Commit {
        sha: api.sha,
        date: api.commit.committer.date,
    })
}

/// Data (ISO 8601, UTC) de uma pasta: quando o pacote foi extraído.
pub fn installed_at(dir: &Path) -> Option<String> {
    let t = std::fs::metadata(dir).ok()?.modified().ok()?;
    let secs = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(iso_utc(secs))
}

/// Epoch → `AAAA-MM-DDTHH:MM:SSZ` (o formato das datas da API do GitHub, que
/// assim se comparam como texto).
pub fn iso_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil_from_days (Howard Hinnant), sem depender de chrono
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// O pacote instalado está atrás do repositório? Com o SHA registrado, compara
/// o SHA; pacote baixado antes do registro, compara a data do commit com a da
/// pasta.
pub fn is_outdated(installed: Option<&Commit>, latest: &Commit, dir_date: Option<&str>) -> bool {
    match installed {
        Some(c) => c.sha != latest.sha,
        None => !latest.date.is_empty() && dir_date.is_some_and(|d| latest.date.as_str() > d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outdated_by_sha_or_by_date() {
        let latest = Commit {
            sha: "b".into(),
            date: "2026-10-22T20:46:37Z".into(),
        };
        let same = Commit {
            sha: "b".into(),
            date: "x".into(),
        };
        let old = Commit {
            sha: "a".into(),
            date: "x".into(),
        };
        assert!(!is_outdated(Some(&same), &latest, None));
        assert!(is_outdated(Some(&old), &latest, None));
        // sem SHA registrado: pela data da pasta
        assert!(is_outdated(None, &latest, Some("2026-06-01T00:00:00Z")));
        assert!(!is_outdated(None, &latest, Some("2026-11-01T00:00:00Z")));
        assert!(!is_outdated(None, &latest, None));
        // HEAD sem a data (só `ls-refs`): pacote antigo não acusa
        let no_date = Commit {
            sha: "b".into(),
            date: String::new(),
        };
        assert!(!is_outdated(None, &no_date, Some("2026-06-01T00:00:00Z")));
    }

    #[test]
    fn iso_utc_matches_github_format() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_792_701_997), "2026-10-22T20:46:37Z");
    }
}
