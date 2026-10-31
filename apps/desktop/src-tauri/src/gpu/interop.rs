//! Quadros dos cores sem cópia: dma_buf no Linux e anel compartilhado no Windows.

use super::*;

impl FrameProcessor {
    /// Handles Vulkan crus DESTE device, pra um core libretro de HW render
    /// Vulkan (etapa 12) renderizar no MESMO device do compositor — aí a
    /// `VkImage` que ele entrega no `set_image` vira `wgpu::Texture` com
    /// `texture_from_raw`, sem cópia nenhuma.
    ///
    /// `None` quando o backend não é Vulkan (wgpu caiu pra GL/D3D) ou quando a
    /// queue do wgpu não serve — a spec do libretro Vulkan exige uma queue com
    /// GRAPHICS **e** COMPUTE, e é a queue do wgpu que o core vai usar.
    // O consumidor (emu-session repassando pro loader) entra na fase B3; por
    // enquanto só o teste usa.
    #[allow(dead_code)]
    /// Modificadores DRM com que ESTE device importa um `dma_buf`
    /// `R8G8B8A8_UNORM` pra amostrar — a lista que o produtor GL (GBM) tem
    /// que usar ao alocar. Negociação do `VK_EXT_image_drm_format_modifier`
    /// (apêndice da extensão, "Negotiation"): 1º
    /// `vkGetPhysicalDeviceFormatProperties2` + `VkDrmFormatModifierPropertiesListEXT`
    /// (modificadores com `SAMPLED_IMAGE`, 1 plano), 2º, por candidato,
    /// `vkGetPhysicalDeviceImageFormatProperties2` com
    /// `VkPhysicalDeviceImageDrmFormatModifierInfoEXT` +
    /// `VkPhysicalDeviceExternalImageFormatInfo(DMA_BUF)` — só entra se der
    /// certo E o `compatibleHandleTypes` incluir `DMA_BUF_EXT`. Vazio = sem
    /// Vulkan / sem a extensão (o chamador cai no readback).
    #[cfg(target_os = "linux")]
    pub fn dmabuf_import_modifiers(&self) -> Vec<u64> {
        use ash::vk;
        let Some(hal) = (unsafe { self.adapter.as_hal::<wgpu::hal::api::Vulkan>() }) else {
            return Vec::new();
        };
        if !hal
            .physical_device_capabilities()
            .supports_extension(ash::ext::image_drm_format_modifier::NAME)
        {
            return Vec::new();
        }
        let instance = hal.shared_instance().raw_instance();
        let phys = hal.raw_physical_device();
        let format = vk::Format::R8G8B8A8_UNORM;
        // 1º passo: quantos, depois a lista.
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default();
        let mut props2 = vk::FormatProperties2::default().push_next(&mut list);
        unsafe { instance.get_physical_device_format_properties2(phys, format, &mut props2) };
        let n = list.drm_format_modifier_count as usize;
        let mut mods = vec![vk::DrmFormatModifierPropertiesEXT::default(); n];
        let mut list = vk::DrmFormatModifierPropertiesListEXT::default()
            .drm_format_modifier_properties(&mut mods);
        let mut props2 = vk::FormatProperties2::default().push_next(&mut list);
        unsafe { instance.get_physical_device_format_properties2(phys, format, &mut props2) };
        let candidates: Vec<u64> = mods
            .iter()
            .filter(|m| {
                m.drm_format_modifier_plane_count == 1
                    && m.drm_format_modifier_tiling_features
                        .contains(vk::FormatFeatureFlags::SAMPLED_IMAGE)
            })
            .map(|m| m.drm_format_modifier)
            .collect();
        // 2º passo: confirma cada um pro uso real (2D, SAMPLED, dma_buf).
        candidates
            .into_iter()
            .filter(|&modifier| {
                let mut mod_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
                    .drm_format_modifier(modifier)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);
                let mut ext_info = vk::PhysicalDeviceExternalImageFormatInfo::default()
                    .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
                let info = vk::PhysicalDeviceImageFormatInfo2::default()
                    .format(format)
                    .ty(vk::ImageType::TYPE_2D)
                    .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
                    .usage(vk::ImageUsageFlags::SAMPLED)
                    .push_next(&mut mod_info)
                    .push_next(&mut ext_info);
                let mut ext_props = vk::ExternalImageFormatProperties::default();
                let mut out = vk::ImageFormatProperties2::default().push_next(&mut ext_props);
                let ok = unsafe {
                    instance.get_physical_device_image_format_properties2(phys, &info, &mut out)
                }
                .is_ok();
                ok && ext_props
                    .external_memory_properties
                    .compatible_handle_types
                    .contains(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
            })
            .collect()
    }

