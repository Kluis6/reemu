//! A regra de três lados do cloud save: o que existe agora neste disco, o que
//! existia no último sync (a base) e o que a nuvem tem decide o que fazer com
//! cada chave. Pura: sem disco, sem rede, sem banco.
//!
//! Nada aqui apaga arquivo. `LocalToBackupAndClearBase` manda o arquivo local
//! para a pasta de backups (quem age é o motor) e só então limpa a base.

use domain::sync::BaseRow;

/// O tipo da chave. Só `File` trata arquivo vazio como ausente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    State,
    Config,
}

/// O arquivo como está em disco agora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub hash: String,
    pub size: u64,
    pub mtime: i64,
}

/// A linha da nuvem para a chave (viva ou apagada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub save_id: String,
    pub version_id: String,
    pub hash: String,
    pub size: i64,
    pub device_token_id: Option<String>,
    pub client_mtime: Option<i64>,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Nothing,
    Upload,
    Download,
    Conflict,
    /// A nuvem apagou a linha (de outro aparelho) e o local não mudou desde o
    /// sync: o arquivo vai para o backup e a base é limpa.
    LocalToBackupAndClearBase,
    /// A marca de apagado é da linha deste próprio aparelho (ou não há mais
    /// nada dos dois lados): limpa a base e não toca no arquivo.
    ClearBaseKeepLocal,
    /// O servidor perdeu a linha (sem remoto, base presente, local = base).
    ReUpload,
}

