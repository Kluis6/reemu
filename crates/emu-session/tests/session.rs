//! `EmuSession` (processo filho descartável) + `FocusController`, contra o
//! core-fake do `core-loader-desktop` — agora spawnado de verdade como
//! `reemu-core-host` (não mais in-process). Os testes serializam por `LOCK`
//! porque libretro é um-core-por-processo E porque cada teste sobe/mata
//! processos reais (evita competir por porta/socket entre testes paralelos).

use core_loader_desktop::testcore_path;
use domain::focus::{FocusManager, InputFocus};
use emu_session::{EmuSession, FocusController, SessionConfig, SessionState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

static LOCK: Mutex<()> = Mutex::new(());
static NONCE: AtomicU32 = AtomicU32::new(0);

fn lock() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// O core-fake tem `need_fullpath = false` → o loader lê o arquivo. Cria um.
fn rom() -> std::path::PathBuf {
    let n = NONCE.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("reemu-sess-{}-{n}.bin", std::process::id()));
    std::fs::write(&p, b"content").unwrap();
    p
}

fn session() -> EmuSession {
    let tmp = std::env::temp_dir();
    EmuSession::spawn(SessionConfig::new(tmp.clone(), tmp.clone(), tmp))
}

/// `s.load(testcore_path(), <rom>)` sem opções — o caso comum dos testes.
fn load(
    s: &EmuSession,
    rom: &std::path::Path,
) -> Result<domain::core_loader::SystemAvInfo, emu_session::SessionError> {
    s.load(testcore_path(), rom.to_str().unwrap(), HashMap::new())
}

#[test]
fn load_runs_and_frames_advance() {
    let _g = lock();
    let s = session();
    assert_eq!(s.state(), SessionState::Idle);

    let av = load(&s, &rom()).expect("load");
    assert_eq!(av.geometry.base_width, 64);
    assert_eq!(av.timing.fps, 60.0);
    assert_eq!(s.state(), SessionState::Running);

    sleep(Duration::from_millis(200));
    let seq = s.frame_seq();
    assert!(seq >= 4, "esperava ~7 frames em 120ms, veio {seq}");

    let frame = s.take_latest_frame().expect("frame publicado");
    assert_eq!(frame.metadata.native_width, 64);
    assert_eq!(frame.metadata.native_height, 48);
    assert!(!s.drain_audio().is_empty());

    s.unload().unwrap();
    assert_eq!(s.state(), SessionState::Idle);
}

#[test]
fn pause_freezes_emulation_then_resume() {
    let _g = lock();
    let s = session();
    load(&s, &rom()).unwrap();
    sleep(Duration::from_millis(100));

    s.set_paused(true); // round-trip: aplicado quando retorna
    assert_eq!(s.state(), SessionState::Paused);
    let frozen_at = s.frame_seq();
    sleep(Duration::from_millis(120));
    assert_eq!(s.frame_seq(), frozen_at, "pausado não deve rodar frame");

    s.set_paused(false);
    assert_eq!(s.state(), SessionState::Running);
    sleep(Duration::from_millis(120));
    assert!(s.frame_seq() > frozen_at, "resumido volta a rodar");
}

#[test]
fn save_and_restore_state_through_session() {
    let _g = lock();
    let s = session();
    load(&s, &rom()).unwrap();
    sleep(Duration::from_millis(100));

    let snap = s.save_state().unwrap().expect("core-fake serializa");
    assert!(!snap.is_empty());
    assert!(s.restore_state(snap).unwrap(), "unserialize ok");
}

#[test]
fn focus_controller_pauses_and_resumes_session() {
    let _g = lock();
    let s = Arc::new(session());
    load(&s, &rom()).unwrap();
    sleep(Duration::from_millis(100));

    let mut focus = FocusController::new(Arc::clone(&s));
    assert_eq!(focus.current(), InputFocus::GameFocused);

    focus.toggle();
    assert_eq!(focus.current(), InputFocus::MenuFocused);
    assert_eq!(s.state(), SessionState::Paused);
    let frozen = s.frame_seq();
    sleep(Duration::from_millis(120));
    assert_eq!(s.frame_seq(), frozen, "MenuFocused pausa o core");

    focus.toggle();
    assert_eq!(focus.current(), InputFocus::GameFocused);
    assert_eq!(s.state(), SessionState::Running);
    sleep(Duration::from_millis(120));
    assert!(s.frame_seq() > frozen);
}

#[test]
fn reload_after_unload_and_bad_core_reports_error() {
    let _g = lock();
    let s = session();

    let err = s
        .load("/nao/existe/core.so", "/tmp/x", HashMap::new())
        .unwrap_err();
    assert!(matches!(err, emu_session::SessionError::Load(_)), "{err:?}");
    assert_eq!(s.state(), SessionState::Idle);

    load(&s, &rom()).unwrap();
    s.unload().unwrap();
    load(&s, &rom()).unwrap();
    assert_eq!(s.state(), SessionState::Running);
}

/// Garantia estrutural do fix do reload de N64: cada `load` sobe um
/// processo `reemu-core-host` NOVO, nunca reusa o anterior — mesmo
/// trocando pro MESMO core. O core-fake em si é re-entrante (não reproduz o
/// crash do parallel_n64), mas isto testa o mecanismo que faz a
/// re-entrância do core deixar de importar: memória de processo sempre
/// parte limpa a cada troca.
#[test]
fn each_load_spawns_a_fresh_process() {
    let _g = lock();
    let s = session();

    load(&s, &rom()).unwrap();
    let pid1 = s.debug_child_pid().expect("pid do 1º processo");

    // Troca pro MESMO core+rom — se fosse reaproveitar o processo, o pid
    // seria igual.
    load(&s, &rom()).unwrap();
    let pid2 = s.debug_child_pid().expect("pid do 2º processo");
    assert_ne!(
        pid1, pid2,
        "o load deveria subir um processo novo, não reusar"
    );

    s.unload().unwrap();
    assert_eq!(s.debug_child_pid(), None, "unload mata o processo");

    load(&s, &rom()).unwrap();
    let pid3 = s.debug_child_pid().expect("pid do 3º processo");
    assert_ne!(pid2, pid3);
}

#[test]
fn save_ram_round_trips_through_session() {
    let _g = lock();
    let rom_path = rom();
    let srm = std::env::temp_dir().join(format!(
        "{}.srm",
        rom_path.file_stem().unwrap().to_str().unwrap()
    ));
    let mut want = vec![0u8; 64];
    want[0] = 0x11;
    want[63] = 0xEE;
    std::fs::write(&srm, &want).unwrap();

    let s = session();
    load(&s, &rom_path).unwrap();
    sleep(Duration::from_millis(80));
    s.unload().unwrap(); // grava a save RAM de volta

    // O `.srm` foi carregado no core no load e regravado no unload. O byte 2 é
    // sobrescrito pelo testcore com a core option `testcore_mark` (default "A");
    // os demais bytes vêm do arquivo original.
    let got = std::fs::read(&srm).unwrap();
    assert_eq!(got.len(), 64);
    assert_eq!(got[0], 0x11, "byte que veio do arquivo");
    assert_eq!(got[63], 0xEE, "byte que veio do arquivo");
    assert_eq!(got[2], b'A', "byte que o core escreveu (core option)");

    std::fs::remove_file(&srm).ok();
    std::fs::remove_file(&rom_path).ok();
}
