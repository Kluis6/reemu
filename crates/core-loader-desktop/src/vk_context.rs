//! Contexto Vulkan compartilhado pra HW render por-core (etapa 12).
//!
//! O frontend cria **uma** `VkInstance` + `VkDevice` (aqui) e a entrega pros
//! dois lados: o wgpu do compositor (`Adapter::device_from_raw`, montado na
//! fase B, lado Tauri) e o core libretro (via
//! `retro_hw_render_interface_vulkan`). Com um device só e um processo só, a
//! `VkImage` que o core entrega no `set_image` vira `wgpu::Texture` direto
//! (`create_texture_from_hal`), sem cópia.
//!
//! **Não** chamamos o `create_device` da negociação do core: o `VkCreateDevice`
//! v1 do flycast ignora `required_device_extensions`/`required_features`, então
//! deixá-lo criar o device impediria habilitar as extensões que o wgpu exige.
//! O flycast aceita um device criado pelo frontend (`VulkanContext::init` só lê
//! `retro_hw_render_interface_vulkan::device`).
//!
//! Ver `docs/ai-context/12-vulkan-hw-render-fase2.md`.
//!
//! Fase A: só a criação do contexto. A negociação (`GET_HW_RENDER_INTERFACE`
//! etc.) e a ponte de frame entram nas fases seguintes — daí o `dead_code`.
#![allow(dead_code)]

use std::ffi::{c_char, CStr, CString};
use std::sync::Mutex;

use ash::vk;

/// Parâmetros de criação do contexto. A fase B preenche
/// `extra_device_extensions` com o conjunto que o `wgpu-hal` reporta em
/// `Adapter::required_device_extensions()`.
pub(crate) struct VkConfig {
    pub app_name: CString,
    /// `VK_API_VERSION_*` — o core pode pedir via `get_application_info`
    /// (flycast pede 1.1). 1.1 é o piso recomendado pela spec do libretro.
    pub api_version: u32,
    pub extra_device_extensions: Vec<CString>,
}

impl Default for VkConfig {
    fn default() -> Self {
        VkConfig {
            app_name: CString::new("ReEmu").unwrap(),
            api_version: vk::make_api_version(0, 1, 1, 0),
            extra_device_extensions: Vec::new(),
        }
    }
}

/// Instância + dispositivo lógico + uma queue graphics/compute. Dono de tudo;
/// derruba na ordem certa no `Drop`.
pub(crate) struct VkContext {
    // ordem de campo = ordem de drop (device antes de instance antes de entry).
    pub device: ash::Device,
    pub physical_device: vk::PhysicalDevice,
    pub queue: vk::Queue,
    pub queue_family_index: u32,
    /// A spec do `retro_hw_render_interface_vulkan` exige `lock_queue`/
    /// `unlock_queue` em volta de QUALQUER `vkQueueSubmit` — o core submete os
    /// command buffers dele direto nesta queue.
    pub queue_lock: Mutex<()>,
    pub instance: ash::Instance,
    pub entry: ash::Entry,
}

// SAFETY: `ash::Device`/`Instance` são `Send + Sync` (só handles + tabelas de
// função imutáveis). A `VkQueue` só é submetida atrás do `queue_lock`.
unsafe impl Send for VkContext {}
unsafe impl Sync for VkContext {}

