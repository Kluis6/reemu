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

impl Kind {
    /// O tipo de uma chave da base: `file:`, `state:` ou `config:`.
    pub fn of_key(key: &str) -> Option<Self> {
        match key.split_once(':')?.0 {
            "file" => Some(Self::File),
            "state" => Some(Self::State),
            "config" => Some(Self::Config),
            _ => None,
        }
    }
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
    /// O DONO da linha (`FileSummary.device_token_id`), nunca quem escreveu a
    /// última versão (`current.device_token_id`). O GameKave não põe dono em
    /// linha `kind=file`: ali o campo vem `None`.
    pub device_token_id: Option<String>,
    pub client_mtime: Option<i64>,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nada a fazer: local e nuvem batem com a base.
    Nothing,
    /// Local e nuvem são iguais mas a base não existe ou ficou velha: o
    /// chamador só grava a base (sem tráfego de bytes).
    Adopt,
    Upload,
    Download,
    Conflict,
    /// A nuvem apagou a linha (de outro aparelho) e o local não mudou desde o
    /// sync: o arquivo vai para o backup e a base é removida.
    LocalToBackupAndClearBase,
    /// D127, linha própria de tipo que não é arquivo: o motor MARCA a base como
    /// apagada (`base_mark_wiped`) e não toca no local.
    MarkBaseWipedKeepLocal,
    /// Base presente, sem arquivo local, e a nuvem apagou a linha ou a perdeu:
    /// o motor REMOVE a base (`base_delete`). Não há arquivo a guardar.
    DropBase,
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
    // Arquivo cuja remoção no site já foi aceita: a base ficou só como marca
    // (a página diz "apagado no site") e vale por ausente. O que aparecer
    // depois, aqui ou na nuvem, é novo.
    let base = base.filter(|b| !(kind == Kind::File && b.wiped_at.is_some()));
    // Remota viva e vazia, para arquivo, vale por ausente: nunca baixa um
    // arquivo vazio por cima de um save.
    let live = remote.filter(|r| !r.deleted && !(kind == Kind::File && r.size == 0));
    let tombstone = remote.filter(|r| r.deleted);

    let Some(base) = base else {
        // Sem base, uma marca de apagado é igual a ausência: nada a restaurar.
        return match (local, live) {
            (None, None) => Action::Nothing,
            (Some(_), None) => Action::Upload,
            (None, Some(_)) => Action::Download,
            (Some(l), Some(r)) if l.hash == r.hash => Action::Adopt,
            (Some(_), Some(_)) => Action::Conflict,
        };
    };

    if let Some(r) = live {
        let Some(l) = local else {
            // A pessoa apagou o arquivo: a nuvem é a cópia da conta.
            return Action::Download;
        };
        if l.hash == r.hash {
            // Já batem com a base: nada. Senão a base está velha: adotar.
            let fresh = l.hash == base.local_hash && r.hash == base.remote_hash;
            return if fresh {
                Action::Nothing
            } else {
                Action::Adopt
            };
        }
        return match (l.hash == base.local_hash, r.hash == base.remote_hash) {
            (true, true) => Action::Nothing,
            (false, true) => Action::Upload,
            (true, false) => Action::Download,
            (false, false) => Action::Conflict,
        };
    }

    match tombstone {
        // Linha apagada.
        Some(r) => {
            // O dono só existe em linha que não é arquivo (ver `Remote`).
            if kind != Kind::File && r.device_token_id.as_deref() == Some(this_device) {
                return Action::MarkBaseWipedKeepLocal;
            }
            match local {
                // Já não há arquivo para guardar: só a base sobra.
                None => Action::DropBase,
                Some(l) if l.hash == base.local_hash => Action::LocalToBackupAndClearBase,
                Some(_) => Action::Conflict,
            }
        }
        // Sem linha nenhuma (ou só uma remota vazia): o servidor a perdeu.
        None => match local {
            None => Action::DropBase,
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
            backup_path: None,
        }
    }

    fn remote(hash: &str) -> Remote {
        Remote {
            save_id: "s1".into(),
            version_id: "v2".into(),
            hash: hash.into(),
            size: 8,
            device_token_id: None,
            client_mtime: Some(1),
            deleted: false,
        }
    }

    fn tomb(device: Option<&str>) -> Remote {
        Remote {
            deleted: true,
            device_token_id: device.map(Into::into),
            ..remote("x")
        }
    }

    fn run(l: Option<&Local>, b: Option<&BaseRow>, r: Option<&Remote>) -> Action {
        decide(Kind::File, l, b, r, ME)
    }

