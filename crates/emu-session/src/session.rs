//! `EmuSession`: API pública idêntica à de sempre (`load`/`unload`/
//! `set_paused`/`save_state`/...), mas o core libretro roda num **processo
//! filho descartável** (`reemu-core-host`) em vez de dentro deste processo.
//!
//! Por quê: alguns cores (parallel_n64...) não são re-entrantes — guardam
//! estado global em C que sobrevive ao `dlclose`, então um 2º `retro_init`
//! no MESMO processo derruba o processo inteiro, sem erro visível. Matando o
//! filho e subindo um novo a cada `load`, a re-entrância do core deixa de
//! importar: memória sempre parte limpa. Ver a memória `n64-reload-crash` e
//! `docs/ai-context/02-core-loader-desktop.md`.
//!
//! O `AudioSink` (cpal) continua **neste** processo (não faz sentido recriar
//! o device de áudio a cada troca de core) — o filho manda os samples crus
//! por IPC. `.srm`/save-state ficam com os arquivos aqui também; só os bytes
//! vêm do filho.

use core_ipc::{Channel, FrameKind, HwPlaneMeta, PortInput, ToChild, ToParent};
use core_loader_desktop::{AnalogState, RetroPadState};
use domain::audio::AudioSink;
use domain::core_loader::{CoreId, CoreLoadError, SystemAvInfo};
use domain::core_options::CoreOptionDefinition;
use domain::frame_source::{DmabufPlaneInfo, Frame, FrameOrigin, GpuTextureHandle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Constrói o `AudioSink` **dentro** da thread supervisora (a `cpal::Stream`
/// é `!Send`). Retorna `None` se o áudio não pôde abrir (o app segue sem
/// som). `SessionConfig.audio_sink = None` = sem áudio (ex: testes).
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
    Load(
        CoreId,
        String,
        HashMap<String, String>,
        Sender<Result<SystemAvInfo, CoreLoadError>>,
    ),
    Unload(Sender<()>),
    SetPaused(bool, Sender<()>),
    SaveState(Sender<Option<Vec<u8>>>),
    RestoreState(Vec<u8>, Sender<bool>),
    /// Troca o `AudioSink` em runtime (mudou o device / sample rate nas configs).
    ReloadAudio(AudioSinkFactory, Sender<()>),
    GetCoreOptions(Sender<(Vec<CoreOptionDefinition>, HashMap<String, String>)>),
    SetCoreOption(String, String, Sender<bool>),
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
    /// PID do processo `reemu-core-host` ativo agora (`None` = ocioso).
    /// Observabilidade/diagnóstico — e a garantia de "processo novo por
    /// load" (o bug de reentrância do N64) é testável a partir disto.
    child_pid: Mutex<Option<u32>>,
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
            child_pid: Mutex::new(None),
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
    /// `initial_option_values` são os valores de core options salvos no DB
    /// (o core pede via `GET_VARIABLE` já durante o load, dentro do filho).
    pub fn load(
        &self,
        core_id: &str,
        rom_path: &str,
        initial_option_values: HashMap<String, String>,
    ) -> Result<SystemAvInfo, SessionError> {
        self.call(|reply| {
            Command::Load(
                CoreId(core_id.to_string()),
                rom_path.to_string(),
                initial_option_values,
                reply,
            )
        })?
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

    /// Recria o `AudioSink` na thread supervisora (mudança de device/sample
    /// rate nas configs, sem precisar recarregar o jogo).
    pub fn reload_audio(&self, factory: AudioSinkFactory) -> Result<(), SessionError> {
        self.call(|reply| Command::ReloadAudio(factory, reply))
    }

    /// Schema + valores atuais de core options do core carregado agora
    /// (vazio se não há core, ou se ele não declara opções).
    pub fn core_options(&self) -> (Vec<CoreOptionDefinition>, HashMap<String, String>) {
        self.call(Command::GetCoreOptions).unwrap_or_default()
    }

    /// Troca uma opção do core em runtime. `false` se não há core ou a
    /// chave/valor não bate no schema.
    pub fn set_core_option(&self, key: &str, value: &str) -> bool {
        self.call(|reply| Command::SetCoreOption(key.to_string(), value.to_string(), reply))
            .unwrap_or(false)
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

    /// PID do processo `reemu-core-host` ativo agora (`None` = ocioso).
    /// Cada `load` sobe um processo NOVO (nunca reusa) — é essa garantia que
    /// isola cores não re-entrantes como o parallel_n64; testável comparando
    /// o PID antes/depois de uma troca.
    pub fn debug_child_pid(&self) -> Option<u32> {
        *self
            .shared
            .child_pid
            .lock()
            .unwrap_or_else(|p| p.into_inner())
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

// --- input: espelho deste processo (o global de verdade é do FILHO) --------

/// Estado do RetroPad deste processo — a thread de gamepad/o teclado do
/// shell escrevem aqui do jeito de sempre; a thread supervisora manda o
/// snapshot pro filho por IPC a cada tick.
static PARENT_PAD: RetroPadState = RetroPadState::new();
static PARENT_ANALOG: AnalogState = AnalogState::new();

pub fn retropad() -> &'static RetroPadState {
    &PARENT_PAD
}

pub fn analog() -> &'static AnalogState {
    &PARENT_ANALOG
}

fn snapshot_input() -> [PortInput; 4] {
    std::array::from_fn(|port| PortInput {
        joypad_mask: PARENT_PAD.mask(port),
        sticks: PARENT_ANALOG.sticks(port),
    })
}

fn gamepad_loop(shared: Arc<Shared>) {
    let mut poller = match input_desktop::GamepadPoller::new() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("gamepad indisponível: {e} — só teclado");
            return;
        }
    };
    while !shared.gamepad_stop.load(Ordering::Relaxed) {
        let outcome = poller.poll(&PARENT_PAD, &PARENT_ANALOG);
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
            PARENT_PAD.clear(); // no menu, nada de input de jogo
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

// --- processo filho ----------------------------------------------------

/// Acha o binário irmão `reemu-core-host` a partir do executável atual.
/// `cargo test` roda de `target/debug/deps/`, o bin do workspace fica 1
/// nível acima; `cargo tauri dev`/produção já ficam no mesmo nível.
fn core_host_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..2 {
        let candidate = dir.join("reemu-core-host");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

/// Uma mensagem `ToParent` + os fds (`SCM_RIGHTS`) que vieram junto dela — no
/// máximo 1 hoje (memfd do anel no `Loaded`, dma_buf num `FrameReady` de
/// interop). Repassado inteiro pra quem consome, pra nunca perder um fd só
/// porque o consumidor não olhou pra ele na hora certa.
struct InboundEvent {
    msg: ToParent,
    fds: Vec<rustix::fd::OwnedFd>,
}

struct ChildProc {
    child: std::process::Child,
    channel: Channel,
    reader: Option<JoinHandle<()>>,
}

impl ChildProc {
    fn spawn() -> Result<(Self, Receiver<InboundEvent>), String> {
        let exe = core_host_path().ok_or_else(|| {
            "binário reemu-core-host não encontrado ao lado do executável".to_string()
        })?;
        let (parent_ch, child_ch) = Channel::pair().map_err(|e| format!("socketpair: {e}"))?;
        child_ch
            .clear_cloexec()
            .map_err(|e| format!("clear CLOEXEC: {e}"))?;
        #[cfg(debug_assertions)]
        child_ch.assert_inheritable();
        let fd_num = child_ch.as_raw_fd();
        let child = std::process::Command::new(exe)
            .arg("--fd")
            .arg(fd_num.to_string())
            .spawn()
            .map_err(|e| format!("spawn reemu-core-host: {e}"))?;
        // O processo filho já herdou o fd no fork; nossa cópia do lado dele
        // não serve mais pra nada.
        drop(child_ch);

        let (etx, erx) = mpsc::channel::<InboundEvent>();
        let reader_channel = parent_ch.clone();
        let reader = std::thread::Builder::new()
            .name("emu-core-host-reader".into())
            .spawn(move || loop {
                match reader_channel.recv::<ToParent>() {
                    Ok(Some((msg, fds))) => {
                        if etx.send(InboundEvent { msg, fds }).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break, // filho fechou o canal
                    Err(e) => {
                        log::warn!("canal IPC com o core-host: {e}");
                        break;
                    }
                }
            })
            .expect("spawn emu-core-host-reader");

        Ok((
            Self {
                child,
                channel: parent_ch,
                reader: Some(reader),
            },
            erx,
        ))
    }

    /// Mata incondicionalmente — é a garantia estrutural contra cores não
    /// re-entrantes (parallel_n64...): o próximo `Load` sempre sobe um
    /// processo NOVO, nunca reusa este.
    fn kill(mut self) {
        let _ = self.channel.send(&ToChild::Shutdown, &[]);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
    }
}

/// Bloqueia até achar um evento que bate no `extract`, processando (via
/// `handle_event`) qualquer coisa "fire and forget" (frame/áudio/log) que
/// vier no meio — nunca dropa um `FrameReady` só porque o supervisor está
/// esperando a resposta de outra coisa. `None` = timeout ou o filho morreu.
fn wait_for_reply<T>(
    erx: &Receiver<InboundEvent>,
    timeout: Duration,
    shared: &Shared,
    sink: &mut Option<Box<dyn AudioSink>>,
    ring: &mut Option<core_ipc::FrameRing>,
    mut extract: impl FnMut(InboundEvent) -> Result<T, InboundEvent>,
) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return None;
        }
        let ev = erx.recv_timeout(remaining).ok()?;
        match extract(ev) {
            Ok(value) => return Some(value),
            Err(ev) => handle_event(ev, shared, sink, ring),
        }
    }
}

/// Processa um evento "fire and forget" do filho: frame novo publica em
/// `shared.latest_frame`, áudio vai pro sink (ou pro buffer sem sink), o
/// resto é log. Respostas de round-trip (`Loaded`, `*Result`, ...) nunca
/// chegam aqui — `wait_for_reply` as intercepta antes.
fn handle_event(
    ev: InboundEvent,
    shared: &Shared,
    sink: &mut Option<Box<dyn AudioSink>>,
    ring: &mut Option<core_ipc::FrameRing>,
) {
    match ev.msg {
        ToParent::FrameReady { slot, meta, kind } => {
            if let Some(frame) = reconstruct_frame(ring.as_ref(), slot, meta, kind, ev.fds) {
                shared.frame_seq.fetch_add(1, Ordering::Relaxed);
                *shared
                    .latest_frame
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some(frame);
            }
        }
        ToParent::AudioBatch {
            samples,
            sample_rate,
        } => match sink.as_mut() {
            Some(s) => s.push_samples(&samples, sample_rate),
            None => shared
                .audio
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend_from_slice(&samples),
        },
        ToParent::AvInfoChanged { fps, sample_rate } => {
            log::info!(
                "timing atualizado em runtime: fps={fps:.3} sample_rate={sample_rate:.0} Hz"
            );
        }
        ToParent::SaveRamRestored(Some(true)) => log::info!("save RAM restaurada"),
        ToParent::SaveRamRestored(Some(false)) => {
            log::warn!("save RAM ignorada (tamanho não bate)")
        }
        ToParent::SaveRamRestored(None) => {}
        ToParent::Warn(msg) => log::warn!("core-host: {msg}"),
        other => log::debug!("evento do core-host fora de um round-trip: {other:?}"),
    }
}

/// Reconstrói o `Frame` de domínio a partir de um `FrameReady`: caminho
/// software lê do anel de shared memory, caminho HW/interop empacota o fd
/// recebido (se houver) num `GpuTextureHandle` — `gpu.rs` não sabe a
/// diferença entre isto e o `GlInteropHandle` de quando tudo era 1 processo.
fn reconstruct_frame(
    ring: Option<&core_ipc::FrameRing>,
    slot: u32,
    meta: domain::frame_source::FrameMetadata,
    kind: FrameKind,
    fds: Vec<rustix::fd::OwnedFd>,
) -> Option<Frame> {
    match kind {
        FrameKind::Software { pitch, format } => {
            let ring = ring?;
            let len = pitch as usize * meta.native_height as usize;
            let data = ring.read_slot_to_vec(slot as usize, len);
            Some(Frame {
                origin: FrameOrigin::SoftwareRawBuffer {
                    data,
                    pitch,
                    format,
                },
                metadata: meta,
            })
        }
        FrameKind::Hardware { flip_y, plane } => {
            let plane = plane.map(|p| dmabuf_plane_info(p, fds));
            Some(Frame {
                origin: FrameOrigin::HardwareTexture(Box::new(IpcGpuTextureHandle {
                    slot,
                    flip_y,
                    plane: Mutex::new(plane),
                })),
                metadata: meta,
            })
        }
    }
}

fn dmabuf_plane_info(meta: HwPlaneMeta, fds: Vec<rustix::fd::OwnedFd>) -> DmabufPlaneInfo {
    use rustix::fd::IntoRawFd;
    // SAFETY/posse: o fd recebido por `SCM_RIGHTS` é nosso a partir daqui;
    // `DmabufPlaneInfo` documenta que a posse passa pra quem chama
    // `take_plane` (fecha ao dropar) — é exatamente o `gpu.rs::import_dmabuf`
    // de sempre, que já sabe fazer isso.
    let fd = fds
        .into_iter()
        .next()
        .map(|f| f.into_raw_fd())
        .unwrap_or(-1);
    DmabufPlaneInfo {
        fd,
        width: meta.width,
        height: meta.height,
        stride: meta.stride,
        offset: meta.offset,
        modifier: meta.modifier,
        fourcc: meta.fourcc,
    }
}

/// Espelha `core_loader_desktop::gl_context::GlInteropHandle` — mesma forma,
/// só que alimentado pela mensagem IPC em vez de um `GlContext` local.
struct IpcGpuTextureHandle {
    slot: u32,
    flip_y: bool,
    plane: Mutex<Option<DmabufPlaneInfo>>,
}

impl GpuTextureHandle for IpcGpuTextureHandle {
    fn slot(&self) -> u32 {
        self.slot
    }

    fn take_plane(&self) -> Option<DmabufPlaneInfo> {
        self.plane.lock().unwrap_or_else(|p| p.into_inner()).take()
    }

    fn flip_y(&self) -> bool {
        self.flip_y
    }
}

/// Pede a save RAM atual ao filho (round-trip curto) — usado no flush
/// periódico e antes de matar um processo (troca de ROM / unload / shutdown).
fn request_save_ram(
    proc: &ChildProc,
    erx: &Receiver<InboundEvent>,
    shared: &Shared,
    sink: &mut Option<Box<dyn AudioSink>>,
    ring: &mut Option<core_ipc::FrameRing>,
) -> Option<Vec<u8>> {
    proc.channel.send(&ToChild::GetSaveRam, &[]).ok()?;
    wait_for_reply(
        erx,
        Duration::from_secs(2),
        shared,
        sink,
        ring,
        |ev| match ev.msg {
            ToParent::SaveRamResult(bytes) => Ok(bytes),
            _ => Err(ev),
        },
    )
    .flatten()
}

/// `<save_dir>/<stem da rom>.srm` — convenção do RetroArch pra battery save.
fn srm_path(dir: &std::path::Path, rom_path: &str) -> PathBuf {
    let stem = std::path::Path::new(rom_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game");
    dir.join(format!("{stem}.srm"))
}

/// Escrita atômica de um `.srm`: `.tmp` no mesmo diretório + `rename` por cima
/// (atômico no mesmo FS; um kill no meio deixa a `.srm` antiga intacta).
fn write_srm(path: &std::path::Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("srm.tmp");
    match std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path)) {
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
    let cores_dir = cfg.cores_dir.clone();
    let system_dir = cfg.system_dir.clone();
    let save_dir = cfg.save_dir.clone();

    let mut proc: Option<ChildProc> = None;
    let mut events: Option<Receiver<InboundEvent>> = None;
    let mut ring: Option<core_ipc::FrameRing> = None;
    let mut current_srm: Option<PathBuf> = None;
    let mut last_srm_flush = Instant::now();

    // O flush periódico da `.srm` sai desta thread (a escrita+rename+fsync
    // podia atrasar o supervisor a cada 10s → hitch no input/frame). Só a
    // ida-e-volta pelo filho fica aqui (barata); a escrita vai pra trás.
    let (srm_tx, srm_rx) = mpsc::channel::<(PathBuf, Vec<u8>)>();
    let srm_writer = std::thread::Builder::new()
        .name("reemu-srm-writer".into())
        .spawn(move || {
            for (path, bytes) in srm_rx {
                write_srm(&path, &bytes);
            }
        })
        .ok();

    loop {
        let cmd = if proc.is_none() {
            rx.recv().ok()
        } else {
            rx.try_recv().ok()
        };

        let Some(cmd) = cmd else {
            if let Some(erx) = events.as_ref() {
                while let Ok(ev) = erx.try_recv() {
                    handle_event(ev, &shared, &mut sink, &mut ring);
                }
            }
            if let Some(p) = proc.as_ref() {
                let _ = p.channel.send(
                    &ToChild::Input {
                        ports: snapshot_input(),
                    },
                    &[],
                );
            }
            if let (Some(p), Some(erx), Some(path)) =
                (proc.as_ref(), events.as_ref(), current_srm.as_ref())
            {
                if last_srm_flush.elapsed() >= SRM_FLUSH_INTERVAL {
                    if let Some(bytes) = request_save_ram(p, erx, &shared, &mut sink, &mut ring) {
                        let _ = srm_tx.send((path.clone(), bytes));
                    }
                    last_srm_flush = Instant::now();
                }
            }
            std::thread::sleep(Duration::from_millis(8));
            continue;
        };

        match cmd {
            Command::Load(id, rom, initial_option_values, reply) => {
                // Salva a save RAM do jogo anterior e mata o processo
                // incondicionalmente — o novo `Load` SEMPRE sobe um processo
                // novo, mesmo que o anterior fosse o mesmo core (é essa
                // garantia que resolve os cores não re-entrantes).
                if let (Some(p), Some(erx)) = (proc.as_ref(), events.as_ref()) {
                    if let (Some(bytes), Some(path)) = (
                        request_save_ram(p, erx, &shared, &mut sink, &mut ring),
                        current_srm.as_ref(),
                    ) {
                        write_srm(path, &bytes);
                    }
                }
                if let Some(p) = proc.take() {
                    p.kill();
                }
                events = None;
                ring = None;
                current_srm = None;
                *shared
                    .latest_frame
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = None;
                *shared.child_pid.lock().unwrap_or_else(|p| p.into_inner()) = None;
                shared.set_state(SessionState::Idle);

                let target_srm = srm_path(&save_dir, &rom);
                let initial_save_ram = std::fs::read(&target_srm).ok();

                match ChildProc::spawn() {
                    Ok((p, erx)) => {
                        let pid = p.child.id();
                        let sent = p.channel.send(
                            &ToChild::Load {
                                core_id: id.0.clone(),
                                rom_path: rom.clone(),
                                cores_dir: cores_dir.clone(),
                                system_dir: system_dir.clone(),
                                save_dir: save_dir.clone(),
                                initial_option_values,
                                initial_save_ram,
                            },
                            &[],
                        );
                        if sent.is_err() {
                            p.kill();
                            let _ = reply.send(Err(CoreLoadError::LoadFailed(
                                "falha ao mandar Load pro core-host".into(),
                            )));
                            continue;
                        }
                        let loaded = wait_for_reply(
                            &erx,
                            Duration::from_secs(30),
                            &shared,
                            &mut sink,
                            &mut ring,
                            |ev| match ev.msg {
                                ToParent::Loaded(result) => Ok((result, ev.fds)),
                                _ => Err(ev),
                            },
                        );
                        match loaded {
                            Some((Ok(av), fds)) => {
                                let max_w =
                                    av.geometry.max_width.max(av.geometry.base_width).max(1);
                                let max_h =
                                    av.geometry.max_height.max(av.geometry.base_height).max(1);
                                let slot_size = (max_w * max_h * 4) as usize;
                                let ring_ok = fds.into_iter().next().and_then(|fd| {
                                    core_ipc::FrameRing::from_fd(fd, slot_size).ok()
                                });
                                if ring_ok.is_none() {
                                    log::warn!(
                                        "core {}: sem anel de frame (fd não veio) — sem vídeo",
                                        id.0
                                    );
                                }
                                ring = ring_ok;
                                *shared.loaded_core.lock().unwrap_or_else(|p| p.into_inner()) =
                                    Some(id.0.clone());
                                *shared.child_pid.lock().unwrap_or_else(|p| p.into_inner()) =
                                    Some(pid);
                                shared.set_state(SessionState::Running);
                                current_srm = Some(target_srm);
                                last_srm_flush = Instant::now();
                                if let Some(s) = sink.as_mut() {
                                    s.resume();
                                }
                                proc = Some(p);
                                events = Some(erx);
                                let _ = reply.send(Ok(av));
                            }
                            Some((Err(msg), _)) => {
                                p.kill();
                                let _ = reply.send(Err(CoreLoadError::LoadFailed(msg)));
                            }
                            None => {
                                p.kill();
                                let _ = reply.send(Err(CoreLoadError::LoadFailed(
                                    "core-host não respondeu (timeout)".into(),
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = reply.send(Err(CoreLoadError::LoadFailed(e)));
                    }
                }
            }
            Command::Unload(reply) => {
                *shared
                    .latest_frame
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = None;
                shared.set_state(SessionState::Idle);
                if let (Some(p), Some(erx)) = (proc.as_ref(), events.as_ref()) {
                    if let (Some(bytes), Some(path)) = (
                        request_save_ram(p, erx, &shared, &mut sink, &mut ring),
                        current_srm.as_ref(),
                    ) {
                        write_srm(path, &bytes);
                    }
                }
                if let Some(p) = proc.take() {
                    p.kill();
                }
                events = None;
                ring = None;
                current_srm = None;
                *shared.loaded_core.lock().unwrap_or_else(|p| p.into_inner()) = None;
                *shared.child_pid.lock().unwrap_or_else(|p| p.into_inner()) = None;
                if let Some(s) = sink.as_mut() {
                    s.pause();
                }
                let _ = reply.send(());
            }
            Command::SetPaused(want_paused, reply) => {
                if let Some(p) = proc.as_ref() {
                    let _ = p.channel.send(&ToChild::SetPaused(want_paused), &[]);
                    if let Some(s) = sink.as_mut() {
                        if want_paused {
                            s.pause();
                        } else {
                            s.resume();
                        }
                    }
                    shared.set_state(if want_paused {
                        SessionState::Paused
                    } else {
                        SessionState::Running
                    });
                }
                let _ = reply.send(());
            }
            Command::SaveState(reply) => {
                let bytes = match (proc.as_ref(), events.as_ref()) {
                    (Some(p), Some(erx)) => {
                        let _ = p.channel.send(&ToChild::SaveState, &[]);
                        wait_for_reply(
                            erx,
                            Duration::from_secs(5),
                            &shared,
                            &mut sink,
                            &mut ring,
                            |ev| match ev.msg {
                                ToParent::SaveStateResult(b) => Ok(b),
                                _ => Err(ev),
                            },
                        )
                        .flatten()
                    }
                    _ => None,
                };
                let _ = reply.send(bytes);
            }
            Command::RestoreState(data, reply) => {
                let ok = match (proc.as_ref(), events.as_ref()) {
                    (Some(p), Some(erx)) => {
                        let _ = p.channel.send(&ToChild::RestoreState(data), &[]);
                        wait_for_reply(
                            erx,
                            Duration::from_secs(5),
                            &shared,
                            &mut sink,
                            &mut ring,
                            |ev| match ev.msg {
                                ToParent::RestoreStateResult(ok) => Ok(ok),
                                _ => Err(ev),
                            },
                        )
                        .unwrap_or(false)
                    }
                    _ => false,
                };
                let _ = reply.send(ok);
            }
            Command::ReloadAudio(make, reply) => {
                // fecha o stream cpal antigo antes de abrir o novo (mesmo device).
                drop(sink.take());
                sink = make();
                if let Some(s) = sink.as_mut() {
                    if matches!(shared.state(), SessionState::Paused) {
                        s.pause();
                    }
                }
                let _ = reply.send(());
            }
            Command::GetCoreOptions(reply) => {
                let result = match (proc.as_ref(), events.as_ref()) {
                    (Some(p), Some(erx)) => {
                        let _ = p.channel.send(&ToChild::GetCoreOptions, &[]);
                        wait_for_reply(
                            erx,
                            Duration::from_secs(2),
                            &shared,
                            &mut sink,
                            &mut ring,
                            |ev| match ev.msg {
                                ToParent::CoreOptionsSnapshot { schema, values } => {
                                    Ok((schema, values))
                                }
                                _ => Err(ev),
                            },
                        )
                    }
                    _ => None,
                };
                let _ = reply.send(result.unwrap_or_default());
            }
            Command::SetCoreOption(key, value, reply) => {
                let ok = match (proc.as_ref(), events.as_ref()) {
                    (Some(p), Some(erx)) => {
                        let _ = p.channel.send(&ToChild::SetCoreOption { key, value }, &[]);
                        wait_for_reply(
                            erx,
                            Duration::from_secs(2),
                            &shared,
                            &mut sink,
                            &mut ring,
                            |ev| match ev.msg {
                                ToParent::SetCoreOptionResult(ok) => Ok(ok),
                                _ => Err(ev),
                            },
                        )
                        .unwrap_or(false)
                    }
                    _ => false,
                };
                let _ = reply.send(ok);
            }
            Command::Shutdown => {
                if let (Some(p), Some(erx)) = (proc.as_ref(), events.as_ref()) {
                    if let (Some(bytes), Some(path)) = (
                        request_save_ram(p, erx, &shared, &mut sink, &mut ring),
                        current_srm.as_ref(),
                    ) {
                        write_srm(path, &bytes);
                    }
                }
                if let Some(p) = proc.take() {
                    p.kill();
                }
                break;
            }
        }
    }

    drop(srm_tx);
    if let Some(t) = srm_writer {
        let _ = t.join();
    }
}

impl Shared {
    fn state(&self) -> SessionState {
        *self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}
