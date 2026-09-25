//! Espera na GPU pelo fim do render de um core GL (interop `dma_buf`), no
//! lugar do `glFinish` do produtor.
//!
//! O core-host cria uma fence nativa no fim do frame
//! (`EGL_ANDROID_native_fence_sync`) e manda o fd de `sync_file` junto do
//! frame. Aqui ele vira um semáforo Vulkan binário:
//!
//! - `vkImportSemaphoreFdKHR` com `VK_EXTERNAL_SEMAPHORE_HANDLE_TYPE_SYNC_FD_BIT`
//!   (`VK_KHR_external_semaphore_fd`). Esse tipo tem transferência por cópia
//!   e só aceita import TEMPORÁRIO (`VK_SEMAPHORE_IMPORT_TEMPORARY_BIT`); o fd
//!   passa a ser do Vulkan quando o import dá certo (spec Vulkan,
//!   "Importing Semaphore Payloads").
//! - `wgpu::hal::vulkan::Queue::add_wait_semaphore` põe a espera no PRÓXIMO
//!   `submit` do wgpu — o que amostra a textura importada. O wgpu encadeia
//!   as submissões com semáforos, então os submits seguintes também ficam
//!   depois do render do core.
//! - Depois da espera o semáforo volta ao payload permanente e pode ser
//!   reimportado, mas só quando nenhum comando pendente o usa
//!   (VUID-vkImportSemaphoreFdKHR-semaphore-01142): ele volta pra lista livre
//!   no `on_submitted_work_done` do submit que esperou.
//!
//! Sem a extensão (ou se o import falhar), espera pela CPU com `poll()` no
//! fd — o `sync_file` fica legível (`POLLIN`) quando a fence sinaliza
//! (kernel, `drivers/dma-buf/sync_file.c`). Ainda assim melhor que o
//! `glFinish`: a thread do core já seguiu.

use ash::vk;
use std::os::fd::{AsRawFd as _, FromRawFd as _, IntoRawFd as _, OwnedFd};
use std::sync::{Arc, Mutex};

const EXT: &std::ffi::CStr = ash::khr::external_semaphore_fd::NAME;

/// O device suporta importar `sync_file` como semáforo?
///
/// # Safety
/// `adapter` tem que ser Vulkan (`as_hal`).
unsafe fn adapter_supports_sync_fd(adapter: &wgpu::Adapter) -> bool {
    let Some(hal) = (unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }) else {
        return false;
    };
    if !hal.physical_device_capabilities().supports_extension(EXT) {
        return false;
    }
    let instance = hal.shared_instance().raw_instance();
    let info = vk::PhysicalDeviceExternalSemaphoreInfo::default()
        .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD);
    let mut props = vk::ExternalSemaphoreProperties::default();
    unsafe {
        instance.get_physical_device_external_semaphore_properties(
            hal.raw_physical_device(),
            &info,
            &mut props,
        )
    };
    props
        .external_semaphore_features
        .contains(vk::ExternalSemaphoreFeatureFlags::IMPORTABLE)
}

/// Abre o device como o `video_surface::create_device_with`, mas pedindo
/// também `VK_KHR_external_semaphore_fd` quando a GPU importa `sync_file`.
/// O 5º elemento diz se a extensão entrou. `None` = não é Vulkan ou algo
/// falhou (o chamador cai no caminho de sempre).
pub(super) fn create_device(
    instance: &wgpu::Instance,
    wanted: wgpu::Features,
) -> Option<(
    wgpu::Adapter,
    wgpu::Device,
    wgpu::Queue,
    wgpu::Features,
    bool,
)> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    }))
    .ok()?;
    if adapter.get_info().backend != wgpu::Backend::Vulkan {
        return None;
    }
    // SAFETY: backend conferido acima.
    if !unsafe { adapter_supports_sync_fd(&adapter) } {
        return None;
    }
    let features = wanted & adapter.features();
    let limits = video_surface::device_limits(&adapter);
    let open = {
        let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
        // SAFETY: a callback só ACRESCENTA uma extensão que o device
        // suporta (conferido em `adapter_supports_sync_fd`).
        unsafe {
            hal.open_with_callback(
                features,
                &limits,
                &wgpu::MemoryHints::default(),
                Some(Box::new(|args| {
                    if !args.extensions.contains(&EXT) {
                        args.extensions.push(EXT);
                    }
                })),
            )
        }
        .inspect_err(|e| log::warn!("device com {EXT:?}: {e}"))
        .ok()?
    };
    // SAFETY: `open` veio deste `adapter`, com exatamente `features`.
    let (device, queue) = unsafe {
        adapter.create_device_from_hal(
            open,
            &wgpu::DeviceDescriptor {
                label: Some("video-surface device (sync_fd)"),
                required_features: features,
                required_limits: limits,
                ..Default::default()
            },
        )
    }
    .inspect_err(|e| log::warn!("create_device_from_hal: {e}"))
    .ok()?;
    Some((adapter, device, queue, features, true))
}