impl VkContext {
    pub fn create(cfg: &VkConfig) -> Result<Self, String> {
        // SAFETY: carrega libvulkan.so.1 do sistema; falha limpa se ausente.
        let entry =
            unsafe { ash::Entry::load() }.map_err(|e| format!("libvulkan indisponível: {e}"))?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(&cfg.app_name)
            .application_version(0)
            .engine_name(&cfg.app_name)
            .engine_version(0)
            .api_version(cfg.api_version);
        let instance_ci = vk::InstanceCreateInfo::default().application_info(&app_info);
        // SAFETY: `instance_ci` referencia `app_info`, vivo até o fim do escopo;
        // a instância é destruída no `Drop`.
        let instance = unsafe { entry.create_instance(&instance_ci, None) }
            .map_err(|e| format!("vkCreateInstance: {e}"))?;

        let cleanup_instance =
            |instance: &ash::Instance| unsafe { instance.destroy_instance(None) };

        let physical_device = match Self::pick_physical_device(&instance) {
            Ok(pd) => pd,
            Err(e) => {
                cleanup_instance(&instance);
                return Err(e);
            }
        };

        let queue_family_index = match Self::pick_queue_family(&instance, physical_device) {
            Ok(qfi) => qfi,
            Err(e) => {
                cleanup_instance(&instance);
                return Err(e);
            }
        };

        let ext_ptrs: Vec<*const c_char> = cfg
            .extra_device_extensions
            .iter()
            .map(|c| c.as_ptr())
            .collect();
        let priorities = [1.0f32];
        let queue_ci = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities)];
        let device_ci = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_ci)
            .enabled_extension_names(&ext_ptrs);
        // SAFETY: `device_ci` referencia `queue_ci`/`ext_ptrs`, vivos até aqui.
        let device = match unsafe { instance.create_device(physical_device, &device_ci, None) } {
            Ok(d) => d,
            Err(e) => {
                cleanup_instance(&instance);
                return Err(format!("vkCreateDevice: {e}"));
            }
        };

        // SAFETY: family/index válidos (uma queue só, criada acima).
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        log::info!(
            "contexto Vulkan pronto (queue family {queue_family_index}, {} ext extra)",
            cfg.extra_device_extensions.len()
        );

        Ok(VkContext {
            device,
            physical_device,
            queue,
            queue_family_index,
            queue_lock: Mutex::new(()),
            instance,
            entry,
        })
    }

    /// `PFN_vkGetInstanceProcAddr` do loader — o campo homônimo de
    /// `retro_hw_render_interface_vulkan`.
    pub fn get_instance_proc_addr(&self) -> vk::PFN_vkGetInstanceProcAddr {
        self.entry.static_fn().get_instance_proc_addr
    }

    /// `PFN_vkGetDeviceProcAddr` — idem.
    pub fn get_device_proc_addr(&self) -> vk::PFN_vkGetDeviceProcAddr {
        self.instance.fp_v1_0().get_device_proc_addr
    }

    fn pick_physical_device(instance: &ash::Instance) -> Result<vk::PhysicalDevice, String> {
        // SAFETY: instância válida.
        let devices = unsafe { instance.enumerate_physical_devices() }
            .map_err(|e| format!("vkEnumeratePhysicalDevices: {e}"))?;
        if devices.is_empty() {
            return Err("nenhum VkPhysicalDevice".into());
        }
        // Prefere discreta (fase B: casar com o GPU que o wgpu escolheu, por
        // UUID / `REEMU_GPU`).
        let mut chosen = devices[0];
        for &pd in &devices {
            let props = unsafe { instance.get_physical_device_properties(pd) };
            if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                chosen = pd;
                break;
            }
        }
        Ok(chosen)
    }

    fn pick_queue_family(instance: &ash::Instance, pd: vk::PhysicalDevice) -> Result<u32, String> {
        // SAFETY: pd válido.
        let families = unsafe { instance.get_physical_device_queue_family_properties(pd) };
        // A spec do libretro Vulkan exige uma queue com GRAPHICS **e** COMPUTE.
        let want = vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE;
        families
            .iter()
            .position(|f| f.queue_flags.contains(want))
            .map(|i| i as u32)
            .ok_or_else(|| "nenhuma queue family com GRAPHICS+COMPUTE".into())
    }

    /// Nome do device (log/diagnóstico).
    pub fn device_name(&self) -> String {
        let props = unsafe {
            self.instance
                .get_physical_device_properties(self.physical_device)
        };
        let raw = props.device_name.as_ptr();
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    }
}

impl Drop for VkContext {
    fn drop(&mut self) {
        // SAFETY: nada mais usa o device/instância a partir daqui; espera a GPU
        // ociosa antes de destruir (o core já foi desativado pelo chamador).
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "precisa de uma ICD Vulkan (libvulkan + GPU/lavapipe)"]
    fn creates_instance_and_device() {
        let ctx = VkContext::create(&VkConfig::default()).expect("criar contexto Vulkan");
        assert_ne!(ctx.queue, vk::Queue::null());
        assert!(!ctx.device_name().is_empty());
        eprintln!("device: {}", ctx.device_name());
    }
}
