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

use crate::local_core::LocalCore;
use core_ipc::{Channel, FrameKind, HwPlaneMeta, PortInput, ToChild, ToParent};
use core_loader_desktop::{AnalogState, RetroPadState};
use domain::audio::AudioSink;
use domain::core_loader::{CoreId, CoreLoadError, SystemAvInfo};
use domain::core_options::CoreOptionDefinition;
use domain::frame_source::{DmabufPlaneInfo, Frame, FrameOrigin, GpuTextureHandle};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
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
    /// Buffers de quadros software já apresentados, pra reuso no próximo
    /// `FrameReady` — sem isto cada quadro alocava (e liberava) um `Vec` do
    /// tamanho do frame (ver `EmuSession::recycle_frame`).
    spare_bufs: Mutex<Vec<Vec<u8>>>,
    /// Plano `dma_buf` de um slot de interop que veio num quadro descartado
    /// antes de alguém importar (o filho só manda o plano no 1º quadro de
    /// cada slot). Sem isto o slot nunca era importado e os quadros dele
    /// sumiam em silêncio — metade da imagem, ou tela preta. Entregue no
    /// próximo quadro do mesmo slot; esvaziado a cada `Load` (os slots são
    /// do processo filho).
    orphan_planes: Mutex<HashMap<u32, OrphanPlane>>,
    /// Sinaliza `latest_frame` preenchido — quem apresenta espera nisto em
    /// vez de dormir um tempo fixo (ver `wait_for_frame`).
    frame_ready: Condvar,
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
    /// Gamepads que conectaram desde o último drain — `(guid_hex, nome)`. O
    /// shell emite cada um pro frontend como `gamepad-connected`.
    gamepad_connected: Mutex<Vec<(String, String)>>,
    /// Guids que desconectaram desde o último drain (`gamepad-disconnected`).
    gamepad_disconnected: Mutex<Vec<String>>,
    /// Pulsos de navegação de menu vindos do gamepad — o shell drena e emite
    /// pro frontend como `menu-nav`.
    nav: Mutex<Vec<input_desktop::NavPulse>>,
    /// PID do processo `reemu-core-host` ativo agora (`None` = ocioso).
    /// Observabilidade/diagnóstico — e a garantia de "processo novo por
    /// load" (o bug de reentrância do N64) é testável a partir disto.
    child_pid: Mutex<Option<u32>>,
    /// Handles crus do `VkDevice` do compositor, publicados pelo shell depois
    /// que o `FrameProcessor` sobe (`EmuSession::attach_vulkan_device`).
    /// `Some` + `REEMU_HW=vulkan` = um core que negocia Vulkan roda
    /// **in-process** (etapa 12 B3b), não no `reemu-core-host`.
    vulkan_shared_device: Mutex<Option<domain::core_loader::VulkanSharedDevice>>,
    /// Fábrica pra cores Vulkan "donos do device" (Beetle PSX HW) — publicada
    /// pelo shell junto do `vulkan_shared_device`. Ver `docs/ai-context/12`
    /// §Beetle (D2/D3).
    vulkan_negotiator: Mutex<Option<domain::core_loader::VulkanDeviceNegotiator>>,
    /// Core Vulkan in-process (etapa 12 B3b/D4). **Dirigido pela thread do
    /// compositor** (o `retro_run` do Beetle submete sozinho na `VkQueue`, que
    /// é a do wgpu — tem que ser a MESMA thread do submit do wgpu). O
    /// `core_loop` só carrega/descarrega e responde comandos; quem chama
    /// `step_vk_local` é o video pump.
    vk_local: Mutex<Option<LocalCore>>,
    /// `true` quando há um `vk_local` carregado (checagem barata sem travar).
    vk_local_active: AtomicBool,
    vk_local_paused: AtomicBool,
    /// Ticks consecutivos com `vk_local` ativo mas sem frame — só pro warn de
    /// diagnóstico em `step_vk_local`.
    vk_local_stall: AtomicU64,
    /// Áudio que o `step_vk_local` (thread do compositor) produziu — o
    /// `core_loop` (dono do `AudioSink` `!Send`) drena isto pro sink.
    vk_local_audio: Mutex<Vec<(Vec<i16>, u32)>>,
    /// Serializa quem submete na `VkQueue` compartilhada: o video pump
    /// (`step_vk_local` + `render_to_surface` do wgpu) e o `core_loop`
    /// (`serialize_state`/`restore_state` — o `retro_serialize` de alguns cores
    /// submete). A `VkQueue` NÃO é sincronizada externamente pelo Vulkan.
    vk_render_gate: Mutex<()>,
}

impl Shared {
    /// Máximo de buffers guardados: um em uso pelo apresentador e um
    /// esperando é o que o pipeline precisa.
    const MAX_SPARE: usize = 2;