    /// O que este device Vulkan oferece pro interop GL → Vulkan no Windows
    /// (B1): `VkPhysicalDeviceIDProperties` (deviceUUID/driverUUID — "must
    /// be used to determine whether a particular external object can be
    /// shared", spec Vulkan) e se as extensões `VK_KHR_external_memory_win32`
    /// e `VK_KHR_external_semaphore_win32` existem. `None` fora do Vulkan.
    /// Anel de `n` imagens `w`×`h` exportáveis pro interop GL → Vulkan no
    /// Windows (B2): cada uma com o handle NT pra duplicar pro processo do
    /// core. Precisa do device aberto com as extensões (`win_interop::create_device`).
    /// B2: aloca o anel pra um pedido do filho, duplica os handles pro
    /// processo `pid` e guarda as imagens. Devolve o que vai na mensagem.
    #[cfg(windows)]
    pub fn install_interop_ring(
        &mut self,
        w: u32,
        h: u32,
        pid: u32,
        vulkan: bool,
    ) -> Result<emu_session::InteropReply, String> {
        /// 3 imagens: o core desenha numa enquanto o app mostra outra.
        const RING: usize = 3;
        let ring = self.export_interop_ring(RING, w, h)?;
        let images = ring
            .iter()
            .map(|img| {
                Ok(emu_session::InteropImage {
                    handle: win_interop::duplicate_into(pid, img.handle)?,
                    alloc_size: img.alloc_size,
                    done_sem: win_interop::duplicate_into(pid, img.done_handle)?,
                    free_sem: win_interop::duplicate_into(pid, img.free_handle)?,
                    memory_type: img.memory_type,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        // B5: as texturas do anel são a entrada da chain por slot (o
        // `bind_interop_input` do Windows não importa nada por quadro).
        self.imported = ring
            .iter()
            .map(|img| {
                let view = img
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                Some((img.texture.clone(), view))
            })
            .collect();
        self.flip_tgt.clear();
        self.win_staged.clear();
        self.interop_acquire = None;
        if vulkan {
            let images: Vec<ash::vk::Image> = ring.iter().map(|i| i.image).collect();
            self.interop_acquire = Some(win_interop::AcquireCmds::new(
                &self.device,
                &self.queue,
                &images,
            )?);
        }
        self.interop_ring = ring;
        self.interop_vk_producer = vulkan;
        Ok(emu_session::InteropReply {
            image_flags: self.interop_ring.first().map_or(0, |i| i.flags),
            image_usage: self.interop_ring.first().map_or(0, |i| i.usage),
            width: w,
            height: h,
            // `allocate_ring` sempre usa `VkMemoryDedicatedAllocateInfo`
            dedicated: true,
            images,
        })
    }

    /// B5 (Windows): fecha a devolução dos slots do anel. Pros `dropped`
    /// (quadros descartados sem amostrar), põe a espera do `done` e o sinal
    /// do `free` na fila; um `submit` vazio garante que tudo o que foi posto
    /// (inclusive pelos slots amostrados, se o quadro não chegou a submeter)
    /// sai agora. Devolve os slots cujo `free` já foi submetido — o chamador
    /// avisa o filho (`EmuSession::release_interop_slot`) só depois disso.
    #[cfg(windows)]
    pub fn flush_interop(&mut self, dropped: &[u32]) -> Vec<u32> {
        for &slot in dropped {
            self.stage_win_slot(slot);
        }
        if self.win_staged.is_empty() {
            return Vec::new();
        }
        self.queue.submit([]);
        std::mem::take(&mut self.win_staged)
    }

    /// Põe na próxima submissão do wgpu a espera do `done` do slot (antes de
    /// amostrar) e o sinal do `free` (depois): "the first synchronization
    /// scope includes every command submitted in the same batch" (spec
    /// Vulkan, sinal de semáforo no `vkQueueSubmit`) — o `free` só dispara
    /// depois da leitura. `false` se o slot não existe no anel.
    #[cfg(windows)]
    pub(super) fn stage_win_slot(&mut self, slot: u32) -> bool {
        let Some(img) = self.interop_ring.get(slot as usize) else {
            return false;
        };
        let (done, free) = (img.done_sem, img.free_sem);
        // C3b: produtor Vulkan — espera o `done` e faz o *acquire* num lote
        // próprio, antes do lote do wgpu que amostra.
        if self.interop_vk_producer {
            match &self.interop_acquire {
                Some(acq) if acq.submit(slot as usize, done) => {}
                _ => return false,
            }
        }
        // SAFETY: semáforos deste device, vivos enquanto o anel existir.
        let Some(q) = (unsafe { self.queue.as_hal::<wgpu::hal::api::Vulkan>() }) else {
            return false;
        };
        if !self.interop_vk_producer {
            q.add_wait_semaphore(done, None, ash::vk::PipelineStageFlags::FRAGMENT_SHADER);
        }
        q.add_signal_semaphore(free, None);
        drop(q);
        self.win_staged.push(slot);
        true
    }

    #[cfg(windows)]
    pub fn export_interop_ring(
        &self,
        n: usize,
        w: u32,
        h: u32,
    ) -> Result<Vec<win_interop::ExportedImage>, String> {
        win_interop::allocate_ring(&self.device, &self.adapter, n, w, h)
    }

    // só o Windows usa (lib.rs); no Linux o interop é por `dma_buf`
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn vk_interop_id(&self) -> Option<domain::gpu_interop::VkInteropId> {
        use ash::vk;
        let hal = unsafe { self.adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
        let caps = hal.physical_device_capabilities();
        let instance = hal.shared_instance().raw_instance();
        let mut id = vk::PhysicalDeviceIDProperties::default();
        let mut props2 = vk::PhysicalDeviceProperties2::default().push_next(&mut id);
        unsafe { instance.get_physical_device_properties2(hal.raw_physical_device(), &mut props2) };
        Some(domain::gpu_interop::VkInteropId {
            device_uuid: id.device_uuid,
            driver_uuid: id.driver_uuid,
            external_memory_win32: caps.supports_extension(ash::khr::external_memory_win32::NAME),
            external_semaphore_win32: caps
                .supports_extension(ash::khr::external_semaphore_win32::NAME),
        })
    }
}
