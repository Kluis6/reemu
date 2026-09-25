//! Entrada da cadeia vinda de GPU: `VkImage` do core (HW render Vulkan) e
//! dma_buf (interop GL).

use super::*;

impl FrameProcessor {
    /// HW render Vulkan (etapa 12): embrulha a `VkImage` que o core entregou
    /// com `texture_from_raw` (MESMO device, sem cópia) e a seleciona como
    /// entrada da chain. Cacheia por `sync_index` — o core cicla um conjunto
    /// fixo de imagens. `false` = falha → canvas vazio.
    pub(super) fn bind_vulkan_input(
        &mut self,
        handle: &dyn domain::frame_source::VulkanImageHandle,
    ) -> bool {
        let slot = handle.sync_index() as usize;
        if slot >= 8 {
            return false;
        }
        // Opção 4: a thread do core só GRAVOU os command buffers; nós (thread do
        // compositor) submetemos, aqui, na MESMA thread que faz o submit do
        // wgpu — sem corrida na VkQueue. Sync conservador: CPU-wait no fence.
        if !self.submit_vulkan_cmds(handle) {
            return false;
        }
        if self.vk_imported.len() <= slot {
            self.vk_imported.resize_with(slot + 1, || None);
        }
        let img_handle = handle.image();
        let stale = self.vk_imported[slot]
            .as_ref()
            .map_or(true, |(cached, _, _)| *cached != img_handle);
        // Scanout num formato packed 16-bit (A1R5G5B5 = 8, R5G5B5A1 = 7,
        // R5G6B5 = 4) que o wgpu não amostra: `vkCmdBlitImage` pra um RGBA8
        // nosso e amostra esse. Roda TODO frame (o conteúdo muda), só o alvo
        // + o wrapper wgpu são cacheados por slot.
        if vk_format_to_wgpu(handle.vk_format()).is_none() {
            return self.bind_via_blit(handle, slot);
        }

        if stale {
            match self.wrap_vulkan_image(handle) {
                Some(tv) => self.vk_imported[slot] = Some((img_handle, tv.0, tv.1)),
                None => return false,
            }
        }
        let Some((_, _, view)) = self.vk_imported.get(slot).and_then(|s| s.as_ref()) else {
            return false;
        };
        self.interop_view = Some(view.clone());
        true
    }

    /// `A1R5G5B5`/packed-16 → RGBA8 por `vkCmdBlitImage` no device adotado.
    pub(super) fn bind_via_blit(
        &mut self,
        handle: &dyn domain::frame_source::VulkanImageHandle,
        slot: usize,
    ) -> bool {
        let Some(qf) = self.adopted_queue_family else {
            log::error!(
                "§Beetle: scanout packed (VkFormat {}) mas o device não é adotado — \
                 sem como converter",
                handle.vk_format()
            );
            return false;
        };
        if self.vk_blit.is_none() {
            match unsafe { VkBlit::new(&self.device, &self.queue, qf) } {
                Some(b) => {
                    log::info!("§Beetle: conversor packed→RGBA8 (vkCmdBlitImage) ativo");
                    self.vk_blit = Some(b);
                }
                None => {
                    log::error!("§Beetle: falha ao criar o conversor packed→RGBA8");
                    return false;
                }
            }
        }
        let (w, h) = (handle.width().max(1), handle.height().max(1));
        let blit = self.vk_blit.as_mut().unwrap();
        if !unsafe { blit.ensure_target(&self.device, slot, w, h) } {
            log::error!("§Beetle: alvo de blit {w}x{h} falhou");
            return false;
        }
        if !unsafe { blit.run(handle, slot) } {
            return false;
        }
        match unsafe { blit.wgpu_view(&self.device, slot) } {
            Some(view) => {
                self.interop_view = Some(view);
                true
            }
            None => false,
        }
    }

