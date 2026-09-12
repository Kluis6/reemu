//! `LocalCore`: um core libretro rodando **dentro deste processo** (o pai),
//! não no `reemu-core-host`. É o caminho do HW render Vulkan da etapa 12
//! (`docs/ai-context/12-vulkan-hw-render-fase2.md`, fase B3b): a `VkImage` que
//! o core entrega tem que ficar no mesmo `VkDevice` do compositor wgpu, então
//! não pode cruzar a fronteira de processo.
//!
//! Trade-off (aceito, opt-in `REEMU_HW=vulkan`): volta a valer a limitação de
//! **um core por processo** da API libretro — cores não re-entrantes
//! (parallel_n64...) podem derrubar o app numa 2ª carga no mesmo processo. Por
//! isso o roteamento local só liga sob o env var explícito de desenvolvimento.
//!
//! O loop de drive aqui espelha `reemu-core-host::run_one_frame` (pacing por
//! acumulador, `take_av_update`, `drain_audio`) — a diferença é que
//! `session.rs` publica o resultado direto em `Shared` em vez de mandar
//! `ToParent` por IPC.

use core_ipc::PortInput;
use core_loader_desktop::{DesktopCore, DesktopCoreLoader};
use domain::core_loader::{CoreId, CoreLoadError, LoadedCore, RenderBackend, SystemAvInfo};
use domain::core_options::CoreOptionDefinition;
use domain::frame_source::{Frame, FrameSource};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Um `retro_run` já rodado: o frame (se veio um novo), o PCM acumulado e a
/// taxa de amostragem vigente. `session.rs` empurra pra `Shared`.
pub(crate) struct FrameTick {
    pub frame: Option<Frame>,
    pub audio: Vec<i16>,
    pub sample_rate: u32,
}

pub(crate) struct LocalCore {
    core: DesktopCore,
    frame_budget: Duration,
    next_deadline: Instant,
    sample_rate: u32,
}

