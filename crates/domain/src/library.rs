//! Biblioteca de ROMs. Matching por hash (CRC32/MD5), não por nome de
//! arquivo (ver `metadata.rs`). `Rom` é a entidade base — scraping, save
//! states e save RAM referenciam `rom_id`.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const ST_HEADER_LEN: usize = 30;
pub const ZX81_P_HEADER_LEN: usize = 13;

/// `.p` do ZX81 guarda a RAM de $4009 até E_LINE − 1.
/// Os ponteiros D_FILE, VARS e E_LINE distinguem o programa de outros arquivos `.p`.
pub fn is_zx81_program(size: u64, header: &[u8; ZX81_P_HEADER_LEN]) -> bool {
    let display = u16::from_le_bytes([header[3], header[4]]);
    let variables = u16::from_le_bytes([header[7], header[8]]);
    let end = u16::from_le_bytes([header[11], header[12]]);
    header[0] == 0
        && display >= 0x407d
        && variables > display
        && end > variables
        && size == u64::from(end - 0x4009)
}

/// Os disquetes `.st` são imagens cruas de setores de 512 bytes. O cartucho
/// Sufami Turbo tem assinatura própria e a BIOS usa uma variante dela.
pub fn system_for_st(size: u64, header: &[u8; ST_HEADER_LEN]) -> Option<&'static str> {
    if header.starts_with(b"BANDAI SFC-ADX") {
        return ((0x80000..=0x100000).contains(&size) && &header[16..30] != b"SFC-ADX BACKUP")
            .then_some("snes");
    }

    if !size.is_multiple_of(512) || !(160 * 1024..=2880 * 1024).contains(&size) {
        return None;
    }
    let bytes_per_sector = u16::from_le_bytes([header[11], header[12]]);
    let total_sectors = u64::from(u16::from_le_bytes([header[19], header[20]]));
    let sectors = size / 512;
    let standard_geometry = [8, 9, 10, 11, 12, 18, 36].iter().any(|per_track| {
        [*per_track, *per_track * 2]
            .into_iter()
            .any(|per_cylinder| {
                sectors.is_multiple_of(per_cylinder)
                    && matches!(sectors / per_cylinder, 40 | 80..=86)
            })
    });
    (standard_geometry || (bytes_per_sector == 512 && total_sectors == sectors))
        .then_some("atarist")
}

pub const DIRKSIMPLE_OFFICIAL_GAMES: &[&str] = &["lair", "cliff"];

/// O DirkSimple usa o nome até o primeiro ponto, com letras ASCII minúsculas,
/// para localizar `data/games/<id>/game.lua` ou `game.luac`.
pub fn dirksimple_game_id(path: &std::path::Path) -> Option<String> {
    let (id, _) = path.file_name()?.to_str()?.split_once('.')?;
    (!id.is_empty()).then(|| id.to_ascii_lowercase())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rom {
    pub id: String,
    pub file_path: String,
    /// CRC32 e MD5 do que identifica o arquivo. Vazios (as colunas são NOT
    /// NULL) quando ele não é lido inteiro: aí o nome, o tamanho e o serial
    /// identificam.
    pub crc32: String,
    pub md5: String,
    pub system_id: String,
    /// Unix timestamp (segundos) de quando a ROM entrou na biblioteca.
    pub added_at: i64,
    /// Unix timestamp (segundos) do último load do jogo. `None` = nunca jogado.
    pub last_played_at: Option<i64>,
    /// Aba "Favoritos" da biblioteca.
    pub is_favorite: bool,
    /// Nome definido manualmente pelo usuário (`set_metadata`). `None` = usa o
    /// nome do arquivo. A plataforma editada vai direto em `system_id`.
    pub user_title: Option<String>,
    /// SHA1 (hex minúsculo) do que foi lido pro CRC32/MD5, na mesma passada.
    /// `None` nas linhas de antes da identidade (o passo em segundo plano
    /// preenche; o scan não relê) e quando não há hash (ver `crc32`); quem
    /// separa os dois casos é `identified_at`.
    pub sha1: Option<String>,
    /// Serial do disco (ex.: `SLUS-00594`), lido da imagem. `None` = não lido.
    pub serial: Option<String>,
    /// Nome do arquivo que foi lido pra identidade: a entrada de dentro do
    /// `.zip`/`.7z`, a 1ª trilha de dados de um `.cue`/`.gdi`, ou o próprio
    /// arquivo.
    pub ident_name: Option<String>,
    /// Tamanho em bytes do que foi lido (sem o header iNES).
    pub ident_size: Option<i64>,
    /// Unix segundos de quando a identidade foi calculada.
    pub identified_at: Option<i64>,
}

#[async_trait]
pub trait RomRepository: Send + Sync {
    async fn add(&self, rom: &Rom) -> Result<(), RepoError>;
    async fn get(&self, id: &str) -> Result<Option<Rom>, RepoError>;
    async fn find_by_path(&self, file_path: &str) -> Result<Option<Rom>, RepoError>;
    /// CRC32 não é único (colisões, dumps distintos) — retorna todas.
    async fn find_by_crc32(&self, crc32: &str) -> Result<Vec<Rom>, RepoError>;
    async fn list_by_system(&self, system_id: &str) -> Result<Vec<Rom>, RepoError>;
    async fn list(&self) -> Result<Vec<Rom>, RepoError>;
    async fn remove(&self, id: &str) -> Result<(), RepoError>;
    /// Troca o sistema de uma ROM já catalogada (a varredura reclassifica
    /// quando a regra de identificação muda — ex.: NAOMI que entrou como
    /// `disc`). Mantém tempo de jogo, favorito e metadados. A identidade fica
    /// pra calcular de novo (`identified_at` nulo): a trilha e o serial
    /// dependem do sistema (a trilha 3 e o serial do Dreamcast, por exemplo).
    async fn set_system(&self, id: &str, system_id: &str) -> Result<(), RepoError>;
    /// Marca a ROM como jogada agora (`last_played_at`).
    async fn mark_played(&self, id: &str, at_unix: i64) -> Result<(), RepoError>;
    /// Soma `secs` ao tempo de jogo acumulado da ROM.
    async fn add_play_time(&self, id: &str, secs: u64) -> Result<(), RepoError>;
    /// Tempo de jogo acumulado, em segundos (0 se nunca jogou ou não existe).
    async fn play_time(&self, id: &str) -> Result<u64, RepoError>;
    /// Liga/desliga o favorito.
    async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), RepoError>;
    /// Edição manual: `title = Some("")` limpa (volta pro nome do arquivo);
    /// `system_id = Some(...)` troca a plataforma. `None` = não mexe no campo.
    async fn set_metadata(
        &self,
        id: &str,
        title: Option<&str>,
        system_id: Option<&str>,
    ) -> Result<(), RepoError>;
}
