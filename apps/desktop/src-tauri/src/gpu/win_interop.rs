//! Interop GL → Vulkan no Windows, lado do app (tarefa B2 do TASKS).
//!
//! O core OpenGL roda no processo filho e o GL não sabe EXPORTAR memória
//! (EXT_external_objects só importa), então quem aloca é o Vulkan daqui: um
//! anel de imagens exportáveis, cada uma com um handle NT do Windows que é
//! duplicado pro processo do core. O filho importa (B3), desenha nelas e
//! avisa por semáforo (B4); o app amostra sem cópia (B5).
//!
//! Desligado por padrão até o B5: só com `REEMU_GL_INTEROP=1`.
//!
//! Regras da especificação Vulkan seguidas (docs.vulkan.org):
//! - a imagem declara o tipo de handle em `VkExternalMemoryImageCreateInfo`;
//! - "handleType must have been included in
//!   VkExportMemoryAllocateInfo::handleTypes when memory was created"
//!   (VUID-VkMemoryGetWin32HandleInfoKHR-handleType-00662);
//! - handle NT: `vkGetMemoryWin32HandleKHR` "no more than once" por memória e
//!   tipo (VUID-...-00663), e o handle é do app — "must release ownership of
//!   them using the CloseHandle system call";
//! - `VK_EXTERNAL_MEMORY_FEATURE_DEDICATED_ONLY_BIT` exige
//!   `VkMemoryDedicatedAllocateInfo`; aqui a alocação é SEMPRE dedicada
//!   (permitido em qualquer caso desde o Vulkan 1.1), e o filho é avisado
//!   (o GL pede `DEDICATED_MEMORY_OBJECT_EXT` nesse caso — B3).
//!
//! Sincronização (B4), no molde do exemplo oficial `open_gl_interop` do
//! Vulkan-Samples: dois semáforos binários exportáveis por imagem — `done`
//! (o GL sinaliza ao terminar o quadro, o app espera antes de amostrar) e
//! `free` (o app sinaliza ao terminar de ler, o GL espera antes de desenhar
//! de novo). Handles NT: "owned by the application ... must release
//! ownership of them using the CloseHandle system call"
//! (`vkGetSemaphoreWin32HandleKHR`), um export por semáforo
//! (VUID-VkSemaphoreGetWin32HandleInfoKHR-handleType-01127). A imagem passa
//! de um lado pro outro em `SHADER_READ_ONLY_OPTIMAL` (= `TextureUses::RESOURCE`
//! do wgpu, = `GL_LAYOUT_SHADER_READ_ONLY_EXT`).

use ash::vk;
use windows_sys::Win32::Foundation::{CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE};

const HANDLE_TYPE: vk::ExternalMemoryHandleTypeFlags =
    vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32;
const SEM_HANDLE_TYPE: vk::ExternalSemaphoreHandleTypeFlags =
    vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_WIN32;
/// Formato do quadro do core (o FBO do GL é RGBA8).
const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

/// O interop está pedido? (`REEMU_GL_INTEROP=1`.)
pub fn requested() -> bool {
    std::env::var("REEMU_GL_INTEROP").is_ok_and(|v| v.trim() == "1")
}

/// Abre o device com `VK_KHR_external_memory_win32` e
/// `VK_KHR_external_semaphore_win32` (mesmo molde do `sync_fd` do Linux).
/// `None` = não pedido, não é Vulkan ou a GPU não tem as extensões — o
/// chamador cai no device de sempre.
pub(super) fn create_device(
    instance: &wgpu::Instance,
    wanted: wgpu::Features,
) -> Option<(wgpu::Adapter, wgpu::Device, wgpu::Queue, wgpu::Features)> {
    if !requested() {
        return None;
    }
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    }))
    .ok()?;
    if adapter.get_info().backend != wgpu::Backend::Vulkan {
        return None;
    }
    let exts = [
        ash::khr::external_memory_win32::NAME,
        ash::khr::external_semaphore_win32::NAME,
    ];
    let features = wanted & adapter.features();
    let limits = video_surface::device_limits(&adapter);
    let open = {
        let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
        let caps = hal.physical_device_capabilities();
        if !exts.iter().all(|e| caps.supports_extension(e)) {
            log::info!("interop GL→Vulkan: GPU sem as extensões Win32 — device de sempre");
            return None;
        }
        // SAFETY: a callback só ACRESCENTA extensões que o device suporta
        // (conferido logo acima).
        unsafe {
            hal.open_with_callback(
                features,
                &limits,
                &wgpu::MemoryHints::default(),
                Some(Box::new(move |args| {
                    for e in exts {
                        if !args.extensions.contains(&e) {
                            args.extensions.push(e);
                        }
                    }
                })),
            )
        }
        .inspect_err(|e| log::warn!("device com as extensões Win32: {e}"))
        .ok()?
    };
    // SAFETY: `open` veio deste `adapter`, com exatamente `features`.
    let (device, queue) = unsafe {
        adapter.create_device_from_hal(
            open,
            &wgpu::DeviceDescriptor {
                label: Some("video-surface device (interop win32)"),
                required_features: features,
                required_limits: limits,
                ..Default::default()
            },
        )
    }
    .inspect_err(|e| log::warn!("create_device_from_hal: {e}"))
    .ok()?;
    log::info!("interop GL→Vulkan: device com external_memory_win32 + external_semaphore_win32");
    Some((adapter, device, queue, features))
}

