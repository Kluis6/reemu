//! `EmuSession` (thread dedicada) + `FocusController`, contra o core-fake
//! do `core-loader-desktop`.
//!
//! libretro é um-core-por-processo → os testes serializam por `LOCK`.

use core_loader_desktop::testcore_path;
use domain::focus::{FocusManager, InputFocus};
use emu_session::{EmuSession, FocusController, SessionConfig, SessionState};
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

#[test]
fn load_runs_and_frames_advance() {
    let _g = lock();
    let s = session();
    assert_eq!(s.state(), SessionState::Idle);

    let av = s
        .load(testcore_path(), rom().to_str().unwrap())
        .expect("load");
    assert_eq!(av.geometry.base_width, 64);
    assert_eq!(av.timing.fps, 60.0);
    assert_eq!(s.state(), SessionState::Running);

    sleep(Duration::from_millis(120));
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
    s.load(testcore_path(), rom().to_str().unwrap()).unwrap();
    sleep(Duration::from_millis(60));

    s.set_paused(true); // round-trip: aplicado quando retorna
    assert_eq!(s.state(), SessionState::Paused);
    let frozen_at = s.frame_seq();
    sleep(Duration::from_millis(80));
    assert_eq!(s.frame_seq(), frozen_at, "pausado não deve rodar frame");

    s.set_paused(false);
    assert_eq!(s.state(), SessionState::Running);
    sleep(Duration::from_millis(80));
    assert!(s.frame_seq() > frozen_at, "resumido volta a rodar");
}

#[test]
fn save_and_restore_state_through_session() {
    let _g = lock();
    let s = session();
    s.load(testcore_path(), rom().to_str().unwrap()).unwrap();
    sleep(Duration::from_millis(50));

    let snap = s.save_state().unwrap().expect("core-fake serializa");
    assert!(!snap.is_empty());
    assert!(s.restore_state(snap).unwrap(), "unserialize ok");
}

#[test]
fn focus_controller_pauses_and_resumes_session() {
    let _g = lock();
    let s = Arc::new(session());
    s.load(testcore_path(), rom().to_str().unwrap()).unwrap();
    sleep(Duration::from_millis(40));

    let mut focus = FocusController::new(Arc::clone(&s));
    assert_eq!(focus.current(), InputFocus::GameFocused);

    focus.toggle();
    assert_eq!(focus.current(), InputFocus::MenuFocused);
    assert_eq!(s.state(), SessionState::Paused);
    let frozen = s.frame_seq();
    sleep(Duration::from_millis(60));
    assert_eq!(s.frame_seq(), frozen, "MenuFocused pausa o core");

    focus.toggle();
    assert_eq!(focus.current(), InputFocus::GameFocused);
    assert_eq!(s.state(), SessionState::Running);
    sleep(Duration::from_millis(60));
    assert!(s.frame_seq() > frozen);
}

#[test]
fn reload_after_unload_and_bad_core_reports_error() {
    let _g = lock();
    let s = session();

    let err = s.load("/nao/existe/core.so", "/tmp/x").unwrap_err();
    assert!(matches!(err, emu_session::SessionError::Load(_)), "{err:?}");
    assert_eq!(s.state(), SessionState::Idle);

    s.load(testcore_path(), rom().to_str().unwrap()).unwrap();
    s.unload().unwrap();
    s.load(testcore_path(), rom().to_str().unwrap()).unwrap();
    assert_eq!(s.state(), SessionState::Running);
}