    #[test]
    fn kind_of_key_reads_the_prefix() {
        assert_eq!(Kind::of_key("file:gba/Zelda.srm"), Some(Kind::File));
        assert_eq!(Kind::of_key("state:snes/x/a.state"), Some(Kind::State));
        assert_eq!(Kind::of_key("config:video"), Some(Kind::Config));
        assert_eq!(Kind::of_key("other:x"), None);
        assert_eq!(Kind::of_key("file"), None);
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
            Action::Adopt
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
            Action::Adopt
        );
    }

    #[test]
    fn decide_tombstone_from_other_device_with_unchanged_local_backs_up() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&tomb(None))),
            Action::LocalToBackupAndClearBase
        );
    }

    #[test]
    fn decide_tombstone_from_other_device_with_changed_local_conflicts() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a2")), Some(&b), Some(&tomb(None))),
            Action::Conflict
        );
    }

    #[test]
    fn decide_own_tombstone_of_a_non_file_marks_the_base_wiped() {
        let b = base("a", "r");
        for kind in [Kind::State, Kind::Config] {
            for l in ["a", "a2"] {
                assert_eq!(
                    decide(kind, Some(&local(l)), Some(&b), Some(&tomb(Some(ME))), ME),
                    Action::MarkBaseWipedKeepLocal
                );
            }
        }
    }

    #[test]
    fn decide_file_tombstone_owned_by_this_device_is_not_special() {
        // O servidor nunca põe dono em linha de arquivo; mesmo que viesse, o
        // arquivo segue a regra comum e vai para o backup.
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&tomb(Some(ME)))),
            Action::LocalToBackupAndClearBase
        );
        assert_eq!(
            run(Some(&local("a2")), Some(&b), Some(&tomb(Some(ME)))),
            Action::Conflict
        );
    }

    #[test]
    fn decide_file_tombstone_without_owner_backs_up() {
        let b = base("a", "r");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&tomb(None))),
            Action::LocalToBackupAndClearBase
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
    fn decide_tombstone_and_no_local_drops_the_base() {
        let b = base("a", "r");
        assert_eq!(run(None, Some(&b), Some(&tomb(None))), Action::DropBase);
        assert_eq!(run(None, Some(&b), None), Action::DropBase);
    }

    /// I-4: a remoção no site aceita deixa a base marcada; para arquivo ela
    /// vale por ausente: sem arquivo, nada (não some a marca); jogo novo
    /// aqui sobe como linha nova; outra máquina que envie desce.
    #[test]
    fn decide_a_wiped_file_base_is_no_base() {
        let wiped = BaseRow {
            wiped_at: Some(5),
            backup_path: Some("/b/a.srm".into()),
            ..base("a", "r")
        };
        assert_eq!(run(None, Some(&wiped), Some(&tomb(None))), Action::Nothing);
        assert_eq!(run(None, Some(&wiped), None), Action::Nothing);
        assert_eq!(
            run(Some(&local("a")), Some(&wiped), Some(&tomb(None))),
            Action::Upload
        );
        assert_eq!(
            run(None, Some(&wiped), Some(&remote("n"))),
            Action::Download
        );
        assert_eq!(
            run(Some(&local("a")), Some(&wiped), Some(&remote("r"))),
            Action::Conflict
        );
        // Para o que não é arquivo a marca ainda é base (D127).
        assert_eq!(
            decide(
                Kind::State,
                Some(&local("a")),
                Some(&wiped),
                Some(&tomb(Some(ME))),
                ME
            ),
            Action::MarkBaseWipedKeepLocal
        );
    }

    #[test]
    fn decide_tombstone_without_base_is_absence() {
        assert_eq!(
            run(Some(&local("a")), None, Some(&tomb(None))),
            Action::Upload
        );
        assert_eq!(run(None, None, Some(&tomb(None))), Action::Nothing);
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

    #[test]
    fn decide_stale_base_with_equal_sides_adopts() {
        let b = base("old", "oldr");
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&remote("a"))),
            Action::Adopt
        );
    }

    #[test]
    fn zero_byte_live_remote_is_absent_for_files() {
        let empty_remote = Remote {
            size: 0,
            ..remote("e3b0")
        };
        // Nunca baixa um arquivo vazio.
        assert_eq!(run(None, None, Some(&empty_remote)), Action::Nothing);
        let b = base("a", "r");
        assert_eq!(run(None, Some(&b), Some(&empty_remote)), Action::DropBase);
        // Com local, a nuvem vazia é uma linha perdida: o local sobe.
        assert_eq!(
            run(Some(&local("a")), Some(&b), Some(&empty_remote)),
            Action::ReUpload
        );
        assert_eq!(
            run(Some(&local("a")), None, Some(&empty_remote)),
            Action::Upload
        );
        // Para estado, um remoto vazio continua sendo um remoto.
        assert_eq!(
            decide(Kind::State, None, None, Some(&empty_remote), ME),
            Action::Download
        );
    }
}
