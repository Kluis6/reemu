//! Hook do `vkCreateDevice` na negociação Vulkan com o core (etapa 12).
//!
//! No caminho core-owned (`from_core_negotiation`) quem cria o `VkDevice` é o
//! `create_device` do core, e o wgpu adota esse device depois. O wgpu precisa
//! saber EXATAMENTE quais extensões foram ligadas, e precisa das dele:
//!
//! - o Beetle PSX HW honra o `required_device_extensions`;
//! - o flycast o IGNORA (`VkCreateDevice` em
//!   `core/rend/vulkan/vk_context_lr.cpp` liga só as que ele escolhe) — e é
//!   nesse mesmo `create_device` que ele inicializa o despachante dinâmico do
//!   `vulkan.hpp`, então não dá pra pular a chamada (pular = todo ponteiro
//!   Vulkan dele nulo = SIGSEGV no `VulkanContext::init`).
//!
//! O core carrega o `vkCreateDevice` pelo `vkGetInstanceProcAddr` que NÓS
//! passamos no `create_device`. Entregamos este aqui: repassa tudo ao
//! verdadeiro, menos `"vkCreateDevice"`, que vira [`create_device`]. O hook
//! soma as extensões e as features do wgpu ao `VkDeviceCreateInfo` do core e
//! registra a lista final.
//!
//! Regras da spec Vulkan seguidas (capítulo "Devices and Queues",
//! `VkDeviceCreateInfo`):
//! - VUID-VkDeviceCreateInfo-pNext-00373: com `VkPhysicalDeviceFeatures2` no
//!   `pNext`, `pEnabledFeatures` tem que ser `NULL` → as features do wgpu
//!   entram NA `VkPhysicalDeviceFeatures2` que já existe;
//! - extensões sem duplicar; VUID-...-03328 (KHR e EXT
//!   `buffer_device_address` juntas) → a do wgpu é pulada se o core já ligou
//!   a outra variante.
//!
//! `VkPhysicalDeviceFeatures` só tem membros `VkBool32` (spec), então o OU é
//! feito membro a membro como `u32`.
//!
//! Estado global: o callback C não tem ponteiro de usuário. Só uma
//! negociação por vez (o `emu-session` serializa o load).

use ash::vk;
use std::ffi::{c_char, CStr, CString};
use std::sync::Mutex;

struct Armed {
    /// `vkCreateDevice` verdadeiro, resolvido com a instância que o core
    /// passou (a spec só devolve comandos globais com instância `NULL`).
    real_create_device: Option<vk::PFN_vkCreateDevice>,
    want_exts: Vec<CString>,
    want_feats: vk::PhysicalDeviceFeatures,
    /// Preenchido pelo hook: extensões realmente ligadas.
    enabled: Option<Vec<CString>>,
}

// SAFETY: só ponteiros de função e dados próprios; acesso sob o Mutex.
unsafe impl Send for Armed {}

static STATE: Mutex<Option<Armed>> = Mutex::new(None);

/// `vkGetInstanceProcAddr` verdadeiro, pra SEMPRE: o core guarda o nosso
/// (o despachante do `vulkan.hpp` do flycast e o VMA dele carregam funções
/// por ele pela vida toda do core), então o repasse não pode depender do
/// hook estar armado — só a troca do `vkCreateDevice` depende.
static REAL_GIPA: Mutex<Option<vk::PFN_vkGetInstanceProcAddr>> = Mutex::new(None);

/// Arma o hook antes de chamar o `create_device` do core.
pub(super) fn arm(
    real_gipa: vk::PFN_vkGetInstanceProcAddr,
    want_exts: &[&CStr],
    want_feats: vk::PhysicalDeviceFeatures,
) {
    *REAL_GIPA.lock().unwrap_or_else(|p| p.into_inner()) = Some(real_gipa);
    *STATE.lock().unwrap_or_else(|p| p.into_inner()) = Some(Armed {
        real_create_device: None,
        want_exts: want_exts.iter().map(|e| (*e).to_owned()).collect(),
        want_feats,
        enabled: None,
    });
}

/// Desarma e devolve a lista de extensões ligadas, se o hook viu o
/// `vkCreateDevice`. Os nomes viram `'static` (vazados — poucos bytes, uma
/// vez por negociação) porque o `AdoptedVulkan` guarda `&'static CStr`.
pub(super) fn disarm() -> Option<Vec<&'static CStr>> {
    let armed = STATE.lock().unwrap_or_else(|p| p.into_inner()).take()?;
    armed.enabled.map(|list| {
        list.into_iter()
            .map(|c| &*Box::leak(c.into_boxed_c_str()))
            .collect()
    })
}

/// `PFN_vkGetInstanceProcAddr` entregue ao core.
pub(super) unsafe extern "system" fn get_instance_proc_addr(
    instance: vk::Instance,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    let real = (*REAL_GIPA.lock().unwrap_or_else(|p| p.into_inner()))?;
    let armed = STATE.lock().unwrap_or_else(|p| p.into_inner()).is_some();
    if armed && !name.is_null() && unsafe { CStr::from_ptr(name) } == c"vkCreateDevice" {
        let real_cd = (unsafe { real(instance, name) })?;
        if let Some(a) = STATE.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
            // SAFETY: ponteiro do loader pra "vkCreateDevice".
            a.real_create_device = Some(unsafe {
                std::mem::transmute::<unsafe extern "system" fn(), vk::PFN_vkCreateDevice>(real_cd)
            });
        }
        // SAFETY: mesma assinatura de `PFN_vkCreateDevice`, devolvida como
        // `PFN_vkVoidFunction` (o chamador converte de volta).
        return Some(unsafe {
            std::mem::transmute::<vk::PFN_vkCreateDevice, unsafe extern "system" fn()>(
                create_device,
            )
        });
    }
    unsafe { real(instance, name) }
}

