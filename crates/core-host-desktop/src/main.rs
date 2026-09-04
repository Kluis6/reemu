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

use core_ipc::{Channel, FrameKind, HwPlaneMeta, PortInput, ToChild, ToParent};
use core_loader_desktop::{DesktopCore, DesktopCoreLoader};
use domain::core_loader::{CoreId, LoadedCore};
use domain::frame_source::{FrameOrigin, FrameSource};
use rustix::fd::{AsFd, FromRawFd, OwnedFd};
use std::os::fd::RawFd;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let fd_arg = std::env::args()
        .skip_while(|a| a != "--fd")
        .nth(1)
        .expect("reemu-core-host: uso: --fd <numero>");
    let fd_num: RawFd = fd_arg.parse().expect("--fd inválido");

    // SAFETY: o pai deixou este fd sem CLOEXEC especificamente pra este
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

/// Métricas de pacing 1×/s sob `REEMU_AUDIO_DEBUG=1` — mesmo formato que
/// `emu_session::LoopDiag` tinha quando isto rodava no processo pai.
struct LoopDiag {
    since: Instant,
    frames: u64,
    over_budget: u64,
    dup_frames: u64,
    busy: Duration,
    worst: Duration,
    audio_samples: u64,
}

impl Default for LoopDiag {
    fn default() -> Self {
        Self {
            since: Instant::now(),
            frames: 0,
            over_budget: 0,
            dup_frames: 0,
            busy: Duration::ZERO,
            worst: Duration::ZERO,
            audio_samples: 0,
        }
    }
}

impl LoopDiag {
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

    fn maybe_report(&mut self, sample_rate: u32) {
        let elapsed = self.since.elapsed();
        if elapsed.as_secs_f32() < 1.0 {
            return;
        }
        let fps = self.frames as f32 / elapsed.as_secs_f32();
        let expected = (sample_rate as f32 * elapsed.as_secs_f32() * 2.0) as u64;
        log::info!(
            "core-host 1s: {:.1} fps, {} frames (retro_run: méd {:.1}ms, pior {:.1}ms, {} \
             acima do budget, {} sem frame novo), áudio {} amostras (esperado ~{})",
            fps,
            self.frames,
            self.busy.as_secs_f32() * 1000.0 / self.frames.max(1) as f32,
            self.worst.as_secs_f32() * 1000.0,
            self.over_budget,
            self.dup_frames,
            self.audio_samples,
            expected,
        );
        *self = Self::default();
    }
}

