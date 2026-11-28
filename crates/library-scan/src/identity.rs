//! Identidade de um arquivo da biblioteca (D138): hashes, nome, tamanho e
//! serial, como a varredura calcula e o passo em segundo plano recalcula.

use crate::archive::{archive_entry_size, hash_archive_entry};
use crate::hash::{file_name_of, FileRomHasher, RomHashes};
use crate::sector::{ext_of, open_file, read_sheet, sheet_data_track, Area, Opener};
use std::io::BufReader;
use std::path::Path;

/// O que identifica um arquivo da biblioteca (D138): os hashes, quando o
/// arquivo é lido inteiro, e sempre o nome e o tamanho do que identifica (a
/// entrada de dentro do `.zip`/`.7z`, a 1ª trilha de dados de um
/// `.cue`/`.gdi`, ou o próprio arquivo), mais o serial do disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// `None` quando o que identifica não foi lido inteiro.
    pub hashes: Option<Digests>,
    /// Nome (só o arquivo, sem pasta) do que identifica.
    pub name: String,
    /// Tamanho em bytes do que identifica (sem o header iNES).
    pub size: u64,
    /// Serial do disco, lido da imagem solta (`None` em cartucho, dentro de
    /// `.zip`/`.7z`, ou quando não se lê).
    pub serial: Option<String>,
    /// A trilha que o `.cue`/`.gdi` cita não estava lá, ou o texto dele não
    /// se leu: o nome e o tamanho são os do próprio cue, por enquanto. A
    /// varredura grava a linha com `identified_at` nulo, e o passo em segundo
    /// plano calcula de novo quando a trilha aparecer.
    pub provisional: bool,
}

/// CRC32 (maiúsculas), MD5 e SHA1 (minúsculas) de uma leitura só.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digests {
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
}

impl From<RomHashes> for Identity {
    fn from(h: RomHashes) -> Self {
        Identity {
            hashes: Some(Digests {
                crc32: h.crc32,
                md5: h.md5,
                sha1: h.sha1,
            }),
            name: h.name,
            size: h.size,
            serial: None,
            provisional: false,
        }
    }
}

/// Contêineres comprimidos identificados só pelo serial (quando existe),
/// nome e tamanho. O hash de um `.cso`, `.chd` ou vídeo recodificado não
/// casa com o índice e custaria ler o arquivo inteiro. Nunca são lidos
/// inteiros (NFR-5). O `.iso` não está aqui: segue o [`DISC_HASH_MAX`] como
/// as outras imagens, porque o de tamanho de CD o ScreenScraper conhece pelo
/// hash.
const SERIAL_ONLY_EXTS: &[&str] = &[
    "cso",
    "chd",
    "ogv",
    "dirksimple",
    "rvz",
    "wia",
    "gcz",
    "wbfs",
];

/// Maior arquivo lido inteiro pro hash (um `.iso`, a trilha de um
/// `.cue`/`.gdi`, um `.img`, `.pbp`, `.cdi`, `.mdf`, `.nrg` solto, e também
/// qualquer outro arquivo ou entrada de `.zip`/`.7z`): 1,5 GiB, na mesma
/// leitura única do CRC32, do MD5 e do SHA1. É um limite da própria leitura,
/// não só do tamanho visto antes: entrada que descomprime mais do que o
/// índice diz, ou arquivo que cresce enquanto é lido, para aqui sem hash.
/// Cabe a
/// maior trilha de CD (~999 MiB crus num CD de 99 minutos: 445.500 setores
/// de 2352 bytes) e a trilha 3 de um
/// GD-ROM de Dreamcast (até 504.150 setores de 2352 bytes, 1.185.760.800
/// bytes), que o catálogo e o ScreenScraper conhecem pelo hash. Um DVD (PS2,
/// até 8,5 GB) passa disso e fica com o serial, o nome e o tamanho, como o
/// `.cso` e o `.chd`.
const DISC_HASH_MAX: u64 = 1536 * 1024 * 1024;
const _: () = assert!(
    DISC_HASH_MAX >= (549_150 - 45_000) * 2352 && DISC_HASH_MAX < 2 * 1024 * 1024 * 1024,
    "cabe a trilha 3 do GD-ROM e nunca uma imagem de vários GB"
);

