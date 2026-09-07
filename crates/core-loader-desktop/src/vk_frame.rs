//! Ponte de frame do HW render Vulkan (etapa 12).
//!
//! Implementa os callbacks que o core chama via `retro_hw_render_interface_vulkan`.
//! Modelo `set_command_buffers` (`vk_rendering` / Beetle PSX — o core NÃO
//! submete; grava e passa pro frontend).
//!
//! **Threading (opção 4):** a thread que dirige o core só GRAVA os command
//! buffers (dentro do `retro_run`). Quem SUBMETE na `VkQueue` é sempre a thread
//! do compositor (junto do submit do wgpu, na mesma thread — sem corrida de
//! queue). O core e o compositor coordenam o reuso de cada slot em voo por um
//! [`VkFrameSync`] (por-slot: "geração já liberada").
//!
//! `handle` (campo da interface) = ponteiro pro `VkFrameBridge`. Todos os
//! callbacks recuperam `&VkFrameBridge` a partir dele.
//!
//! Ver `docs/ai-context/12-vulkan-hw-render-fase2.md`.
#![allow(dead_code)]

use std::os::raw::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use ash::vk::{self, Handle};

use crate::vk_context::VkContext;
use crate::vk_sys;

/// Frames em voo. O core dimensiona os recursos dele pelo `get_sync_index_mask`.
pub(crate) const RING: usize = 3;

/// Coordenação core-loop ↔ compositor pro reuso dos slots em voo.
///
/// O core, no `wait_sync_index`, bloqueia até o compositor (ou o descarte de
/// frame no `emu-session`) liberar a geração anterior daquele slot. Sem isso o
/// core reescreveria o command pool / a `VkImage` de um slot que o compositor
/// ainda está submetendo → concurrent pool access (UB).
pub struct VkFrameSync {
    /// Maior `generation` já liberada pra cada slot.
    released: [AtomicU64; RING],
    lock: Mutex<()>,
    cv: Condvar,
    /// Só pra o `wait_sync_index` não travar pra sempre se o compositor sumir.
    shutdown: std::sync::atomic::AtomicBool,
}

