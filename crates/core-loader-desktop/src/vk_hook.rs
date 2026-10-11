//! Hook mínimo do `vkCreateDevice` pro `create_device` do core no processo
//! filho (C3b): soma as extensões do interop ao `VkDeviceCreateInfo` que o
//! core monta. O Beetle honra o `required_device_extensions`; o flycast o
//! ignora e liga só as dele (`core/rend/vulkan/vk_context_lr.cpp`) — sem
//! isto, o device dele não teria `VK_KHR_external_memory_win32` e o anel
//! não importaria. Mesma ideia do `device_hook` do app (que também soma
//! features do wgpu), reduzida às extensões.
//!
//! O core carrega o `vkCreateDevice` pelo `vkGetInstanceProcAddr` que
//! passamos no `create_device`: este repassa tudo ao verdadeiro, menos
//! `"vkCreateDevice"`. Extensões sem duplicar ("ppEnabledExtensionNames must
//! not contain duplicates", `VkDeviceCreateInfo`). Estado global: o callback
//! C não tem ponteiro de usuário; um core por processo.

use ash::vk;
use std::ffi::{c_char, CStr, CString};
use std::sync::Mutex;

struct Armed {
    real_create_device: Option<vk::PFN_vkCreateDevice>,
    add: Vec<CString>,
    /// Preenchido pelo hook: as extensões realmente ligadas.
    enabled: Option<Vec<CString>>,
}

// SAFETY: só ponteiros de função e dados próprios, sob o Mutex.
unsafe impl Send for Armed {}

static STATE: Mutex<Option<Armed>> = Mutex::new(None);
/// `vkGetInstanceProcAddr` verdadeiro, pra sempre: o core guarda o nosso
/// pela vida toda dele (o despachante do flycast carrega tudo por ele).
static REAL_GIPA: Mutex<Option<vk::PFN_vkGetInstanceProcAddr>> = Mutex::new(None);

/// Arma o hook antes de chamar o `create_device` do core.
pub(crate) fn arm(real_gipa: vk::PFN_vkGetInstanceProcAddr, add: &[&CStr]) {
    *REAL_GIPA.lock().unwrap_or_else(|p| p.into_inner()) = Some(real_gipa);
    *STATE.lock().unwrap_or_else(|p| p.into_inner()) = Some(Armed {
        real_create_device: None,
        add: add.iter().map(|e| (*e).to_owned()).collect(),
        enabled: None,
    });
}

/// Desarma e devolve as extensões ligadas, se o hook viu o `vkCreateDevice`.
pub(crate) fn disarm() -> Option<Vec<CString>> {
    STATE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()?
        .enabled
}

/// `PFN_vkGetInstanceProcAddr` entregue ao core.
pub(crate) unsafe extern "system" fn get_instance_proc_addr(
    instance: vk::Instance,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    let real = (*REAL_GIPA.lock().unwrap_or_else(|p| p.into_inner()))?;
    let armed = STATE.lock().unwrap_or_else(|p| p.into_inner()).is_some();
    if armed && !name.is_null() && CStr::from_ptr(name) == c"vkCreateDevice" {
        let real_cd = real(instance, name)?;
        if let Some(a) = STATE.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
            // SAFETY: ponteiro do loader pra "vkCreateDevice".
            a.real_create_device = Some(std::mem::transmute::<
                unsafe extern "system" fn(),
                vk::PFN_vkCreateDevice,
            >(real_cd));
        }
        // SAFETY: mesma assinatura de `PFN_vkCreateDevice`.
        return Some(std::mem::transmute::<
            vk::PFN_vkCreateDevice,
            unsafe extern "system" fn(),
        >(create_device));
    }
    real(instance, name)
}

/// `vkCreateDevice` do core, com as extensões do interop somadas.
unsafe extern "system" fn create_device(
    physical_device: vk::PhysicalDevice,
    create_info: *const vk::DeviceCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    device: *mut vk::Device,
) -> vk::Result {
    let (real, add) = {
        let g = STATE.lock().unwrap_or_else(|p| p.into_inner());
        match g.as_ref() {
            Some(a) => (a.real_create_device, a.add.clone()),
            None => (None, Vec::new()),
        }
    };
    let Some(real) = real else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    if create_info.is_null() {
        return real(physical_device, create_info, allocator, device);
    }
    let ci = *create_info;
    let mut names: Vec<*const c_char> = if ci.enabled_extension_count > 0 {
        std::slice::from_raw_parts(
            ci.pp_enabled_extension_names,
            ci.enabled_extension_count as usize,
        )
        .to_vec()
    } else {
        Vec::new()
    };
    for e in &add {
        if !names.iter().any(|&n| CStr::from_ptr(n) == e.as_c_str()) {
            names.push(e.as_ptr());
        }
    }
    let mut patched = ci;
    patched.enabled_extension_count = names.len() as u32;
    patched.pp_enabled_extension_names = names.as_ptr();
    let r = real(physical_device, &patched, allocator, device);
    if r == vk::Result::SUCCESS {
        let list = names
            .iter()
            .map(|&n| CStr::from_ptr(n).to_owned())
            .collect();
        if let Some(a) = STATE.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
            a.enabled = Some(list);
        }
    }
    r
}