/// OU membro a membro (todos `VkBool32`).
fn or_features(dst: &mut vk::PhysicalDeviceFeatures, src: &vk::PhysicalDeviceFeatures) {
    const N: usize = std::mem::size_of::<vk::PhysicalDeviceFeatures>() / 4;
    // SAFETY: a struct é `repr(C)` só de `Bool32` (u32) — spec Vulkan.
    let d = unsafe { &mut *(dst as *mut _ as *mut [u32; N]) };
    let s = unsafe { &*(src as *const _ as *const [u32; N]) };
    for (a, b) in d.iter_mut().zip(s) {
        *a |= *b;
    }
}

/// Soma `want` a `have` sem duplicar; pula uma variante de
/// `buffer_device_address` se a outra já estiver (VUID-...-03328).
fn merge_extensions(have: &[CString], want: &[CString]) -> Vec<CString> {
    const BDA: [&CStr; 2] = [
        c"VK_KHR_buffer_device_address",
        c"VK_EXT_buffer_device_address",
    ];
    let mut out: Vec<CString> = have.to_vec();
    for w in want {
        if out.iter().any(|h| h == w) {
            continue;
        }
        let is_bda = BDA.contains(&w.as_c_str());
        if is_bda && out.iter().any(|h| BDA.contains(&h.as_c_str())) {
            continue;
        }
        out.push(w.clone());
    }
    out
}

unsafe extern "system" fn create_device(
    physical_device: vk::PhysicalDevice,
    p_create_info: *const vk::DeviceCreateInfo<'_>,
    p_allocator: *const vk::AllocationCallbacks<'_>,
    p_device: *mut vk::Device,
) -> vk::Result {
    // Copia o necessário e SOLTA o lock antes de chamar o driver.
    let (real, want_exts, want_feats) = {
        let guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
        let Some(armed) = guard.as_ref() else {
            return vk::Result::ERROR_INITIALIZATION_FAILED;
        };
        let Some(real) = armed.real_create_device else {
            return vk::Result::ERROR_INITIALIZATION_FAILED;
        };
        (real, armed.want_exts.clone(), armed.want_feats)
    };
    let info = unsafe { &*p_create_info };

    // Extensões: as do core + as do wgpu.
    let have: Vec<CString> = (0..info.enabled_extension_count as usize)
        .map(|i| unsafe { CStr::from_ptr(*info.pp_enabled_extension_names.add(i)) }.to_owned())
        .collect();
    let merged = merge_extensions(&have, &want_exts);
    let merged_ptrs: Vec<*const c_char> = merged.iter().map(|c| c.as_ptr()).collect();

    // Features: dentro da `VkPhysicalDeviceFeatures2` do pNext se houver
    // (VUID-00373), senão no `pEnabledFeatures` (cópia nossa).
    let mut feats2_found = false;
    let mut next = info.p_next as *mut vk::BaseOutStructure<'_>;
    while !next.is_null() {
        // SAFETY: cadeia `pNext` válida do core; só lemos `s_type`/`p_next`
        // e, no tipo certo, escrevemos nos `VkBool32` da própria struct.
        let base = unsafe { &mut *next };
        if base.s_type == vk::StructureType::PHYSICAL_DEVICE_FEATURES_2 {
            let f2 = unsafe { &mut *(next as *mut vk::PhysicalDeviceFeatures2<'_>) };
            or_features(&mut f2.features, &want_feats);
            feats2_found = true;
        }
        next = base.p_next;
    }
    let mut feats = if info.p_enabled_features.is_null() {
        vk::PhysicalDeviceFeatures::default()
    } else {
        unsafe { *info.p_enabled_features }
    };
    if !feats2_found {
        or_features(&mut feats, &want_feats);
    }

    let mut patched = *info;
    patched.enabled_extension_count = merged_ptrs.len() as u32;
    patched.pp_enabled_extension_names = merged_ptrs.as_ptr();
    if !feats2_found {
        patched.p_enabled_features = &feats;
    }
    let r = unsafe { real(physical_device, &patched, p_allocator, p_device) };
    if r == vk::Result::SUCCESS {
        if let Some(a) = STATE.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
            a.enabled = Some(merged);
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_dedups_and_respects_bda_variants() {
        let have = vec![
            CString::from(c"VK_KHR_swapchain"),
            CString::from(c"VK_KHR_buffer_device_address"),
        ];
        let want = vec![
            CString::from(c"VK_KHR_swapchain"),
            CString::from(c"VK_EXT_buffer_device_address"),
            CString::from(c"VK_KHR_external_memory_fd"),
        ];
        let out = merge_extensions(&have, &want);
        let names: Vec<&str> = out.iter().map(|c| c.to_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                "VK_KHR_swapchain",
                "VK_KHR_buffer_device_address",
                "VK_KHR_external_memory_fd"
            ]
        );
    }

    #[test]
    fn features_are_ored() {
        let mut a = vk::PhysicalDeviceFeatures {
            sampler_anisotropy: vk::TRUE,
            ..Default::default()
        };
        let b = vk::PhysicalDeviceFeatures {
            independent_blend: vk::TRUE,
            ..Default::default()
        };
        or_features(&mut a, &b);
        assert_eq!(a.sampler_anisotropy, vk::TRUE);
        assert_eq!(a.independent_blend, vk::TRUE);
        assert_eq!(a.geometry_shader, vk::FALSE);
    }
}