fn run(channel: Channel, rx: Receiver<ToChild>) {
    let mut core: Option<DesktopCore> = None;
    let mut ring: Option<core_ipc::FrameRing> = None;
    let mut frame_slot = 0u32;
    let mut paused = false;
    let mut core_sample_rate = 32_000u32;
    let mut frame_budget = Duration::from_micros(16_667);
    let mut next_deadline = Instant::now();
    let mut diag = std::env::var_os("REEMU_AUDIO_DEBUG")
        .is_some()
        .then(LoopDiag::default);

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
                &mut frame_budget,
                &mut next_deadline,
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
            } => {
                core = None; // não deveria haver um core já — o pai mata e sobe de novo a cada troca.
                core_loader_desktop::set_pending_core_option_values(initial_option_values);
                let loader = DesktopCoreLoader::new(cores_dir, system_dir, save_dir);
                match loader.open_core(&CoreId(core_id.clone()), &rom_path) {
                    Ok(mut c) => {
                        let av = c.system_av_info();
                        let fps = av.timing.fps.max(1.0);
                        frame_budget = Duration::from_secs_f64(1.0 / fps);
                        next_deadline = Instant::now();
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
                        let new_ring = match core_ipc::FrameRing::create(slot_size) {
                            Ok(r) => r,
                            Err(e) => {
                                let _ = channel.send(
                                    &ToParent::Loaded(Err(format!("anel de frame: {e}"))),
                                    &[],
                                );
                                continue;
                            }
                        };
                        let ring_fd = new_ring.fd();
                        let _ = channel.send(&ToParent::Loaded(Ok(av)), &[ring_fd]);
                        let _ = channel.send(&ToParent::SaveRamRestored(restored), &[]);
                        ring = Some(new_ring);
                        frame_slot = 0;
                        paused = false;
                        diag = std::env::var_os("REEMU_AUDIO_DEBUG")
                            .is_some()
                            .then(LoopDiag::default);
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
                    next_deadline = Instant::now();
                }
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

fn apply_input(ports: &[PortInput; 4]) {
    let pad = core_loader_desktop::retropad();
    let analog = core_loader_desktop::analog();
    for (port, input) in ports.iter().enumerate() {
        pad.set_mask(port, input.joypad_mask);
        analog.set_stick(port, 0, input.sticks[0].0, input.sticks[0].1);
        analog.set_stick(port, 1, input.sticks[1].0, input.sticks[1].1);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_one_frame(
    core: &mut Option<DesktopCore>,
    channel: &Channel,
    ring: &mut Option<core_ipc::FrameRing>,
    frame_slot: &mut u32,
    core_sample_rate: &mut u32,
    frame_budget: &mut Duration,
    next_deadline: &mut Instant,
    diag: &mut Option<LoopDiag>,
) {
    let Some(c) = core.as_mut() else { return };

    if let Some(t) = c.take_av_update() {
        let fps = t.fps.max(1.0);
        *frame_budget = Duration::from_secs_f64(1.0 / fps);
        *core_sample_rate = (t.sample_rate.round() as u32).max(1);
        *next_deadline = Instant::now();
        log::info!(
            "timing atualizado em runtime: fps={:.3} sample_rate={} Hz",
            fps,
            core_sample_rate
        );
    }

    let t0 = diag.as_ref().map(|_| Instant::now());
    let produced = c.next_frame();
    if let (Some(d), Some(t0)) = (diag.as_mut(), t0) {
        d.record_frame(t0.elapsed(), *frame_budget, produced.is_none());
    }
    if let (Some(frame), Some(ring)) = (produced, ring.as_ref()) {
        send_frame(channel, ring, frame_slot, frame);
    }

    let audio = c.drain_audio();
    if let Some(d) = diag.as_mut() {
        d.audio_samples += audio.len() as u64;
        d.maybe_report(*core_sample_rate);
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

    pace(frame_budget, next_deadline);
}

fn send_frame(
    channel: &Channel,
    ring: &core_ipc::FrameRing,
    frame_slot: &mut u32,
    frame: domain::frame_source::Frame,
) {
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
        }
        FrameOrigin::HardwareTexture(handle) => {
            let flip_y = handle.flip_y();
            let slot = handle.slot();
            match handle.take_plane() {
                Some(plane) => {
                    // SAFETY: posse do fd foi transferida por `take_plane`
                    // (ver `domain::frame_source::DmabufPlaneInfo`) — fecha
                    // ao sair do escopo, depois que `send` já o duplicou pro
                    // outro lado dentro do `sendmsg`.
                    let owned = unsafe { OwnedFd::from_raw_fd(plane.fd) };
                    let meta = HwPlaneMeta {
                        width: plane.width,
                        height: plane.height,
                        stride: plane.stride,
                        offset: plane.offset,
                        modifier: plane.modifier,
                        fourcc: plane.fourcc,
                    };
                    let _ = channel.send(
                        &ToParent::FrameReady {
                            slot,
                            meta: frame.metadata,
                            kind: FrameKind::Hardware {
                                flip_y,
                                plane: Some(meta),
                            },
                        },
                        &[owned.as_fd()],
                    );
                }
                None => {
                    let _ = channel.send(
                        &ToParent::FrameReady {
                            slot,
                            meta: frame.metadata,
                            kind: FrameKind::Hardware {
                                flip_y,
                                plane: None,
                            },
                        },
                        &[],
                    );
                }
            }
        }
    }
}

/// Pacing por acumulador + spin — idêntico ao que `emu_session::core_loop`
/// fazia antes disso virar o loop do processo filho.
fn pace(frame_budget: &Duration, next_deadline: &mut Instant) {
    *next_deadline += *frame_budget;
    let now = Instant::now();
    if now < *next_deadline {
        if let Some(coarse) = (*next_deadline - now).checked_sub(Duration::from_micros(600)) {
            std::thread::sleep(coarse);
        }
        loop {
            for _ in 0..64 {
                std::hint::spin_loop();
            }
            if Instant::now() >= *next_deadline {
                break;
            }
        }
    } else if now.duration_since(*next_deadline) > *frame_budget * 4 {
        *next_deadline = now;
    }
}
