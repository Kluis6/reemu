//! Saber se um core instalado tem versão nova no buildbot, sem baixar nada.
//!
//! Comparar arquivos não serve (medido em 2026-10-30, com 127 DLLs de 08/10):
//! o buildbot recompila quase tudo toda noite, e o CRC do `.index-extended`
//! muda mesmo sem mudança no código, porque a DLL grava a data da compilação
//! (snes9x: 8 bytes diferentes em 3,8 MB, sem commit novo). O tamanho também
//! não serve: o vecx mudou de commit com o mesmo tamanho.
//!
//! O que funciona é o commit que o core declara em `library_version`
//! (`1.63 fae2fea`) contra o HEAD do repositório dele. A data do índice cobre
//! o que o HEAD não diz: se a DLL é tão nova quanto a última compilação, baixar
//! de novo não traria nada, mesmo com o HEAD à frente (compilação atrasada ou
//! quebrada, como a do MAME).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app_error::AppError;
use crate::core_catalog::{buildbot_os, dylib_ext};

/// Core → repositório. Só entra o que foi conferido em 2026-10-30: ou o commit
/// da DLL instalada era o HEAD, ou o HEAD estava dentro da DLL noturna. Quem
/// não declara commit ou compila de outro branch fica de fora: sem aviso é
/// melhor que aviso falso.
const UPSTREAM: &[(&str, &str)] = &[
    ("a5200_libretro", "libretro/a5200"),
    ("atari800_libretro", "libretro/libretro-atari800"),
    ("bluemsx_libretro", "libretro/blueMSX-libretro"),
    ("bsnes2014_accuracy_libretro", "libretro/bsnes2014"),
    ("bsnes2014_balanced_libretro", "libretro/bsnes2014"),
    ("bsnes2014_performance_libretro", "libretro/bsnes2014"),
    (
        "bsnes_cplusplus98_libretro",
        "libretro/bsnes-libretro-cplusplus98",
    ),
    ("bsnes_mercury_accuracy_libretro", "libretro/bsnes-mercury"),
    ("bsnes_mercury_balanced_libretro", "libretro/bsnes-mercury"),
    (
        "bsnes_mercury_performance_libretro",
        "libretro/bsnes-mercury",
    ),
    ("cdi2015_libretro", "libretro/mame2015-libretro"),
    ("clownmdemu_libretro", "Clownacy/clownmdemu-libretro"),
    ("crocods_libretro", "libretro/libretro-crocods"),
    ("desmume_libretro", "libretro/desmume"),
    ("desmume2015_libretro", "libretro/desmume2015"),
    ("dosbox_libretro", "libretro/dosbox-libretro"),
    ("dosbox_svn_libretro", "libretro/dosbox-svn"),
    ("fbalpha2012_libretro", "libretro/fbalpha2012"),
    ("fbneo_libretro", "libretro/FBNeo"),
    ("fceumm_libretro", "libretro/libretro-fceumm"),
    ("flycast_libretro", "flyinghead/flycast"),
    ("fmsx_libretro", "libretro/fmsx-libretro"),
    ("freeintv_libretro", "libretro/FreeIntv"),
    ("fuse_libretro", "libretro/fuse-libretro"),
    ("gearboy_libretro", "drhelius/Gearboy"),
    ("gearcoleco_libretro", "drhelius/Gearcoleco"),
    ("geargrafx_libretro", "drhelius/Geargrafx"),
    ("gearlynx_libretro", "drhelius/Gearlynx"),
    ("gearsystem_libretro", "drhelius/Gearsystem"),
    ("genesis_plus_gx_libretro", "libretro/Genesis-Plus-GX"),
    (
        "genesis_plus_gx_wide_libretro",
        "libretro/Genesis-Plus-GX-Wide",
    ),
    ("gpsp_libretro", "libretro/gpsp"),
    ("handy_libretro", "libretro/libretro-handy"),
    ("hbmame_libretro", "libretro/hbmame-libretro"),
    ("mame_libretro", "libretro/mame"),
    ("mame2000_libretro", "libretro/mame2000-libretro"),
    ("mame2003_libretro", "libretro/mame2003-libretro"),
    ("mame2010_libretro", "libretro/mame2010-libretro"),
    ("mame2015_libretro", "libretro/mame2015-libretro"),
    ("mednafen_gba_libretro", "libretro/beetle-gba-libretro"),
    ("mednafen_lynx_libretro", "libretro/beetle-lynx-libretro"),
    ("mednafen_ngp_libretro", "libretro/beetle-ngp-libretro"),
    ("mednafen_pce_libretro", "libretro/beetle-pce-libretro"),
    ("mednafen_pcfx_libretro", "libretro/beetle-pcfx-libretro"),
    ("mednafen_psx_libretro", "libretro/beetle-psx-libretro"),
    ("mednafen_psx_hw_libretro", "libretro/beetle-psx-libretro"),
    ("mednafen_snes_libretro", "libretro/beetle-bsnes-libretro"),
    ("mednafen_supafaust_libretro", "libretro/supafaust"),
    (
        "mednafen_supergrafx_libretro",
        "libretro/beetle-supergrafx-libretro",
    ),
    ("mednafen_vb_libretro", "libretro/beetle-vb-libretro"),
    ("mednafen_wswan_libretro", "libretro/beetle-wswan-libretro"),
    ("melonds_libretro", "libretro/melonDS"),
    ("meteor_libretro", "libretro/meteor-libretro"),
    ("mgba_libretro", "libretro/mgba"),
    (
        "mupen64plus_next_libretro",
        "libretro/mupen64plus-libretro-nx",
    ),
    ("neocd_libretro", "libretro/neocd_libretro"),
    ("nestopia_libretro", "libretro/nestopia"),
    ("o2em_libretro", "libretro/libretro-o2em"),
    ("pcsx_rearmed_libretro", "libretro/pcsx_rearmed"),
    ("picodrive_libretro", "libretro/picodrive"),
    ("potator_libretro", "libretro/potator"),
    ("prboom_libretro", "libretro/libretro-prboom"),
    ("prosystem_libretro", "libretro/prosystem-libretro"),
    ("puae_libretro", "libretro/libretro-uae"),
    ("quicknes_libretro", "libretro/QuickNES_Core"),
    ("race_libretro", "libretro/RACE"),
    ("scummvm_libretro", "libretro/scummvm"),
    ("smsplus_libretro", "libretro/smsplus-gx"),
    ("snes9x_libretro", "libretro/snes9x"),
    ("snes9x2005_libretro", "libretro/snes9x2005"),
    ("snes9x2005_plus_libretro", "libretro/snes9x2005"),
    ("snes9x2010_libretro", "libretro/snes9x2010"),
    ("stella_libretro", "stella-emu/stella"),
    ("stella2014_libretro", "libretro/stella2014-libretro"),
    ("stella2023_libretro", "libretro/stella2023"),
    ("swanstation_libretro", "libretro/swanstation"),
    ("tgbdual_libretro", "libretro/tgbdual-libretro"),
    ("vbam_libretro", "libretro/vbam-libretro"),
    ("vecx_libretro", "libretro/libretro-vecx"),
    ("vice_x128_libretro", "libretro/vice-libretro"),
    ("vice_x64_libretro", "libretro/vice-libretro"),
    ("vice_x64sc_libretro", "libretro/vice-libretro"),
    ("vice_xscpu64_libretro", "libretro/vice-libretro"),
    ("virtualjaguar_libretro", "libretro/virtualjaguar-libretro"),
    ("yabause_libretro", "libretro/yabause"),
];

