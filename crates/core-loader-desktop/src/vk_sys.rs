//! Declarações FFI de `RETRO_HW_RENDER_INTERFACE_VULKAN` (etapa 12).
//!
//! Layout/assinaturas conferidos contra os headers oficiais do libretro
//! (RetroArch, master), **não** implementados de memória:
//!   - `libretro-common/include/libretro.h`
//!   - `libretro-common/include/libretro_vulkan.h`
//!
//! Versões alvo:
//!   - `RETRO_HW_RENDER_INTERFACE_VULKAN_VERSION` = 5
//!   - `RETRO_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_VULKAN_VERSION` = 2
//!
//! Os tipos `Vk*` vêm do crate `ash` (mesmos handles `#[repr(transparent)]`
//! e structs `#[repr(C)]` que o header C usa). Nada aqui é consumido ainda —
//! é o binding base; a negociação de contexto e a ponte de frame (VkImage do
//! core → dma_buf → import no processo pai) entram nos passos seguintes.
#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_uint, c_void};

use ash::vk;

// --- RETRO_ENVIRONMENT_* específicos de Vulkan (libretro.h) ---
// Ambos carregam o bit EXPERIMENTAL (`| 0x10000`).
pub const RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE: c_uint =
    41 | crate::sys::RETRO_ENVIRONMENT_EXPERIMENTAL;
pub const RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE: c_uint =
    43 | crate::sys::RETRO_ENVIRONMENT_EXPERIMENTAL;

// --- enum retro_hw_render_interface_type (libretro.h) ---
pub const RETRO_HW_RENDER_INTERFACE_VULKAN: c_uint = 0;

// --- enum retro_hw_render_context_negotiation_interface_type (libretro.h) ---
pub const RETRO_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_VULKAN: c_uint = 0;

// --- #define (libretro_vulkan.h) ---
pub const RETRO_HW_RENDER_INTERFACE_VULKAN_VERSION: c_uint = 5;
pub const RETRO_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_VULKAN_VERSION: c_uint = 2;

/// Cabeçalho comum de `struct retro_hw_render_interface` (libretro.h) — o
/// frontend faz cast pelo `interface_type` pra versão concreta (`_vulkan`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_hw_render_interface {
    pub interface_type: c_uint,
    pub interface_version: c_uint,
}

/// Cabeçalho comum de `struct retro_hw_render_context_negotiation_interface`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_hw_render_context_negotiation_interface {
    pub interface_type: c_uint,
    pub interface_version: c_uint,
}

/// `struct retro_vulkan_image` (libretro_vulkan.h:32).
///
/// O core preenche isto e passa pro `set_image` antes de chamar
/// `retro_video_refresh(RETRO_HW_FRAME_BUFFER_VALID, ...)`. `create_info` é
/// copiado por valor (o `pNext` não pode ser deep-copied), então o ponteiro
/// precisa continuar válido até `retro_video_refresh` retornar.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_vulkan_image {
    pub image_view: vk::ImageView,
    pub image_layout: vk::ImageLayout,
    pub create_info: vk::ImageViewCreateInfo<'static>,
}

/// `struct retro_vulkan_context` (libretro_vulkan.h:57) — preenchido pelo core
/// no `create_device`/`create_device2`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_vulkan_context {
    pub gpu: vk::PhysicalDevice,
    pub device: vk::Device,
    pub queue: vk::Queue,
    pub queue_family_index: u32,
    pub presentation_queue: vk::Queue,
    pub presentation_queue_family_index: u32,
}

// --- callbacks do frontend chamados pelo core (libretro_vulkan.h:39-55) ---
// Todos recebem o `handle` opaco de `retro_hw_render_interface_vulkan`.

pub type retro_vulkan_set_image_t = Option<
    unsafe extern "C" fn(
        handle: *mut c_void,
        image: *const retro_vulkan_image,
        num_semaphores: u32,
        semaphores: *const vk::Semaphore,
        src_queue_family: u32,
    ),
>;
pub type retro_vulkan_get_sync_index_t = Option<unsafe extern "C" fn(handle: *mut c_void) -> u32>;
pub type retro_vulkan_get_sync_index_mask_t =
    Option<unsafe extern "C" fn(handle: *mut c_void) -> u32>;
pub type retro_vulkan_set_command_buffers_t =
    Option<unsafe extern "C" fn(handle: *mut c_void, num_cmd: u32, cmd: *const vk::CommandBuffer)>;
pub type retro_vulkan_wait_sync_index_t = Option<unsafe extern "C" fn(handle: *mut c_void)>;
pub type retro_vulkan_lock_queue_t = Option<unsafe extern "C" fn(handle: *mut c_void)>;
pub type retro_vulkan_unlock_queue_t = Option<unsafe extern "C" fn(handle: *mut c_void)>;
pub type retro_vulkan_set_signal_semaphore_t =
    Option<unsafe extern "C" fn(handle: *mut c_void, semaphore: vk::Semaphore)>;

/// `retro_vulkan_get_application_info_t` — devolve o `VkApplicationInfo` que o
/// core quer (controla a apiVersion do instance). v2: não pode ser NULL nem
/// retornar NULL.
pub type retro_vulkan_get_application_info_t =
    Option<unsafe extern "C" fn() -> *const vk::ApplicationInfo<'static>>;