    fn recycle(&self, frame: Frame) {
        match frame.origin {
            FrameOrigin::SoftwareRawBuffer { data, .. } => {
                let mut pool = self.spare_bufs.lock().unwrap_or_else(|p| p.into_inner());
                if pool.len() < Self::MAX_SPARE {
                    pool.push(data);
                }
            }
            FrameOrigin::HardwareTexture(handle) => {
                stash_orphan_plane(&self.orphan_planes, handle.as_ref());
            }
            _ => {}
        }
    }

    fn take_spare(&self) -> Option<Vec<u8>> {
        self.spare_bufs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop()
    }

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
            spare_bufs: Mutex::new(Vec::new()),
            orphan_planes: Mutex::new(HashMap::new()),
            frame_ready: Condvar::new(),
            audio: Mutex::new(Vec::new()),
            state: Mutex::new(SessionState::Idle),
            loaded_core: Mutex::new(None),
            game_focused: AtomicBool::new(true),
            menu_requested: AtomicBool::new(false),
            gamepad_stop: AtomicBool::new(false),
            captured_inputs: Mutex::new(Vec::new()),
            gamepads: Mutex::new(Vec::new()),
            gamepad_connected: Mutex::new(Vec::new()),
            gamepad_disconnected: Mutex::new(Vec::new()),
            nav: Mutex::new(Vec::new()),
            child_pid: Mutex::new(None),
            vulkan_shared_device: Mutex::new(None),
            vulkan_negotiator: Mutex::new(None),
            vk_local: Mutex::new(None),
            vk_local_active: AtomicBool::new(false),
            vk_local_paused: AtomicBool::new(false),
            vk_local_stall: AtomicU64::new(0),
            vk_local_audio: Mutex::new(Vec::new()),
            vk_render_gate: Mutex::new(()),
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

    /// Publica os handles do `VkDevice` do compositor (do
    /// `FrameProcessor::vulkan_shared_device()`). Chamado pelo shell depois que
    /// o `FrameProcessor` sobe. A partir daí, com `REEMU_HW=vulkan`, um core
    /// que negocia Vulkan roda in-process (etapa 12 B3b) em vez de no
    /// `reemu-core-host` — a `VkImage` dele fica no mesmo device do compositor.
    pub fn attach_vulkan_device(&self, device: domain::core_loader::VulkanSharedDevice) {
        *self
            .shared
            .vulkan_shared_device
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(device);
    }

    /// Publica a fábrica pra cores Vulkan "donos do device" (Beetle PSX HW) —
    /// chamada pelo shell junto do `attach_vulkan_device`. Ver
    /// `docs/ai-context/12-vulkan-hw-render-fase2.md` §Beetle.
    pub fn attach_vulkan_negotiator(
        &self,
        negotiator: domain::core_loader::VulkanDeviceNegotiator,
    ) {
        *self
            .shared
            .vulkan_negotiator
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(negotiator);
    }