pub fn decide(
    kind: Kind,
    local: Option<&Local>,
    base: Option<&BaseRow>,
    remote: Option<&Remote>,
    this_device: &str,
) -> Action {
    // `.srm` vazio é "sem save": nunca sobe e nunca disputa com a nuvem.
    let local = match (kind, local) {
        (Kind::File, Some(l)) if l.size == 0 => None,
        (_, l) => l,
    };
    let live = remote.filter(|r| !r.deleted);

    let Some(base) = base else {
        // Sem base, uma marca de apagado é igual a ausência: nada a restaurar.
        return match (local, live) {
            (None, None) => Action::Nothing,
            (Some(_), None) => Action::Upload,
            (None, Some(_)) => Action::Download,
            (Some(l), Some(r)) if l.hash == r.hash => Action::Nothing,
            (Some(_), Some(_)) => Action::Conflict,
        };
    };

    if let Some(r) = live {
        let Some(l) = local else {
            // A pessoa apagou o arquivo: a nuvem é a cópia da conta.
            return Action::Download;
        };
        if l.hash == r.hash {
            return Action::Nothing; // adotar: o chamador grava a base
        }
        return match (l.hash == base.local_hash, r.hash == base.remote_hash) {
            (true, true) => Action::Nothing,
            (false, true) => Action::Upload,
            (true, false) => Action::Download,
            (false, false) => Action::Conflict,
        };
    }

    match remote {
        // Linha apagada.
        Some(r) => {
            if r.device_token_id.as_deref() == Some(this_device) {
                return Action::ClearBaseKeepLocal;
            }
            match local {
                // Já não há arquivo para guardar: só a base sobra.
                None => Action::ClearBaseKeepLocal,
                Some(l) if l.hash == base.local_hash => Action::LocalToBackupAndClearBase,
                Some(_) => Action::Conflict,
            }
        }
        // Sem linha nenhuma: o servidor a perdeu.
        None => match local {
            None => Action::ClearBaseKeepLocal,
            Some(l) if l.hash == base.local_hash => Action::ReUpload,
            // Não há versão remota para disputar: o local mudou, sobe.
            Some(_) => Action::Upload,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "dev-me";

    fn local(hash: &str) -> Local {
        Local {
            hash: hash.into(),
            size: 8,
            mtime: 1,
        }
    }

    fn base(local_hash: &str, remote_hash: &str) -> BaseRow {
        BaseRow {
            key: "file:nes/a.srm".into(),
            kind: "file".into(),
            remote_save_id: "s1".into(),
            remote_version_id: "v1".into(),
            remote_hash: remote_hash.into(),
            local_hash: local_hash.into(),
            local_path: "/saves/a.srm".into(),
            rom_crc32: None,
            synced_at: 1,
            wiped_at: None,
        }
    }

    fn remote(hash: &str) -> Remote {
        Remote {
            save_id: "s1".into(),
            version_id: "v2".into(),
            hash: hash.into(),
            size: 8,
            device_token_id: Some("dev-other".into()),
            client_mtime: Some(1),
            deleted: false,
        }
    }

    fn tomb(device: &str) -> Remote {
        Remote {
            deleted: true,
            device_token_id: Some(device.into()),
            ..remote("x")
        }
    }

    fn run(l: Option<&Local>, b: Option<&BaseRow>, r: Option<&Remote>) -> Action {
        decide(Kind::File, l, b, r, ME)
    }

    #[test]
    fn decide_none_none_none_is_nothing() {
        assert_eq!(run(None, None, None), Action::Nothing);
    }

    #[test]
    fn decide_local_only_uploads() {
        assert_eq!(run(Some(&local("a")), None, None), Action::Upload);
    }

    #[test]
    fn decide_remote_only_downloads() {
        assert_eq!(run(None, None, Some(&remote("a"))), Action::Download);
    }

    #[test]
    fn decide_no_base_same_hash_adopts() {
        assert_eq!(
            run(Some(&local("a")), None, Some(&remote("a"))),
            Action::Nothing
        );
    }

    #[test]
    fn decide_no_base_different_hash_conflicts() {
        assert_eq!(
            run(Some(&local("a")), None, Some(&remote("b"))),
            Action::Conflict
        );
    }

    #[test]
    fn decide_nothing_changed_is_nothing() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&remote("r"))),
            Action::Nothing
        );
    }

    #[test]
    fn decide_only_local_changed_uploads() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a2")), Some(&b), Some(&remote("r"))),
            Action::Upload
        );
    }

    #[test]
    fn decide_only_remote_changed_downloads() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&remote("r2"))),
            Action::Download
        );
    }

    #[test]
    fn decide_both_changed_conflicts() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a2")), Some(&b), Some(&remote("r2"))),
            Action::Conflict
        );
    }

    #[test]
    fn decide_both_changed_to_same_bytes_adopts() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("z")), Some(&b), Some(&remote("z"))),
            Action::Nothing
        );
    }

    #[test]
    fn decide_tombstone_from_other_device_with_unchanged_local_backs_up() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&tomb("dev-other"))),
            Action::LocalToBackupAndClearBase
        );
    }

    #[test]
    fn decide_tombstone_from_other_device_with_changed_local_conflicts() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a2")), Some(&b), Some(&tomb("dev-other"))),
            Action::Conflict
        );
    }

    #[test]
    fn decide_tombstone_of_this_device_keeps_local() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&tomb(ME))),
            Action::ClearBaseKeepLocal
        );
        assert_eq!(
            run(Some(&local("a2")), Some(&b), Some(&tomb(ME))),
            Action::ClearBaseKeepLocal
        );
    }

    #[test]
    fn decide_server_lost_the_row_reuploads() {
        let b = base("a", "r");
        assert_eq!(run(Some(&local("a")), Some(&b), None), Action::ReUpload);
    }

    #[test]
    fn decide_server_lost_the_row_and_local_changed_uploads() {
        let b = base("a", "r");
        assert_eq!(run(Some(&local("a2")), Some(&b), None), Action::Upload);
    }

    #[test]
    fn decide_deleted_local_with_live_remote_downloads() {
        let b = base("a", "r");
        assert_eq!(run(None, Some(&b), Some(&remote("r"))), Action::Download);
    }

    #[test]
    fn decide_tombstone_and_no_local_only_clears_the_base() {
        let b = base("a", "r");
        assert_eq!(
            run(None, Some(&b), Some(&tomb("dev-other"))),
            Action::ClearBaseKeepLocal
        );
        assert_eq!(run(None, Some(&b), None), Action::ClearBaseKeepLocal);
    }

    #[test]
    fn decide_tombstone_without_base_is_absence() {
        assert_eq!(
            run(Some(&local("a")), None, Some(&tomb("dev-other"))),
            Action::Upload
        );
        assert_eq!(run(None, None, Some(&tomb("dev-other"))), Action::Nothing);
    }

    #[test]
    fn zero_byte_local_is_absent() {
        let empty = Local {
            hash: "e3b0".into(),
            size: 0,
            mtime: 1,
        };
        // Nunca sobe um .srm vazio.
        assert_eq!(run(Some(&empty), None, None), Action::Nothing);
        // Com a nuvem tendo o save, o vazio é "arquivo apagado": baixa.
        assert_eq!(
            run(Some(&empty), None, Some(&remote("a"))),
            Action::Download
        );
        let b = base("a", "r");
        assert_eq!(
            run(Some(&empty), Some(&b), Some(&remote("r"))),
            Action::Download
        );
        // Para outros tipos o vazio é um arquivo como qualquer outro.
        assert_eq!(
            decide(Kind::State, Some(&empty), None, None, ME),
            Action::Upload
        );
    }

    #[test]
    fn first_sync_with_diverging_files_conflicts_everywhere() {
        // Duas máquinas configuradas separadamente: nada é sobrescrito.
        for (l, r) in [("a", "b"), ("c", "d")] {
            assert_eq!(
                run(Some(&local(l)), None, Some(&remote(r))),
                Action::Conflict
            );
        }
    }
}