/// A identidade de `path` como a varredura calcula (o passo em segundo plano
/// recalcula as linhas antigas com ela). `archived_entry`: a ROM de dentro de
/// um `.zip`/`.7z`, se for o caso.
pub fn identify(
    path: &Path,
    system_id: &str,
    archived_entry: Option<&str>,
) -> std::io::Result<Identity> {
    identify_with(path, system_id, archived_entry, &open_file, DISC_HASH_MAX)
}

/// [`identify`] com outro jeito de abrir os arquivos e outro limite de
/// imagem lida inteira (os testes contam o que é lido sem gravar GBs).
fn identify_with(
    path: &Path,
    system_id: &str,
    archived_entry: Option<&str>,
    open: Opener,
    hash_max: u64,
) -> std::io::Result<Identity> {
    let mut id = match archived_entry {
        Some(entry) => archived_identity(path, entry, hash_max)?,
        None => file_identity(path, system_id, open, hash_max)?,
    };
    // Serial do disco (D138), só pros sistemas de disco e só da imagem solta
    // (de dentro de um `.zip`/`.7z` seria descomprimir o disco).
    id.serial = archived_entry
        .is_none()
        .then(|| crate::serial::read_serial_with(path, system_id, open))
        .flatten();
    Ok(id)
}

/// Identidade de um arquivo solto. `.cue` e `.gdi` são identificados pela
/// 1ª trilha de dados (a trilha 3 do GD-ROM no Dreamcast, quando há), não
/// pelo texto deles: é o hash da trilha que o Redump, o ScreenScraper e o
/// catálogo conhecem (D138). A trilha só é alcançada dentro da pasta do
/// cue ([`crate::disc_sniff::sibling`]) e se for arquivo comum; sem ela, o
/// nome e o tamanho do próprio cue, sem hash (provisório se a trilha só não
/// está lá ainda, ou se o texto não se leu). `.cso` e `.chd` nunca são
/// lidos ([`SERIAL_ONLY_EXTS`]); `.iso`, imagem ou trilha acima de
/// `hash_max` também não.
fn file_identity(
    path: &Path,
    system_id: &str,
    open: Opener,
    hash_max: u64,
) -> std::io::Result<Identity> {
    let ext = ext_of(path);
    match ext.as_str() {
        "cue" | "gdi" => {
            let Some(text) = read_sheet(path, open) else {
                return provisional(path);
            };
            match sheet_data_track(path, &text, Area::for_system(system_id)) {
                Some(track) if track.file.is_file() => {
                    bounded_identity(&track.file, open, hash_max)
                }
                // A trilha citada não está lá (ainda: o `.bin` pode chegar
                // depois do `.cue`).
                Some(_) => provisional(path),
                // Sem trilha de dados, ou citando fora da pasta: não muda.
                None => unhashed(path),
            }
        }
        e if SERIAL_ONLY_EXTS.contains(&e) => unhashed(path),
        _ => bounded_identity(path, open, hash_max),
    }
}

/// Arquivo solto (ROM, imagem ou trilha): CRC32, MD5 e SHA1 numa leitura só,
/// se couber em `hash_max`, pelo tamanho de agora e na própria leitura. Não
/// coube: o nome e o tamanho, sem hash.
fn bounded_identity(path: &Path, open: Opener, hash_max: u64) -> std::io::Result<Identity> {
    let id = unhashed(path)?;
    if id.size > hash_max {
        return Ok(id);
    }
    match FileRomHasher::hash_reader_capped(BufReader::new(open(path)?), &id.name, hash_max)? {
        Some(h) => Ok(h.into()),
        // Cresceu enquanto era lido (cópia em andamento): o tamanho de agora.
        None => unhashed(path),
    }
}

