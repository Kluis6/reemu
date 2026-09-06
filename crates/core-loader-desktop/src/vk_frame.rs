//! Ponte de frame do HW render Vulkan (etapa 12).
//!
//! Implementa os callbacks que o core chama via `retro_hw_render_interface_vulkan`
//! e faz a submissão dos command buffers que o core entrega em
//! `set_command_buffers` (modelo do `vk_rendering` / Beetle PSX — o core NÃO
//! submete; grava e passa pro frontend).
//!
//! `handle` (campo da interface) = ponteiro pro `VkFrameBridge`. Todos os
//! callbacks recuperam `&VkFrameBridge` a partir dele — a API libretro Vulkan
//! foi desenhada pra isso, então não precisamos de estado global aqui (ao
//! contrário dos callbacks de FBO do caminho GL).
//!
//! Ver `docs/ai-context/12-vulkan-hw-render-fase2.md`.
#![allow(dead_code)]

use std::os::raw::c_void;
use std::sync::Mutex;

use ash::vk;

use crate::vk_context::VkContext;
use crate::vk_sys;

/// Frames em voo. O core dimensiona os recursos dele pelo `get_sync_index_mask`.
pub(crate) const RING: usize = 3;

/// Imagem pronta de um frame — o que o compositor (fase B) embrulha com
/// `wgpu::Device::create_texture_from_hal`.
#[derive(Clone, Copy)]
pub(crate) struct ReadyImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub layout: vk::ImageLayout,
    pub sync_index: u32,
}

struct Inner {
    /// Índice do slot em voo do frame corrente. Avança em `begin_frame`.
    current_index: u32,
    fences: [vk::Fence; RING],
    /// `fences[i]` já foi passado a um `queue_submit` (senão `wait_for_fences`
    /// nele trava pra sempre).
    fence_pending: [bool; RING],
    /// Última `retro_vulkan_image` de `set_image` (struct é `Copy`; ignoramos
    /// `create_info.p_next` — os cores-alvo passam NULL).
    pending_image: Option<vk_sys::retro_vulkan_image>,
    /// Command buffers de `set_command_buffers`, ainda não submetidos.
    pending_cmds: Vec<vk::CommandBuffer>,
    /// De `set_signal_semaphore` (não usado na fase A).
    signal_semaphore: vk::Semaphore,
    /// Quantos frames o core já entregou e nós submetemos com sucesso.
    /// Observável só pra diagnóstico/teste.
    submitted: u64,
}

/// Dono do contexto Vulkan + estado por-frame. Vive num `Box` do `DesktopCore`
/// (endereço estável — `interface.handle` aponta pra cá).
pub(crate) struct VkFrameBridge {
    pub ctx: VkContext,
    inner: Mutex<Inner>,
    /// Entregue ao core em `GET_HW_RENDER_INTERFACE`. `handle` é corrigido pra
    /// `&self` logo após o `Box::new`.
    interface: vk_sys::retro_hw_render_interface_vulkan,
}

