//! Interop GL → Vulkan no Windows (tarefa B1 do TASKS): o core OpenGL roda no
//! processo filho e, em vez de ler o quadro de volta pra CPU, desenharia
//! direto numa imagem do Vulkan do app. Este módulo só decide SE dá pra fazer
//! isso numa máquina — a detecção; a partilha em si é B2–B5.
//!
//! Regras, da documentação oficial (Khronos):
//! - GL: as extensões `GL_EXT_memory_object` + `GL_EXT_semaphore`
//!   (EXT_external_objects.txt) e as variantes Win32
//!   `GL_EXT_memory_object_win32` + `GL_EXT_semaphore_win32`
//!   (EXT_external_objects_win32.txt). O GL expõe a identidade da GPU por
//!   `DEVICE_UUID_EXT` e `DRIVER_UUID_EXT` ("can be used to correlate devices
//!   and determine driver compatibility across process and API boundaries").
//! - Vulkan: `VK_KHR_external_memory_win32` + `VK_KHR_external_semaphore_win32`
//!   no device, e `VkPhysicalDeviceIDProperties`: "deviceUUID and/or
//!   driverUUID must be used to determine whether a particular external
//!   object can be shared".
//!
//! Logo: as quatro extensões GL, as duas Vulkan, e o UUID do device E o do
//! driver iguais dos dois lados.

use serde::{Deserialize, Serialize};

/// Tamanho dos UUIDs (`UUID_SIZE_EXT` = 16 no GL, `VK_UUID_SIZE` = 16).
pub const UUID_SIZE: usize = 16;

/// O que o Vulkan do app oferece pro interop (vai do pai pro filho no `Load`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VkInteropId {
    /// `VkPhysicalDeviceIDProperties::deviceUUID`.
    pub device_uuid: [u8; UUID_SIZE],
    /// `VkPhysicalDeviceIDProperties::driverUUID`.
    pub driver_uuid: [u8; UUID_SIZE],
    /// O device suporta `VK_KHR_external_memory_win32`.
    pub external_memory_win32: bool,
    /// O device suporta `VK_KHR_external_semaphore_win32`.
    pub external_semaphore_win32: bool,
}

/// O que o contexto GL do filho oferece.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlInteropCaps {
    pub memory_object: bool,
    pub memory_object_win32: bool,
    pub semaphore: bool,
    pub semaphore_win32: bool,
    /// `DEVICE_UUID_EXT` de cada device do contexto (`NUM_DEVICE_UUIDS_EXT`).
    pub device_uuids: Vec<[u8; UUID_SIZE]>,
    /// `DRIVER_UUID_EXT`.
    pub driver_uuid: Option<[u8; UUID_SIZE]>,
}

/// Por que o interop não dá (pra log), ou `Ok` se dá.
pub fn interop_compatible(
    gl: &GlInteropCaps,
    vk: Option<&VkInteropId>,
) -> Result<(), &'static str> {
    let Some(vk) = vk else {
        return Err("o app não informou o Vulkan (sem GPU Vulkan ou versão antiga)");
    };
    if !(vk.external_memory_win32 && vk.external_semaphore_win32) {
        return Err("Vulkan sem VK_KHR_external_memory_win32/external_semaphore_win32");
    }
    if !(gl.memory_object && gl.semaphore) {
        return Err("GL sem GL_EXT_memory_object/GL_EXT_semaphore");
    }
    if !(gl.memory_object_win32 && gl.semaphore_win32) {
        return Err("GL sem GL_EXT_memory_object_win32/GL_EXT_semaphore_win32");
    }
    if !gl.device_uuids.contains(&vk.device_uuid) {
        return Err("GPU diferente (deviceUUID do GL não bate com o do Vulkan)");
    }
    if gl.driver_uuid != Some(vk.driver_uuid) {
        return Err("driver diferente (driverUUID do GL não bate com o do Vulkan)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gl_ok() -> GlInteropCaps {
        GlInteropCaps {
            memory_object: true,
            memory_object_win32: true,
            semaphore: true,
            semaphore_win32: true,
            device_uuids: vec![[1; 16]],
            driver_uuid: Some([2; 16]),
        }
    }

    fn vk_ok() -> VkInteropId {
        VkInteropId {
            device_uuid: [1; 16],
            driver_uuid: [2; 16],
            external_memory_win32: true,
            external_semaphore_win32: true,
        }
    }

    #[test]
    fn same_gpu_and_driver_with_all_extensions_is_compatible() {
        assert_eq!(interop_compatible(&gl_ok(), Some(&vk_ok())), Ok(()));
    }

    #[test]
    fn any_missing_piece_says_why() {
        assert!(interop_compatible(&gl_ok(), None).is_err());

        let mut vk = vk_ok();
        vk.external_semaphore_win32 = false;
        assert!(interop_compatible(&gl_ok(), Some(&vk))
            .unwrap_err()
            .contains("Vulkan"));

        let mut gl = gl_ok();
        gl.semaphore_win32 = false;
        assert!(interop_compatible(&gl, Some(&vk_ok()))
            .unwrap_err()
            .contains("win32"));

        let mut gl = gl_ok();
        gl.device_uuids = vec![[9; 16]];
        assert!(interop_compatible(&gl, Some(&vk_ok()))
            .unwrap_err()
            .contains("GPU"));

        let mut gl = gl_ok();
        gl.driver_uuid = Some([9; 16]);
        assert!(interop_compatible(&gl, Some(&vk_ok()))
            .unwrap_err()
            .contains("driver"));
    }

    #[test]
    fn matches_any_device_of_the_gl_context() {
        // contexto com mais de um device (NUM_DEVICE_UUIDS_EXT > 1)
        let mut gl = gl_ok();
        gl.device_uuids = vec![[7; 16], [1; 16]];
        assert_eq!(interop_compatible(&gl, Some(&vk_ok())), Ok(()));
    }
}
