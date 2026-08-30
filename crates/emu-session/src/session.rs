use core_loader_desktop::DesktopCoreLoader;
use domain::audio::AudioSink;
use domain::core_loader::{CoreId, CoreLoadError, LoadedCore, SystemAvInfo};
use domain::frame_source::{Frame, FrameSource};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    /// Liga o poll de gamepad físico (`gilrs`) numa thread própria.
    pub enable_gamepad: bool,
}

impl SessionConfig {
    pub fn new(cores_dir: PathBuf, system_dir: PathBuf, save_dir: PathBuf) -> Self {
        Self {
            cores_dir,
            system_dir,
            save_dir,
            audio_sink: None,
            enable_gamepad: false,
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
    /// Troca o `AudioSink` em runtime (mudou o device / sample rate nas configs).
    ReloadAudio(AudioSinkFactory, Sender<()>),
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
    /// `true` = input vai pro jogo; `false` (menu) = a thread de gamepad
    /// solta o RetroPad.
    game_focused: AtomicBool,
    /// Botão de menu do gamepad (`Mode`) foi pressionado — o shell consome.
    menu_requested: AtomicBool,
    /// Sinaliza a thread de gamepad pra encerrar.
    gamepad_stop: AtomicBool,
    /// Eventos brutos capturados pela thread de gamepad em modo de binding —
    /// o shell drena e repassa pro frontend.
    captured_inputs: Mutex<Vec<domain::input::RawInputEvent>>,
    /// Gamepads conectados agora: `(guid_hex, nome)`. Atualizado pela thread
    /// de gamepad; o shell lê pra UI de mapeamento.
    gamepads: Mutex<Vec<(String, String)>>,
    /// Pulsos de navegação de menu vindos do gamepad — o shell drena e emite
    /// pro frontend como `menu-nav`.
    nav: Mutex<Vec<input_desktop::NavPulse>>,
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
    gamepad_thread: Option<JoinHandle<()>>,
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
            game_focused: AtomicBool::new(true),
            menu_requested: AtomicBool::new(false),
            gamepad_stop: AtomicBool::new(false),
            captured_inputs: Mutex::new(Vec::new()),
            gamepads: Mutex::new(Vec::new()),
            nav: Mutex::new(Vec::new()),
        });