// SAFETY: como o `GlContext`, o bridge só é tocado pela thread que dirige o
// core. `interface.handle` é auto-referencial (aponta pro próprio `Box`) e os
// ponteiros de função são `static`.
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
                fences,
                fence_pending: [false; RING],
                pending_image: None,
                pending_cmds: Vec::new(),
                signal_semaphore: vk::Semaphore::null(),
                submitted: 0,
            }),
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

    /// Antes de `retro_run`: avança o slot em voo. Mão-única com o
    /// `get_sync_index` que o core vai chamar em seguida.
    pub fn begin_frame(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.current_index = (inner.current_index + 1) % RING as u32;
        inner.pending_image = None;
        inner.pending_cmds.clear();
    }

    /// Depois de `retro_run`: submete o que o core entregou e devolve a imagem
    /// pronta. `None` = o core não produziu frame HW neste `run` (dup/software).
    ///
    /// Fase B: `wait` conservador (CPU-wait no fence) antes de devolver.
    pub fn submit_pending(&self, wait: bool) -> Result<Option<ReadyImage>, String> {
        let mut inner = self.inner.lock().unwrap();
        let Some(image) = inner.pending_image else {
            return Ok(None);
        };
        let idx = inner.current_index as usize;
        let fence = inner.fences[idx];
        let cmds = std::mem::take(&mut inner.pending_cmds);

        if !cmds.is_empty() {
            let submit = vk::SubmitInfo::default().command_buffers(&cmds);
            let _q = self.ctx.queue_lock.lock().unwrap();
            // SAFETY: `submit` referencia `cmds` (vivo); queue serializada pelo
            // `queue_lock`.
            unsafe {
                self.ctx
                    .device
                    .queue_submit(self.ctx.queue, &[submit], fence)
            }
            .map_err(|e| format!("vkQueueSubmit: {e}"))?;
            inner.fence_pending[idx] = true;
        }

        if wait && inner.fence_pending[idx] {
            // SAFETY: fence foi submetido acima (ou num ciclo anterior deste
            // índice); espera e reseta.
            unsafe {
                self.ctx
                    .device
                    .wait_for_fences(&[fence], true, u64::MAX)
                    .map_err(|e| format!("vkWaitForFences: {e}"))?;
                self.ctx
                    .device
                    .reset_fences(&[fence])
                    .map_err(|e| format!("vkResetFences: {e}"))?;
            }
            inner.fence_pending[idx] = false;
        }

        inner.submitted += 1;
        Ok(Some(ReadyImage {
            image: image.create_info.image,
            view: image.image_view,
            layout: image.image_layout,
            sync_index: inner.current_index,
        }))
    }

    /// Frames entregues pelo core e submetidos com sucesso. Diagnóstico/teste.
    pub fn frames_submitted(&self) -> u64 {
        self.inner.lock().unwrap().submitted
    }
}

impl Drop for VkFrameBridge {
    fn drop(&mut self) {
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
    let b = bridge(handle);
    // Os cores-alvo passam 0 semáforos (`num_semaphores == 0`) e
    // `src_queue_family == VK_QUEUE_FAMILY_IGNORED` — sem transferência de
    // ownership. A sincronização vem do barrier que o core grava no próprio
    // command buffer (release pra SHADER_READ_ONLY_OPTIMAL).
    b.inner.lock().unwrap().pending_image = Some(*image);
}

unsafe extern "C" fn cb_set_command_buffers(
    handle: *mut c_void,
    num_cmd: u32,
    cmd: *const vk::CommandBuffer,
) {
    if handle.is_null() || cmd.is_null() || num_cmd == 0 {
        return;
    }
    let b = bridge(handle);
    let slice = std::slice::from_raw_parts(cmd, num_cmd as usize);
    let mut inner = b.inner.lock().unwrap();
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
    let inner = b.inner.lock().unwrap();
    let idx = inner.current_index as usize;
    if !inner.fence_pending[idx] {
        return; // ainda não submetido neste índice
    }
    let fence = inner.fences[idx];
    drop(inner);
    // SAFETY: fence submetido; espera CPU e reseta pro core reusar o slot.
    let _ = b.ctx.device.wait_for_fences(&[fence], true, u64::MAX);
    let _ = b.ctx.device.reset_fences(&[fence]);
    b.inner.lock().unwrap().fence_pending[idx] = false;
}

unsafe extern "C" fn cb_lock_queue(handle: *mut c_void) {
    // Fase A: os cores-alvo (`vk_rendering`, Beetle PSX) não submetem eles
    // mesmos — só o frontend, atrás do `queue_lock`. flycast (fase C) submete
    // sozinho e aí isto precisa travar o `queue_lock` de verdade (guard
    // vazado + `cb_unlock_queue` recompõe). TODO fase C.
    let _ = handle;
    log::debug!("vk lock_queue (no-op fase A)");
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
}
