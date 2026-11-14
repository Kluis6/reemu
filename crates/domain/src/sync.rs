//! Cloud save: o que a máquina lembra do último sync e os conflitos abertos.

use serde::{Deserialize, Serialize};

/// Uma linha da base: o estado de uma chave no último sync bem-sucedido.
/// `key` é `file:<plataforma>/<arquivo>`, `state:<plataforma>/<core>/<arquivo>`
/// ou `config:<grupo>`; `kind` é `file`, `state` ou `config`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseRow {
    pub key: String,
    pub kind: String,
    pub remote_save_id: String,
    pub remote_version_id: String,
    /// SHA-256 (hex) que o servidor informou.
    pub remote_hash: String,
    /// SHA-256 (hex) dos bytes em disco naquele momento.
    pub local_hash: String,
    /// Caminho absoluto a que a chave corresponde nesta máquina.
    pub local_path: String,
    /// CRC32 da ROM quando a chave pertence a uma ROM da biblioteca.
    pub rom_crc32: Option<String>,
    pub synced_at: i64,
    /// Marcado quando o servidor apagou a linha deste aparelho e, num
    /// arquivo, quando a remoção feita no site foi aceita: a base fica como
    /// marca de "apagado no site" e vale por ausente na regra de três lados.
    pub wiped_at: Option<i64>,
    /// Onde o arquivo foi parar quando a remoção no site foi aceita.
    pub backup_path: Option<String>,
    /// O tamanho e o mtime (nanossegundos Unix) do arquivo quando a base foi
    /// gravada: iguais no disco, `local_hash` vale sem reler. `None` nos game
    /// saves e nas linhas de antes da coluna.
    #[serde(default)]
    pub local_size: Option<i64>,
    #[serde(default)]
    pub local_mtime_ns: Option<i64>,
    /// A linha de `save_states` dos bytes da base (só nos states): uma vaga
    /// gravada de novo é outra linha, mesmo com o mesmo tamanho e mtime.
    #[serde(default)]
    pub local_row: Option<String>,
}

/// Um arquivo cujo lado local e cujo lado remoto mudaram: nada é sobrescrito
/// até a pessoa escolher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRow {
    pub key: String,
    pub local_hash: String,
    pub local_mtime: i64,
    pub local_size: i64,
    pub remote_save_id: String,
    pub remote_version_id: String,
    pub remote_hash: String,
    pub remote_device_label: Option<String>,
    pub remote_client_mtime: Option<i64>,
    pub remote_size: i64,
    /// O lado da nuvem é uma remoção feita no site, não outra versão.
    pub remote_deleted: bool,
    pub rom_id: Option<String>,
    pub seen_at: i64,
}