/// A ROM de dentro de um `.zip`/`.7z`. Um `.cso`/`.chd` lá dentro, ou
/// qualquer entrada acima de `hash_max`, fica com o nome e o tamanho do
/// índice do arquivo, sem descomprimir. O resto é descomprimido aos poucos
/// direto no hash, nunca inteiro na memória, e para em `hash_max` mesmo que
/// o índice minta.
fn archived_identity(path: &Path, entry: &str, hash_max: u64) -> std::io::Result<Identity> {
    let index_only = Identity {
        hashes: None,
        name: file_name_of(entry),
        size: archive_entry_size(path, entry)?,
        serial: None,
        provisional: false,
    };
    if SERIAL_ONLY_EXTS.contains(&ext_of(Path::new(entry)).as_str()) || index_only.size > hash_max {
        return Ok(index_only);
    }
    Ok(
        match hash_archive_entry(path, entry, &index_only.name, hash_max)? {
            Some(h) => h.into(),
            None => index_only,
        },
    )
}

/// Nome e tamanho de `path`, sem ler o conteúdo.
fn unhashed(path: &Path) -> std::io::Result<Identity> {
    Ok(Identity {
        hashes: None,
        name: file_name_of(&path.to_string_lossy()),
        size: std::fs::metadata(path)?.len(),
        serial: None,
        provisional: false,
    })
}

