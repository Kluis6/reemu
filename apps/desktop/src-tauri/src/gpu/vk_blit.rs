//! Blit Vulkan (etapa 12): converte o scanout packed do core pra RGBA8 numa
//! `VkImage` que o wgpu adota.

/// Um alvo RGBA8 do conversor packed→RGBA8, por slot do ring do core.
pub(super) struct VkBlitTarget {
    pub(super) image: ash::vk::Image,
    pub(super) memory: ash::vk::DeviceMemory,
    pub(super) w: u32,
    pub(super) h: u32,
    /// Já foi transicionado pra `SHADER_READ_ONLY_OPTIMAL` ao menos uma vez
    /// (o 1º barrier usa `old_layout = UNDEFINED`).
    pub(super) ready: bool,
    /// `wgpu::Texture` embrulhando `image` (external — wgpu não destrói).
    pub(super) wrapped: Option<(wgpu::Texture, wgpu::TextureView)>,
}

/// Converte o scanout packed 16-bit de um core Vulkan (Beetle PSX HW com dither,
/// ou jogo em modo 16bpp — `VK_FORMAT_A1R5G5B5_UNORM_PACK16` etc.) pra RGBA8
/// via `vkCmdBlitImage` no device adotado, já que o wgpu não amostra esses
/// formatos. Recursos crus de `ash` — destruídos no `Drop` após
/// `device_wait_idle`.
pub(super) struct VkBlit {
    pub(super) device: ash::Device,
    pub(super) queue: ash::vk::Queue,
    pub(super) mem_props: ash::vk::PhysicalDeviceMemoryProperties,
    pub(super) pool: ash::vk::CommandPool,
    /// Um command buffer + fence POR SLOT (fase C): o blit é submetido sem
    /// esperar a CPU; a fence do slot só é esperada antes de regravar o
    /// command buffer daquele slot (spec Vulkan: não regravar um command
    /// buffer em estado pendente). A ordem na GPU vem das barreiras (a 1ª
    /// tem `srcStage` com FRAGMENT_SHADER, encadeando com a barreira do
    /// core que libera a imagem pra leitura no fragment shader).
    pub(super) slots: Vec<BlitSlot>,
    pub(super) targets: Vec<Option<VkBlitTarget>>,
}

/// Command buffer + fence de um slot do blit.
pub(super) struct BlitSlot {
    cmd: ash::vk::CommandBuffer,
    fence: ash::vk::Fence,
    /// Submetido e ainda não esperado.
    pending: bool,
}

