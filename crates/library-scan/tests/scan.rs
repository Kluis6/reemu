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
async fn daphne_scan_pairs_rom_and_media_with_the_framefile() {
    let dir = scratch_dir();
    write(
        &dir,
        "Arcade/framefile/lair.txt",
        b"../vldp/lair\r\n0 color.m2v\r\n",
    );
    write(
        &dir,
        "Arcade/framefile/readme.txt",
        b"Instructions\nLabel value\n",
    );
    write(&dir, "Arcade/roms/lair.zip", b"test rom zip");
    write(&dir, "Arcade/roms/unrelated.zip", b"existing arcade set");
    write(&dir, "Arcade/vldp/lair/color.m2v", b"test video");
    write(&dir, "Arcade/vldp/lair/color.ogg", b"test audio");
    write(&dir, "Daphne/vldp/ace/ace.txt", b".\n0 ace.M2V\n");
    write(&dir, "Daphne/launch.bat", b"example");
    write(&dir, "example.txt", b".\n0 demo.m2v\n");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 1, |_| {}).await.unwrap();
    assert_eq!(report.added, 3);
    let laserdisc = repo.list_by_system("laserdisc").await.unwrap();
    assert_eq!(laserdisc.len(), 2);
    assert!(laserdisc.iter().all(|rom| rom.file_path.ends_with(".txt")));
    let arcade = repo.list_by_system("arcade").await.unwrap();
    assert_eq!(arcade.len(), 1);
    assert!(arcade[0].file_path.ends_with("unrelated.zip"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn laserdisc_scan_accepts_game_videos_and_ignores_unrelated_ogv() {
    let dir = scratch_dir();
    write(&dir, "outros/LAIR.capture.OGV", b"OggS");
    write(&dir, "Cliff.dirksimple", b"OggS");
    write(&dir, "Laser Disc/Teste/abertura.ogv", b"OggS");
    write(&dir, "Videos/demo.ogv", b"OggS");
    let mut archive =
        zip::ZipWriter::new(std::fs::File::create(dir.join("laserdisc.zip")).unwrap());
    archive
        .start_file("cliff.dirksimple", zip::write::SimpleFileOptions::default())
        .unwrap();
    archive.write_all(b"OggS").unwrap();
    archive.finish().unwrap();

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 1, |_| {}).await.unwrap();
    assert_eq!(report.added, 3);
    let media = repo.list_by_system("laserdisc").await.unwrap();
    assert_eq!(media.len(), 3);
    assert!(media
        .iter()
        .all(|rom| rom.ident_size == Some(4) && rom.crc32.is_empty() && rom.md5.is_empty()));
    let _ = std::fs::remove_dir_all(&dir);
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
            reclassified: 0,
            removed: 0,
            unrecognized_archives: 0,
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
async fn snes_expansions_and_atari_st_disks_scan_as_distinct_systems() {
    let dir = scratch_dir();
    let mut cart = vec![0; 0x80000];
    cart[..14].copy_from_slice(b"BANDAI SFC-ADX");
    write(&dir, "Broadcast.bs", b"fixture");
    write(&dir, "Cartridge.st", &cart);
    let disk = vec![0; 720 * 1024];
    write(&dir, "Atari.st", &disk);
    write(&dir, "Invalid.st", &vec![0; 0x80000]);
    write(&dir, "Downloads/Unrelated.gem", b"ruby package");
    write(&dir, "Atari ST/Two Disks.m3u", b"../Atari.st\n");
    write(&dir, "Atari ST/Two Disks UTF-8.m3u8", b"../Atari.st\n");
    let packed = dir.join("Cartridge.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&packed).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("Invalid.st", options).unwrap();
    zip.write_all(&vec![0; 0x80000]).unwrap();
    zip.start_file("folder/Cartridge.ST", options).unwrap();
    zip.write_all(&cart).unwrap();
    zip.finish().unwrap();

    let mut sevenz = sevenz_rust2::ArchiveWriter::create(dir.join("Cartridge.7z")).unwrap();
    sevenz
        .push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("folder/Cartridge.st"),
            Some(std::io::Cursor::new(&cart)),
        )
        .unwrap();
    drop(sevenz.finish().unwrap());

    let disk_zip = dir.join("Atari.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&disk_zip).unwrap());
    zip.start_file("folder/Atari.ST", options).unwrap();
    zip.write_all(&disk).unwrap();
    zip.finish().unwrap();

    let repo = db::RomsRepo::new(db::connect_in_memory().await.unwrap());
    let report = scan_into(&repo, &dir, 1, |_| {}).await.unwrap();
    assert_eq!(report.added, 8);
    let games = repo.list_by_system("snes").await.unwrap();
    assert_eq!(games.len(), 4);
    let disks = repo.list_by_system("atarist").await.unwrap();
    assert_eq!(disks.len(), 4);
    assert!(disks.iter().any(|rom| rom.file_path.ends_with("Atari.st")));
    assert!(disks
        .iter()
        .any(|rom| rom.file_path.ends_with("Two Disks.m3u")));
    assert!(disks.iter().any(|rom| {
        rom.file_path.ends_with("Atari.zip") && rom.ident_name.as_deref() == Some("Atari.ST")
    }));
    for (archive, entry) in [
        ("Cartridge.zip", "Cartridge.ST"),
        ("Cartridge.7z", "Cartridge.st"),
    ] {
        assert!(games.iter().any(|rom| {
            rom.file_path.ends_with(archive) && rom.ident_name.as_deref() == Some(entry)
        }));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn sharp_x68000_media_preserves_atari_and_amiga_classification() {
    let dir = scratch_dir();
    let xdf = vec![0; 77 * 2 * 8 * 1024];
    let mut dim = vec![0; 256 + xdf.len()];
    dim[1..=154].fill(1);
    write(&dir, "Disk.xdf", &xdf);
    write(&dir, "Sharp Disk.dim", &dim);
    write(&dir, "Atari ST/Atari Disk.dim", &[0x42; 64]);
    write(&dir, "Sharp X68000/Hard disk.hdf", b"x68000 hdf");
    write(&dir, "Amiga/Hard disk.hdf", b"amiga hdf");
    write(&dir, "Sharp X68000/Two Drives.m3u", b"../Disk.xdf\n");
    write(&dir, "Migrate.cmd", b"not emulator media");

    for (file, entry, data) in [
        ("Disks.zip", "nested/Sharp Disk.DIM", dim.as_slice()),
        (
            "Sharp X68000/Shared extensions.zip",
            "Disk 2.IMG",
            xdf.as_slice(),
        ),
    ] {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join(file)).unwrap());
        zip.start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
        zip.finish().unwrap();
    }

    let repo = db::RomsRepo::new(db::connect_in_memory().await.unwrap());
    let report = scan_into(&repo, &dir, 1, |_| {}).await.unwrap();
    assert_eq!(report.added, 8);
    assert_eq!(repo.list_by_system("atarist").await.unwrap().len(), 1);
    assert_eq!(repo.list_by_system("amiga").await.unwrap().len(), 1);
    let sharp = repo.list_by_system("x68000").await.unwrap();
    assert_eq!(sharp.len(), 6);
    for name in ["Disks.zip", "Shared extensions.zip"] {
        let archive = sharp
            .iter()
            .find(|rom| rom.file_path.ends_with(name))
            .unwrap();
        assert_eq!(archive.ident_name.as_deref(), Some(name));
        assert_eq!(
            archive.ident_size,
            Some(std::fs::metadata(&archive.file_path).unwrap().len() as i64)
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn zx81_tapes_keep_zx_spectrum_and_other_p_files_separate() {
    let dir = scratch_dir();
    let mut program = vec![0; 0x407f - 0x4009];
    program[3..5].copy_from_slice(&0x407du16.to_le_bytes());
    program[7..9].copy_from_slice(&0x407eu16.to_le_bytes());
    program[11..13].copy_from_slice(&0x407fu16.to_le_bytes());
    write(&dir, "jogo.P", &program);
    write(&dir, "exemplo.p", b"program example; begin end.");
    write(&dir, "colecao.t81", b"EO81");
    write(&dir, "Sinclair - ZX 81/jogo.tzx", b"ZXTape!");
    write(&dir, "zxspectrum/jogo.tzx", b"ZXTape!");

    for (folder, name, contents) in [
        ("Sinclair - ZX 81", "fita.TZX", &b"ZXTape!"[..]),
        ("zxspectrum", "fita.TZX", &b"ZXTape!"[..]),
        ("", "jogo.p", program.as_slice()),
    ] {
        let mut zip =
            zip::ZipWriter::new(std::fs::File::create(dir.join(folder).join("midia.zip")).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("exemplo.p", opts).unwrap();
        zip.write_all(b"program example; begin end.").unwrap();
        zip.start_file(name, opts).unwrap();
        zip.write_all(contents).unwrap();
        zip.finish().unwrap();
    }

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (7, 0), "{report:?}");
    let zx81 = repo.list_by_system("zx81").await.unwrap();
    let spectrum = repo.list_by_system("zxspectrum").await.unwrap();
    assert_eq!((zx81.len(), spectrum.len()), (5, 2));
    assert!(zx81
        .iter()
        .any(|rom| rom.ident_name.as_deref() == Some("jogo.p")));
    for roms in [&zx81, &spectrum] {
        assert!(roms
            .iter()
            .any(|rom| rom.ident_name.as_deref() == Some("fita.TZX")));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn arduboy_packages_and_hex_need_different_import_contexts() {
    let dir = scratch_dir();
    let hex = b":02000000FFCF30\n:00000001FF\n";
    write(&dir, "firmware.hex", hex);
    write(&dir, "Arduboy FX/sub/jogo.HEX", hex);
    write(&dir, "pacote.arduboy", b"pacote inteiro");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("externo.zip")).unwrap());
    zip.start_file("jogo.arduboy", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"pacote inteiro").unwrap();
    zip.finish().unwrap();

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (2, 0), "{report:?}");
    let roms = repo.list_by_system("arduboy").await.unwrap();
    assert_eq!(roms.len(), 2);
    assert!(roms.iter().any(|rom| rom.file_path.ends_with("jogo.HEX")));
    assert!(roms
        .iter()
        .any(|rom| rom.file_path.ends_with("pacote.arduboy")));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn uzebox_scan_uses_the_rom_entry_inside_zip() {
    let dir = scratch_dir();
    let rom = b"Uzebox test data";
    write(&dir, "jogo.UZE", rom);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("compactado.zip")).unwrap());
    zip.start_file("jogo.uze", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(rom).unwrap();
    zip.finish().unwrap();

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (2, 0), "{report:?}");
    let games = repo.list_by_system("uzebox").await.unwrap();
    assert_eq!(games.len(), 2);
    assert!(games.iter().all(|game| game.crc32 == crc_of(rom)));
    assert!(games
        .iter()
        .any(|game| game.ident_name.as_deref() == Some("jogo.uze")));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn vaporspec_scan_uses_the_rom_entry_inside_zip() {
    let dir = scratch_dir();
    let rom = b"\x01\x00\x00\x06";
    write(&dir, "jogo.VAPORBIN", rom);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("compactado.zip")).unwrap());
    zip.start_file("jogo.vaporbin", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(rom).unwrap();
    zip.finish().unwrap();

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (2, 0), "{report:?}");
    let games = repo.list_by_system("vaporspec").await.unwrap();
    assert_eq!(games.len(), 2);
    assert!(games.iter().all(|game| game.crc32 == crc_of(rom)));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn megaduck_bin_needs_a_platform_folder_and_duck_takes_priority() {
    let dir = scratch_dir();
    write(&dir, "outro.bin", b"ambiguous");
    write(&dir, "jogo.DUCK", b"plain duck");
    write(&dir, "Cougar Boy/pasta/avulso.BIN", b"raw bin");
    std::fs::create_dir(dir.join("Mega Duck")).unwrap();
    for (path, explicit_duck) in [
        ("Mega Duck/bin.zip", false),
        ("Mega Duck/misto.zip", true),
        ("ignorar.zip", false),
    ] {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join(path)).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("outro.bin", options).unwrap();
        zip.write_all(b"generic bin").unwrap();
        if explicit_duck {
            zip.start_file("jogo.DUCK", options).unwrap();
            zip.write_all(b"explicit duck").unwrap();
        }
        zip.finish().unwrap();
    }

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (4, 0), "{report:?}");
    let games = repo.list_by_system("megaduck").await.unwrap();
    assert_eq!(games.len(), 4);
    let mixed = games
        .iter()
        .find(|game| game.file_path.ends_with("misto.zip"))
        .unwrap();
    assert_eq!(mixed.ident_name.as_deref(), Some("jogo.DUCK"));
    assert_eq!(mixed.crc32, crc_of(b"explicit duck"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn chip8_scan_recognizes_variants_as_one_platform() {
    let dir = scratch_dir();
    let rom_bytes = b"\x00\xe0\x12\x00";
    for ext in ["ch8", "sc8", "xo8", "hc8"] {
        write(&dir, &format!("teste.{ext}"), rom_bytes);
    }
    let mut zip = zip::ZipWriter::new(std::fs::File::create(dir.join("teste.zip")).unwrap());
    zip.start_file("jogo.XO8", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(rom_bytes).unwrap();
    zip.finish().unwrap();

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (5, 0), "{report:?}");
    let roms = repo.list_by_system("chip8").await.unwrap();
    assert_eq!(roms.len(), 5);
    assert!(roms.iter().all(|rom| rom.crc32 == crc_of(rom_bytes)));
    let packed = roms
        .iter()
        .find(|rom| rom.file_path.ends_with(".zip"))
        .unwrap();
    assert_eq!(packed.ident_name.as_deref(), Some("jogo.XO8"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn msx_media_with_ambiguous_extensions_needs_an_msx_folder() {
    let dir = scratch_dir();
    let rom_bytes = b"MSX cartridge in ZIP";
    for folder in ["MSX2+", "outros"] {
        write(&dir, &format!("{folder}/boot.dmk"), b"disk image");
        let archive = dir.join(folder).join("cart.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(archive).unwrap());
        zip.start_file("cart.ROM", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(rom_bytes).unwrap();
        zip.finish().unwrap();
    }
    write(&dir, "MSX2+/boot.m3u", b"boot.dmk\n");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let report = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((report.added, report.errors), (3, 0), "{report:?}");
    let msx = repo.list_by_system("msx").await.unwrap();
    assert_eq!(msx.len(), 3);
    let cart = msx
        .iter()
        .find(|rom| rom.file_path.ends_with("cart.zip"))
        .unwrap();
    assert_eq!(cart.ident_name.as_deref(), Some("cart.ROM"));
    assert_eq!(cart.crc32, crc_of(rom_bytes));
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
async fn rom_inside_7z_is_catalogued_by_inner_extension_and_hash() {
    let dir = scratch_dir();
    let rom_bytes = b"n64-rom-payload-inside-7z";

    // .7z com uma ROM .z64 dentro.
    let archive_path = dir.join("Mario 64.7z");
    {
        let mut w = sevenz_rust2::ArchiveWriter::create(&archive_path).unwrap();
        w.push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("Super Mario 64 (USA).z64"),
            Some(std::io::Cursor::new(rom_bytes.to_vec())),
        )
        .unwrap();
        w.finish().unwrap();
    }

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 1);
    assert_eq!(r.skipped_unrecognized, 0);

    let n64 = repo.list_by_system("n64").await.unwrap();
    assert_eq!(n64.len(), 1);
    // hash é o da ROM crua, não o do .7z
    assert_eq!(n64[0].crc32, crc_of(rom_bytes));
    assert_eq!(
        n64[0].file_path,
        archive_path.to_string_lossy(),
        "file_path aponta pro .7z"
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

/// Zip de set de arcade (chip dumps, sem cartucho dentro).
fn write_arcade_zip(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let f = std::fs::File::create(path).unwrap();
    let mut zw = zip::ZipWriter::new(f);
    let opts =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zw.start_file("chip.p1", opts).unwrap();
    zw.write_all(path.to_string_lossy().as_bytes()).unwrap();
    zw.finish().unwrap();
}

/// FinalBurn Neo sem `.dat`: o set entra pela pasta (inclusive nomes como
/// `fbalpha` e `mame2003-plus`); BIOS de placa (`neogeo.zip`, `pgm.zip`) não
/// entra como jogo e sai se uma varredura antiga a catalogou; um set numa
/// pasta sem nome de arcade é contado à parte pra tela avisar.
#[tokio::test]
async fn arcade_bios_skipped_aliases_and_unrecognized_archives() {
    let dir = scratch_dir();
    write_arcade_zip(&dir.join("fbneo/kof98.zip"));
    write_arcade_zip(&dir.join("fbneo/neogeo.zip"));
    write_arcade_zip(&dir.join("fbalpha/mslug.zip"));
    write_arcade_zip(&dir.join("mame2003-plus/pgm.zip"));
    write_arcade_zip(&dir.join("mame2003-plus/1941.zip"));
    write_arcade_zip(&dir.join("jogos/sf2.zip"));

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    // varredura antiga catalogou a BIOS como jogo
    repo.add(&domain::library::Rom {
        id: "velho".into(),
        file_path: dir
            .join("fbneo")
            .join("neogeo.zip")
            .to_string_lossy()
            .into_owned(),
        crc32: String::new(),
        md5: String::new(),
        system_id: "arcade".into(),
        added_at: 0,
        last_played_at: None,
        is_favorite: false,
        user_title: None,
        ..Default::default()
    })
    .await
    .unwrap();

    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 3, "kof98, mslug e 1941");
    assert_eq!(r.removed, 1, "a neogeo.zip antiga sai");
    assert_eq!(
        r.unrecognized_archives, 1,
        "sf2.zip fora de pasta de arcade"
    );
    let mut names: Vec<String> = repo
        .list_by_system("arcade")
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            std::path::Path::new(&r.file_path)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["1941", "kof98", "mslug"]);
    let _ = std::fs::remove_dir_all(dir);
}

/// NAOMI/Atomiswave no formato do MAME (layout do RetroBat): o `.zip`/`.7z`
/// inteiro é o jogo, com o sistema da pasta; o `.chd` na subpasta com o id
/// do jogo é parte dele, não uma entrada. Entradas de uma varredura antiga
/// (`.chd` catalogado, imagem de disco do Redump) saem da biblioteca.
#[tokio::test]
async fn naomi_and_atomiswave_mame_sets() {
    let dir = scratch_dir();
    write(&dir, "naomi/azumanga.zip", b"naomi-set");
    write(&dir, "naomi/azumanga/gdl-0018.chd", b"gdrom");
    write(&dir, "naomi/18wheelr.zip", b"naomi-set-2");
    write(&dir, "atomiswave/anmlbskt.zip", b"aw-set");
    // Imagens de disco (Redump) em pasta de NAOMI: o flycast só roda o
    // formato do MAME, então não entram — nem soltas, nem dentro de .7z.
    write(&dir, "naomi/solto/jogo.chd", b"chd-solto");
    write(
        &dir,
        "naomi/japan/Radirgy (Japan).cue",
        b"FILE x.bin BINARY",
    );
    let redump = dir.join("naomi/japan/Chaos Field (Japan).7z");
    {
        let mut w = sevenz_rust2::ArchiveWriter::create(&redump).unwrap();
        w.push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("Chaos Field (Japan).cue"),
            Some(std::io::Cursor::new(b"FILE x.bin BINARY".to_vec())),
        )
        .unwrap();
        w.finish().unwrap();
    }

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    // estado de uma varredura antiga: o .chd do set catalogado e o .7z como disc
    for (path, sys) in [
        ("naomi/azumanga/gdl-0018.chd", "naomi"),
        ("naomi/japan/Chaos Field (Japan).7z", "disc"),
    ] {
        repo.add(&domain::library::Rom {
            id: path.to_string(),
            // separador nativo: é o caminho que a varredura vai procurar
            file_path: path
                .split('/')
                .fold(dir.clone(), |p, c| p.join(c))
                .to_string_lossy()
                .into_owned(),
            crc32: "0".into(),
            md5: "0".into(),
            system_id: sys.into(),
            added_at: 0,
            last_played_at: None,
            is_favorite: false,
            user_title: None,
            ..Default::default()
        })
        .await
        .unwrap();
    }

    // a contagem rápida não abre o .7z: conta os 3 sets + o .7z
    assert_eq!(library_scan::count_roms(&dir), 4);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((r.added, r.reclassified, r.removed), (3, 0, 2), "{r:?}");

    let naomi = repo.list_by_system("naomi").await.unwrap();
    let mut names: Vec<_> = naomi
        .iter()
        .map(|r| r.file_path.rsplit(['/', '\\']).next().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, ["18wheelr.zip", "azumanga.zip"]);
    assert_eq!(repo.list_by_system("atomiswave").await.unwrap().len(), 1);
    assert!(repo.list_by_system("disc").await.unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn generic_extension_counts_only_inside_its_system_folder() {
    let dir = scratch_dir();
    write(&dir, "msx/Metal Gear (Japan).rom", b"msx-cart");
    write(&dir, "odyssey2/K.C. Munchkin (USA).bin", b"o2-cart");
    write(&dir, "zxspectrum/Manic Miner.tap", b"zx-tape");
    write(&dir, "nds/Mario Kart DS (USA).nds", b"nds-cart");
    // Atari 2600/7800: `.bin` (e `.BIN`, como no RetroBat) só dentro da pasta
    write(&dir, "atari2600/Pitfall! (USA).bin", b"2600-a");
    write(&dir, "atari2600/Adventure.BIN", b"2600-b");
    write(&dir, "atari7800/Galaga (USA).bin", b"7800-cart");
    // mesmas extensões genéricas fora da pasta do sistema → ignoradas
    write(&dir, "Random.rom", b"x");
    write(&dir, "firmware.bin", b"y");
    // `.bin` dentro da pasta de um sistema que NÃO o aceita → ignorado
    write(&dir, "nds/extra.bin", b"z");

    assert_eq!(
        library_scan::count_roms(&dir),
        7,
        "barra de progresso usa a mesma regra"
    );

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 7);
    assert_eq!(r.skipped_unrecognized, 3);
    for sys in ["msx", "odyssey2", "zxspectrum", "nds", "atari7800"] {
        assert_eq!(repo.list_by_system(sys).await.unwrap().len(), 1, "{sys}");
    }
    assert_eq!(repo.list_by_system("atari2600").await.unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

/// DOS: `.zip`/`.exe` SOLTOS na pasta `dos/` são jogos; os executáveis de
/// dentro da pasta de um jogo (instalador, setup…) não viram entradas; o
/// `.zip` é o jogo inteiro (hash do arquivo, sem extrair o `.bin` de dentro,
/// que seria confundido com uma ROM). `.scummvm` vale em qualquer pasta.
#[tokio::test]
async fn dos_and_scummvm_games() {
    let dir = scratch_dir();
    let zip_path = dir.join("dos/Doom.zip");
    std::fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        for (name, bytes) in [("DOOM.EXE", &b"mz-doom"[..]), ("DATA.BIN", &b"bin"[..])] {
            zw.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zw.write_all(bytes).unwrap();
        }
        zw.finish().unwrap();
    }
    write(&dir, "dos/Keen.exe", b"mz-keen");
    write(&dir, "dos/Wolf3D/WOLF3D.EXE", b"mz-wolf"); // dentro da pasta do jogo
    write(&dir, "dos/Wolf3D/INSTALL.BAT", b"@echo off");
    write(&dir, "outros/setup.exe", b"mz-qualquer"); // fora da pasta dos/
    write(&dir, "jogos/Monkey Island/monkey.scummvm", b"monkey");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 3, "{r:?}");
    assert_eq!(r.skipped_unrecognized, 3, "{r:?}"); // WOLF3D.EXE, INSTALL.BAT, setup.exe

    let mut dos: Vec<_> = repo
        .list_by_system("dos")
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                r.file_path.rsplit(['/', '\\']).next().unwrap().to_string(),
                r.crc32,
            )
        })
        .collect();
    dos.sort();
    let zip_crc = crc_of(&std::fs::read(&zip_path).unwrap());
    assert_eq!(
        dos,
        vec![
            ("Doom.zip".to_string(), zip_crc),
            ("Keen.exe".to_string(), crc_of(b"mz-keen")),
        ]
    );
    assert_eq!(repo.list_by_system("scummvm").await.unwrap().len(), 1);
}

/// A pasta do sistema adicionada COMO RAIZ (`…/roms/atari2600`, não
/// `…/roms`): o nome da raiz identifica o sistema.
#[tokio::test]
async fn system_folder_as_scan_root() {
    let dir = scratch_dir();
    write(&dir, "atari2600/Pitfall! (USA).bin", b"2600");
    write(&dir, "atomiswave/anmlbskt.zip", b"aw-set");
    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    for sys in ["atari2600", "atomiswave"] {
        let root = dir.join(sys);
        assert_eq!(library_scan::count_roms(&root), 1, "{sys}");
        let r = scan_into(&repo, &root, 0, |_| {}).await.unwrap();
        assert_eq!(r.added, 1, "{sys}: {r:?}");
        assert_eq!(repo.list_by_system(sys).await.unwrap().len(), 1, "{sys}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// Diagnóstico: varre `REEMU_SCAN_DIR` num banco em memória e imprime quantas
/// ROMs caíram em cada sistema (não toca a biblioteca do app).
#[tokio::test]
#[ignore = "diagnóstico manual (REEMU_SCAN_DIR)"]
async fn scan_real_folder() {
    let Ok(dir) = std::env::var("REEMU_SCAN_DIR") else {
        return;
    };
    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, std::path::Path::new(&dir), 0, |_| {})
        .await
        .unwrap();
    let mut by: std::collections::BTreeMap<String, usize> = Default::default();
    for rom in repo.list().await.unwrap() {
        *by.entry(rom.system_id).or_default() += 1;
    }
    eprintln!("{r:?}\n{by:?}");
}

#[tokio::test]
async fn zip_inner_entry_name_and_size() {
    let dir = scratch_dir();
    let rom_bytes = b"n64-rom-payload-inside-zip-identity";
    let zip_path = dir.join("Embrulho.zip");
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        zw.start_file(
            "Super Mario 64 (USA).z64",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zw.write_all(rom_bytes).unwrap();
        zw.finish().unwrap();
    }
    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    scan_into(&repo, &dir, 0, |_| {}).await.unwrap();

    let rom = &repo.list_by_system("n64").await.unwrap()[0];
    // Nome e tamanho são os da entrada de dentro, não os do .zip.
    assert_eq!(rom.ident_name.as_deref(), Some("Super Mario 64 (USA).z64"));
    assert_eq!(rom.ident_size, Some(rom_bytes.len() as i64));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn scanned_rom_row_carries_sha1_name_size_and_time() {
    let dir = scratch_dir();
    let bytes = b"abc";
    std::fs::write(dir.join("Teste (World).gba"), bytes).unwrap();
    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    scan_into(&repo, &dir, 1234, |_| {}).await.unwrap();

    let rom = &repo.list_by_system("gba").await.unwrap()[0];
    assert_eq!(
        rom.sha1.as_deref(),
        Some("a9993e364706816aba3e25717850c26c9cd0d89d")
    );
    assert_eq!(rom.ident_name.as_deref(), Some("Teste (World).gba"));
    assert_eq!(rom.ident_size, Some(3));
    assert_eq!(rom.identified_at, Some(1234));
    assert_eq!(rom.serial, None, "cartucho não tem serial");
    let _ = std::fs::remove_dir_all(dir);
}

/// Imagem de Saturn de 32 KiB: só o cabeçalho do setor 0.
fn saturn_iso(product: &[u8; 10]) -> Vec<u8> {
    let mut img = vec![0u8; 16 * 2048];
    img[..16].copy_from_slice(b"SEGA SEGASATURN ");
    img[0x10..0x20].copy_from_slice(b"SEGA TP T-81    ");
    img[0x20..0x2A].copy_from_slice(product);
    img
}

#[tokio::test]
async fn disc_serial_is_stored_for_disc_systems() {
    let dir = scratch_dir();
    write(&dir, "saturn/Jogo (USA).iso", &saturn_iso(b"MK-81022  "));
    // Sem pasta de sistema: a assinatura diz Saturn, e o serial vem junto.
    write(&dir, "Solto (Japan).iso", &saturn_iso(b"T-4507G   "));
    // Disco de PS1 ilegível: entra na biblioteca, sem serial.
    write(&dir, "psx/Quebrado (USA).iso", &[0xAB; 4096]);
    write(&dir, "Cartucho (World).gba", b"gba");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!(r.added, 4);
    assert_eq!(r.errors, 0);

    let roms = repo.list().await.unwrap();
    let serial = |name: &str| {
        roms.iter()
            .find(|r| r.ident_name.as_deref() == Some(name))
            .unwrap()
            .serial
            .clone()
    };
    assert_eq!(serial("Jogo (USA).iso").as_deref(), Some("MK-81022"));
    assert_eq!(serial("Solto (Japan).iso").as_deref(), Some("T-4507G"));
    assert_eq!(serial("Quebrado (USA).iso"), None);
    assert_eq!(serial("Cartucho (World).gba"), None);
    let _ = std::fs::remove_dir_all(dir);
}

/// CSO e CHD entram sem hash (CRC32 e MD5 vazios, SHA1 nulo) e com o nome,
/// o tamanho e o serial; um `.iso` de tamanho de CD entra com o hash e o
/// serial; um `.cue`, com o hash da trilha de dados.
#[tokio::test]
async fn disc_rows_keep_name_size_and_serial() {
    let dir = scratch_dir();
    let iso = saturn_iso(b"MK-81022  ");
    write(&dir, "saturn/Jogo (USA).iso", &iso);
    write(&dir, "psp/Jogo (USA).cso", b"CISO-fake");
    write(&dir, "psx/Jogo (USA).chd", b"MComprHD-fake");
    write(&dir, "saturn/Outro (USA) (Track 1).bin", &iso);
    write(
        &dir,
        "saturn/Outro (USA).cue",
        b"FILE \"Outro (USA) (Track 1).bin\" BINARY\n  TRACK 01 MODE1/2048\n    INDEX 01 00:00:00\n",
    );

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 77, |_| {}).await.unwrap();
    assert_eq!((r.added, r.errors), (4, 0), "{r:?}");

    let roms = repo.list().await.unwrap();
    let row = |name: &str| {
        roms.iter()
            .find(|r| r.ident_name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{name}"))
    };
    for (name, size) in [("Jogo (USA).cso", 9), ("Jogo (USA).chd", 13)] {
        let rom = row(name);
        assert_eq!((rom.crc32.as_str(), rom.md5.as_str()), ("", ""), "{name}");
        assert_eq!(rom.sha1, None, "{name}");
        assert_eq!(rom.ident_size, Some(size as i64), "{name}");
        assert_eq!(rom.serial, None, "{name}");
        assert_eq!(rom.identified_at, Some(77), "{name}");
    }
    let disc = row("Jogo (USA).iso");
    assert_eq!(disc.crc32, crc_of(&iso));
    assert_eq!(disc.md5.len(), 32);
    assert_eq!(disc.sha1.as_ref().map(String::len), Some(40));
    assert_eq!(disc.ident_size, Some(iso.len() as i64));
    assert_eq!(disc.serial.as_deref(), Some("MK-81022"));
    assert_eq!(disc.identified_at, Some(77));
    let cue = row("Outro (USA) (Track 1).bin");
    assert_eq!(cue.crc32, crc_of(&iso));
    assert_eq!(cue.serial.as_deref(), Some("MK-81022"));
    assert!(cue.file_path.ends_with("Outro (USA).cue"));
    let _ = std::fs::remove_dir_all(dir);
}

/// `.cue` copiado antes do `.bin`: entra com o nome e o tamanho do cue e
/// `identified_at` nulo, pro passo em segundo plano identificar depois.
#[tokio::test]
async fn cue_without_its_track_yet_is_left_for_late_identification() {
    let dir = scratch_dir();
    write(
        &dir,
        "psx/Jogo (USA).cue",
        b"FILE \"Jogo (USA).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    );
    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let r = scan_into(&repo, &dir, 55, |_| {}).await.unwrap();
    assert_eq!((r.added, r.errors), (1, 0), "{r:?}");
    let rom = &repo.list_by_system("psx").await.unwrap()[0];
    assert_eq!(rom.ident_name.as_deref(), Some("Jogo (USA).cue"));
    assert_eq!(rom.identified_at, None);
    assert_eq!((rom.crc32.as_str(), rom.sha1.as_deref()), ("", None));
    let _ = std::fs::remove_dir_all(dir);
}

/// Linha que a varredura reclassifica (entrou como `disc`, a pasta diz
/// `dreamcast`): a identidade fica pra calcular de novo, porque a trilha e o
/// serial dependem do sistema. O resto da linha fica como estava.
#[tokio::test]
async fn reclassified_row_is_left_for_late_identification() {
    let dir = scratch_dir();
    write(
        &dir,
        "dreamcast/Jogo (USA).cue",
        b"FILE \"Jogo (USA).bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n",
    );
    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    let path = dir
        .join("dreamcast")
        .join("Jogo (USA).cue")
        .to_string_lossy()
        .into_owned();
    repo.add(&domain::library::Rom {
        id: "velha".into(),
        file_path: path.clone(),
        crc32: "AAAAAAAA".into(),
        md5: "m".into(),
        system_id: "disc".into(),
        identified_at: Some(1),
        ident_name: Some("Jogo (USA).bin".into()),
        ..Default::default()
    })
    .await
    .unwrap();

    let r = scan_into(&repo, &dir, 2, |_| {}).await.unwrap();
    assert_eq!((r.reclassified, r.added), (1, 0), "{r:?}");
    let rom = repo.find_by_path(&path).await.unwrap().unwrap();
    assert_eq!(rom.system_id, "dreamcast");
    assert_eq!(rom.identified_at, None);
    assert_eq!(
        rom.crc32, "AAAAAAAA",
        "o hash antigo fica até ser recalculado"
    );
    let _ = std::fs::remove_dir_all(dir);
}
