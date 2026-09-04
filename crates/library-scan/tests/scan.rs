//! Varredura ponta a ponta: diretório com arquivos fake → `RomsRepo`
//! (SQLite in-memory).

use domain::library::RomRepository;
use library_scan::{scan_into, ScanReport};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

fn scratch_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "reemu-scan-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::File::create(p).unwrap().write_all(bytes).unwrap();
}

#[tokio::test]
async fn scans_recursively_infers_system_and_dedups() {
    let dir = scratch_dir();
    write(&dir, "Mario.nes", b"nes-rom-data");
    write(&dir, "Zelda.sfc", b"snes-rom-data");
    write(&dir, "sub/Metroid.nes", b"another-nes");
    write(&dir, "readme.txt", b"not a rom"); // extensão desconhecida
    write(&dir, "cover.png", b"\x89PNG"); // idem

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db.clone());

    let r = scan_into(&repo, &dir, 1_700_000_000, |_| {}).await.unwrap();
    assert_eq!(
        r,
        ScanReport {
            found: 3,
            added: 3,
            skipped_known: 0,
            skipped_unrecognized: 2,
            errors: 0,
        }
    );

    assert_eq!(repo.list_by_system("nes").await.unwrap().len(), 2);
    assert_eq!(repo.list_by_system("snes").await.unwrap().len(), 1);

    let zelda = repo
        .find_by_path(&dir.join("Zelda.sfc").to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(zelda.system_id, "snes");
    assert_eq!(zelda.crc32.len(), 8);
    assert_eq!(zelda.md5.len(), 32);

    // 2ª varredura: nada novo (dedup por file_path)
    let r2 = scan_into(&repo, &dir, 1_700_000_001, |_| {}).await.unwrap();
    assert_eq!(r2.added, 0);
    assert_eq!(r2.skipped_known, 3);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn rom_inside_zip_is_catalogued_by_inner_extension_and_hash() {
    let dir = scratch_dir();
    let rom_bytes = b"n64-rom-payload-inside-zip";

    // .zip com uma ROM .z64 dentro (store, sem compressão).
    let zip_path = dir.join("Mario 64.zip");
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file(
            "Super Mario 64 (USA).z64",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zw.write_all(rom_bytes).unwrap();
        zw.finish().unwrap();
    }

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 1);
    assert_eq!(r.skipped_unrecognized, 0);

    let n64 = repo.list_by_system("n64").await.unwrap();
    assert_eq!(n64.len(), 1);
    // hash é o da ROM crua, não o do .zip
    assert_eq!(n64[0].crc32, crc_of(rom_bytes));
    assert_eq!(
        n64[0].file_path,
        zip_path.to_string_lossy(),
        "file_path aponta pro .zip"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn same_rom_different_paths_both_catalogued() {
    let dir = scratch_dir();
    write(&dir, "a/Game.gba", b"identical-bytes");
    write(&dir, "b/Game.gba", b"identical-bytes");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 2);

    let by_crc = repo
        .find_by_crc32(&crc_of(b"identical-bytes"))
        .await
        .unwrap();
    assert_eq!(by_crc.len(), 2, "mesmo hash, dois caminhos");
    let _ = std::fs::remove_dir_all(dir);
}

fn crc_of(bytes: &[u8]) -> String {
    let mut h = crc32fast::Hasher::new();
    h.update(bytes);
    format!("{:08X}", h.finalize())
}

#[tokio::test]
async fn disc_extension_disambiguated_by_folder_name() {
    let dir = scratch_dir();
    write(&dir, "psx/Crash Bandicoot (USA).iso", b"psx-disc-image");
    write(&dir, "dreamcast/Shenmue (USA).cue", b"dreamcast-disc-image");
    // sem pasta reconhecida → cai no balde genérico de sempre (regressão).
    write(&dir, "Some Game (USA).iso", b"unlabeled-disc-image");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 3);
    assert_eq!(r.skipped_unrecognized, 0);

    assert_eq!(repo.list_by_system("psx").await.unwrap().len(), 1);
    assert_eq!(repo.list_by_system("dreamcast").await.unwrap().len(), 1);
    assert_eq!(
        repo.list_by_system("disc").await.unwrap().len(),
        1,
        "sem pasta de sistema, continua caindo no balde genérico"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn arcade_zip_without_a_cartridge_entry_is_catalogued_by_folder() {
    let dir = scratch_dir();
    let set_bytes = b"fake-mame-romset-bytes";

    // Set de arcade real: entradas sem extensão de ROM de cartucho (chip
    // dumps avulsos) — hoje isso faria `peek_zip` devolver `None`.
    let zip_path = dir.join("arcade/sfiii3n.zip");
    std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zw.start_file("sfiii3n.06", opts).unwrap();
        zw.write_all(set_bytes).unwrap();
        zw.start_file("sfiii3n.key", opts).unwrap();
        zw.write_all(b"more-chip-data").unwrap();
        zw.finish().unwrap();
    }

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 1, "não pode ficar skipped_unrecognized");
    assert_eq!(r.skipped_unrecognized, 0);

    let arcade = repo.list_by_system("arcade").await.unwrap();
    assert_eq!(arcade.len(), 1);
    // hash é do .zip INTEIRO (não tem "a ROM crua" — o set é a unidade).
    let whole_zip = std::fs::read(&zip_path).unwrap();
    assert_eq!(arcade[0].crc32, crc_of(&whole_zip));
    let _ = std::fs::remove_dir_all(dir);
}