/// Como [`unhashed`], marcado pra ser calculado de novo.
fn provisional(path: &Path) -> std::io::Result<Identity> {
    Ok(Identity {
        provisional: true,
        ..unhashed(path)?
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::read_archive_entry;
    use crate::sector::{ReadSeek, READ_BUDGET};
    use crate::test_io::Counting;
    use std::io::{Cursor, Read, Seek, SeekFrom, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// A identidade de um arquivo solto e quantos bytes foram lidos, de
    /// todos os arquivos (a imagem, o cue, a trilha), pra chegar nela.
    fn identify_counting(path: &Path, system: &str, hash_max: u64) -> (Identity, u64) {
        let read = Arc::new(AtomicU64::new(0));
        let open = |p: &Path| -> std::io::Result<Box<dyn ReadSeek>> {
            Ok(Box::new(Counting {
                inner: open_file(p)?,
                read: read.clone(),
            }))
        };
        let id = identify_with(path, system, None, &open, hash_max).unwrap();
        (id, read.load(Ordering::Relaxed))
    }

    fn digests_of(bytes: &[u8]) -> Digests {
        Identity::from(FileRomHasher::hash_reader(Cursor::new(bytes), "x").unwrap())
            .hashes
            .unwrap()
    }

    fn put(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// Dois arquivos de trilha de bytes diferentes (o cue diz o modo de cada um).
    fn tracks(dir: &Path) -> (Vec<u8>, Vec<u8>) {
        let t1 = vec![0xA5u8; 2352 * 4];
        let t2: Vec<u8> = (0..2352 * 8).map(|i| (i % 251) as u8).collect();
        put(dir, "Jogo (Track 1).bin", &t1);
        put(dir, "Jogo (Track 2).bin", &t2);
        (t1, t2)
    }

    #[test]
    fn cue_hashes_first_data_track() {
        let dir = tempfile::tempdir().unwrap();
        let (t1, t2) = tracks(dir.path());

        // Áudio primeiro: pulado; vale a 1ª trilha MODE1/ ou MODE2/.
        let text = "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\nFILE \"Jogo (Track 2).bin\" BINARY\n  TRACK 02 MODE2/2352\n    INDEX 00 00:00:00\n    INDEX 01 00:02:00\n";
        let cue = put(dir.path(), "Jogo.cue", text.as_bytes());
        let id = identify(&cue, "psx", None).unwrap();
        assert_eq!(id.name, "Jogo (Track 2).bin");
        assert_eq!(id.size, t2.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&t2)));
        assert_ne!(
            id.hashes,
            Some(digests_of(text.as_bytes())),
            "o texto do cue não é hasheado"
        );

        // Dados primeiro: a trilha 1 vale, mesmo com áudio depois.
        let text = "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\nFILE \"Jogo (Track 2).bin\" BINARY\n  TRACK 02 AUDIO\n    INDEX 01 00:00:00\n";
        let cue = put(dir.path(), "Dados.cue", text.as_bytes());
        let id = identify(&cue, "saturn", None).unwrap();
        assert_eq!(id.name, "Jogo (Track 1).bin");
        assert_eq!(id.size, t1.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&t1)));
    }

    #[test]
    fn gdi_hashes_track_3_for_dreamcast_when_present_else_first_data() {
        let dir = tempfile::tempdir().unwrap();
        let t1: Vec<u8> = vec![0x01; 2352 * 3];
        let t3: Vec<u8> = (0..2352 * 5).map(|i| (i % 13) as u8).collect();
        put(dir.path(), "track01.bin", &t1);
        put(dir.path(), "track02.raw", &[0x02; 2352 * 2]);
        put(dir.path(), "track 03.bin", &t3);

        let gdi = put(
            dir.path(),
            "Jogo.gdi",
            b"3\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n3 45000 4 2352 \"track 03.bin\" 0\n",
        );
        let id = identify(&gdi, "dreamcast", None).unwrap();
        assert_eq!(id.name, "track 03.bin");
        assert_eq!(id.size, t3.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&t3)));
        // O mesmo gdi como outro sistema (pasta que diz outra coisa): a área
        // de alta densidade é só do Dreamcast, vale a 1ª trilha de dados.
        let id = identify(&gdi, "psx", None).unwrap();
        assert_eq!(id.name, "track01.bin");
        assert_eq!(id.hashes, Some(digests_of(&t1)));

        // Sem a trilha 3 (sem área de alta densidade): a 1ª trilha de dados.
        let gdi = put(
            dir.path(),
            "Curto.gdi",
            b"2\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n",
        );
        let id = identify(&gdi, "dreamcast", None).unwrap();
        assert_eq!(id.name, "track01.bin");
        assert_eq!(id.hashes, Some(digests_of(&t1)));
    }

    #[test]
    fn missing_track_file_falls_back_to_name_and_size_of_cue() {
        let root = tempfile::tempdir().unwrap();
        let lib = root.path().join("lib");
        std::fs::create_dir_all(lib.join("Pasta.bin")).unwrap();
        // Existe, mas fora da pasta do cue: não é alcançado.
        put(root.path(), "Fora.bin", &[0x11; 4096]);
        let cue = |name: &str, file: &str, mode: &str| {
            put(
                &lib,
                name,
                format!("FILE \"{file}\" BINARY\n  TRACK 01 {mode}\n    INDEX 01 00:00:00\n")
                    .as_bytes(),
            )
        };
        // `provisional`: a trilha pode aparecer depois (calcula de novo); o
        // resto não muda com o tempo.
        for (sheet, provisional, why) in [
            (
                cue("Sumiu.cue", "Sumiu.bin", "MODE2/2352"),
                true,
                "trilha que não existe",
            ),
            (
                cue("Pasta.cue", "Pasta.bin", "MODE2/2352"),
                true,
                "trilha que é pasta",
            ),
            (
                cue("Sobe.cue", "../Fora.bin", "MODE2/2352"),
                false,
                "trilha fora da pasta",
            ),
            (cue("Musica.cue", "Pasta.bin", "AUDIO"), false, "só áudio"),
            (
                put(&lib, "Gd.gdi", b"1\n1 0 4 2352 nada.bin 0\n"),
                true,
                "gdi sem a trilha",
            ),
        ] {
            let id = identify(&sheet, "psx", None).unwrap();
            let own = sheet.file_name().unwrap().to_string_lossy().into_owned();
            assert_eq!(id.name, own, "{why}");
            assert_eq!(id.size, std::fs::metadata(&sheet).unwrap().len(), "{why}");
            assert_eq!(id.hashes, None, "{why}");
            assert_eq!(id.provisional, provisional, "{why}");
        }
    }

    /// Cue gravado com BOM (Bloco de Notas antigo) e cue/gdi em Windows-1252
    /// citando `Pokémon.bin`: a trilha é achada e hasheada.
    #[test]
    fn sheet_with_bom_or_cp1252_finds_its_track() {
        let dir = tempfile::tempdir().unwrap();
        let track: Vec<u8> = (0..2352 * 4).map(|i| (i % 19) as u8).collect();
        put(dir.path(), "Pokémon.bin", &track);
        let cue = "FILE \"Pokémon.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";

        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend_from_slice(cue.as_bytes());
        let cp1252 =
            b"FILE \"Pok\xE9mon.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
        let gdi = b"1\n1 0 4 2352 \"Pok\xE9mon.bin\" 0\n";
        for (sheet, bytes, system) in [
            ("Bom.cue", &bom[..], "psx"),
            ("Antigo.cue", &cp1252[..], "psx"),
            ("Antigo.gdi", &gdi[..], "dreamcast"),
        ] {
            let path = put(dir.path(), sheet, bytes);
            let id = identify(&path, system, None).unwrap();
            assert_eq!(id.name, "Pokémon.bin", "{sheet}");
            assert_eq!(id.hashes, Some(digests_of(&track)), "{sheet}");
            assert!(!id.provisional, "{sheet}");
        }
    }

    /// Fonte que entrega `left` bytes e finge que dá pra voltar: um arquivo
    /// que cresce enquanto é lido (no disco ele mede menos).
    struct Growing {
        left: u64,
    }

    impl Read for Growing {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf
                .len()
                .min(usize::try_from(self.left).unwrap_or(usize::MAX));
            buf[..n].fill(0x22);
            self.left -= n as u64;
            Ok(n)
        }
    }

    impl Seek for Growing {
        fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
            Ok(0)
        }
    }

    /// Entrada guardada sem compressão (`.zip` ou `.7z` em COPY) de 128 KiB,
    /// com um byte estragado perto do fim: lida até o fim, o CRC não bate.
    fn bomb(dir: &Path, archive: &str) -> PathBuf {
        let path = dir.join(archive);
        let data = vec![0x5Au8; 128 * 1024];
        if archive.ends_with(".zip") {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            zw.start_file(
                "Bomba.bin",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zw.write_all(&data).unwrap();
            zw.finish().unwrap();
        } else {
            let mut w = sevenz_rust2::ArchiveWriter::create(&path).unwrap();
            w.set_content_methods(vec![sevenz_rust2::EncoderConfiguration::new(
                sevenz_rust2::EncoderMethod::COPY,
            )]);
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Bomba.bin"),
                Some(Cursor::new(data)),
            )
            .unwrap();
            w.finish().unwrap();
        }
        let mut bytes = std::fs::read(&path).unwrap();
        let at = bytes.windows(4096).position(|w| w == [0x5A; 4096]).unwrap();
        bytes[at + 120 * 1024] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        path
    }

    /// Toda leitura de identidade para no limite, não só a de imagem de
    /// disco: a entrada de cartucho grande fica com o índice; a leitura de
    /// uma entrada que passa do limite para nele, valha o índice o que valer
    /// (zip bomb); e o arquivo solto que cresce enquanto é lido também.
    #[test]
    fn every_identity_read_stops_at_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        const MIB: u64 = 1024 * 1024;

        // Cartucho (`.nes`) de 1 MiB num zip, limite de 64 KiB: pelo índice.
        let zip_path = dir.path().join("Cartucho.zip");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            zw.start_file(
                "Jogo (USA).nes",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
            std::io::copy(&mut std::io::repeat(0x5A).take(MIB), &mut zw).unwrap();
            zw.finish().unwrap();
        }
        let id = identify_with(
            &zip_path,
            "nes",
            Some("Jogo (USA).nes"),
            &open_file,
            64 * 1024,
        )
        .unwrap();
        assert_eq!(
            (id.hashes, id.name.as_str(), id.size),
            (None, "Jogo (USA).nes", MIB)
        );

        // A leitura em si para no limite: com 32 KiB ela nunca chega ao byte
        // estragado nem ao CRC do fim. Controle: lida inteira, falha.
        for archive in ["Bomba.zip", "Bomba.7z"] {
            let path = bomb(dir.path(), archive);
            assert!(
                hash_archive_entry(&path, "Bomba.bin", "Bomba.bin", MIB).is_err(),
                "{archive}: controle"
            );
            let h = hash_archive_entry(&path, "Bomba.bin", "Bomba.bin", 32 * 1024).unwrap();
            assert_eq!(h, None, "{archive}");
        }

        // No disco 64 KiB (dentro do limite de 1 MiB), mas a leitura entrega
        // 16 MiB: para em 1 MiB + 1 byte, sem hash, com o tamanho do disco.
        let rom = put(dir.path(), "Jogo.gba", &[0x11; 64 * 1024]);
        let read = Arc::new(AtomicU64::new(0));
        let growing = |_: &Path| -> std::io::Result<Box<dyn ReadSeek>> {
            Ok(Box::new(Counting {
                inner: Box::new(Growing { left: 16 * MIB }),
                read: read.clone(),
            }))
        };
        let id = identify_with(&rom, "gba", None, &growing, MIB).unwrap();
        assert_eq!(id.hashes, None);
        assert_eq!((id.name.as_str(), id.size), ("Jogo.gba", 64 * 1024));
        assert_eq!(read.load(Ordering::Relaxed), MIB + 1);
    }

    /// Cue cujo texto não se lê, ou cuja trilha ainda não chegou: provisório.
    /// Quando a trilha aparece, a mesma conta dá a identidade dela.
    #[test]
    fn unreadable_sheet_or_missing_track_is_identified_again_later() {
        let dir = tempfile::tempdir().unwrap();
        let text =
            "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
        let cue = put(dir.path(), "Jogo.cue", text.as_bytes());

        let id = identify(&cue, "psx", None).unwrap();
        assert!(id.provisional, "trilha que ainda não chegou");
        assert_eq!((id.name.as_str(), id.hashes.as_ref()), ("Jogo.cue", None));

        let track: Vec<u8> = (0..2352 * 4).map(|i| (i % 17) as u8).collect();
        put(dir.path(), "Jogo (Track 1).bin", &track);
        let id = identify(&cue, "psx", None).unwrap();
        assert!(!id.provisional);
        assert_eq!(id.name, "Jogo (Track 1).bin");
        assert_eq!(id.hashes, Some(digests_of(&track)));

        // O texto do cue não se lê (o arquivo existe, a leitura falha).
        let refuse = |p: &Path| -> std::io::Result<Box<dyn ReadSeek>> {
            if p.extension().is_some_and(|e| e == "cue") {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            open_file(p)
        };
        let id = identify_with(&cue, "psx", None, &refuse, DISC_HASH_MAX).unwrap();
        assert!(id.provisional, "texto que não se lê");
        assert_eq!((id.name.as_str(), id.hashes), ("Jogo.cue", None));
    }

    /// Zip com uma entrada guardada sem compressão cujos dados foram
    /// estragados: ler a entrada falha (o CRC não bate).
    fn corrupt_zip(dir: &Path, entry: &str) -> PathBuf {
        let zip_path = dir.join(format!("{entry}.zip"));
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            zw.start_file(
                entry,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zw.write_all(&vec![0x5A; 64 * 1024]).unwrap();
            zw.finish().unwrap();
        }
        let mut bytes = std::fs::read(&zip_path).unwrap();
        let data = bytes.windows(64).position(|w| w == [0x5A; 64]).unwrap();
        bytes[data + 1000] ^= 0xFF;
        std::fs::write(&zip_path, &bytes).unwrap();
        assert!(read_archive_entry(&zip_path, entry).is_err());
        zip_path
    }

    /// Imagem de Saturn com `size` bytes: o cabeçalho do setor 0 e o resto
    /// preenchido.
    fn saturn_image(size: usize) -> Vec<u8> {
        let mut img = vec![0x5Au8; size];
        img[..16].copy_from_slice(b"SEGA SEGASATURN ");
        img[0x20..0x2A].copy_from_slice(b"MK-81022  ");
        img
    }

    /// `.cso` e `.chd` em qualquer tamanho, e o `.iso` acima do limite: o
    /// nome, o tamanho e o serial, sem ler a imagem inteira.
    #[test]
    fn cso_chd_and_iso_above_the_cap_are_never_hashed_whole() {
        let dir = tempfile::tempdir().unwrap();
        // Maior que o orçamento do serial (2 MiB): ler inteiro passaria dele.
        const SIZE: usize = 3 * 1024 * 1024;
        // Abaixo do tamanho da imagem: o `.iso` fica acima do limite.
        const CAP: u64 = 1024 * 1024;
        let iso = put(dir.path(), "Jogo (USA).iso", &saturn_image(SIZE));
        let cso = put(dir.path(), "Jogo (USA).cso", &vec![0x5A; SIZE]);
        let chd = put(dir.path(), "Jogo (USA).chd", &vec![0x5A; SIZE]);

        for (path, cap) in [(&iso, CAP), (&cso, DISC_HASH_MAX), (&chd, DISC_HASH_MAX)] {
            // Sistema sem serial: a identidade não lê um byte.
            let (id, read) = identify_counting(path, "disc", cap);
            assert_eq!(read, 0, "{path:?}");
            assert_eq!(id.hashes, None, "{path:?}");
            assert_eq!(id.size, SIZE as u64);
            assert_eq!(
                id.name,
                path.file_name().unwrap().to_string_lossy().into_owned()
            );
        }
        // Com o sistema, só o que o serial lê, dentro do orçamento.
        for (path, system, cap) in [
            (&iso, "saturn", CAP),
            (&cso, "psp", DISC_HASH_MAX),
            (&chd, "psx", DISC_HASH_MAX),
        ] {
            let (id, read) = identify_counting(path, system, cap);
            assert!(read <= READ_BUDGET, "{system}: leu {read} bytes");
            assert_eq!(id.hashes, None, "{system}");
        }
        let (id, _) = identify_counting(&iso, "saturn", CAP);
        assert_eq!(id.serial.as_deref(), Some("MK-81022"));

        // Dentro de um `.zip`: o tamanho vem do índice e nada é
        // descomprimido (ler a entrada estragada falharia).
        let zip_chd = corrupt_zip(dir.path(), "Jogo (USA).chd");
        let id = identify(&zip_chd, "psx", Some("Jogo (USA).chd")).unwrap();
        assert_eq!(id.hashes, None);
        assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).chd", 64 * 1024));
        let zip_iso = corrupt_zip(dir.path(), "Jogo (USA).iso");
        let id = identify_with(
            &zip_iso,
            "saturn",
            Some("Jogo (USA).iso"),
            &open_file,
            32 * 1024,
        )
        .unwrap();
        assert_eq!(id.hashes, None, "iso acima do limite dentro do zip");
        assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).iso", 64 * 1024));

        // E de um `.7z`, também pelo índice.
        let sz = dir.path().join("Jogo.7z");
        let mut w = sevenz_rust2::ArchiveWriter::create(&sz).unwrap();
        w.push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("Jogo (USA).chd"),
            Some(Cursor::new(vec![0x5A; 5000])),
        )
        .unwrap();
        w.finish().unwrap();
        let id = identify(&sz, "psx", Some("Jogo (USA).chd")).unwrap();
        assert_eq!(id.hashes, None);
        assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).chd", 5000));
    }

    /// Um `.iso` até o limite tem o hash (CRC32, MD5 e SHA1 numa leitura só)
    /// e o serial; um byte acima, só o serial, o nome e o tamanho. Dentro de
    /// um `.zip`, o mesmo limite, hasheado aos poucos.
    #[test]
    fn iso_up_to_the_cap_is_hashed_with_its_serial() {
        let dir = tempfile::tempdir().unwrap();
        const SIZE: usize = 3 * 1024 * 1024;
        let img = saturn_image(SIZE);
        let iso = put(dir.path(), "Jogo (USA).iso", &img);

        // Abaixo do limite de verdade, e exatamente no limite.
        for cap in [DISC_HASH_MAX, SIZE as u64] {
            let (id, read) = identify_counting(&iso, "saturn", cap);
            assert_eq!(id.hashes, Some(digests_of(&img)), "limite {cap}");
            assert_eq!(id.serial.as_deref(), Some("MK-81022"), "limite {cap}");
            assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).iso", SIZE as u64));
            // Uma leitura inteira pro hash, mais o pouco que o serial lê.
            let whole = SIZE as u64;
            assert!((whole..=whole + READ_BUDGET).contains(&read), "leu {read}");
        }

        // Um byte acima do limite: sem a leitura inteira.
        let (id, read) = identify_counting(&iso, "saturn", SIZE as u64 - 1);
        assert_eq!(id.hashes, None);
        assert_eq!(id.serial.as_deref(), Some("MK-81022"));
        assert_eq!(id.size, SIZE as u64);
        assert!(read <= READ_BUDGET, "leu {read}");

        let zip_path = dir.path().join("Jogo.zip");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            zw.start_file(
                "Jogo (USA).iso",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
            zw.write_all(&img).unwrap();
            zw.finish().unwrap();
        }
        let id = identify(&zip_path, "saturn", Some("Jogo (USA).iso")).unwrap();
        assert_eq!(id.hashes, Some(digests_of(&img)));
        assert_eq!(id.size, SIZE as u64);
        let id = identify_with(
            &zip_path,
            "saturn",
            Some("Jogo (USA).iso"),
            &open_file,
            SIZE as u64 - 1,
        )
        .unwrap();
        assert_eq!((id.hashes, id.size), (None, SIZE as u64));
    }

    /// Trilha ou imagem acima do limite: o nome e o tamanho dela, sem ler.
    /// O limite de verdade (1,5 GiB) está preso por um `assert!` de compilação
    /// junto do [`DISC_HASH_MAX`]; aqui ele vem pequeno pra não gravar GBs.
    #[test]
    fn disc_image_above_the_limit_is_identified_by_name_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let track: Vec<u8> = (0..64 * 1024).map(|i| (i % 7) as u8).collect();
        put(dir.path(), "Jogo (Track 1).bin", &track);
        let text = "FILE \"Jogo (Track 1).bin\" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
";
        let cue = put(dir.path(), "Jogo.cue", text.as_bytes());

        let (id, read) = identify_counting(&cue, "disc", 32 * 1024);
        assert_eq!(id.name, "Jogo (Track 1).bin", "a trilha, não o cue");
        assert_eq!(id.size, track.len() as u64);
        assert_eq!(id.hashes, None);
        assert_eq!(read, text.len() as u64, "só o texto do cue foi lido");
        // No limite: a trilha é lida e hasheada.
        let (id, _) = identify_counting(&cue, "disc", track.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&track)));

        // Imagem solta (`.img`, `.pbp`...) segue a mesma regra.
        let img = put(dir.path(), "Jogo.img", &track);
        let (id, read) = identify_counting(&img, "disc", 32 * 1024);
        assert_eq!((id.hashes, id.size, read), (None, track.len() as u64, 0));
        let (id, _) = identify_counting(&img, "disc", track.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&track)));
    }
}