// --- negociação de contexto (libretro_vulkan.h:69-113) ---

/// v1 (deprecated): o core cria/escolhe device e preenche `context`.
pub type retro_vulkan_create_device_t = Option<
    unsafe extern "C" fn(
        context: *mut retro_vulkan_context,
        instance: vk::Instance,
        gpu: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
        required_device_extensions: *const *const c_char,
        num_required_device_extensions: c_uint,
        required_device_layers: *const *const c_char,
        num_required_device_layers: c_uint,
        required_features: *const vk::PhysicalDeviceFeatures,
    ) -> bool,
>;

pub type retro_vulkan_destroy_device_t = Option<unsafe extern "C" fn()>;

/// v2: wrapper que o core DEVE usar em vez de `vkCreateInstance` direto.
pub type retro_vulkan_create_instance_wrapper_t = Option<
    unsafe extern "C" fn(
        opaque: *mut c_void,
        create_info: *const vk::InstanceCreateInfo<'static>,
    ) -> vk::Instance,
>;

/// v2: o core cria o `VkInstance` (via o wrapper) e o devolve.
pub type retro_vulkan_create_instance_t = Option<
    unsafe extern "C" fn(
        get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
        app: *const vk::ApplicationInfo<'static>,
        create_instance_wrapper: retro_vulkan_create_instance_wrapper_t,
        opaque: *mut c_void,
    ) -> vk::Instance,
>;

/// v2: wrapper que o core DEVE usar em vez de `vkCreateDevice` direto.
pub type retro_vulkan_create_device_wrapper_t = Option<
    unsafe extern "C" fn(
        gpu: vk::PhysicalDevice,
        opaque: *mut c_void,
        create_info: *const vk::DeviceCreateInfo<'static>,
    ) -> vk::Device,
>;

/// v2: tem precedência sobre `create_device` quando ambos os lados são >= v2.
pub type retro_vulkan_create_device2_t = Option<
    unsafe extern "C" fn(
        context: *mut retro_vulkan_context,
        instance: vk::Instance,
        gpu: vk::PhysicalDevice,
        surface: vk::SurfaceKHR,
        get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
        create_device_wrapper: retro_vulkan_create_device_wrapper_t,
        opaque: *mut c_void,
    ) -> bool,
>;

/// `struct retro_hw_render_context_negotiation_interface_vulkan`
/// (libretro_vulkan.h:116). O core passa isto via
/// `SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_hw_render_context_negotiation_interface_vulkan {
    /// = `RETRO_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_VULKAN`.
    pub interface_type: c_uint,
    /// <= `..._VULKAN_VERSION`.
    pub interface_version: c_uint,
    pub get_application_info: retro_vulkan_get_application_info_t,
    pub create_device: retro_vulkan_create_device_t,
    pub destroy_device: retro_vulkan_destroy_device_t,
    // Campos abaixo só válidos se `interface_version >= 2`.
    pub create_instance: retro_vulkan_create_instance_t,
    pub create_device2: retro_vulkan_create_device2_t,
}

/// `struct retro_hw_render_interface_vulkan` (libretro_vulkan.h:236). O
/// frontend preenche e devolve via `GET_HW_RENDER_INTERFACE`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_hw_render_interface_vulkan {
    /// = `RETRO_HW_RENDER_INTERFACE_VULKAN`.
    pub interface_type: c_uint,
    /// = `RETRO_HW_RENDER_INTERFACE_VULKAN_VERSION`.
    pub interface_version: c_uint,
    /// Handle opaco do backend Vulkan do frontend — repassado em TODOS os
    /// function pointers abaixo.
    pub handle: *mut c_void,
    pub instance: vk::Instance,
    pub gpu: vk::PhysicalDevice,
    pub device: vk::Device,
    pub get_device_proc_addr: vk::PFN_vkGetDeviceProcAddr,
    pub get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    /// Queue com suporte a graphics+compute; constante durante toda a vida do
    /// contexto.
    pub queue: vk::Queue,
    pub queue_index: c_uint,
    pub set_image: retro_vulkan_set_image_t,
    pub get_sync_index: retro_vulkan_get_sync_index_t,
    pub get_sync_index_mask: retro_vulkan_get_sync_index_mask_t,
    pub set_command_buffers: retro_vulkan_set_command_buffers_t,
    pub wait_sync_index: retro_vulkan_wait_sync_index_t,
    pub lock_queue: retro_vulkan_lock_queue_t,
    pub unlock_queue: retro_vulkan_unlock_queue_t,
    pub set_signal_semaphore: retro_vulkan_set_signal_semaphore_t,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_constants_carry_experimental_bit() {
        assert_eq!(RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE, 41 | 0x10000);
        assert_eq!(
            RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE,
            43 | 0x10000
        );
    }

    /// O frontend só pode fazer o cast pro tipo `_vulkan` depois de casar o
    /// cabeçalho comum; os offsets têm que bater.
    #[test]
    fn header_prefix_matches_concrete_structs() {
        use std::mem::offset_of;
        assert_eq!(
            offset_of!(retro_hw_render_interface_vulkan, interface_type),
            offset_of!(retro_hw_render_interface, interface_type),
        );
        assert_eq!(
            offset_of!(retro_hw_render_interface_vulkan, interface_version),
            offset_of!(retro_hw_render_interface, interface_version),
        );
    }
}