/// Uma imagem do anel: a textura do wgpu (dona da `VkImage` e da memória) e
/// o handle NT exportado (do app até ser duplicado e fechado).
pub struct ExportedImage {
    /// Dona da `VkImage` e da memória (soltar destrói as duas). Só é LIDA no
    /// B5, quando o app amostrar o quadro que o core desenhou aqui.
    #[allow(dead_code)]
    pub texture: wgpu::Texture,
    /// Handle NT como número (`HANDLE` é ponteiro cru e não é `Send`; o
    /// `FrameProcessor` passa entre threads).
    pub handle: usize,
    /// Tamanho da alocação — o GL precisa dele no import (`glImportMemoryWin32HandleEXT`).
    pub alloc_size: u64,
    /// O GL sinaliza ao terminar de desenhar; o app espera antes de amostrar.
    /// Destruído junto com a textura (callback de drop do wgpu, depois da
    /// GPU largar a imagem).
    #[allow(dead_code)] // lido no B5 (e no teste do B4)
    pub done_sem: vk::Semaphore,
    /// O app sinaliza ao terminar de ler; o GL espera antes de redesenhar.
    #[allow(dead_code)] // lido no B5 (e no teste do B4)
    pub free_sem: vk::Semaphore,
    /// Handles NT exportados de `done_sem`/`free_sem`.
    pub done_handle: usize,
    pub free_handle: usize,
}

impl Drop for ExportedImage {
    fn drop(&mut self) {
        for h in [self.handle, self.done_handle, self.free_handle] {
            if h != 0 {
                // SAFETY: handle NT nosso, ainda não fechado.
                unsafe { CloseHandle(h as HANDLE) };
            }
        }
    }
}

/// Semáforo binário exportável como handle NT opaco + o handle.
fn exportable_semaphore(
    raw: &ash::Device,
    win32: &ash::khr::external_semaphore_win32::Device,
) -> Result<(vk::Semaphore, usize), String> {
    let mut export = vk::ExportSemaphoreCreateInfo::default().handle_types(SEM_HANDLE_TYPE);
    let info = vk::SemaphoreCreateInfo::default().push_next(&mut export);
    let sem = unsafe { raw.create_semaphore(&info, None) }
        .map_err(|e| format!("vkCreateSemaphore (export): {e}"))?;
    let get = vk::SemaphoreGetWin32HandleInfoKHR::default()
        .semaphore(sem)
        .handle_type(SEM_HANDLE_TYPE);
    match unsafe { win32.get_semaphore_win32_handle(&get) } {
        Ok(h) => Ok((sem, h as usize)),
        Err(e) => {
            unsafe { raw.destroy_semaphore(sem, None) };
            Err(format!("vkGetSemaphoreWin32HandleKHR: {e}"))
        }
    }
}

/// Usos de imagem que as features do formato permitem (o mapa feature → uso
/// das "Format Feature" da spec Vulkan). `INPUT_ATTACHMENT` vale pra formato
/// de color attachment.
fn usage_from_features(f: vk::FormatFeatureFlags) -> vk::ImageUsageFlags {
    use vk::{FormatFeatureFlags as F, ImageUsageFlags as U};
    let mut u = U::empty();
    for (feat, usage) in [
        (F::SAMPLED_IMAGE, U::SAMPLED),
        (F::STORAGE_IMAGE, U::STORAGE),
        (
            F::COLOR_ATTACHMENT,
            U::COLOR_ATTACHMENT | U::INPUT_ATTACHMENT,
        ),
        (F::TRANSFER_SRC, U::TRANSFER_SRC),
        (F::TRANSFER_DST, U::TRANSFER_DST),
    ] {
        if f.contains(feat) {
            u |= usage;
        }
    }
    u
}

