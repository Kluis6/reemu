use core_loader_desktop::DesktopCoreLoader;
use domain::audio::AudioSink;
use domain::core_loader::{CoreId, CoreLoadError, LoadedCore, SystemAvInfo};
use domain::frame_source::{Frame, FrameSource};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Constrói o `AudioSink` **dentro** da thread do core (a `cpal::Stream` é
/// `!Send`). Retorna `None` se o áudio não pôde abrir (o app segue sem som).
/// `SessionConfig.audio_sink = None` = sem áudio (ex: testes).
pub type AudioSinkFactory = Box<dyn FnOnce() -> Option<Box<dyn AudioSink>> + Send>;

pub struct SessionConfig {
    pub cores_dir: PathBuf,
    pub system_dir: PathBuf,
    pub save_dir: PathBuf,
    pub audio_sink: Option<AudioSinkFactory>,
}

impl SessionConfig {
    pub fn new(cores_dir: PathBuf, system_dir: PathBuf, save_dir: PathBuf) -> Self {
        Self {
            cores_dir,
            system_dir,
            save_dir,
            audio_sink: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Running,
    Paused,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Load(#[from] CoreLoadError),
    #[error("a thread de emulação não está respondendo")]
    ThreadDown,
}

enum Command {
    Load(CoreId, String, Sender<Result<SystemAvInfo, CoreLoadError>>),
    Unload(Sender<()>),
    SetPaused(bool, Sender<()>),
    SaveState(Sender<Option<Vec<u8>>>),
    RestoreState(Vec<u8>, Sender<bool>),
    Shutdown,
}

struct Shared {
    frame_seq: AtomicU64,
    latest_frame: Mutex<Option<Frame>>,
    audio: Mutex<Vec<i16>>,
    state: Mutex<SessionState>,
    /// Identificador do core carregado (o que foi passado pra `load`). `None`
    /// quando ocioso. Usado pra validar save states (não portáveis entre cores).
    loaded_core: Mutex<Option<String>>,
}

impl Shared {
    fn set_state(&self, s: SessionState) {
        *self.state.lock().unwrap_or_else(|p| p.into_inner()) = s;
    }
}

pub struct EmuSession {
    tx: Sender<Command>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl EmuSession {
    pub fn spawn(cfg: SessionConfig) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            frame_seq: AtomicU64::new(0),
            latest_frame: Mutex::new(None),
            audio: Mutex::new(Vec::new()),
            state: Mutex::new(SessionState::Idle),
            loaded_core: Mutex::new(None),
        });
        let thread = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("emu-core-loop".into())
                .spawn(move || core_loop(cfg, rx, shared))
                .expect("spawn emu-core-loop")
        };
        Self {
            tx,
            shared,
            thread: Some(thread),
        }
    }

    fn send(&self, cmd: Command) -> Result<(), SessionError> {
        self.tx.send(cmd).map_err(|_| SessionError::ThreadDown)
    }

    fn call<T>(&self, make: impl FnOnce(Sender<T>) -> Command) -> Result<T, SessionError> {
        let (rtx, rrx) = mpsc::channel();
        self.send(make(rtx))?;
        rrx.recv().map_err(|_| SessionError::ThreadDown)
    }

    /// Carrega e começa a rodar. Bloqueia até o core abrir (ou falhar).
    pub fn load(&self, core_id: &str, rom_path: &str) -> Result<SystemAvInfo, SessionError> {
        self.call(|reply| Command::Load(CoreId(core_id.to_string()), rom_path.to_string(), reply))?
            .map_err(SessionError::from)
    }

    pub fn unload(&self) -> Result<(), SessionError> {
        self.call(Command::Unload)
    }

    /// Bloqueia até a thread aplicar (round-trip curto, <= 1 frame).
    pub fn set_paused(&self, paused: bool) {
        let _ = self.call(|reply| Command::SetPaused(paused, reply));
    }

    /// Serializa o estado do core (chamado entre frames pela thread).
    pub fn save_state(&self) -> Result<Option<Vec<u8>>, SessionError> {
        self.call(Command::SaveState)
    }

    pub fn restore_state(&self, data: Vec<u8>) -> Result<bool, SessionError> {
        self.call(|reply| Command::RestoreState(data, reply))
    }

    pub fn state(&self) -> SessionState {
        *self.shared.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Identificador do core carregado (pra validar save states).
    pub fn loaded_core(&self) -> Option<String> {
        self.shared
            .loaded_core
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Contador monotônico de frames produzidos — útil pra medir progresso.
    pub fn frame_seq(&self) -> u64 {
        self.shared.frame_seq.load(Ordering::Relaxed)
    }

    /// Pega (movendo) o frame mais recente ainda não consumido.
    pub fn take_latest_frame(&self) -> Option<Frame> {
        self.shared
            .latest_frame
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
    }

    /// PCM interleaved estéreo acumulado desde o último drain.
    pub fn drain_audio(&self) -> Vec<i16> {
        std::mem::take(&mut *self.shared.audio.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

impl Drop for EmuSession {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn core_loop(mut cfg: SessionConfig, rx: Receiver<Command>, shared: Arc<Shared>) {
    // A stream do cpal é `!Send` — construída aqui, nesta thread.
    let mut sink: Option<Box<dyn AudioSink>> = cfg.audio_sink.take().and_then(|make| make());
    let loader = DesktopCoreLoader::new(cfg.cores_dir, cfg.system_dir, cfg.save_dir);
    let mut core = None;
    let mut paused = false;
    let mut core_sample_rate = 32_000u32;
    let mut frame_budget = Duration::from_micros(16_667);

    loop {
        // Ocioso ou pausado: bloqueia esperando comando. Rodando: só drena.
        let cmd = if core.is_none() || paused {
            rx.recv().ok()
        } else {
            rx.try_recv().ok()
        };

        if let Some(cmd) = cmd {
            match cmd {
                Command::Load(id, rom, reply) => {
                    core = None; // teardown do anterior antes de abrir outro
                    shared.set_state(SessionState::Idle);
                    match loader.open_core(&id, &rom) {
                        Ok(c) => {
                            let av = c.system_av_info();
                            let fps = av.timing.fps.max(1.0);
                            frame_budget = Duration::from_secs_f64(1.0 / fps);
                            core_sample_rate = (av.timing.sample_rate.round() as u32).max(1);
                            core = Some(c);
                            paused = false;
                            if let Some(s) = sink.as_mut() {
                                s.resume();
                            }
                            *shared.loaded_core.lock().unwrap_or_else(|p| p.into_inner()) =
                                Some(id.0.clone());
                            shared.set_state(SessionState::Running);
                            let _ = reply.send(Ok(av));
                        }
                        Err(e) => {
                            *shared.loaded_core.lock().unwrap_or_else(|p| p.into_inner()) = None;
                            shared.set_state(SessionState::Idle);
                            let _ = reply.send(Err(e));
                        }
                    }
                }
                Command::Unload(reply) => {
                    core = None;
                    paused = false;
                    *shared.loaded_core.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    if let Some(s) = sink.as_mut() {
                        s.pause();
                    }
                    shared.set_state(SessionState::Idle);
                    let _ = reply.send(());
                }
                Command::SetPaused(p, reply) => {
                    if core.is_some() {
                        paused = p;
                        if let Some(s) = sink.as_mut() {
                            if p {
                                s.pause();
                            } else {
                                s.resume();
                            }
                        }
                        shared.set_state(if p {
                            SessionState::Paused
                        } else {
                            SessionState::Running
                        });
                    }
                    let _ = reply.send(());
                }
                Command::SaveState(reply) => {
                    let bytes = core.as_mut().and_then(|c| c.serialize_state());
                    let _ = reply.send(bytes);
                }
                Command::RestoreState(data, reply) => {
                    let ok = core
                        .as_mut()
                        .map(|c| c.restore_state(&data))
                        .unwrap_or(false);
                    let _ = reply.send(ok);
                }
                Command::Shutdown => break,
            }
            continue;
        }

        // Sem comando pendente: roda um frame (só chega aqui se há core e não pausado).
        if let Some(c) = core.as_mut() {
            let start = Instant::now();
            if let Some(frame) = c.next_frame() {
                shared.frame_seq.fetch_add(1, Ordering::Relaxed);
                *shared
                    .latest_frame
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some(frame);
            }
            let audio = c.drain_audio();
            if !audio.is_empty() {
                match sink.as_mut() {
                    // Com sink real, o áudio vai direto pro device (o Vec
                    // cresceria sem ninguém drenar).
                    Some(s) => s.push_samples(&audio, core_sample_rate),
                    None => shared
                        .audio
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend_from_slice(&audio),
                }
            }
            if let Some(rest) = frame_budget.checked_sub(start.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    }
    // `core` dropa aqui → DesktopCore::Drop faz unload_game + deinit.
}