    /// Roda um frame do core Vulkan in-process, SE houver um e não estiver
    /// pausado. **Tem que ser chamado pela thread do compositor** (a mesma que
    /// submete o wgpu) — o `retro_run` de cores como o Beetle submete direto na
    /// `VkQueue` compartilhada, e Vulkan proíbe usar uma queue de duas threads.
    /// O shell chama isto no video pump, antes do submit do wgpu, e usa o
    /// `Frame` devolvido. `None` = não há core Vulkan local / está pausado /
    /// frame duplicado.
    /// Trava o "portão" da `VkQueue` compartilhada. O video pump segura isto
    /// enquanto roda `step_vk_local` + o submit do wgpu (`render_to_surface`);
    /// o `core_loop` pega antes de `serialize_state`/`restore_state` (o
    /// `retro_serialize` de alguns cores submete). Mantém os submits das duas
    /// threads serializados — Vulkan não sincroniza a queue sozinho.
    pub fn lock_vk_queue(&self) -> std::sync::MutexGuard<'_, ()> {
        self.shared
            .vk_render_gate
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    pub fn step_vk_local(&self) -> Option<Frame> {
        if !self.shared.vk_local_active.load(Ordering::Acquire)
            || self.shared.vk_local_paused.load(Ordering::Acquire)
        {
            return None;
        }
        let mut guard = self
            .shared
            .vk_local
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let lc = guard.as_mut()?;
        lc.apply_input(&snapshot_input());
        let tick = lc.run_frame();
        if !tick.audio.is_empty() {
            self.shared
                .vk_local_audio
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push((tick.audio, tick.sample_rate));
        }
        match tick.frame.as_ref() {
            Some(_) => {
                self.shared.vk_local_stall.store(0, Ordering::Relaxed);
                let n = self.shared.frame_seq.fetch_add(1, Ordering::Relaxed);
                if n == 0 {
                    log::info!("etapa 12: 1º frame Vulkan in-process produzido");
                }
            }
            None => {
                // Ativo mas sem frame por muito tempo = o core não está
                // entregando a VkImage (formato de scanout, barrier, etc).
                let n = self.shared.vk_local_stall.fetch_add(1, Ordering::Relaxed);
                if n == 120 {
                    log::warn!(
                        "etapa 12: core Vulkan ativo mas 120 ticks SEM frame — \
                         a VkImage não está chegando no compositor"
                    );
                }
            }
        }
        tick.frame
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

    /// Drena os gamepads que conectaram desde o último drain. O shell emite
    /// cada um pro frontend como `gamepad-connected` (toast + ícone).
    pub fn take_gamepad_connected(&self) -> Vec<(String, String)> {
        std::mem::take(
            &mut *self
                .shared
                .gamepad_connected
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
        )
    }

    /// Drena os guids que desconectaram desde o último drain
    /// (`gamepad-disconnected`).
    pub fn take_gamepad_disconnected(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .shared
                .gamepad_disconnected
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
        )
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

    /// Devolve um quadro já apresentado: o buffer de um quadro software volta
    /// pro pool e o próximo `FrameReady` copia o anel pra dentro dele, sem
    /// alocar. Opcional — quem não devolve só perde o reuso.
    pub fn recycle_frame(&self, frame: Frame) {
        self.shared.recycle(frame);
    }

    /// Bloqueia até existir um frame não consumido ou `timeout` passar — NÃO
    /// o consome (quem apresenta chama `take_latest_frame` depois). Serve pra
    /// o loop de vídeo acordar no instante em que o frame chega, em vez de
    /// dormir um tempo fixo fora de fase com o core (15 ms fixos contra os
    /// 16,67 ms de um core a 60 fps perdiam/repetiam frames).
    pub fn wait_for_frame(&self, timeout: std::time::Duration) {
        let guard = self
            .shared
            .latest_frame
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _ = self
            .shared
            .frame_ready
            .wait_timeout_while(guard, timeout, |f| f.is_none());
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

type LocalVkRoute = (
    Option<domain::core_loader::VulkanSharedDevice>,
    Option<domain::core_loader::VulkanDeviceNegotiator>,
);

/// Cores que fazem HW render Vulkan e por isso TÊM que rodar in-process (a
/// `VkImage` não cruza a fronteira de processo — no `reemu-core-host` o frame é
/// descartado e a tela fica preta). Casado pelo basename do id/caminho.
/// `parallel_n64` fica de fora de propósito: é o core não re-entrante que essa
/// arquitetura de processo-filho protege — pra ele o Vulkan exige
/// `REEMU_HW=vulkan` explícito.
const VK_CAPABLE_CORES: &[&str] = &[
    "mednafen_psx_hw",
    "beetle_psx_hw",
    "flycast",
    "mupen64plus_next",
];

/// Se um core deve rodar in-process (etapa 12): precisa de um caminho de device
/// publicado pelo shell (`attach_vulkan_device` / `attach_vulkan_negotiator`)
/// E ou (a) `REEMU_HW=vulkan` (força qualquer core), ou (b) o core está na
/// lista dos que fazem Vulkan HW render (`VK_CAPABLE_CORES`) — senão um core
/// tipo o `mednafen_psx_hw` configurado pra Vulkan iria pro processo filho e a
/// tela ficaria preta. Se o core acabar não sendo Vulkan, `LocalCore::load`
/// devolve `HwRenderUnsupported` e o loop cai pro filho (+ cache).
///
/// A escolha automática (b) só vale no Linux, onde o caminho foi validado. No
/// Windows o `flycast` carregado in-process derrubou o app inteiro
/// (`STATUS_ACCESS_VIOLATION`, 2026-09-25) — sem o isolamento do processo
/// filho, um core que quebra leva a interface junto. Lá só com `REEMU_HW=vulkan`.
fn route_local_device(shared: &Shared, core_id: &str) -> Option<LocalVkRoute> {
    let forced = matches!(
        std::env::var("REEMU_HW")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Ok("vulkan") | Ok("vk")
    );
    let base = core_id.rsplit(['/', '\\']).next().unwrap_or(core_id);
    let vk_capable = cfg!(target_os = "linux") && VK_CAPABLE_CORES.iter().any(|c| base.contains(c));
    if !forced && !vk_capable {
        return None;
    }
    let device = *shared
        .vulkan_shared_device
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let negotiator = shared
        .vulkan_negotiator
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    (device.is_some() || negotiator.is_some()).then_some((device, negotiator))
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
        if !outcome.connected.is_empty() {
            shared
                .gamepad_connected
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend(outcome.connected);
        }
        if !outcome.disconnected.is_empty() {
            shared
                .gamepad_disconnected
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .extend(outcome.disconnected);
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
/// Nome do binário do processo filho, com a extensão certa por plataforma
/// (`.exe` no Windows — sem isto, `core_host_path` nunca achava nada lá).
fn core_host_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "reemu-core-host.exe"
    } else {
        "reemu-core-host"
    }
}

fn core_host_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let name = core_host_bin_name();

    // Dev / instalação Windows: o `resource_dir` do Tauri é a MESMA pasta
    // do executável lá (ver docs.rs/tauri `PathResolver::resource_dir`) —
    // sobe até 2 níveis a partir dele.
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..2 {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }

    // Linux empacotado (.deb/AppImage): o `resource_dir` do Tauri fica em
    // `/usr/lib/<productName>` (ou `${APPDIR}/usr/lib/<productName>` dentro
    // do AppImage) — NÃO ao lado do executável (`/usr/bin/reemu-desktop`) e
    // NÃO no nome do binário: é o `productName` do tauri.conf.json ("ReEmu"),
    // confirmado empacotando um .deb/.AppImage real e inspecionando o
    // conteúdo (`dpkg-deb -c` / `--appimage-extract`). Replica a mesma
    // convenção sem precisar de `AppHandle` aqui (este crate não depende de
    // Tauri, mantém a regra de dependência hexagonal).
    if cfg!(target_os = "linux") {
        const PRODUCT_NAME: &str = "ReEmu";
        if let Ok(appdir) = std::env::var("APPDIR") {
            let candidate = PathBuf::from(appdir)
                .join("usr/lib")
                .join(PRODUCT_NAME)
                .join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        let candidate = PathBuf::from("/usr/lib").join(PRODUCT_NAME).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Uma mensagem `ToParent` + os fds (`SCM_RIGHTS`) que vieram junto dela —
/// memfd do anel no `Loaded`; dma_buf e/ou fence `sync_file` num
/// `FrameReady` de interop. Repassado inteiro pra quem consome, pra nunca perder um fd só
/// porque o consumidor não olhou pra ele na hora certa.
struct InboundEvent {
    msg: ToParent,
    fds: Vec<core_ipc::InlineHandle>,
}

struct ChildProc {
    child: std::process::Child,
    channel: Channel,
    reader: Option<JoinHandle<()>>,
}

impl ChildProc {
    /// `env`: variável extra pro filho — hoje só o `GLIBC_TUNABLES` de
    /// pilha executável, quando o core pede (ver
    /// `core_loader_desktop::exec_stack_env`).
    fn spawn(
        env: Option<(&'static str, String)>,
    ) -> Result<(Self, Receiver<InboundEvent>), String> {
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
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--fd").arg(fd_num.to_string());
        if let Some((k, v)) = env {
            cmd.env(k, v);
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("spawn reemu-core-host: {e}"))?;
        // O processo filho já herdou o fd no fork; nossa cópia do lado dele
        // não serve mais pra nada.
        drop(child_ch);

        let (etx, erx) = mpsc::channel::<InboundEvent>();
        let reader_channel = parent_ch.clone();
        let reader = std::thread::Builder::new()
            .name("emu-core-host-reader".into())
            .spawn(move || {
                let mut errs = 0u32;
                loop {
                    match reader_channel.recv::<ToParent>() {
                        Ok(Some((msg, fds))) => {
                            errs = 0;
                            if etx.send(InboundEvent { msg, fds }).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break, // filho fechou o canal
                        Err(e) => {
                            // Uma mensagem ruim (ex.: save state grande demais
                            // pro canal) não deve matar a sessão — registra e
                            // segue. Só desiste se vier erro atrás de erro.
                            log::warn!("canal IPC com o core-host: {e}");
                            errs += 1;
                            if errs >= 16 {
                                log::error!(
                                    "core-host: 16 erros seguidos no canal — encerrando leitura"
                                );
                                break;
                            }
                        }
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
            let spare = match kind {
                FrameKind::Software { .. } => shared.take_spare(),
                FrameKind::Hardware { .. } => None,
            };
            if let Some(frame) = reconstruct_frame(
                ring.as_ref(),
                slot,
                meta,
                kind,
                ev.fds,
                spare,
                &shared.orphan_planes,
            ) {
                shared.frame_seq.fetch_add(1, Ordering::Relaxed);
                let replaced = shared
                    .latest_frame
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .replace(frame);
                shared.frame_ready.notify_all();
                // Quadro que chegou antes do anterior ser apresentado: o
                // buffer do anterior volta pro pool (fora do lock).
                if let Some(old) = replaced {
                    shared.recycle(old);
                }
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
    fds: Vec<core_ipc::InlineHandle>,
    spare: Option<Vec<u8>>,
    orphans: &Mutex<HashMap<u32, OrphanPlane>>,
) -> Option<Frame> {
    match kind {
        FrameKind::Software { pitch, format } => {
            let ring = ring?;
            let len = pitch as usize * meta.native_height as usize;
            let mut data = spare.unwrap_or_default();
            ring.read_slot_into(slot as usize, len, &mut data);
            Some(Frame {
                origin: FrameOrigin::SoftwareRawBuffer {
                    data,
                    pitch,
                    format,
                },
                metadata: meta,
            })
        }
        FrameKind::Hardware {
            flip_y,
            plane,
            sync,
        } => {
            // fds fora de banda: plano (se `plane`), depois a fence (se `sync`).
            let mut fds = fds.into_iter();
            let plane = match plane {
                Some(p) => {
                    // plano novo do slot: um órfão antigo (de outro buffer)
                    // não serve mais — sai do mapa e fecha
                    orphans
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .remove(&slot);
                    Some(dmabuf_plane_info(p, fds.next()))
                }
                None => orphans
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&slot)
                    .and_then(|mut o| o.0.take()),
            };
            let sync_fd = if sync {
                fds.next().map(inline_into_raw_fd)
            } else {
                None
            };
            Some(Frame {
                origin: FrameOrigin::HardwareTexture(Box::new(IpcGpuTextureHandle {
                    slot,
                    flip_y,
                    plane: Mutex::new(plane),
                    sync_fd: Mutex::new(sync_fd),
                })),
                metadata: meta,
            })
        }
    }
}

/// fd recebido por `SCM_RIGHTS` → fd cru, com a posse passando pra quem
/// chamar `take_plane`/`take_sync_fd`.
#[cfg(unix)]
fn inline_into_raw_fd(f: core_ipc::InlineHandle) -> i32 {
    use rustix::fd::IntoRawFd;
    f.into_raw_fd()
}

#[cfg(windows)]
fn inline_into_raw_fd(_f: core_ipc::InlineHandle) -> i32 {
    -1
}

#[cfg(unix)]
fn dmabuf_plane_info(meta: HwPlaneMeta, fd: Option<core_ipc::InlineHandle>) -> DmabufPlaneInfo {
    // SAFETY/posse: o fd recebido por `SCM_RIGHTS` é nosso a partir daqui;
    // `DmabufPlaneInfo` documenta que a posse passa pra quem chama
    // `take_plane` (fecha ao dropar) — é exatamente o `gpu.rs::import_dmabuf`
    // de sempre, que já sabe fazer isso.
    let fd = fd.map(inline_into_raw_fd).unwrap_or(-1);
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

// dma_buf não existe no Windows (ver `core-host-desktop::send_frame` —
// `FrameOrigin::HardwareTexture` ali só loga e descarta, nunca manda
// `FrameReady{kind: Hardware, ..}`) — este caminho nunca é exercitado na
// prática ali, mas o tipo da mensagem IPC é compartilhado, então precisa
// compilar.
#[cfg(windows)]
fn dmabuf_plane_info(meta: HwPlaneMeta, _fd: Option<core_ipc::InlineHandle>) -> DmabufPlaneInfo {
    DmabufPlaneInfo {
        fd: -1,
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
/// Quadro de interop descartado com o plano ainda dentro: guarda pro próximo
/// quadro do slot (ver `Shared::orphan_planes`).
fn stash_orphan_plane(orphans: &Mutex<HashMap<u32, OrphanPlane>>, handle: &dyn GpuTextureHandle) {
    if let Some(plane) = handle.take_plane() {
        orphans
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(handle.slot(), OrphanPlane(Some(plane)));
    }
}

/// Plano guardado em `Shared::orphan_planes` — fecha o fd se for
/// descartado sem ser entregue.
struct OrphanPlane(Option<DmabufPlaneInfo>);

impl Drop for OrphanPlane {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(p) = self.0.take() {
            if p.fd >= 0 {
                use rustix::fd::FromRawFd;
                // SAFETY: fd nosso (veio de `take_plane`), nunca entregue.
                drop(unsafe { rustix::fd::OwnedFd::from_raw_fd(p.fd) });
            }
        }
    }
}

struct IpcGpuTextureHandle {
    slot: u32,
    flip_y: bool,
    plane: Mutex<Option<DmabufPlaneInfo>>,
    sync_fd: Mutex<Option<i32>>,
}

impl Drop for IpcGpuTextureHandle {
    /// Quadro descartado sem ninguém pegar a fence / o plano: fecha os fds
    /// (o plano normalmente já foi resgatado por `Shared::recycle`).
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use rustix::fd::FromRawFd;
            let sync = self.sync_fd.get_mut().ok().and_then(|f| f.take());
            let plane = self
                .plane
                .get_mut()
                .ok()
                .and_then(|p| p.take())
                .map(|p| p.fd);
            for fd in sync.into_iter().chain(plane).filter(|&fd| fd >= 0) {
                // SAFETY: fds nossos, nunca entregues (`take_*` não chamado).
                drop(unsafe { rustix::fd::OwnedFd::from_raw_fd(fd) });
            }
        }
    }
}

impl GpuTextureHandle for IpcGpuTextureHandle {
    fn slot(&self) -> u32 {
        self.slot
    }

    fn take_plane(&self) -> Option<DmabufPlaneInfo> {
        self.plane.lock().unwrap_or_else(|p| p.into_inner()).take()
    }

    fn take_sync_fd(&self) -> Option<i32> {
        self.sync_fd
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
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

/// Descarrega o core Vulkan in-process (se houver). Marca ocioso e solta o
/// frame ANTES de dropar o core — o video pump para de amostrar a `VkImage`
/// que o `Drop` do `VkFrameBridge` vai esperar (`device_wait_idle`) e destruir.
/// Salva a `.srm` e drena o áudio pendente.
fn teardown_vk_local(
    shared: &Shared,
    sink: &mut Option<Box<dyn AudioSink>>,
    current_srm: Option<&std::path::Path>,
) {
    if !shared.vk_local_active.swap(false, Ordering::AcqRel) {
        return;
    }
    shared.vk_local_paused.store(false, Ordering::Release);
    shared.set_state(SessionState::Idle);
    *shared
        .latest_frame
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;
    let lc = shared
        .vk_local
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    if let (Some(lc), Some(path)) = (lc.as_ref(), current_srm) {
        if let Some(bytes) = lc.save_ram() {
            write_srm(path, &bytes);
        }
    }
    drop(lc);
    let leftover = std::mem::take(
        &mut *shared
            .vk_local_audio
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    );
    for (samples, rate) in leftover {
        if let Some(s) = sink.as_mut() {
            s.push_samples(&samples, rate);
        }
    }
}

fn core_loop(mut cfg: SessionConfig, rx: Receiver<Command>, shared: Arc<Shared>) {
    // A stream do cpal é `!Send` — construída aqui, nesta thread.
    let mut sink: Option<Box<dyn AudioSink>> = cfg.audio_sink.take().and_then(|make| make());
    let cores_dir = cfg.cores_dir.clone();
    let system_dir = cfg.system_dir.clone();
    let save_dir = cfg.save_dir.clone();

    let mut proc: Option<ChildProc> = None;
    let mut events: Option<Receiver<InboundEvent>> = None;
    let mut ring: Option<core_ipc::FrameRing> = None;
    // Caminho in-process (etapa 12 B3b/D4): o `LocalCore` vive em
    // `shared.vk_local` e é DIRIGIDO pela thread do compositor
    // (`step_vk_local`). Este loop só carrega/descarrega e responde comandos.
    // Cores que já provamos localmente e NÃO negociaram Vulkan — não tenta de
    // novo (um 2º `retro_init` no processo pai derruba cores não re-entrantes
    // como o parallel_n64, e esses vão pro processo filho de qualquer jeito).
    let mut known_non_vulkan: std::collections::HashSet<String> = HashSet::new();
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
        let vk_local_active = shared.vk_local_active.load(Ordering::Acquire);
        let idle_blocks = proc.is_none() && !vk_local_active;
        let cmd = if idle_blocks {
            rx.recv().ok()
        } else {
            rx.try_recv().ok()
        };

        let Some(cmd) = cmd else {
            if vk_local_active {
                // O core Vulkan é dirigido pelo video pump (`step_vk_local`);
                // aqui só drenamos o áudio que ele produziu pro sink (que é
                // `!Send` e vive nesta thread) e fazemos o flush da `.srm`.
                let batches = std::mem::take(
                    &mut *shared
                        .vk_local_audio
                        .lock()
                        .unwrap_or_else(|p| p.into_inner()),
                );
                for (samples, rate) in batches {
                    match sink.as_mut() {
                        Some(s) => s.push_samples(&samples, rate),
                        None => shared
                            .audio
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .extend_from_slice(&samples),
                    }
                }
                if let Some(path) = current_srm.as_ref() {
                    if last_srm_flush.elapsed() >= SRM_FLUSH_INTERVAL {
                        let bytes = shared
                            .vk_local
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .as_ref()
                            .and_then(|lc| lc.save_ram());
                        if let Some(bytes) = bytes {
                            let _ = srm_tx.send((path.clone(), bytes));
                        }
                        last_srm_flush = Instant::now();
                    }
                }
                std::thread::sleep(Duration::from_millis(8));
                continue;
            }
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
                teardown_vk_local(&shared, &mut sink, current_srm.as_deref());
                shared.set_state(SessionState::Idle);
                *shared
                    .latest_frame
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = None;
                if let Some(p) = proc.take() {
                    p.kill();
                }
                events = None;
                ring = None;
                current_srm = None;
                *shared.child_pid.lock().unwrap_or_else(|p| p.into_inner()) = None;

                let target_srm = srm_path(&save_dir, &rom);
                let initial_save_ram = std::fs::read(&target_srm).ok();

                // Etapa 12 B3b: com `REEMU_HW=vulkan` + device do compositor
                // publicado, tenta rodar o core AQUI (in-process). Se ele não
                // negociar Vulkan, `LocalCore::load` devolve
                // `HwRenderUnsupported` e caímos pro processo filho (que isola
                // cores não re-entrantes).
                let route = route_local_device(&shared, &id.0);
                log::info!(
                    "etapa 12: core {} → rota {} (device={}, negotiator={}, known_non_vk={})",
                    id.0,
                    if route.is_some() && !known_non_vulkan.contains(&id.0) {
                        "LOCAL (in-process)"
                    } else {
                        "processo filho"
                    },
                    route.as_ref().map(|(d, _)| d.is_some()).unwrap_or(false),
                    route.as_ref().map(|(_, n)| n.is_some()).unwrap_or(false),
                    known_non_vulkan.contains(&id.0),
                );
                match route {
                    Some((device, negotiator)) if !known_non_vulkan.contains(&id.0) => {
                        // Beetle PSX HW: com dither ligado, o scanout Vulkan sai
                        // num formato packed 16-bit (A1R5G5B5) que o wgpu não
                        // amostra. "disabled" força RGBA8. Só default — o
                        // override do usuário (cascata de core options) já vem
                        // por cima em `initial_option_values`.
                        let mut vk_opts = initial_option_values.clone();
                        if id.0.contains("psx_hw") {
                            vk_opts
                                .entry("beetle_psx_hw_dither_mode".to_string())
                                .or_insert_with(|| "disabled".to_string());
                        }
                        log::info!("etapa 12: chamando LocalCore::load pra {}", id.0);
                        match LocalCore::load(
                            &id.0,
                            &rom,
                            cores_dir.clone(),
                            system_dir.clone(),
                            save_dir.clone(),
                            vk_opts,
                            initial_save_ram.clone(),
                            device,
                            negotiator,
                        ) {
                            Ok((lc, av)) => {
                                log::info!(
                                    "etapa 12: core Vulkan in-process ATIVO ({}) — {}x{}",
                                    id.0,
                                    av.geometry.base_width,
                                    av.geometry.base_height
                                );
                                *shared.loaded_core.lock().unwrap_or_else(|p| p.into_inner()) =
                                    Some(id.0.clone());
                                *shared.vk_local.lock().unwrap_or_else(|p| p.into_inner()) =
                                    Some(lc);
                                shared.vk_local_paused.store(false, Ordering::Release);
                                shared.vk_local_active.store(true, Ordering::Release);
                                shared.set_state(SessionState::Running);
                                current_srm = Some(target_srm);
                                last_srm_flush = Instant::now();
                                if let Some(s) = sink.as_mut() {
                                    s.resume();
                                }
                                let _ = reply.send(Ok(av));
                                continue;
                            }
                            Err(CoreLoadError::HwRenderUnsupported(reason)) => {
                                known_non_vulkan.insert(id.0.clone());
                                log::info!("core {}: {reason} — usando o processo filho", id.0);
                            }
                            Err(e) => {
                                log::error!("etapa 12: LocalCore::load({}) FALHOU: {e}", id.0);
                                let _ = reply.send(Err(e));
                                continue;
                            }
                        }
                    }
                    _ => {}
                }

                // Core que pede pilha executável (melonDS no buildbot): a
                // glibc ≥ 2.41 recusa o dlopen, a não ser que o processo
                // parta com `glibc.rtld.execstack=2` — só o filho deste core.
                let child_env = core_loader_desktop::core_file(&cores_dir, &id.0)
                    .and_then(|p| core_loader_desktop::exec_stack_env(&p));
                if child_env.is_some() {
                    log::info!(
                        "core {}: pede pilha executável — filho com glibc.rtld.execstack=2",
                        id.0
                    );
                }
                match ChildProc::spawn(child_env) {
                    Ok((mut p, erx)) => {
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
                                // Unix: o memfd do anel veio via `SCM_RIGHTS`
                                // junto do `Loaded` (`fds`). Windows: o anel é
                                // nomeado (derivado do nome do pipe, ver
                                // `core_ipc::shm_ring_win`) — o filho já criou
                                // com esse nome, o pai só abre; `fds` vem
                                // sempre vazio ali (sem `SCM_RIGHTS`).
                                #[cfg(unix)]
                                let ring_ok = fds.into_iter().next().and_then(|fd| {
                                    core_ipc::FrameRing::from_fd(fd, slot_size).ok()
                                });
                                #[cfg(windows)]
                                let ring_ok = {
                                    let _ = fds;
                                    core_ipc::FrameRing::open(&p.channel, slot_size).ok()
                                };
                                if ring_ok.is_none() {
                                    log::warn!(
                                        "core {}: sem anel de frame (fd não veio) — sem vídeo",
                                        id.0
                                    );
                                }
                                ring = ring_ok;
                                shared
                                    .orphan_planes
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner())
                                    .clear();
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
                                // `None` = timeout OU canal fechado. Se o
                                // processo já morreu (ex.: o core caiu no
                                // `retro_init`), diz isso — "timeout" levava
                                // o usuário a procurar problema de rede.
                                // (o canal fecha um instante antes do
                                // processo virar "esperável" — tenta 5×20 ms)
                                let mut exited = None;
                                for _ in 0..5 {
                                    exited = p.child.try_wait().ok().flatten();
                                    if exited.is_some() {
                                        break;
                                    }
                                    std::thread::sleep(Duration::from_millis(20));
                                }
                                p.kill();
                                let msg = match exited {
                                    Some(status) => format!(
                                        "core-host: o core encerrou inesperadamente ao carregar o jogo ({status})"
                                    ),
                                    None => "core-host não respondeu (timeout)".to_string(),
                                };
                                let _ = reply.send(Err(CoreLoadError::LoadFailed(msg)));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = reply.send(Err(CoreLoadError::LoadFailed(e)));
                    }
                }
            }
            Command::Unload(reply) => {
                teardown_vk_local(&shared, &mut sink, current_srm.as_deref());
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
                let vk_active = shared.vk_local_active.load(Ordering::Acquire);
                let have_core = proc.is_some() || vk_active;
                // Ida e volta: espera o filho confirmar. Sem isso o
                // `set_paused(true)` retornava com um `FrameReady` do frame em
                // andamento ainda a caminho, e ele chegava DEPOIS (o frame
                // avançava com a sessão "pausada").
                if let (Some(p), Some(erx)) = (proc.as_ref(), events.as_ref()) {
                    let _ = p.channel.send(&ToChild::SetPaused(want_paused), &[]);
                    let acked = wait_for_reply(
                        erx,
                        Duration::from_secs(2),
                        &shared,
                        &mut sink,
                        &mut ring,
                        |ev| match ev.msg {
                            ToParent::PausedAck => Ok(()),
                            _ => Err(ev),
                        },
                    );
                    if acked.is_none() {
                        log::warn!("core-host não confirmou o SetPaused({want_paused}) em 2s");
                    }
                }
                if vk_active {
                    shared.vk_local_paused.store(want_paused, Ordering::Release);
                    if let Some(lc) = shared
                        .vk_local
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                    {
                        lc.set_paused(want_paused);
                    }
                }
                if have_core {
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
                let bytes = if shared.vk_local_active.load(Ordering::Acquire) {
                    // Portão da VkQueue: o `retro_serialize` pode submeter, e o
                    // video pump submete o wgpu na mesma queue de outra thread.
                    let _gate = shared
                        .vk_render_gate
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    shared
                        .vk_local
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                        .and_then(|lc| lc.serialize_state())
                } else {
                    match (proc.as_ref(), events.as_ref()) {
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
                    }
                };
                let _ = reply.send(bytes);
            }
            Command::RestoreState(data, reply) => {
                let ok = if shared.vk_local_active.load(Ordering::Acquire) {
                    let _gate = shared
                        .vk_render_gate
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    shared
                        .vk_local
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_mut()
                        .is_some_and(|lc| lc.restore_state(&data))
                } else {
                    match (proc.as_ref(), events.as_ref()) {
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
                    }
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
                let result = if shared.vk_local_active.load(Ordering::Acquire) {
                    shared
                        .vk_local
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_ref()
                        .map(|lc| lc.core_options())
                } else {
                    match (proc.as_ref(), events.as_ref()) {
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
                    }
                };
                let _ = reply.send(result.unwrap_or_default());
            }
            Command::SetCoreOption(key, value, reply) => {
                let ok = if shared.vk_local_active.load(Ordering::Acquire) {
                    shared
                        .vk_local
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_ref()
                        .is_some_and(|lc| lc.set_core_option(&key, &value))
                } else {
                    match (proc.as_ref(), events.as_ref()) {
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
                    }
                };
                let _ = reply.send(ok);
            }
            Command::Shutdown => {
                teardown_vk_local(&shared, &mut sink, current_srm.as_deref());
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

#[cfg(all(test, unix))]
mod orphan_plane_tests {
    use super::*;

    fn meta() -> HwPlaneMeta {
        HwPlaneMeta {
            width: 64,
            height: 64,
            stride: 256,
            offset: 0,
            modifier: 0,
            fourcc: 0,
        }
    }

    fn hw(
        plane: Option<HwPlaneMeta>,
        fds: Vec<core_ipc::InlineHandle>,
        orphans: &Mutex<HashMap<u32, OrphanPlane>>,
    ) -> Frame {
        let frame_meta = domain::frame_source::FrameMetadata {
            native_width: 64,
            native_height: 64,
            aspect_ratio: 1.0,
            rotation_degrees: 0,
        };
        let kind = FrameKind::Hardware {
            flip_y: false,
            plane,
            sync: false,
        };
        reconstruct_frame(None, 1, frame_meta, kind, fds, None, orphans).unwrap()
    }

    /// Regressão da "metade dos quadros some" com interop: o plano só vem no
    /// 1º quadro do slot; se esse quadro é descartado antes de importar, o
    /// próximo quadro do MESMO slot tem que trazê-lo.
    #[test]
    fn plane_of_a_dropped_frame_goes_to_the_next_frame_of_the_slot() {
        let orphans = Mutex::new(HashMap::new());
        let fd: core_ipc::InlineHandle = std::fs::File::open("/dev/null").unwrap().into();

        let first = hw(Some(meta()), vec![fd], &orphans);
        let FrameOrigin::HardwareTexture(h) = first.origin else {
            unreachable!()
        };
        stash_orphan_plane(&orphans, h.as_ref()); // descartado sem importar

        let second = hw(None, vec![], &orphans);
        let FrameOrigin::HardwareTexture(h2) = second.origin else {
            unreachable!()
        };
        let plane = h2.take_plane().expect("plano resgatado no quadro seguinte");
        assert!(plane.fd >= 0);
        assert_eq!(plane.width, 64);
        // e não fica duplicado pra um terceiro quadro
        assert!(orphans.lock().unwrap().is_empty());
        // SAFETY: fd nosso (acabou de sair de `take_plane`).
        drop(unsafe { <rustix::fd::OwnedFd as rustix::fd::FromRawFd>::from_raw_fd(plane.fd) });
    }
}