/// Importa fences de `sync_file` como semáforos e agenda a espera no
/// próximo submit do wgpu.
pub(super) struct SyncFdImporter {
    device: wgpu::Device,
    ext: ash::khr::external_semaphore_fd::Device,
    raw: ash::Device,
    /// Semáforos que já podem ser reimportados.
    free: Arc<Mutex<Vec<vk::Semaphore>>>,
    /// Agendados desde o último `stage_wait` — ainda sem submit garantido.
    pending: Vec<vk::Semaphore>,
    /// Todos os criados (pra destruir no `Drop`).
    all: Vec<vk::Semaphore>,
}

impl SyncFdImporter {
    /// `None` se o device não é Vulkan. Só chame com um device aberto por
    /// [`create_device`] (a extensão precisa estar ligada).
    pub(super) fn new(device: &wgpu::Device) -> Option<Self> {
        // SAFETY: só lemos os handles crus; o `wgpu::Device` guardado mantém
        // o `VkDevice` vivo até o `Drop` daqui.
        let hal = unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }?;
        let raw = hal.raw_device().clone();
        let ext = ash::khr::external_semaphore_fd::Device::new(
            hal.shared_instance().raw_instance(),
            &raw,
        );
        drop(hal);
        Some(Self {
            device: device.clone(),
            ext,
            raw,
            free: Arc::default(),
            pending: Vec::new(),
            all: Vec::new(),
        })
    }

    /// Faz o próximo `submit` de `queue` esperar a fence `fd`. `Err(fd)` =
    /// não deu (o fd volta; espere com [`cpu_wait`]).
    pub(super) fn stage_wait(&mut self, queue: &wgpu::Queue, fd: OwnedFd) -> Result<(), OwnedFd> {
        // Os agendados no frame anterior já foram submetidos: devolvem à
        // lista livre quando essa leva terminar na GPU.
        if !self.pending.is_empty() {
            let done = std::mem::take(&mut self.pending);
            let free = Arc::clone(&self.free);
            queue.on_submitted_work_done(move || {
                free.lock().unwrap_or_else(|e| e.into_inner()).extend(done);
            });
        }
        let reused = self.free.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let sem = match reused {
            Some(s) => s,
            None => {
                // Callbacks só rodam no poll: dá uma chance antes de criar mais.
                if self.all.len() >= 8 {
                    let _ = self.device.poll(wgpu::PollType::Poll);
                }
                match self.free.lock().unwrap_or_else(|e| e.into_inner()).pop() {
                    Some(s) => s,
                    None => match unsafe {
                        self.raw
                            .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    } {
                        Ok(s) => {
                            self.all.push(s);
                            s
                        }
                        Err(e) => {
                            log::warn!("vkCreateSemaphore: {e}");
                            return Err(fd);
                        }
                    },
                }
            }
        };
        let raw_fd = fd.into_raw_fd();
        let info = vk::ImportSemaphoreFdInfoKHR::default()
            .semaphore(sem)
            .flags(vk::SemaphoreImportFlags::TEMPORARY)
            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::SYNC_FD)
            .fd(raw_fd);
        // SAFETY: `sem` não está em uso por nenhum comando pendente (veio da
        // lista livre ou é novo). No sucesso o fd passa a ser do Vulkan.
        if let Err(e) = unsafe { self.ext.import_semaphore_fd(&info) } {
            log::warn!("vkImportSemaphoreFdKHR: {e}");
            self.free
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(sem);
            // SAFETY: import falhou → o fd continua nosso.
            return Err(unsafe { OwnedFd::from_raw_fd(raw_fd) });
        }
        // SAFETY: só agenda a espera no hal; o semáforo vive até o `Drop`.
        match unsafe { queue.as_hal::<wgpu::hal::api::Vulkan>() } {
            Some(q) => {
                // O render do core só é lido pelo shader de fragmento (flip /
                // 1º passe da chain).
                q.add_wait_semaphore(sem, None, vk::PipelineStageFlags::FRAGMENT_SHADER);
                self.pending.push(sem);
                Ok(())
            }
            None => {
                // Sem fila hal não há como esperar; o payload importado é
                // descartado junto do semáforo (não volta pra lista livre).
                log::warn!("fila wgpu sem hal Vulkan — sem espera na GPU");
                Ok(())
            }
        }
    }
}

impl Drop for SyncFdImporter {
    fn drop(&mut self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        for s in self.all.drain(..) {
            // SAFETY: GPU ociosa (poll acima) → nenhum submit usa `s`.
            unsafe { self.raw.destroy_semaphore(s, None) };
        }
    }
}

/// Espera pela CPU até a fence sinalizar (ou `timeout_ms`). Fecha o fd.
pub(super) fn cpu_wait(fd: OwnedFd, timeout_ms: u16) {
    let mut pfd = [rustix::event::PollFd::new(
        &fd,
        rustix::event::PollFlags::IN,
    )];
    let t = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: i64::from(timeout_ms) * 1_000_000,
    };
    if let Err(e) = rustix::event::poll(&mut pfd, Some(&t)) {
        log::warn!("poll(sync_file {}): {e}", fd.as_raw_fd());
    }
}
