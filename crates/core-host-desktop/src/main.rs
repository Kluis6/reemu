//! `reemu-core-host`: processo filho descartável que carrega e roda UM core
//! libretro. O pai (`emu-session`) mata este processo e sobe um novo a cada
//! `Load` — cores não re-entrantes (parallel_n64...) nunca veem um 2º
//! `retro_init` no mesmo processo, que é a causa raiz do crash que esta
//! arquitetura resolve. Ver `docs/ai-context/02-core-loader-desktop.md` e a
//! memória `n64-reload-crash`.
//!
//! Reusa `core-loader-desktop` inteiro (dlopen/FFI/GL/dmabuf) sem nenhuma
//! mudança de comportamento — só o "escrever em `Shared`" de
//! `emu_session::core_loop` vira "mandar mensagem `ToParent`".

#[cfg(unix)]
use core_ipc::HwPlaneMeta;
use core_ipc::{Channel, FrameKind, PortInput, ToChild, ToParent};
use core_loader_desktop::{DesktopCore, DesktopCoreLoader, PaceStats, Pacer};
use domain::core_loader::{CoreId, LoadedCore};
use domain::frame_source::{FrameOrigin, FrameSource};
#[cfg(unix)]
use rustix::fd::{AsFd, FromRawFd, OwnedFd};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// Este processo só roda o core — sobe a prioridade dele pra, sob carga, o
/// escalonador não atrasar o `retro_run` (o pacing dorme o resto do quadro,
/// então não rouba CPU dos outros). Melhor esforço:
/// - Windows: classe `ABOVE_NORMAL` — não exige administrador.
/// - Unix: `nice -5` só com privilégio (`CAP_SYS_NICE` / `RLIMIT_NICE`); sem
///   ele o sistema recusa e o processo segue na prioridade normal.
fn raise_priority() {
    #[cfg(unix)]
    match rustix::process::setpriority_process(None, -5) {
        Ok(()) => log::info!("core-host: prioridade elevada (nice -5)"),
        Err(e) => log::debug!("core-host: prioridade normal (sem permissão pra elevar: {e})"),
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
        };
        // SAFETY: pseudo-handle do próprio processo, sempre válido.
        let ok = unsafe { SetPriorityClass(GetCurrentProcess(), ABOVE_NORMAL_PRIORITY_CLASS) } != 0;
        if ok {
            log::info!("core-host: prioridade ABOVE_NORMAL");
        } else {
            log::warn!("core-host: SetPriorityClass falhou — prioridade normal");
        }
    }
}

/// `reemu-core-host --probe <pasta de cores> <core_id>`: abre o core sem
/// jogo (`DesktopCoreLoader::probe_core`) e sai. Usado pelo teste de fumaça
/// do catálogo — num processo à parte, um core que cai no `retro_init` só
/// derruba este processo. Resultado numa linha com prefixo fixo no stdout
/// (o core pode escrever o que quiser antes): `REEMU_PROBE_OK\t<nome>\t
/// <versão>\t<extensões>` com saída 0, ou `REEMU_PROBE_ERR\t<erro>` com
/// saída 2.
fn probe(args: &[String]) -> ! {
    let (Some(cores_dir), Some(core_id)) = (args.first(), args.get(1)) else {
        eprintln!("uso: reemu-core-host --probe <pasta de cores> <core_id>");
        std::process::exit(64);
    };
    let sandbox = Sandbox::new("probe");
    let loader = DesktopCoreLoader::new(cores_dir, &sandbox.0, &sandbox.0);
    let code = match loader.probe_core(&CoreId(core_id.clone())) {
        Ok(p) => {
            println!(
                "REEMU_PROBE_OK\t{}\t{}\t{}",
                p.library_name, p.library_version, p.valid_extensions
            );
            0
        }
        Err(e) => {
            println!("REEMU_PROBE_ERR\t{e}");
            2
        }
    };
    drop(sandbox);
    std::process::exit(code);
}

/// Pasta de sistema/saves só deste processo, vazia, apagada no fim. Não
/// usar o `temp_dir()` inteiro: o pcsx_rearmed, sem achar o BIOS pelos nomes
/// conhecidos, abre TODO arquivo da pasta de sistema (`find_any_bios`, em
/// frontend/libretro.c) — no runner do GitHub o `/tmp` tem FIFOs, e abrir
/// um FIFO bloqueia até alguém abrir a outra ponta (o probe travava em
/// `wait_for_partner` no `openat`).
struct Sandbox(std::path::PathBuf);