impl VkBlit {
    /// Command buffer do `slot`, pronto pra regravar: cria na 1ª vez; se o
    /// uso anterior ainda está pendente, espera a fence dele e a reseta.
    unsafe fn slot_cmd(&mut self, slot: usize) -> Option<(ash::vk::CommandBuffer, ash::vk::Fence)> {
        use ash::vk;
        while self.slots.len() <= slot {
            let cmd = unsafe {
                self.device.allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(self.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
            }
            .ok()?[0];
            let fence = unsafe {
                self.device
                    .create_fence(&vk::FenceCreateInfo::default(), None)
            }
            .ok()?;
            self.slots.push(BlitSlot {
                cmd,
                fence,
                pending: false,
            });
        }
        let s = &mut self.slots[slot];
        if s.pending {
            unsafe {
                let _ = self.device.wait_for_fences(&[s.fence], true, u64::MAX);
                let _ = self.device.reset_fences(&[s.fence]);
            }
            s.pending = false;
        }
        Some((s.cmd, s.fence))
    }

    /// # Safety
    /// `device`/`queue` são do device Vulkan adotado (§Beetle); `qf` é a queue
    /// family da `queue`.
    pub(super) unsafe fn new(device: &wgpu::Device, queue: &wgpu::Queue, qf: u32) -> Option<Self> {
        use ash::vk;
        let (raw_device, mem_props) = unsafe {
            let hd = device.as_hal::<wgpu::hal::api::Vulkan>()?;
            let phys = hd.raw_physical_device();
            let instance = hd.shared_instance().raw_instance().clone();
            (
                hd.raw_device().clone(),
                instance.get_physical_device_memory_properties(phys),
            )
        };
        let raw_queue = unsafe { queue.as_hal::<wgpu::hal::api::Vulkan>()?.as_raw() };

        let pool = unsafe {
            raw_device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(qf)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }
        .inspect_err(|e| log::error!("vk_blit: create_command_pool: {e}"))
        .ok()?;
        Some(Self {
            device: raw_device,
            queue: raw_queue,
            mem_props,
            pool,
            slots: Vec::new(),
            targets: Vec::new(),
        })
    }

    pub(super) fn mem_type(&self, bits: u32, want: ash::vk::MemoryPropertyFlags) -> Option<u32> {
        (0..self.mem_props.memory_type_count).find(|&i| {
            (bits & (1 << i)) != 0
                && self.mem_props.memory_types[i as usize]
                    .property_flags
                    .contains(want)
        })
    }

    /// Garante um alvo RGBA8 `w×h` no `slot`. `false` = falhou.
    pub(super) unsafe fn ensure_target(
        &mut self,
        _device: &wgpu::Device,
        slot: usize,
        w: u32,
        h: u32,
    ) -> bool {
        use ash::vk;
        if self.targets.len() <= slot {
            self.targets.resize_with(slot + 1, || None);
        }
        if let Some(t) = &self.targets[slot] {
            if t.w == w && t.h == h {
                return true;
            }
            let old = self.targets[slot].take().unwrap();
            unsafe {
                let _ = self.device.device_wait_idle();
                self.destroy_target(old);
            }
        }
        let img = match unsafe {
            self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: w,
                        height: h,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .initial_layout(vk::ImageLayout::UNDEFINED),
                None,
            )
        } {
            Ok(i) => i,
            Err(e) => {
                log::error!("vk_blit: create_image: {e}");
                return false;
            }
        };
        let req = unsafe { self.device.get_image_memory_requirements(img) };
        let Some(mt) = self.mem_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)
        else {
            log::error!("vk_blit: sem tipo de memória DEVICE_LOCAL");
            unsafe { self.device.destroy_image(img, None) };
            return false;
        };
        let memory = match unsafe {
            self.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(mt),
                None,
            )
        } {
            Ok(m) => m,
            Err(e) => {
                log::error!("vk_blit: allocate_memory: {e}");
                unsafe { self.device.destroy_image(img, None) };
                return false;
            }
        };
        if let Err(e) = unsafe { self.device.bind_image_memory(img, memory, 0) } {
            log::error!("vk_blit: bind_image_memory: {e}");
            unsafe {
                self.device.destroy_image(img, None);
                self.device.free_memory(memory, None);
            }
            return false;
        }
        self.targets[slot] = Some(VkBlitTarget {
            image: img,
            memory,
            w,
            h,
            ready: false,
            wrapped: None,
        });
        true
    }

    /// Blita `handle` (packed, `SHADER_READ_ONLY_OPTIMAL`) → o RGBA8 do `slot`.
    pub(super) unsafe fn run(
        &mut self,
        handle: &dyn domain::frame_source::VulkanImageHandle,
        slot: usize,
    ) -> bool {
        use ash::vk::{self, Handle as _};
        let Some(t) = self.targets.get(slot).and_then(|s| s.as_ref()) else {
            return false;
        };
        let (dst, w, h, ready) = (t.image, t.w, t.h, t.ready);
        let src = vk::Image::from_raw(handle.image());
        let sub = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1);
        let layers = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .layer_count(1);
        let end = vk::Offset3D {
            x: w as i32,
            y: h as i32,
            z: 1,
        };
        let ignore = vk::QUEUE_FAMILY_IGNORED;
        let Some((cmd, fence)) = (unsafe { self.slot_cmd(slot) }) else {
            return false;
        };

        let ok = unsafe {
            self.device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())
                .is_ok()
                && self
                    .device
                    .begin_command_buffer(
                        cmd,
                        &vk::CommandBufferBeginInfo::default()
                            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                    )
                    .is_ok()
        };
        if !ok {
            return false;
        }
        unsafe {
            let to_transfer = [
                vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::SHADER_READ)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .src_queue_family_index(ignore)
                    .dst_queue_family_index(ignore)
                    .image(src)
                    .subresource_range(sub),
                vk::ImageMemoryBarrier::default()
                    .old_layout(if ready {
                        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                    } else {
                        vk::ImageLayout::UNDEFINED
                    })
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .src_queue_family_index(ignore)
                    .dst_queue_family_index(ignore)
                    .image(dst)
                    .subresource_range(sub),
            ];
            self.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &to_transfer,
            );

            let region = vk::ImageBlit::default()
                .src_subresource(layers)
                .src_offsets([vk::Offset3D::default(), end])
                .dst_subresource(layers)
                .dst_offsets([vk::Offset3D::default(), end]);
            self.device.cmd_blit_image(
                cmd,
                src,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
                vk::Filter::NEAREST,
            );

            let to_shader = [
                vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .src_queue_family_index(ignore)
                    .dst_queue_family_index(ignore)
                    .image(src)
                    .subresource_range(sub),
                vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .src_queue_family_index(ignore)
                    .dst_queue_family_index(ignore)
                    .image(dst)
                    .subresource_range(sub),
            ];
            self.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &to_shader,
            );

            if self.device.end_command_buffer(cmd).is_err() {
                return false;
            }
            let cmds = [cmd];
            let submit = vk::SubmitInfo::default().command_buffers(&cmds);
            if let Err(e) = self.device.queue_submit(self.queue, &[submit], fence) {
                log::error!("vk_blit: queue_submit: {e}");
                return false;
            }
            // Sem esperar a CPU (fase C) — ver `slots`.
            self.slots[slot].pending = true;
        }
        if let Some(t) = self.targets[slot].as_mut() {
            t.ready = true;
        }
        true
    }

    /// `wgpu::TextureView` do RGBA8 do `slot` (embrulha a `VkImage` uma vez).
    pub(super) unsafe fn wgpu_view(
        &mut self,
        device: &wgpu::Device,
        slot: usize,
    ) -> Option<wgpu::TextureView> {
        use ash::vk::Handle as _;
        let t = self.targets.get_mut(slot).and_then(|s| s.as_mut())?;
        if t.wrapped.is_none() {
            let size = wgpu::Extent3d {
                width: t.w,
                height: t.h,
                depth_or_array_layers: 1,
            };
            let hal_desc = wgpu::hal::TextureDescriptor {
                label: Some("vk blit rgba8"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUses::RESOURCE,
                memory_flags: wgpu::hal::MemoryFlags::empty(),
                view_formats: vec![],
            };
            let hal_tex = unsafe {
                let hd = device.as_hal::<wgpu::hal::api::Vulkan>()?;
                hd.texture_from_raw(
                    ash::vk::Image::from_raw(t.image.as_raw()),
                    &hal_desc,
                    // external: wgpu NÃO destrói — o `VkBlit::drop` faz.
                    Some(Box::new(|| {})),
                    wgpu::hal::vulkan::TextureMemory::External,
                )
            };
            let desc = wgpu::TextureDescriptor {
                label: Some("vk blit"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            };
            let tex = unsafe {
                device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                    hal_tex,
                    &desc,
                    wgpu::TextureUses::RESOURCE,
                )
            };
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            t.wrapped = Some((tex, view));
        }
        Some(t.wrapped.as_ref().unwrap().1.clone())
    }

    pub(super) unsafe fn destroy_target(&self, t: VkBlitTarget) {
        drop(t.wrapped); // dropa a wgpu::Texture (external — não toca a VkImage)
        unsafe {
            self.device.destroy_image(t.image, None);
            self.device.free_memory(t.memory, None);
        }
    }
}

impl Drop for VkBlit {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            for t in std::mem::take(&mut self.targets).into_iter().flatten() {
                self.destroy_target(t);
            }
            for sl in self.slots.drain(..) {
                self.device.destroy_fence(sl.fence, None);
            }
            // (os command buffers vão junto com o pool)
            self.device.destroy_command_pool(self.pool, None);
        }
    }
}