impl VkFrameSync {
    fn new() -> Arc<Self> {
        Arc::new(VkFrameSync {
            released: [const { AtomicU64::new(0) }; RING],
            lock: Mutex::new(()),
            cv: Condvar::new(),
            shutdown: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// O compositor terminou (submeteu + esperou) o frame `gen` do slot `slot`,
    /// ou o `emu-session` descartou esse frame. Idempotente / monotônico.
    pub fn release(&self, slot: u32, gen: u64) {
        let s = slot as usize % RING;
        // `fetch_max`: liberações fora de ordem (descarte) não regridem.
        let prev = self.released[s].fetch_max(gen, Ordering::AcqRel);
        if gen > prev {
            let _g = self.lock.lock().unwrap();
            self.cv.notify_all();
        }
    }

    /// O core vai reusar o slot `slot` pra `gen` — bloqueia até a geração
    /// `gen - RING` (o uso anterior desse slot) ter sido liberada.
    fn wait_free(&self, slot: u32, gen: u64) {
        let Some(need) = gen.checked_sub(RING as u64) else {
            return; // primeiros RING frames: slot nunca foi usado
        };
        let s = slot as usize % RING;
        let mut g = self.lock.lock().unwrap();
        loop {
            if self.shutdown.load(Ordering::Acquire) {
                return;
            }
            if self.released[s].load(Ordering::Acquire) >= need {
                return;
            }
            // timeout curto: destrava se o compositor morrer sem `shutdown`.
            let (ng, to) = self
                .cv
                .wait_timeout(g, std::time::Duration::from_millis(100))
                .unwrap();
            g = ng;
            if to.timed_out() && self.released[s].load(Ordering::Acquire) < need {
                log::warn!("vk wait_sync_index slot {s}: compositor atrasado (gen {need})");
            }
        }
    }

    fn stop(&self) {
        self.shutdown.store(true, Ordering::Release);
        let _g = self.lock.lock().unwrap();
        self.cv.notify_all();
    }
}

/// Trabalho de um frame que a thread do compositor tem que submeter.
pub(crate) struct PendingFrame {
    /// Command buffers de `set_command_buffers` (o core já gravou; ninguém
    /// submeteu ainda). Vazio = frame dup / o core submeteu sozinho (flycast).
    pub cmd_buffers: Vec<vk::CommandBuffer>,
    /// Fence pra o compositor sinalizar no `vkQueueSubmit` e esperar.
    pub fence: vk::Fence,
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub sync_index: u32,
    pub generation: u64,
}

struct Inner {
    current_index: u32,
    generation: u64,
    fences: [vk::Fence; RING],
    pending_image: Option<vk_sys::retro_vulkan_image>,
    pending_cmds: Vec<vk::CommandBuffer>,
    signal_semaphore: vk::Semaphore,
    /// Frames entregues pelo core (diagnóstico/teste).
    delivered: u64,
}

/// Dono do contexto Vulkan + estado por-frame. Vive num `Box` do `DesktopCore`
/// (endereço estável — `interface.handle` aponta pra cá).
pub(crate) struct VkFrameBridge {
    pub ctx: VkContext,
    inner: Mutex<Inner>,
    sync: Arc<VkFrameSync>,
    interface: vk_sys::retro_hw_render_interface_vulkan,
}

// SAFETY: como o `GlContext`, o bridge só é tocado pela thread que dirige o
// core. `interface.handle` é auto-referencial e os ponteiros de função são
// `static`. O `sync` (Arc) é o único ponto compartilhado e é `Send + Sync`.
unsafe impl Send for VkFrameBridge {}

impl VkFrameBridge {
    pub fn new(ctx: VkContext) -> Result<Box<Self>, String> {
        let mut fences = [vk::Fence::null(); RING];
        for f in &mut fences {
            let ci = vk::FenceCreateInfo::default();
            // SAFETY: device válido; fence destruído no Drop.
            *f = unsafe { ctx.device.create_fence(&ci, None) }
                .map_err(|e| format!("vkCreateFence: {e}"))?;
        }

        let interface = vk_sys::retro_hw_render_interface_vulkan {
            interface_type: vk_sys::RETRO_HW_RENDER_INTERFACE_VULKAN,
            interface_version: vk_sys::RETRO_HW_RENDER_INTERFACE_VULKAN_VERSION,
            handle: std::ptr::null_mut(), // corrigido abaixo
            instance: ctx.instance.handle(),
            gpu: ctx.physical_device,
            device: ctx.device.handle(),
            get_device_proc_addr: ctx.get_device_proc_addr(),
            get_instance_proc_addr: ctx.get_instance_proc_addr(),
            queue: ctx.queue,
            queue_index: ctx.queue_family_index,
            set_image: Some(cb_set_image),
            get_sync_index: Some(cb_get_sync_index),
            get_sync_index_mask: Some(cb_get_sync_index_mask),
            set_command_buffers: Some(cb_set_command_buffers),
            wait_sync_index: Some(cb_wait_sync_index),
            lock_queue: Some(cb_lock_queue),
            unlock_queue: Some(cb_unlock_queue),
            set_signal_semaphore: Some(cb_set_signal_semaphore),
        };

        let mut boxed = Box::new(VkFrameBridge {
            ctx,
            inner: Mutex::new(Inner {
                current_index: 0,
                generation: 0,
                fences,
                pending_image: None,
                pending_cmds: Vec::new(),
                signal_semaphore: vk::Semaphore::null(),
                delivered: 0,
            }),
            sync: VkFrameSync::new(),
            interface,
        });
        let self_ptr = boxed.as_ref() as *const VkFrameBridge as *mut c_void;
        boxed.interface.handle = self_ptr;
        Ok(boxed)
    }

    /// Ponteiro pra `retro_hw_render_interface_vulkan` — o que
    /// `GET_HW_RENDER_INTERFACE` devolve. Estável pela vida do `Box`.
    pub fn interface_ptr(&self) -> *const c_void {
        &self.interface as *const _ as *const c_void
    }

    /// Handle de coordenação — clonado pro compositor e pro `emu-session`.
    pub fn sync(&self) -> Arc<VkFrameSync> {
        Arc::clone(&self.sync)
    }

    /// Antes de `retro_run`: nova geração + avança o slot. Mão-única com o
    /// `get_sync_index`/`wait_sync_index` que o core chama em seguida.
    pub fn begin_frame(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.generation += 1;
        inner.current_index = (inner.generation % RING as u64) as u32;
        inner.pending_image = None;
        inner.pending_cmds.clear();
    }

    /// Depois de `retro_run`: pega (sem submeter) o trabalho que o core gravou.
    /// `None` = frame dup / software.
    pub fn take_pending(&self) -> Option<PendingFrame> {
        let mut inner = self.inner.lock().unwrap();
        let image = inner.pending_image.take()?;
        let idx = inner.current_index as usize;
        let cmds = std::mem::take(&mut inner.pending_cmds);
        inner.delivered += 1;
        Some(PendingFrame {
            cmd_buffers: cmds,
            fence: inner.fences[idx],
            image: image.create_info.image,
            view: image.image_view,
            format: image.create_info.format,
            sync_index: inner.current_index,
            generation: inner.generation,
        })
    }

    /// Frames entregues pelo core (diagnóstico/teste).
    pub fn frames_delivered(&self) -> u64 {
        self.inner.lock().unwrap().delivered
    }
}

impl Drop for VkFrameBridge {
    fn drop(&mut self) {
        self.sync.stop();
        let inner = self.inner.lock().unwrap();
        // SAFETY: o core já foi desativado; espera a GPU e destrói os fences
        // antes do `VkContext` derrubar o device.
        unsafe {
            let _ = self.ctx.device.device_wait_idle();
            for &f in &inner.fences {
                if f != vk::Fence::null() {
                    self.ctx.device.destroy_fence(f, None);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Callbacks extern "C" — `handle` = *const VkFrameBridge
// ---------------------------------------------------------------------------

/// SAFETY: `handle` foi preenchido por nós com `&VkFrameBridge` vivo pela
/// duração do core; o core só chama isto na thread que dirige o core.
unsafe fn bridge<'a>(handle: *mut c_void) -> &'a VkFrameBridge {
    &*(handle as *const VkFrameBridge)
}

unsafe extern "C" fn cb_set_image(
    handle: *mut c_void,
    image: *const vk_sys::retro_vulkan_image,
    _num_semaphores: u32,
    _semaphores: *const vk::Semaphore,
    _src_queue_family: u32,
) {
    if handle.is_null() || image.is_null() {
        return;
    }
    // Os cores-alvo passam 0 semáforos e `src_queue_family == IGNORED` — sem
    // transferência de ownership. A sincronização vem do barrier que o core
    // grava no próprio command buffer (release pra SHADER_READ_ONLY_OPTIMAL).
    bridge(handle).inner.lock().unwrap().pending_image = Some(*image);
}

unsafe extern "C" fn cb_set_command_buffers(
    handle: *mut c_void,
    num_cmd: u32,
    cmd: *const vk::CommandBuffer,
) {
    if handle.is_null() || cmd.is_null() || num_cmd == 0 {
        return;
    }
    let slice = std::slice::from_raw_parts(cmd, num_cmd as usize);
    let mut inner = bridge(handle).inner.lock().unwrap();
    inner.pending_cmds.clear();
    inner.pending_cmds.extend_from_slice(slice);
}

unsafe extern "C" fn cb_get_sync_index(handle: *mut c_void) -> u32 {
    if handle.is_null() {
        return 0;
    }
    bridge(handle).inner.lock().unwrap().current_index
}

unsafe extern "C" fn cb_get_sync_index_mask(handle: *mut c_void) -> u32 {
    let _ = handle;
    (1u32 << RING) - 1
}

unsafe extern "C" fn cb_wait_sync_index(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }
    let b = bridge(handle);
    let (idx, gen) = {
        let inner = b.inner.lock().unwrap();
        (inner.current_index, inner.generation)
    };
    // Bloqueia até o compositor liberar o uso anterior deste slot.
    b.sync.wait_free(idx, gen);
}

unsafe extern "C" fn cb_lock_queue(handle: *mut c_void) {
    // Os cores-alvo (`vk_rendering`, Beetle PSX) não submetem — só o
    // compositor, na thread dele. flycast (fase C) submete sozinho e aí isto
    // precisa travar de verdade (guard vazado + `cb_unlock_queue` recompõe).
    let _ = handle;
    log::debug!("vk lock_queue (no-op: só o compositor submete)");
}

unsafe extern "C" fn cb_unlock_queue(handle: *mut c_void) {
    let _ = handle;
}

unsafe extern "C" fn cb_set_signal_semaphore(handle: *mut c_void, semaphore: vk::Semaphore) {
    if handle.is_null() {
        return;
    }
    bridge(handle).inner.lock().unwrap().signal_semaphore = semaphore;
}

// ---------------------------------------------------------------------------
// Handle de frame entregue ao compositor
// ---------------------------------------------------------------------------

/// `domain::frame_source::VulkanImageHandle` concreto. Carrega os handles crus
/// da `VkImage`/view + os command buffers que o compositor tem que submeter.
///
/// `Drop` libera o slot no [`VkFrameSync`] — não importa quem largou o `Frame`
/// (compositor depois de processar, ou `emu-session` descartando um frame
/// atrasado), o core destrava.
pub(crate) struct VkImageFrame {
    image: u64,
    view: u64,
    format: u32,
    width: u32,
    height: u32,
    sync_index: u32,
    generation: u64,
    fence: u64,
    cmd_buffers: Vec<u64>,
    sync: Arc<VkFrameSync>,
    released: std::sync::atomic::AtomicBool,
}

impl VkImageFrame {
    pub fn new(p: PendingFrame, width: u32, height: u32, sync: Arc<VkFrameSync>) -> Self {
        Self {
            image: p.image.as_raw(),
            view: p.view.as_raw(),
            format: p.format.as_raw() as u32,
            width,
            height,
            sync_index: p.sync_index,
            generation: p.generation,
            fence: p.fence.as_raw(),
            cmd_buffers: p.cmd_buffers.iter().map(|c| c.as_raw()).collect(),
            sync,
            released: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl domain::frame_source::VulkanImageHandle for VkImageFrame {
    fn image(&self) -> u64 {
        self.image
    }
    fn image_view(&self) -> u64 {
        self.view
    }
    fn vk_format(&self) -> u32 {
        self.format
    }
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn sync_index(&self) -> u32 {
        self.sync_index
    }
    fn command_buffers(&self) -> &[u64] {
        &self.cmd_buffers
    }
    fn fence(&self) -> u64 {
        self.fence
    }
    fn release(&self) {
        if !self
            .released
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            self.sync.release(self.sync_index, self.generation);
        }
    }
}

impl Drop for VkImageFrame {
    fn drop(&mut self) {
        if !self
            .released
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            self.sync.release(self.sync_index, self.generation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_index_mask_matches_ring() {
        // RING=3 → mask 0b111; o core faz `while (m >>= 1) n++` → n = 3.
        let mask = unsafe { cb_get_sync_index_mask(std::ptr::null_mut()) };
        assert_eq!(mask, 0b111);
        let mut m = mask;
        let mut n = 1;
        while {
            m >>= 1;
            m != 0
        } {
            n += 1;
        }
        assert_eq!(n, RING);
    }

    #[test]
    fn frame_sync_gates_slot_reuse() {
        let s = VkFrameSync::new();
        // gens 1..=RING não bloqueiam (slot nunca usado).
        s.wait_free(0, 1);
        s.wait_free(1, 2);
        s.wait_free(2, 3);
        // gen RING+1 no slot 1 precisa da gen 1 liberada.
        let s2 = Arc::clone(&s);
        let h = std::thread::spawn(move || {
            s2.wait_free(1, RING as u64 + 1);
        });
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert!(!h.is_finished(), "devia bloquear até liberar a gen 1");
        s.release(1, 1);
        h.join().unwrap();
    }
}
