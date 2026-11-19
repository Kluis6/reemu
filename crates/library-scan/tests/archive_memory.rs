//! Imagem de disco dentro de `.zip`/`.7z` é hasheada aos poucos: a maior
//! alocação durante a identificação fica muito abaixo do tamanho da entrada.
//! Binário de teste próprio porque o alocador global conta o processo todo.

use library_scan::{identify, Digests, FileRomHasher};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Registra o tamanho da maior alocação (ou realocação) pedida.
struct Largest;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Largest {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LARGEST.fetch_max(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: Largest = Largest;

/// Bem maior que qualquer buffer do caminho de leitura.
const ENTRY: u64 = 48 * 1024 * 1024;
const NAME: &str = "Jogo (USA).img";

fn content() -> impl Read {
    std::io::repeat(0x5A).take(ENTRY)
}

#[test]
fn zipped_disc_image_is_hashed_without_holding_it_in_memory() {
    let dir = tempfile::tempdir().unwrap();

    let zip_path = dir.path().join("Jogo.zip");
    {
        let mut zw = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        zw.start_file(
            NAME,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        std::io::copy(&mut content(), &mut zw).unwrap();
        zw.finish().unwrap().flush().unwrap();
    }

    // LZMA2 com dicionário de 1 MiB: o do descompressor é a única alocação
    // grande legítima do caminho.
    let sz = dir.path().join("Jogo.7z");
    {
        let mut w = sevenz_rust2::ArchiveWriter::create(&sz).unwrap();
        let mut lzma2 = sevenz_rust2::encoder_options::Lzma2Options::from_level(0);
        lzma2.set_dictionary_size(1024 * 1024);
        w.set_content_methods(vec![lzma2.into()]);
        w.push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("Outro.bin"),
            Some(std::io::repeat(0x11).take(3 * 1024 * 1024)),
        )
        .unwrap();
        w.push_archive_entry(sevenz_rust2::ArchiveEntry::new_file(NAME), Some(content()))
            .unwrap();
        w.finish().unwrap();
    }

    let h = FileRomHasher::hash_reader(content(), NAME).unwrap();
    let expected = Digests {
        crc32: h.crc32,
        md5: h.md5,
        sha1: h.sha1,
    };

    for archive in [&zip_path, &sz] {
        LARGEST.store(0, Ordering::Relaxed);
        let id = identify(archive, "disc", Some(NAME)).unwrap();
        let largest = LARGEST.load(Ordering::Relaxed);
        assert!(
            largest < 4 * 1024 * 1024,
            "{archive:?}: alocou {largest} bytes de uma vez"
        );
        assert_eq!(id.hashes.as_ref(), Some(&expected), "{archive:?}");
        assert_eq!((id.name.as_str(), id.size), (NAME, ENTRY), "{archive:?}");
    }
}