/// A GPU exporta `FORMAT` com esses flags/usos como handle NT opaco?
/// (`vkGetPhysicalDeviceImageFormatProperties2` +
/// `VkPhysicalDeviceExternalImageFormatInfo` → `VkExternalImageFormatProperties`.)
fn exportable(
    instance: &ash::Instance,
    phys: vk::PhysicalDevice,
    flags: vk::ImageCreateFlags,
    usage: vk::ImageUsageFlags,
) -> bool {
    let mut ext_info =
        vk::PhysicalDeviceExternalImageFormatInfo::default().handle_type(HANDLE_TYPE);
    let info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(FORMAT)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(usage)
        .flags(flags)
        .push_next(&mut ext_info);
    let mut ext_props = vk::ExternalImageFormatProperties::default();
    let mut props = vk::ImageFormatProperties2::default().push_next(&mut ext_props);
    unsafe { instance.get_physical_device_image_format_properties2(phys, &info, &mut props) }
        .is_ok()
        && ext_props
            .external_memory_properties
            .external_memory_features
            .contains(vk::ExternalMemoryFeatureFlags::EXPORTABLE)
}

/// Aloca `n` imagens `w`×`h` exportáveis como handle NT opaco.
pub(super) fn allocate_ring(
    device: &wgpu::Device,
    adapter: &wgpu::Adapter,
    n: usize,
    w: u32,
    h: u32,
) -> Result<Vec<ExportedImage>, String> {
    let hal_adapter =
        unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }.ok_or("adapter não é Vulkan")?;
    let instance = hal_adapter.shared_instance().raw_instance().clone();
    let phys = hal_adapter.raw_physical_device();
    let hal_dev =
        unsafe { device.as_hal::<wgpu::hal::api::Vulkan>() }.ok_or("device não é Vulkan")?;
    let raw = hal_dev.raw_device().clone();
    // A imagem vai virar textura no GL do filho (B3). EXT_external_objects,
    // issue 13: "Require all supported usage flags be specified by the
    // application on the Vulkan side if the image is intended to alias with
    // an OpenGL texture" — então os usos saem das features do formato
    // (`vkGetPhysicalDeviceFormatProperties`, `optimalTilingFeatures`).
    // Issue 17: com GL ≥ 4.3 / texture_view no outro lado, "Vulkan
    // applications will be required to set the mutable format bit" →
    // `VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT`.
    let flags = vk::ImageCreateFlags::MUTABLE_FORMAT;
    let fmt = unsafe { instance.get_physical_device_format_properties(phys, FORMAT) };
    let usage = usage_from_features(fmt.optimal_tiling_features);
    let base = vk::ImageUsageFlags::COLOR_ATTACHMENT
        | vk::ImageUsageFlags::SAMPLED
        | vk::ImageUsageFlags::TRANSFER_SRC
        | vk::ImageUsageFlags::TRANSFER_DST;
    // Sem export com todos os usos, tenta os mínimos (o GL pode recusar —
    // aí o filho cai no readback).
    let usage = if exportable(&instance, phys, flags, usage) {
        usage
    } else if exportable(&instance, phys, flags, base) {
        log::warn!(
            "interop GL→Vulkan: export com todos os usos ({usage:?}) recusado — só {base:?}"
        );
        base
    } else {
        return Err("a GPU não exporta essa imagem como handle Win32 opaco".into());
    };

    // O semáforo exporta como handle NT opaco?
    // (`vkGetPhysicalDeviceExternalSemaphoreProperties`.)
    let sem_props = unsafe {
        let info = vk::PhysicalDeviceExternalSemaphoreInfo::default().handle_type(SEM_HANDLE_TYPE);
        let mut props = vk::ExternalSemaphoreProperties::default();
        instance.get_physical_device_external_semaphore_properties(phys, &info, &mut props);
        props
    };
    if !sem_props
        .external_semaphore_features
        .contains(vk::ExternalSemaphoreFeatureFlags::EXPORTABLE)
    {
        return Err("a GPU não exporta semáforo como handle Win32 opaco".into());
    }
    let sem_win32 = ash::khr::external_semaphore_win32::Device::new(&instance, &raw);

    let mem_props = unsafe { instance.get_physical_device_memory_properties(phys) };
    let win32 = ash::khr::external_memory_win32::Device::new(&instance, &raw);
    let mut ring = Vec::with_capacity(n);
    for i in 0..n {
        let mut ext_img = vk::ExternalMemoryImageCreateInfo::default().handle_types(HANDLE_TYPE);
        let img_info = vk::ImageCreateInfo::default()
            .flags(flags)
            .image_type(vk::ImageType::TYPE_2D)
            .format(FORMAT)
            .extent(vk::Extent3D {
                width: w,
                height: h,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .push_next(&mut ext_img);
        let image = unsafe { raw.create_image(&img_info, None) }
            .map_err(|e| format!("vkCreateImage: {e}"))?;
        let req = unsafe { raw.get_image_memory_requirements(image) };
        let Some(type_index) = (0..mem_props.memory_type_count).find(|&t| {
            req.memory_type_bits & (1 << t) != 0
                && mem_props.memory_types[t as usize]
                    .property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        }) else {
            unsafe { raw.destroy_image(image, None) };
            return Err("nenhum tipo de memória DEVICE_LOCAL pra imagem".into());
        };
        let mut export = vk::ExportMemoryAllocateInfo::default().handle_types(HANDLE_TYPE);
        let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(req.size)
            .memory_type_index(type_index)
            .push_next(&mut export)
            .push_next(&mut dedicated);
        let memory = match unsafe { raw.allocate_memory(&alloc, None) } {
            Ok(m) => m,
            Err(e) => {
                unsafe { raw.destroy_image(image, None) };
                return Err(format!("vkAllocateMemory: {e}"));
            }
        };
        let cleanup = |raw: &ash::Device| unsafe {
            raw.destroy_image(image, None);
            raw.free_memory(memory, None);
        };
        if let Err(e) = unsafe { raw.bind_image_memory(image, memory, 0) } {
            cleanup(&raw);
            return Err(format!("vkBindImageMemory: {e}"));
        }
        let get = vk::MemoryGetWin32HandleInfoKHR::default()
            .memory(memory)
            .handle_type(HANDLE_TYPE);
        let handle = match unsafe { win32.get_memory_win32_handle(&get) } {
            Ok(h) => h as usize,
            Err(e) => {
                cleanup(&raw);
                return Err(format!("vkGetMemoryWin32HandleKHR: {e}"));
            }
        };
        let close = |h: usize| unsafe {
            CloseHandle(h as HANDLE);
        };
        let (done_sem, done_handle) = match exportable_semaphore(&raw, &sem_win32) {
            Ok(s) => s,
            Err(e) => {
                close(handle);
                cleanup(&raw);
                return Err(e);
            }
        };
        let (free_sem, free_handle) = match exportable_semaphore(&raw, &sem_win32) {
            Ok(s) => s,
            Err(e) => {
                close(handle);
                close(done_handle);
                unsafe { raw.destroy_semaphore(done_sem, None) };
                cleanup(&raw);
                return Err(e);
            }
        };

        // A textura do wgpu passa a ser a dona: destrói imagem e memória
        // quando for solta.
        let raw_drop = raw.clone();
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: Some("interop win32"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUses::RESOURCE,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        // SAFETY: `image` acabou de ser criada neste device e ligada à memória.
        let hal_tex = unsafe {
            hal_dev.texture_from_raw(
                image,
                &hal_desc,
                Some(Box::new(move || {
                    raw_drop.destroy_image(image, None);
                    raw_drop.free_memory(memory, None);
                    raw_drop.destroy_semaphore(done_sem, None);
                    raw_drop.destroy_semaphore(free_sem, None);
                })),
                wgpu::hal::vulkan::TextureMemory::External,
            )
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("interop win32"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        // SAFETY: `hal_tex` recém-embrulhado deste device.
        let texture = unsafe {
            device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                hal_tex,
                &desc,
                wgpu::TextureUses::RESOURCE,
            )
        };
        log::debug!("interop win32: imagem {i} {w}x{h}, {} bytes", req.size);
        ring.push(ExportedImage {
            texture,
            handle,
            alloc_size: req.size,
            done_sem,
            free_sem,
            done_handle,
            free_handle,
        });
    }
    Ok(ring)
}

/// Duplica `handle` pro processo `pid` (o `reemu-core-host`) e devolve o
/// valor do handle LÁ — é esse número que vai na mensagem. O nosso continua
/// aberto (fecha no `Drop` da `ExportedImage`).
pub fn duplicate_into(pid: u32, handle: usize) -> Result<u64, String> {
    // SAFETY: chamadas Win32 com handles válidos; o do processo é fechado.
    unsafe {
        let proc = OpenProcess(PROCESS_DUP_HANDLE, 0, pid);
        if proc.is_null() {
            return Err(format!(
                "OpenProcess({pid}): {}",
                std::io::Error::last_os_error()
            ));
        }
        let mut out: HANDLE = std::ptr::null_mut();
        let ok = DuplicateHandle(
            GetCurrentProcess(),
            handle as HANDLE,
            proc,
            &mut out,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        );
        let err = std::io::Error::last_os_error();
        CloseHandle(proc);
        if ok == 0 {
            return Err(format!("DuplicateHandle: {err}"));
        }
        Ok(out as u64)
    }
}