    /// Submete na `VkQueue` do wgpu os command buffers que o core gravou pra
    /// este frame (`set_command_buffers`), sinaliza `fence`, e espera (CPU) —
    /// sync conservador da fase B. `handle.command_buffers()` vazio = frame
    /// dup (só re-seleciona a textura). `false` = erro de submissão.
    ///
    /// `handle.release()` (chamado no `Drop` do `Frame`) destrava o
    /// `wait_sync_index` do core pra este slot.
    pub(super) fn submit_vulkan_cmds(
        &self,
        handle: &dyn domain::frame_source::VulkanImageHandle,
    ) -> bool {
        use ash::vk::Handle as _;
        let cmds = handle.command_buffers();
        if cmds.is_empty() {
            return true;
        }
        let fence = ash::vk::Fence::from_raw(handle.fence());
        if fence.is_null() {
            log::warn!("vk frame sem fence — pulando");
            return false;
        }
        let cmd_bufs: Vec<ash::vk::CommandBuffer> = cmds
            .iter()
            .map(|&c| ash::vk::CommandBuffer::from_raw(c))
            .collect();

        // SAFETY: `hal_queue`/`hal_dev` são do MESMO device que o core adotou;
        // `cmd_bufs` foram gravados pelo core e não estão submetidos. Estamos na
        // thread do compositor — nenhum outro submit concorre.
        let ok = unsafe {
            let Some(hal_queue) = self.queue.as_hal::<wgpu::hal::api::Vulkan>() else {
                return false;
            };
            let raw_queue = hal_queue.as_raw();
            let raw_device = hal_queue.raw_device().clone();
            drop(hal_queue);

            let submit = ash::vk::SubmitInfo::default().command_buffers(&cmd_bufs);
            if let Err(e) = raw_device.queue_submit(raw_queue, &[submit], fence) {
                log::warn!("vkQueueSubmit (core cmds): {e}");
                return false;
            }
            match raw_device.wait_for_fences(&[fence], true, u64::MAX) {
                Ok(()) => {
                    let _ = raw_device.reset_fences(&[fence]);
                    true
                }
                Err(e) => {
                    log::warn!("vkWaitForFences (core cmds): {e}");
                    false
                }
            }
        };
        ok
    }