impl LocalCore {
    /// Carrega o core neste processo, adotando o `VkDevice` do compositor. Só
    /// segue se o core realmente negociou Vulkan — senão o chamador deve cair
    /// pro caminho do processo filho (que isola cores não re-entrantes).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn load(
        core_id: &str,
        rom_path: &str,
        cores_dir: PathBuf,
        system_dir: PathBuf,
        save_dir: PathBuf,
        initial_option_values: HashMap<String, String>,
        initial_save_ram: Option<Vec<u8>>,
        shared_device: Option<domain::core_loader::VulkanSharedDevice>,
        negotiator: Option<domain::core_loader::VulkanDeviceNegotiator>,
    ) -> Result<(Self, SystemAvInfo), CoreLoadError> {
        // NB: `silence_core_stdout` (a versão PERMANENTE) NÃO aqui — este
        // caminho roda no processo principal (Tauri/webview/wgpu), matar o
        // stdout pra sempre calaria eles também. `with_core_stdout_silenced`
        // muta só durante o `open_core` (onde o Beetle spamma `[hdcache]`/
        // `Creating shader module`) e restaura o stdout logo depois.
        core_loader_desktop::set_pending_core_option_values(initial_option_values);
        let mut loader = DesktopCoreLoader::new(cores_dir, system_dir, save_dir).vulkan_only();
        if let Some(s) = shared_device {
            loader = loader.with_vulkan_shared_device(s);
        }
        if let Some(n) = negotiator {
            loader = loader.with_vulkan_negotiator(n);
        }
        // `vulkan_only()` já aborta antes de montar contexto GL se não for
        // Vulkan — o `?` propaga o `HwRenderUnsupported` pro `session.rs`
        // cair pro processo filho.
        let mut core = core_loader_desktop::with_core_stdout_silenced(|| {
            loader.open_core(&CoreId(core_id.to_string()), rom_path)
        })?;
        debug_assert_eq!(
            core.render_requirements().render_backend,
            RenderBackend::Vulkan
        );

        if let Some(bytes) = initial_save_ram {
            if core.restore_save_ram(&bytes) {
                log::info!("save RAM restaurada ({} bytes)", bytes.len());
            } else {
                log::warn!("save RAM ignorada (tamanho não bate)");
            }
        }

        let av = core.system_av_info();
        let fps = av.timing.fps.max(1.0);
        let sample_rate = (av.timing.sample_rate.round() as u32).max(1);
        log::info!(
            "core Vulkan {core_id} in-process: fps={fps:.3} sample_rate={sample_rate} Hz"
        );

        Ok((
            Self {
                core,
                frame_budget: Duration::from_secs_f64(1.0 / fps),
                next_deadline: Instant::now(),
                sample_rate,
            },
            av,
        ))
    }

    /// Copia o snapshot de input do pai pros globais que o `input_state_cb` do
    /// core lê (mesmo processo — espelha `core-host::apply_input`).
    pub(crate) fn apply_input(&self, ports: &[PortInput; 4]) {
        let pad = core_loader_desktop::retropad();
        let analog = core_loader_desktop::analog();
        for (port, input) in ports.iter().enumerate() {
            pad.set_mask(port, input.joypad_mask);
            analog.set_stick(port, 0, input.sticks[0].0, input.sticks[0].1);
            analog.set_stick(port, 1, input.sticks[1].0, input.sticks[1].1);
        }
    }

    /// Roda um `retro_run` e pacing (bloqueia até o próximo deadline).
    pub(crate) fn run_frame(&mut self) -> FrameTick {
        if let Some(t) = self.core.take_av_update() {
            let fps = t.fps.max(1.0);
            self.frame_budget = Duration::from_secs_f64(1.0 / fps);
            self.sample_rate = (t.sample_rate.round() as u32).max(1);
            self.next_deadline = Instant::now();
            log::info!(
                "timing atualizado em runtime: fps={fps:.3} sample_rate={} Hz",
                self.sample_rate
            );
        }

        let frame = self.core.next_frame();
        let audio = self.core.drain_audio();
        self.pace();
        FrameTick {
            frame,
            audio,
            sample_rate: self.sample_rate,
        }
    }

    pub(crate) fn set_paused(&mut self, paused: bool) {
        if !paused {
            self.next_deadline = Instant::now();
        }
    }

    pub(crate) fn serialize_state(&mut self) -> Option<Vec<u8>> {
        self.core.serialize_state()
    }

    pub(crate) fn restore_state(&mut self, data: &[u8]) -> bool {
        self.core.restore_state(data)
    }

    pub(crate) fn save_ram(&self) -> Option<Vec<u8>> {
        self.core.save_ram()
    }

    pub(crate) fn core_options(
        &self,
    ) -> (Vec<CoreOptionDefinition>, HashMap<String, String>) {
        (
            core_loader_desktop::core_options(),
            core_loader_desktop::core_option_values(),
        )
    }

    pub(crate) fn set_core_option(&self, key: &str, value: &str) -> bool {
        core_loader_desktop::set_core_option(key, value)
    }

    /// Pacing idêntico ao de `reemu-core-host::pace`: dorme o grosso, depois
    /// spin fino até o deadline; se atrasou muito, ressincroniza.
    fn pace(&mut self) {
        self.next_deadline += self.frame_budget;
        let now = Instant::now();
        if now < self.next_deadline {
            if let Some(coarse) =
                (self.next_deadline - now).checked_sub(Duration::from_micros(600))
            {
                std::thread::sleep(coarse);
            }
            loop {
                for _ in 0..64 {
                    std::hint::spin_loop();
                }
                if Instant::now() >= self.next_deadline {
                    break;
                }
            }
        } else if now.duration_since(self.next_deadline) > self.frame_budget * 4 {
            self.next_deadline = now;
        }
    }
}