        let gamepad_thread = cfg.enable_gamepad.then(|| {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("emu-gamepad".into())
                .spawn(move || gamepad_loop(shared))
                .expect("spawn emu-gamepad")
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
            gamepad_thread,
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

    /// Recria o `AudioSink` na thread do core (mudança de device/sample rate
    /// nas configs, sem precisar recarregar o jogo).
    pub fn reload_audio(&self, factory: AudioSinkFactory) -> Result<(), SessionError> {
        self.call(|reply| Command::ReloadAudio(factory, reply))
    }

    pub fn state(&self) -> SessionState {
        *self.shared.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Roteia input pro jogo (`true`) ou segura tudo (`false`, menu). Chamado
    /// pelo `FocusController`.
    pub fn set_game_focused(&self, focused: bool) {
        self.shared.game_focused.store(focused, Ordering::Relaxed);
    }

    /// `true` uma vez se o botão de menu do gamepad foi pressionado desde a
    /// última chamada (o shell abre/fecha o menu).
    pub fn take_menu_request(&self) -> bool {
        self.shared.menu_requested.swap(false, Ordering::Relaxed)
    }

    /// Drena os eventos brutos de gamepad capturados em modo de binding
    /// (`input_desktop::capture`). O shell emite cada um pro frontend.
    pub fn take_captured_inputs(&self) -> Vec<domain::input::RawInputEvent> {
        std::mem::take(
            &mut *self
                .shared
                .captured_inputs
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
        )
    }

    /// Drena os pulsos de navegação de menu do gamepad (d-pad/stick/A/B). O
    /// shell emite cada um pro frontend como `menu-nav`.
    pub fn take_nav_pulses(&self) -> Vec<input_desktop::NavPulse> {
        std::mem::take(&mut *self.shared.nav.lock().unwrap_or_else(|p| p.into_inner()))
    }

    /// Gamepads conectados agora: `(guid_hex, nome)`.
    pub fn connected_gamepads(&self) -> Vec<(String, String)> {
        self.shared
            .gamepads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
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
        self.shared.gamepad_stop.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self.gamepad_thread.take() {
            let _ = t.join();
        }
    }
}

fn gamepad_loop(shared: Arc<Shared>) {
    let mut poller = match input_desktop::GamepadPoller::new() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("gamepad indisponível: {e} — só teclado");
            return;
        }
    };
    let pad = core_loader_desktop::retropad();
    while !shared.gamepad_stop.load(Ordering::Relaxed) {
        let outcome = poller.poll(pad);
        if outcome.menu_pressed {
            shared.menu_requested.store(true, Ordering::Relaxed);
        }
        if !outcome.captured.is_empty() {
            shared
                .captured_inputs
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend(outcome.captured);
        }
        if !outcome.nav.is_empty() {
            shared
                .nav
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend(outcome.nav);
        }
        {
            let mut g = shared.gamepads.lock().unwrap_or_else(|p| p.into_inner());
            if *g != outcome.gamepads {
                *g = outcome.gamepads;
            }
        }
        if !shared.game_focused.load(Ordering::Relaxed) {
            pad.clear(); // no menu, nada de input de jogo
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

/// `<save_dir>/<stem da rom>.srm` — convenção do RetroArch pra battery save.
fn srm_path(dir: &std::path::Path, rom_path: &str) -> PathBuf {
    let stem = std::path::Path::new(rom_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game");
    dir.join(format!("{stem}.srm"))
}

fn flush_save_ram(core: &core_loader_desktop::DesktopCore, path: &std::path::Path) {
    let Some(bytes) = core.save_ram() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Escrita atômica: grava num `.tmp` no mesmo diretório e renomeia por cima
    // (rename é atômico no mesmo filesystem). Um kill no meio da escrita deixa
    // a `.srm` antiga intacta em vez de truncada.
    let tmp = path.with_extension("srm.tmp");
    let write = std::fs::write(&tmp, &bytes).and_then(|()| std::fs::rename(&tmp, path));
    match write {
        Ok(()) => log::debug!("save RAM ({} bytes) → {path:?}", bytes.len()),
        Err(e) => {
            log::warn!("save RAM {path:?}: {e}");
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

/// De quanto em quanto tempo a save RAM é gravada em disco enquanto o jogo roda.
const SRM_FLUSH_INTERVAL: Duration = Duration::from_secs(10);

fn core_loop(mut cfg: SessionConfig, rx: Receiver<Command>, shared: Arc<Shared>) {
    // A stream do cpal é `!Send` — construída aqui, nesta thread.
    let mut sink: Option<Box<dyn AudioSink>> = cfg.audio_sink.take().and_then(|make| make());
    let save_dir = cfg.save_dir.clone();
    let loader = DesktopCoreLoader::new(cfg.cores_dir, cfg.system_dir, cfg.save_dir);
    let mut core: Option<core_loader_desktop::DesktopCore> = None;
    let mut current_srm: Option<PathBuf> = None;
    let mut last_srm_flush = Instant::now();
    let mut paused = false;
    let mut core_sample_rate = 32_000u32;
    let mut frame_budget = Duration::from_micros(16_667);
    // Alvo do próximo frame (pacing por acumulador — corrige o overshoot do
    // `thread::sleep`, que é a causa de microstutter, sobretudo em cores com
    // fps ≠ 60 como o GBA em 59.73).
    let mut next_deadline = Instant::now();

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
                    // Grava a save RAM do jogo anterior antes de trocar.
                    if let (Some(c), Some(p)) = (core.as_ref(), current_srm.as_ref()) {
                        flush_save_ram(c, p);
                    }
                    core = None; // teardown do anterior antes de abrir outro
                    current_srm = None;
                    shared.set_state(SessionState::Idle);
                    match loader.open_core(&id, &rom) {
                        Ok(mut c) => {
                            let av = c.system_av_info();
                            let fps = av.timing.fps.max(1.0);
                            frame_budget = Duration::from_secs_f64(1.0 / fps);
                            next_deadline = Instant::now();
                            core_sample_rate = (av.timing.sample_rate.round() as u32).max(1);
                            // Carrega a battery save, se existir (antes do 1º frame).
                            let srm = srm_path(&save_dir, &rom);
                            if let Ok(bytes) = std::fs::read(&srm) {
                                if c.restore_save_ram(&bytes) {
                                    log::info!("save RAM restaurada de {srm:?}");
                                } else {
                                    log::warn!("save RAM {srm:?} ignorada (tamanho não bate)");
                                }
                            }
                            current_srm = Some(srm);
                            last_srm_flush = Instant::now();
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
                    if let (Some(c), Some(p)) = (core.as_ref(), current_srm.as_ref()) {
                        flush_save_ram(c, p);
                    }
                    core = None;
                    current_srm = None;
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
                        if !p {
                            next_deadline = Instant::now(); // não "recupera" a pausa
                        }
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
                Command::ReloadAudio(make, reply) => {
                    // fecha o stream cpal antigo antes de abrir o novo (mesmo device).
                    drop(sink.take());
                    sink = make();
                    if let (Some(s), true) = (sink.as_mut(), paused) {
                        s.pause();
                    }
                    let _ = reply.send(());
                }
                Command::Shutdown => {
                    if let (Some(c), Some(p)) = (core.as_ref(), current_srm.as_ref()) {
                        flush_save_ram(c, p);
                    }
                    break;
                }
            }
            continue;
        }

        // Sem comando pendente: roda um frame (só chega aqui se há core e não pausado).
        if let Some(c) = core.as_mut() {
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
            // Flush periódico da save RAM (barato: só grava se o jogo tem SRAM).
            if last_srm_flush.elapsed() >= SRM_FLUSH_INTERVAL {
                if let Some(p) = current_srm.as_ref() {
                    flush_save_ram(c, p);
                }
                last_srm_flush = Instant::now();
            }

            // Pacing por acumulador + spin no final. `thread::sleep` do Linux
            // passa do ponto (às vezes vários ms); dormir quase tudo e girar o
            // resto tira o microstutter. O overshoot de um frame vira uma
            // espera menor no próximo (o alvo é fixo, não re-baseado).
            next_deadline += frame_budget;
            let now = Instant::now();
            if now < next_deadline {
                let wait = next_deadline - now;
                if let Some(coarse) = wait.checked_sub(Duration::from_micros(1200)) {
                    std::thread::sleep(coarse);
                }
                while Instant::now() < next_deadline {
                    std::hint::spin_loop();
                }
            } else if now.duration_since(next_deadline) > frame_budget * 4 {
                // muito pra trás (core lento / stall) — desiste de recuperar.
                next_deadline = now;
            }
        }
    }
    // `core` dropa aqui → DesktopCore::Drop faz unload_game + deinit.
}