impl Sandbox {
    fn new(kind: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("reemu-{kind}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `reemu-core-host --run <pasta de cores> <core_id> <rom> <quadros>`: carrega
/// o jogo, roda `quadros` `retro_run` e sai — fase 2 do teste de fumaça do
/// catálogo. Resultado no stdout: `REEMU_RUN_OK\t<quadros de vídeo>\t
/// <quadros com cor>\t<amostras de áudio>` (saída 0) ou `REEMU_RUN_ERR\t
/// <erro>` (saída 2). "Com cor" = algum pixel não preto (as ROMs de teste
/// pintam o fundo).
fn run_rom(args: &[String]) -> ! {
    let (Some(cores_dir), Some(core_id), Some(rom), Some(frames)) = (
        args.first(),
        args.get(1),
        args.get(2),
        args.get(3).and_then(|n| n.parse::<u32>().ok()),
    ) else {
        eprintln!("uso: reemu-core-host --run <pasta de cores> <core_id> <rom> <quadros>");
        std::process::exit(64);
    };
    let sandbox = Sandbox::new("run");
    let loader = DesktopCoreLoader::new(cores_dir, &sandbox.0, &sandbox.0);
    let code = match loader.open_core(&CoreId(core_id.clone()), rom) {
        Err(e) => {
            println!("REEMU_RUN_ERR\t{e}");
            2
        }
        Ok(mut core) => {
            let (mut video, mut colored, mut audio) = (0u32, 0u32, 0usize);
            for _ in 0..frames {
                if let Some(frame) = core.next_frame() {
                    video += 1;
                    if let FrameOrigin::SoftwareRawBuffer {
                        data,
                        pitch,
                        format,
                    } = &frame.origin
                    {
                        let rgba = domain::frame_source::to_rgba8(
                            data,
                            frame.metadata.native_width,
                            frame.metadata.native_height,
                            *pitch,
                            *format,
                        );
                        if rgba
                            .chunks_exact(4)
                            .any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16)
                        {
                            colored += 1;
                        }
                    }
                }
                audio += core.drain_audio().len();
            }
            println!("REEMU_RUN_OK\t{video}\t{colored}\t{audio}");
            // o processo sai já: sem `retro_deinit` (alguns cores caem nele,
            // como o VBA-M no probe)
            std::mem::forget(core);
            0
        }
    };
    drop(sandbox);
    std::process::exit(code);
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--probe") {
        probe(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("--run") {
        run_rom(&args[1..]);
    }
    raise_priority();

    let fd_arg = std::env::args()
        .skip_while(|a| a != "--fd")
        .nth(1)
        .expect("reemu-core-host: uso: --fd <numero>");
    // Tipo inferido do uso logo abaixo: `RawFd` (i32) no Unix, `ChannelArg`
    // (handles + nome do canal) no Windows.
    let fd_num = fd_arg.parse().expect("--fd inválido");

    // SAFETY: o pai deixou este fd/handle herdável especificamente pra este
    // processo herdar (ver `core-ipc::Channel::clear_cloexec`); ninguém mais
    // neste processo novo pode ter reivindicado o mesmo número ainda.
    let channel = unsafe { Channel::from_inherited_fd(fd_num) };

    let (tx, rx) = mpsc::channel::<ToChild>();
    {
        let channel = channel.clone();
        std::thread::Builder::new()
            .name("core-host-reader".into())
            .spawn(move || {
                loop {
                    match channel.recv::<ToChild>() {
                        Ok(Some((msg, _fds))) => {
                            if tx.send(msg).is_err() {
                                break;
                            }
                        }
                        Ok(None) => break, // pai fechou o canal
                        Err(e) => {
                            log::warn!("canal IPC: {e}");
                            break;
                        }
                    }
                }
            })
            .expect("spawn core-host-reader");
    }

    run(channel, rx);
    log::info!("reemu-core-host: encerrando");
}

/// Diagnóstico de desempenho do loop do core, 1 linha por segundo no log.
/// Liga com `REEMU_PERF=1` (ou o antigo `REEMU_AUDIO_DEBUG=1`). Mede o que
/// as tarefas de desempenho do TASKS.md precisam pra decidir: regularidade
/// real entre frames, custo do `retro_run`, custo de mandar o frame pro pai
/// (anel + IPC) e quanto o pacing dorme vs. queima CPU em spin.
fn diag_enabled() -> bool {
    std::env::var_os("REEMU_PERF").is_some() || std::env::var_os("REEMU_AUDIO_DEBUG").is_some()
}

struct LoopDiag {
    since: Instant,
    last_frame_start: Option<Instant>,
    /// Intervalo entre o início de frames consecutivos, em ms.
    intervals: Vec<f32>,
    frames: u64,
    over_budget: u64,
    dup_frames: u64,
    busy: Duration,
    worst: Duration,
    send: Duration,
    send_worst: Duration,
    slept: Duration,
    spun: Duration,
    late: u64,
    audio_samples: u64,
}

impl Default for LoopDiag {
    fn default() -> Self {
        Self {
            since: Instant::now(),
            last_frame_start: None,
            intervals: Vec::with_capacity(128),
            frames: 0,
            over_budget: 0,
            dup_frames: 0,
            busy: Duration::ZERO,
            worst: Duration::ZERO,
            send: Duration::ZERO,
            send_worst: Duration::ZERO,
            slept: Duration::ZERO,
            spun: Duration::ZERO,
            late: 0,
            audio_samples: 0,
        }
    }
}

impl LoopDiag {
    fn frame_start(&mut self, now: Instant) {
        if let Some(prev) = self.last_frame_start {
            self.intervals
                .push(now.duration_since(prev).as_secs_f32() * 1000.0);
        }
        self.last_frame_start = Some(now);
    }

    fn record_frame(&mut self, took: Duration, budget: Duration, duped: bool) {
        self.frames += 1;
        self.busy += took;
        self.worst = self.worst.max(took);
        if took > budget {
            self.over_budget += 1;
        }
        if duped {
            self.dup_frames += 1;
        }
    }

    fn record_send(&mut self, took: Duration) {
        self.send += took;
        self.send_worst = self.send_worst.max(took);
    }

    fn record_pace(&mut self, p: &PaceStats) {
        self.slept += p.slept;
        self.spun += p.spun;
        if p.late {
            self.late += 1;
        }
    }

    fn maybe_report(&mut self, sample_rate: u32, budget: Duration) {
        let elapsed = self.since.elapsed();
        if elapsed.as_secs_f32() < 1.0 {
            return;
        }
        let secs = elapsed.as_secs_f32();
        let n = self.frames.max(1) as f32;
        let ms = |d: Duration| d.as_secs_f32() * 1000.0;
        let mut iv = std::mem::take(&mut self.intervals);
        iv.sort_by(f32::total_cmp);
        let pct = |p: f32| {
            iv.get(((iv.len() as f32 - 1.0) * p).round() as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let expected = (sample_rate as f32 * secs * 2.0) as u64;
        log::info!(
            "perf core 1s: {:.1} fps (alvo {:.1}) | intervalo méd {:.2} p99 {:.2} máx {:.2} ms | \
             retro_run méd {:.2} pior {:.2} ms, {} acima do budget, {} sem frame novo | \
             envio méd {:.2} pior {:.2} ms | pacing: dormiu {:.0} ms, spin {:.1} ms \
             ({:.1}% de 1 CPU), {} atrasados | áudio {} amostras (esperado ~{})",
            self.frames as f32 / secs,
            1.0 / budget.as_secs_f32(),
            iv.iter().sum::<f32>() / iv.len().max(1) as f32,
            pct(0.99),
            iv.last().copied().unwrap_or(0.0),
            ms(self.busy) / n,
            ms(self.worst),
            self.over_budget,
            self.dup_frames,
            ms(self.send) / n,
            ms(self.send_worst),
            ms(self.slept),
            ms(self.spun),
            self.spun.as_secs_f32() / secs * 100.0,
            self.late,
            self.audio_samples,
            expected,
        );
        let last = self.last_frame_start;
        *self = Self::default();
        self.last_frame_start = last;
    }
}

fn run(channel: Channel, rx: Receiver<ToChild>) {
    let mut core: Option<DesktopCore> = None;
    let mut ring: Option<core_ipc::FrameRing> = None;
    let mut frame_slot = 0u32;
    let mut paused = false;
    let mut core_sample_rate = 32_000u32;
    let mut pacer = Pacer::new(Duration::from_micros(16_667));
    let mut diag = diag_enabled().then(LoopDiag::default);

    loop {
        let msg = if core.is_none() || paused {
            rx.recv().ok()
        } else {
            rx.try_recv().ok()
        };

        let Some(msg) = msg else {
            // Canal fechado (pai sumiu/morreu) — nada mais a fazer, ninguém
            // vai ler o que a gente mandar. Sai em vez de girar pra sempre
            // como órfão.
            if matches!(rx.try_recv(), Err(mpsc::TryRecvError::Disconnected)) {
                break;
            }
            run_one_frame(
                &mut core,
                &channel,
                &mut ring,
                &mut frame_slot,
                &mut core_sample_rate,
                &mut pacer,
                &mut diag,
            );
            continue;
        };

        match msg {
            ToChild::Load {
                core_id,
                rom_path,
                cores_dir,
                system_dir,
                save_dir,
                initial_option_values,
                initial_save_ram,
                dmabuf_modifiers,
            } => {
                core = None; // não deveria haver um core já — o pai mata e sobe de novo a cada troca.
                core_loader_desktop::set_dmabuf_import_modifiers(dmabuf_modifiers);
                core_loader_desktop::silence_core_stdout();
                core_loader_desktop::set_pending_core_option_values(initial_option_values);
                let loader = DesktopCoreLoader::new(cores_dir, system_dir, save_dir);
                match loader.open_core(&CoreId(core_id.clone()), &rom_path) {
                    Ok(mut c) => {
                        let av = c.system_av_info();
                        let fps = av.timing.fps.max(1.0);
                        pacer.set_budget(Duration::from_secs_f64(1.0 / fps));
                        core_sample_rate = (av.timing.sample_rate.round() as u32).max(1);
                        log::info!(
                            "core {core_id}: fps={:.3} sample_rate={:.0} Hz",
                            fps,
                            av.timing.sample_rate
                        );

                        let restored = initial_save_ram.map(|bytes| {
                            let ok = c.restore_save_ram(&bytes);
                            if ok {
                                log::info!("save RAM restaurada ({} bytes)", bytes.len());
                            } else {
                                log::warn!("save RAM ignorada (tamanho não bate)");
                            }
                            ok
                        });

                        let max_w = av.geometry.max_width.max(av.geometry.base_width).max(1);
                        let max_h = av.geometry.max_height.max(av.geometry.base_height).max(1);
                        let slot_size = (max_w * max_h * 4) as usize;
                        #[cfg(unix)]
                        let new_ring = core_ipc::FrameRing::create(slot_size);
                        #[cfg(windows)]
                        let new_ring = core_ipc::FrameRing::create(&channel, slot_size);
                        let new_ring = match new_ring {
                            Ok(r) => r,
                            Err(e) => {
                                let _ = channel.send(
                                    &ToParent::Loaded(Err(format!("anel de frame: {e}"))),
                                    &[],
                                );
                                continue;
                            }
                        };
                        // Unix: o memfd do anel vai junto via `SCM_RIGHTS`.
                        // Windows: o anel já é nomeado (derivado do nome do
                        // pipe, ver `core_ipc::shm_ring_win`) — o pai abre
                        // pelo mesmo nome, nada extra viaja na mensagem.
                        #[cfg(unix)]
                        let _ = channel.send(&ToParent::Loaded(Ok(av)), &[new_ring.fd()]);
                        #[cfg(windows)]
                        let _ = channel.send(&ToParent::Loaded(Ok(av)), &[]);
                        let _ = channel.send(&ToParent::SaveRamRestored(restored), &[]);
                        ring = Some(new_ring);
                        frame_slot = 0;
                        paused = false;
                        diag = diag_enabled().then(LoopDiag::default);
                        core = Some(c);
                    }
                    Err(e) => {
                        let _ = channel.send(&ToParent::Loaded(Err(e.to_string())), &[]);
                    }
                }
            }
            ToChild::SetPaused(p) => {
                paused = p;
                if !p {
                    pacer.reset();
                }
                let _ = channel.send(&ToParent::PausedAck, &[]);
            }
            ToChild::SaveState => {
                let bytes = core.as_mut().and_then(|c| c.serialize_state());
                let _ = channel.send(&ToParent::SaveStateResult(bytes), &[]);
            }
            ToChild::RestoreState(data) => {
                let ok = core
                    .as_mut()
                    .map(|c| c.restore_state(&data))
                    .unwrap_or(false);
                let _ = channel.send(&ToParent::RestoreStateResult(ok), &[]);
            }
            ToChild::GetSaveRam => {
                let bytes = core.as_ref().and_then(|c| c.save_ram());
                let _ = channel.send(&ToParent::SaveRamResult(bytes), &[]);
            }
            ToChild::SetSaveRam(bytes) => {
                if let Some(c) = core.as_mut() {
                    c.restore_save_ram(&bytes);
                }
            }
            ToChild::Input { ports } => {
                apply_input(&ports);
            }
            ToChild::SetCoreOption { key, value } => {
                let ok = core_loader_desktop::set_core_option(&key, &value);
                let _ = channel.send(&ToParent::SetCoreOptionResult(ok), &[]);
            }
            ToChild::GetCoreOptions => {
                let _ = channel.send(
                    &ToParent::CoreOptionsSnapshot {
                        schema: core_loader_desktop::core_options(),
                        values: core_loader_desktop::core_option_values(),
                    },
                    &[],
                );
            }
            ToChild::Shutdown => break,
        }
    }
    // `core` dropa aqui (se ainda `Some`) → `DesktopCore::Drop` faz o
    // teardown libretro/GL. O processo sai logo em seguida de qualquer jeito
    // — não importa mais se esse teardown deixa estado global sujo.
}

/// Nomes dos botões num `joypad_mask` (bit = id libretro), pro
/// `REEMU_INPUT_DEBUG`.
fn mask_names(mask: u16) -> String {
    const NAMES: [&str; 16] = [
        "B", "Y", "Select", "Start", "Cima", "Baixo", "Esq", "Dir", "A", "X", "L1", "R1", "L2",
        "R2", "L3", "R3",
    ];
    let v: Vec<&str> = (0..16)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| NAMES[i])
        .collect();
    if v.is_empty() {
        "(nada)".into()
    } else {
        v.join("+")
    }
}

fn apply_input(ports: &[PortInput; 4]) {
    static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let debug = *DEBUG.get_or_init(|| std::env::var_os("REEMU_INPUT_DEBUG").is_some());
    let pad = core_loader_desktop::retropad();
    let analog = core_loader_desktop::analog();
    for (port, input) in ports.iter().enumerate() {
        // Diagnóstico: o que o core VAI ver nesta porta (botões + analógico
        // esquerdo), só quando muda.
        if debug && pad.mask(port) != input.joypad_mask {
            log::info!(
                "entrada porta {}: {} (analógico esq {:?})",
                port + 1,
                mask_names(input.joypad_mask),
                input.sticks[0]
            );
        }
        pad.set_mask(port, input.joypad_mask);
        analog.set_stick(port, 0, input.sticks[0].0, input.sticks[0].1);
        analog.set_stick(port, 1, input.sticks[1].0, input.sticks[1].1);
        analog.set_triggers(port, input.triggers.0, input.triggers.1);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_one_frame(
    core: &mut Option<DesktopCore>,
    channel: &Channel,
    ring: &mut Option<core_ipc::FrameRing>,
    frame_slot: &mut u32,
    core_sample_rate: &mut u32,
    pacer: &mut Pacer,
    diag: &mut Option<LoopDiag>,
) {
    let Some(c) = core.as_mut() else { return };

    if let Some(t) = c.take_av_update() {
        let fps = t.fps.max(1.0);
        pacer.set_budget(Duration::from_secs_f64(1.0 / fps));
        *core_sample_rate = (t.sample_rate.round() as u32).max(1);
        log::info!(
            "timing atualizado em runtime: fps={:.3} sample_rate={} Hz",
            fps,
            core_sample_rate
        );
    }

    let t0 = diag.as_mut().map(|d| {
        let now = Instant::now();
        d.frame_start(now);
        now
    });
    let produced = c.next_frame();
    if let (Some(d), Some(t0)) = (diag.as_mut(), t0) {
        d.record_frame(t0.elapsed(), pacer.budget(), produced.is_none());
    }
    // 1ª leitura de analógico pelo core → avisa o pai (uma vez por processo;
    // cada jogo roda num filho novo).
    static ANALOG_SENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if core_loader_desktop::analog().is_used()
        && !ANALOG_SENT.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        let _ = channel.send(&ToParent::AnalogUsed, &[]);
    }
    if let (Some(frame), Some(ring)) = (produced, ring.as_ref()) {
        let ts = diag.as_ref().map(|_| Instant::now());
        if let Some(buf) = send_frame(channel, ring, frame_slot, frame) {
            c.recycle_frame_buffer(buf);
        }
        if let (Some(d), Some(ts)) = (diag.as_mut(), ts) {
            d.record_send(ts.elapsed());
        }
    }

    let audio = c.drain_audio();
    if let Some(d) = diag.as_mut() {
        d.audio_samples += audio.len() as u64;
        d.maybe_report(*core_sample_rate, pacer.budget());
    }
    if !audio.is_empty() {
        let _ = channel.send(
            &ToParent::AudioBatch {
                samples: audio,
                sample_rate: *core_sample_rate,
            },
            &[],
        );
    }

    let p = pacer.pace();
    if let Some(d) = diag.as_mut() {
        d.record_pace(&p);
    }
}

fn send_frame(
    channel: &Channel,
    ring: &core_ipc::FrameRing,
    frame_slot: &mut u32,
    frame: domain::frame_source::Frame,
) -> Option<Vec<u8>> {
    match frame.origin {
        FrameOrigin::SoftwareRawBuffer {
            data,
            pitch,
            format,
        } => {
            let slot = *frame_slot;
            ring.write_slot(slot as usize, &data);
            *frame_slot = (slot + 1) % core_ipc::SLOTS as u32;
            let _ = channel.send(
                &ToParent::FrameReady {
                    slot,
                    meta: frame.metadata,
                    kind: FrameKind::Software { pitch, format },
                },
                &[],
            );
            // Já está no anel: o buffer volta pro core reusar no próximo quadro.
            Some(data)
        }
        FrameOrigin::HardwareVulkanImage(_) => {
            // HW render Vulkan (etapa 12) roda IN-PROCESS no pai (device do
            // compositor). Se chegou aqui é bug de roteamento — a VkImage não
            // cruza processo.
            log::error!("frame Vulkan no core-host (bug): HW render Vulkan é in-process");
            None
        }
        // dma_buf (GBM/DRM) é um conceito de interop gráfico específico do
        // Linux (padrão; `REEMU_GL_INTEROP=0` desliga) — não existe
        // equivalente no Windows (lá seria D3D11/D3D12 shared handle, um
        // subsistema totalmente diferente, fora do escopo desta porta de
        // IPC). No Windows este caminho não é alcançado na prática (nada em
        // `core-loader-desktop` produz `HardwareTexture` lá), mas precisa
        // compilar — loga e descarta o frame em vez de travar.
        #[cfg(windows)]
        FrameOrigin::HardwareTexture(_handle) => {
            log::error!("frame HardwareTexture (dma_buf) no core-host: sem suporte no Windows");
            None
        }
        #[cfg(unix)]
        FrameOrigin::HardwareTexture(handle) => {
            let flip_y = handle.flip_y();
            let slot = handle.slot();
            // SAFETY (os dois): `take_plane`/`take_sync_fd` transferem a posse
            // do fd (ver `domain::frame_source`) — fecham ao sair do escopo,
            // depois que `send` já os duplicou pro outro lado no `sendmsg`.
            let plane = handle.take_plane();
            let plane_fd = plane
                .as_ref()
                .map(|p| unsafe { OwnedFd::from_raw_fd(p.fd) });
            let sync_fd = handle
                .take_sync_fd()
                .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) });
            let meta = plane.map(|plane| HwPlaneMeta {
                width: plane.width,
                height: plane.height,
                stride: plane.stride,
                offset: plane.offset,
                modifier: plane.modifier,
                fourcc: plane.fourcc,
            });
            // Ordem fora de banda: plano (se houver), depois a fence.
            let fds: Vec<_> = plane_fd
                .iter()
                .chain(sync_fd.iter())
                .map(|f| f.as_fd())
                .collect();
            let _ = channel.send(
                &ToParent::FrameReady {
                    slot,
                    meta: frame.metadata,
                    kind: FrameKind::Hardware {
                        flip_y,
                        plane: meta,
                        sync: sync_fd.is_some(),
                    },
                },
                &fds,
            );
            None
        }
    }
}