    /// `VkImage` do core → `(wgpu::Texture, TextureView)` sem cópia. O core já
    /// deixou a imagem em `SHADER_READ_ONLY_OPTIMAL` (barrier no cmd buffer
    /// dele) e o adapter já esperou (CPU) a submissão — então passamos
    /// `initial_state = RESOURCE`. `drop_callback = Some(no-op)` diz ao
    /// wgpu-hal pra NÃO destruir a imagem (é do core).
    pub(super) fn wrap_vulkan_image(
        &self,
        handle: &dyn domain::frame_source::VulkanImageHandle,
    ) -> Option<(wgpu::Texture, wgpu::TextureView)> {
        let (w, h) = (handle.width().max(1), handle.height().max(1));
        let vkf = handle.vk_format();
        let Some(format) = vk_format_to_wgpu(vkf) else {
            log::error!(
                "§Beetle: scanout VkFormat {vkf} sem equivalente wgpu — frame não entra na chain \
                 (com dither ligado o Beetle usa A1R5G5B5; a opção deveria estar 'disabled')"
            );
            return None;
        };
        log::info!("§Beetle: 1ª VkImage {w}x{h} VkFormat {vkf} → {format:?}");
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: Some("vk hw-render image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUses::RESOURCE,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        // SAFETY: `image` é uma VkImage viva do MESMO device (o core adotou o
        // nosso). `Some(no-op)` = external, wgpu não destrói. `initial_state`
        // bate com o layout real (o core faz o release barrier + CPU-wait).
        let hal_tex = unsafe {
            use ash::vk::Handle as _;
            let vk_image = ash::vk::Image::from_raw(handle.image());
            let hal_dev = self.device.as_hal::<wgpu::hal::api::Vulkan>()?;
            hal_dev.texture_from_raw(
                vk_image,
                &hal_desc,
                Some(Box::new(|| {})),
                wgpu::hal::vulkan::TextureMemory::External,
            )
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("vk hw-render"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        // SAFETY: `hal_tex` recém-embrulhado deste device; layout coerente.
        let tex = unsafe {
            self.device
                .create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                    hal_tex,
                    &desc,
                    wgpu::TextureUses::RESOURCE,
                )
        };
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Some((tex, view))
    }

    /// Importa (1ª vez do slot) e seleciona a textura `dma_buf` como entrada da
    /// chain neste frame. `false` = interop indisponível / falhou → o chamador
    /// devolve `None` e o `poll_frame` cai no caminho de canvas vazio.
    pub(super) fn bind_interop_input(
        &mut self,
        handle: &dyn domain::frame_source::GpuTextureHandle,
        w: u32,
        h: u32,
        enc: &mut wgpu::CommandEncoder,
    ) -> bool {
        // Fence de fim de render do core (no lugar do `glFinish` dele): a
        // GPU espera por ela antes do submit que amostra a textura. Sempre
        // consumida — mesmo sem interop, pra não vazar o fd.
        #[cfg(target_os = "linux")]
        if let Some(fd) = handle.take_sync_fd() {
            use std::os::fd::FromRawFd as _;
            // SAFETY: `take_sync_fd` transfere a posse do fd.
            let fd = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
            let rest = match self.sync_fd.as_mut() {
                Some(imp) => imp.stage_wait(&self.queue, fd).err(),
                None => Some(fd),
            };
            if let Some(fd) = rest {
                // 50 ms de teto, como o antigo modo `fence` do produtor.
                super::sync_fd::cpu_wait(fd, 50);
            }
        }
        if !self.interop_ok {
            return false;
        }
        let slot = handle.slot() as usize;
        if slot >= 8 {
            return false;
        }
        if self.imported.len() <= slot {
            self.imported.resize_with(slot + 1, || None);
        }
        if let Some(plane) = handle.take_plane() {
            match self.import_dmabuf(&plane, w, h) {
                Ok(tv) => self.imported[slot] = Some(tv),
                Err(e) => {
                    // SAFETY: import falhou sem assumir o fd → fecha aqui.
                    unsafe { close_raw_fd(plane.fd) };
                    log::warn!("importar dma_buf (slot {slot}): {e} — interop desligado");
                    self.interop_ok = false;
                    return false;
                }
            }
        }
        let Some((imp_tex, imp_view)) = self.imported.get(slot).and_then(|s| s.as_ref()) else {
            return false;
        };

        // O buffer tem o tamanho MÁXIMO do core; o quadro ocupa só `w`×`h`
        // (ex.: flycast aloca 853x853 e desenha 640x480). Recorta — e inverte
        // o Y se o core é bottom-left — num alvo do tamanho do quadro.
        let (tw, th) = {
            let s = imp_tex.size();
            (s.width, s.height)
        };
        let (w, h) = (w.clamp(1, tw), h.clamp(1, th));
        let flip = handle.flip_y();
        if !flip && (w, h) == (tw, th) {
            self.interop_view = Some(imp_view.clone());
            return true;
        }
        if self.flip_tgt.len() <= slot {
            self.flip_tgt.resize_with(slot + 1, || None);
        }
        if !matches!(&self.flip_tgt[slot], Some((_, _, tgt_w, tgt_h)) if *tgt_w == w && *tgt_h == h)
        {
            let (t, v) = new_tex(
                &self.device,
                w,
                h,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            self.flip_tgt[slot] = Some((t, v, w, h));
        }
        let params = [
            w as f32 / tw as f32,
            h as f32 / th as f32,
            if flip { 1.0 } else { 0.0 },
            0.0f32,
        ];
        let bytes: Vec<u8> = params.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.queue.write_buffer(&self.flip.params, 0, &bytes);
        let src_view = &self.imported[slot].as_ref().unwrap().1;
        let dst_view = &self.flip_tgt[slot].as_ref().unwrap().1;
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("flip bg"),
            layout: &self.flip.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_nearest),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.flip.params.as_entire_binding(),
                },
            ],
        });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("flip pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&self.flip.pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.draw(0..3, 0..1);
        }
        self.interop_view = Some(self.flip_tgt[slot].as_ref().unwrap().1.clone());
        true
    }

    /// dma_buf só existe em Unix — no Windows o GL HW render sempre vai pelo
    /// readback (ver `core-loader-desktop::gl_context`), nunca chega aqui.
    #[cfg(not(unix))]
    pub(super) fn import_dmabuf(
        &self,
        _plane: &domain::frame_source::DmabufPlaneInfo,
        _w: u32,
        _h: u32,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), String> {
        Err("interop dma_buf só existe em Unix".into())
    }

    #[cfg(unix)]
    pub(super) fn import_dmabuf(
        &self,
        plane: &domain::frame_source::DmabufPlaneInfo,
        w: u32,
        h: u32,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), String> {
        use std::os::fd::FromRawFd as _;
        let size = wgpu::Extent3d {
            width: w.max(plane.width),
            height: h.max(plane.height),
            depth_or_array_layers: 1,
        };
        let hal_desc = wgpu::hal::TextureDescriptor {
            label: Some("dmabuf import"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUses::RESOURCE,
            memory_flags: wgpu::hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        // SAFETY: fd é um dma_buf válido (GBM); layout casa com `plane`.
        let hal_tex = unsafe {
            let owned = std::os::fd::OwnedFd::from_raw_fd(plane.fd);
            let hal_dev = self
                .device
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or("device não-Vulkan")?;
            hal_dev
                .texture_from_dmabuf_fd(
                    owned,
                    &hal_desc,
                    plane.modifier,
                    plane.stride as u64,
                    plane.offset as u64,
                )
                .map_err(|e| format!("texture_from_dmabuf_fd: {e:?}"))?
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("dmabuf"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        // SAFETY: `hal_tex` recém-criado pra este device, layout coerente.
        let tex = unsafe {
            self.device
                .create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                    hal_tex,
                    &desc,
                    wgpu::TextureUses::RESOURCE,
                )
        };
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        Ok((tex, view))
    }
}