fn upstream_repo(core_id: &str) -> Option<&'static str> {
    UPSTREAM
        .iter()
        .find(|(id, _)| *id == core_id)
        .map(|(_, repo)| *repo)
}

/// O commit dentro de `library_version`, nos formatos que os cores usam:
/// `1.63 fae2fea`, `3.8.17-6-g8fac4fd`, `GITa49cfac`, `fcbce3ae-2026.3.1git`.
fn commit_in_version(version: &str) -> Option<&str> {
    version
        .split(|c: char| !c.is_ascii_alphanumeric())
        .rev()
        .map(|t| {
            t.strip_prefix("GIT")
                .or_else(|| t.strip_prefix('g'))
                .unwrap_or(t)
        })
        .find(|t| {
            (7..=40).contains(&t.len())
                && t.chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        })
}

/// Dias desde 1970 de uma data `AAAA-MM-DD` (algoritmo `days_from_civil`).
fn day_number(date: &str) -> Option<i64> {
    let mut parts = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn file_day(modified: SystemTime) -> Option<i64> {
    let secs = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((secs / 86_400) as i64)
}

/// Linhas `data código arquivo` do `.index-extended` → core → dia da compilação.
fn parse_index(text: &str, ext: &str) -> HashMap<String, i64> {
    let suffix = format!(".{ext}.zip");
    text.lines()
        .filter_map(|line| {
            let mut f = line.split_whitespace();
            let (date, _code, file) = (f.next()?, f.next()?, f.next()?);
            Some((file.strip_suffix(&suffix)?.to_string(), day_number(date)?))
        })
        .collect()
}

fn is_outdated(commit: &str, head: &str, built_day: i64, dll_day: i64) -> bool {
    built_day > dll_day && !head.starts_with(commit)
}

/// O HEAD na resposta do `ls-refs` (pkt-line: `0032<oid> HEAD\n0000`).
fn parse_ls_refs(body: &str) -> Option<String> {
    let oid = body.get(4..)?.split_whitespace().next()?;
    (oid.len() == 40 && oid.chars().all(|c| c.is_ascii_hexdigit())).then(|| oid.to_string())
}

/// `ls-refs` do protocolo Git v2 (gitprotocol-v2 e gitprotocol-http): pede só o
/// HEAD, ~50 bytes de resposta mesmo num repositório como o do MAME.
async fn remote_head(repo: &str) -> Option<String> {
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

/// Situação de um core da tabela, para a fumaça do catálogo; `None` = fora dela.
/// HEAD à frente não é falha: o buildbot pode estar um dia atrás.
#[cfg(test)]
pub(crate) async fn table_check(core_id: &str, version: &str) -> Option<String> {
    let repo = upstream_repo(core_id)?;
    let Some(commit) = commit_in_version(version) else {
        return Some(format!("FALHA: `{version}` não traz o commit"));
    };
    let mut head = remote_head(repo).await;
    if head.is_none() {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        head = remote_head(repo).await;
    }
    Some(match head {
        None => format!("FALHA: sem HEAD em {repo}"),
        Some(h) if h.starts_with(commit) => format!("em dia ({commit})"),
        Some(h) => format!("HEAD à frente ({commit} → {})", &h[..7]),
    })
}

/// Quantos repositórios consultar de uma vez.
const HEAD_REQUESTS: usize = 8;

struct Candidate {
    core_id: String,
    commit: String,
    repo: &'static str,
    built_day: i64,
    dll_day: i64,
}

/// Os cores instalados com versão nova. Sem o índice, erro; sem o HEAD de um
/// repositório, aquele core só não entra.
pub async fn check(cores_dir: &Path) -> Result<Vec<String>, AppError> {
    let url = format!(
        "https://buildbot.libretro.com/nightly/{}/x86_64/latest/.index-extended",
        buildbot_os()
    );
    let index = crate::http::client()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::from_http("índice dos cores", e))?
        .error_for_status()
        .map_err(|e| AppError::from_http("índice dos cores", e))?
        .text()
        .await
        .map_err(|e| AppError::from_http("índice dos cores", e))?;
    let built = parse_index(&index, dylib_ext());

    // Ler `library_version` abre as DLLs (uma vez por execução; ver `discover`).
    let dir = cores_dir.to_path_buf();
    let installed = tauri::async_runtime::spawn_blocking(move || emu_session::discover_cores(&dir))
        .await
        .map_err(|e| e.to_string())?;

    // O que já é tão novo quanto a última compilação nem vai à rede.
    let candidates: Vec<Candidate> = installed
        .iter()
        .filter_map(|c| {
            let modified = std::fs::metadata(&c.path).ok()?.modified().ok()?;
            Some(Candidate {
                core_id: c.core_id.clone(),
                commit: commit_in_version(&c.library_version)?.to_string(),
                repo: upstream_repo(&c.core_id)?,
                built_day: *built.get(&c.core_id)?,
                dll_day: file_day(modified)?,
            })
        })
        .filter(|c| c.built_day > c.dll_day)
        .collect();

    let repos: Vec<&'static str> = candidates
        .iter()
        .map(|c| c.repo)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut heads = HashMap::new();
    for chunk in repos.chunks(HEAD_REQUESTS) {
        let tasks: Vec<_> = chunk
            .iter()
            .map(|&repo| {
                tauri::async_runtime::spawn(async move { (repo, remote_head(repo).await) })
            })
            .collect();
        for task in tasks {
            if let Ok((repo, Some(head))) = task.await {
                heads.insert(repo, head);
            }
        }
    }

    let mut outdated: Vec<String> = candidates
        .into_iter()
        .filter(|c| {
            heads
                .get(c.repo)
                .is_some_and(|head| is_outdated(&c.commit, head, c.built_day, c.dll_day))
        })
        .map(|c| c.core_id)
        .collect();
    outdated.sort();
    Ok(outdated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "usa a rede e os cores de REEMU_TEST_CORES_DIR"]
    async fn check_real() {
        let dir = std::env::var("REEMU_TEST_CORES_DIR").expect("REEMU_TEST_CORES_DIR");
        let outdated = check(Path::new(&dir)).await.unwrap();
        println!("{} com atualização: {outdated:?}", outdated.len());
    }

    #[test]
    fn commit_nos_formatos_dos_cores() {
        for (version, commit) in [
            ("1.63 fae2fea", Some("fae2fea")),
            ("3.8.17-6-g8fac4fd", Some("8fac4fd")),
            ("v1.0.0.03 261007 GITa49cfac", Some("a49cfac")),
            ("fcbce3ae-2026.3.1git", Some("fcbce3ae")),
            ("v3.7.1 dbed684-dirty", Some("dbed684")),
            ("0.289 (e858fcb0)", Some("e858fcb0")),
            ("v1.1.0-5819380", Some("5819380")),
            ("0d9853d", Some("0d9853d")),
            ("Alpha v1.3.1", None),
            ("1.0-preview7", None),
            ("", None),
        ] {
            assert_eq!(commit_in_version(version), commit, "{version}");
        }
    }

    #[test]
    fn tabela_so_com_cores_do_catalogo_sem_repetir() {
        let mut ids: Vec<&str> = UPSTREAM.iter().map(|(id, _)| *id).collect();
        let fora: Vec<&&str> = ids
            .iter()
            .filter(|id| crate::core_catalog::find(id).is_none())
            .collect();
        assert!(fora.is_empty(), "fora do catálogo: {fora:?}");
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn indice_e_datas() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2026-10-30"), Some(20_756));
        assert_eq!(day_number("2026-13-01"), None);
        let index = "2026-10-30 f7aed2c1 snes9x_libretro.dll.zip\n\
                     2026-10-16 5daeb9d6 mame_libretro.dll.zip\n\
                     lixo\n\
                     2026-10-30 e211e6cb snes9x_libretro.so.zip\n";
        let built = parse_index(index, "dll");
        assert_eq!(built.len(), 2);
        assert_eq!(built["snes9x_libretro"], 20_756);
    }

    #[test]
    fn so_avisa_com_commit_novo_e_compilacao_mais_nova_que_a_dll() {
        let head = "fae2fea08f74180759ef540ee94259213f503480";
        // recompilado sem commit novo (o caso de quase todo core, toda noite)
        assert!(!is_outdated("fae2fea", head, 20, 10));
        assert!(is_outdated("8f671cc", head, 20, 10));
        // HEAD à frente, mas a DLL já é a última compilação (build quebrado)
        assert!(!is_outdated("8f671cc", head, 20, 20));
    }

    #[test]
    fn resposta_do_ls_refs() {
        assert_eq!(
            parse_ls_refs("0032fae2fea08f74180759ef540ee94259213f503480 HEAD\n0000").as_deref(),
            Some("fae2fea08f74180759ef540ee94259213f503480")
        );
        assert_eq!(parse_ls_refs("0000"), None);
        assert_eq!(parse_ls_refs(""), None);
    }
}
