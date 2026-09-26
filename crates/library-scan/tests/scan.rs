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
            reclassified: 0,
            removed: 0,
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

/// NAOMI/Atomiswave no formato do MAME (layout do RetroBat): o `.zip`/`.7z`
/// inteiro é o jogo, com o sistema da pasta; o `.chd` na subpasta com o id
/// do jogo é parte dele, não uma entrada. Entradas de uma varredura antiga
/// (`.chd` catalogado, `.7z` como `disc`) são corrigidas.
#[tokio::test]
async fn naomi_and_atomiswave_mame_sets() {
    let dir = scratch_dir();
    write(&dir, "naomi/azumanga.zip", b"naomi-set");
    write(&dir, "naomi/azumanga/gdl-0018.chd", b"gdrom");
    write(&dir, "naomi/18wheelr.zip", b"naomi-set-2");
    write(&dir, "naomi/japan/Chaos Field (Japan).7z", b"7z-gdrom");
    write(&dir, "atomiswave/anmlbskt.zip", b"aw-set");
    // .chd sem .zip irmão continua sendo disco (da pasta do sistema)
    write(&dir, "naomi/solto/jogo.chd", b"chd-solto");

    let db = db::connect_in_memory().await.unwrap();
    let repo = db::RomsRepo::new(db);
    // estado de uma varredura antiga: o .chd do set catalogado e o .7z como disc
    for (path, sys) in [
        ("naomi/azumanga/gdl-0018.chd", "naomi"),
        ("naomi/japan/Chaos Field (Japan).7z", "disc"),
    ] {
        repo.add(&domain::library::Rom {
            id: path.to_string(),
            file_path: dir.join(path).to_string_lossy().into_owned(),
            crc32: "0".into(),
            md5: "0".into(),
            system_id: sys.into(),
            added_at: 0,
            last_played_at: None,
            is_favorite: false,
            user_title: None,
        })
        .await
        .unwrap();
    }
    repo.set_favorite("naomi/japan/Chaos Field (Japan).7z", true)
        .await
        .unwrap();

    assert_eq!(library_scan::count_roms(&dir), 5, "o .chd do set não conta");
    let r = scan_into(&repo, &dir, 0, |_| {}).await.unwrap();
    assert_eq!((r.added, r.reclassified, r.removed), (4, 1, 1), "{r:?}");

    let naomi = repo.list_by_system("naomi").await.unwrap();
    let mut names: Vec<_> = naomi
        .iter()
        .map(|r| r.file_path.rsplit('/').next().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "18wheelr.zip",
            "Chaos Field (Japan).7z",
            "azumanga.zip",
            "jogo.chd"
        ]
    );
    // o reclassificado mantém o que tinha (favorito)
    assert!(naomi
        .iter()
        .any(|r| r.file_path.ends_with(".7z") && r.is_favorite));
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
        .map(|r| (r.file_path.rsplit('/').next().unwrap().to_string(), r.crc32))
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
