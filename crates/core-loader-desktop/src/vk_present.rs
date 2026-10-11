//! Apresentador do HW render Vulkan quando o core roda no processo filho
//! (`reemu-core-host`, tarefa C3 do TASKS).
//!
//! In-process, quem submete os command buffers do core e lê a `VkImage` dele
//! é o compositor (wgpu, no MESMO device). No processo filho não há
//! compositor: este módulo faz o papel dele — submete o que o core gravou
//! (`set_command_buffers`) e, na mesma submissão, copia a imagem do core pra
//! um alvo RGBA8 nosso e dali pra um buffer visível pela CPU. O quadro sai
//! como RGBA pelo anel de memória do `core-ipc`, como o readback do GL (fase
//! A).
//!
//! **C3b (Windows): anel compartilhado com o app.** Com o device na GPU do
//! app e as extensões Win32 (`VkContext::interop_ready`), o apresentador pede
//! o anel (o mesmo do GL, B2–B5) e desenha direto no slot: importa a memória
//! (`VkImportMemoryWin32HandleInfoKHR`, alocação dedicada numa imagem com os
//! MESMOS parâmetros — VUID-VkMemoryDedicatedAllocateInfo-image-01876 —,
//! mesmo `allocationSize` e `memoryTypeIndex` — VUID-VkMemoryAllocateInfo-
//! allocationSize-01743) e os semáforos (`vkImportSemaphoreWin32HandleKHR`).
//! Por quadro: espera o `free` (se o slot já foi entregue), desenha, faz o
//! *release* pra `VK_QUEUE_FAMILY_EXTERNAL` (`COLOR_ATTACHMENT_OPTIMAL` →
//! `SHADER_READ_ONLY_OPTIMAL`; o app faz o *acquire* com os mesmos layouts)
//! e sinaliza o `done`; o quadro vai só com o índice. Sem slot livre, cai na
//! cópia pra CPU.
//!
//! **A cópia é um passe de desenho, não `vkCmdBlitImage`:** o
//! `libretro_vulkan.h` pede que a imagem do `set_image` tenha
//! `TRANSFER_SRC` e `SAMPLED`, mas o flycast entrega sem `TRANSFER_SRC` (a
//! camada de validação acusou VUID-vkCmdBlitImage-srcImage-00219,
//! 2026-10-11). Um triângulo que cobre o alvo lê cada texel com
//! `textureLoad` (`OpImageFetch`: "No sampler is used", spec Vulkan,
//! Textures) — só exige `SAMPLED`, converte o formato do core (B8G8R8A8,
//! packed 16 bits…) pra RGBA8 e não muda o layout da imagem do core: o
//! descritor usa o `image_layout` que ele informou (`GENERAL` ou
//! `SHADER_READ_ONLY_OPTIMAL`; layout diferente do real devolve "poison").
//!
//! Sincronização (`libretro_vulkan.h`): com `set_command_buffers`,
//! "vkCmdPipelineBarrier must be used for synchronization" — a barreira de
//! liberação que o core grava (destino no fragment shader) cobre a leitura
//! deste passe, que vem depois na mesma submissão. A imagem só volta pro
//! core (`wait_sync_index`) depois da fence desta submissão.

use ash::vk;

use crate::vk_context::VkContext;
use crate::vk_frame::PendingFrame;

/// Triângulo que cobre o alvo; cada pixel lê o texel de mesma coordenada da
/// imagem do core (o quadro ocupa o canto de cima à esquerda dela).
const SHADER: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(p.xy), 0);
}
"#;

/// WGSL → SPIR-V 1.0 (naga). Binding `@group(0) @binding(0)` vira o set 0,
/// binding 0 (`fake_missing_bindings`).
fn spirv() -> Result<Vec<u32>, String> {
    let module = naga::front::wgsl::parse_str(SHADER).map_err(|e| format!("WGSL: {e}"))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| format!("validação do shader: {e}"))?;
    naga::back::spv::write_vec(&module, &info, &naga::back::spv::Options::default(), None)
        .map_err(|e| format!("SPIR-V: {e}"))
}

/// Alvo RGBA8 + buffer de leitura, no tamanho do último quadro.
struct Target {
    image: vk::Image,
    image_mem: vk::DeviceMemory,
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
    buf: vk::Buffer,
    buf_mem: vk::DeviceMemory,
    ptr: *const u8,
    w: u32,
    h: u32,
}

pub(crate) struct VkReadback {
    device: ash::Device,
    queue: vk::Queue,
    mem_props: vk::PhysicalDeviceMemoryProperties,
    pool: vk::CommandPool,
    cmd: vk::CommandBuffer,
    fence: vk::Fence,
    shader: vk::ShaderModule,
    set_layout: vk::DescriptorSetLayout,
    pipeline_layout: vk::PipelineLayout,
    render_pass: vk::RenderPass,
    pipeline: vk::Pipeline,
    desc_pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    target: Option<Target>,
    ring_pass: vk::RenderPass,
    /// C3b: o anel importado do app (Windows).
    #[cfg(windows)]
    ring: Option<ring::VkRing>,
    /// Família da fila do device (o *release* do anel, só no Windows).
    #[cfg_attr(not(windows), allow(dead_code))]
    queue_family: u32,
}

/// O que saiu de um quadro.
pub(crate) enum Presented {
    /// Copiado pra CPU (RGBA8, `w`×`h`).
    Rgba(Vec<u8>),
    /// Desenhado no slot `n` do anel compartilhado com o app (C3b).
    #[cfg_attr(not(windows), allow(dead_code))]
    Slot(u32),
}

/// Render pass de 1 attachment RGBA8, conteúdo anterior descartado
/// (UNDEFINED, DONT_CARE). As dependências cobrem os dois usos: antes, a
/// leitura anterior do alvo (cópia pro buffer, ou o app lendo o slot —
/// esperado pelo semáforo `free` em COLOR_ATTACHMENT_OUTPUT) termina antes
/// da escrita; depois, a escrita fica visível pra cópia.
fn make_render_pass(
    d: &ash::Device,
    final_layout: vk::ImageLayout,
) -> Result<vk::RenderPass, String> {
    let attachments = [vk::AttachmentDescription::default()
        .format(vk::Format::R8G8B8A8_UNORM)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::DONT_CARE)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(final_layout)];
    let color = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let subpasses = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color)];
    let deps = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            )
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::TRANSFER_READ)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::TRANSFER)
            .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
            .dst_access_mask(vk::AccessFlags::TRANSFER_READ),
    ];
    // SAFETY: device válido; destruído no `Drop` do dono.
    unsafe {
        d.create_render_pass(
            &vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(&subpasses)
                .dependencies(&deps),
            None,
        )
    }
    .map_err(|e| format!("vkCreateRenderPass: {e}"))
}

// SAFETY: como o `VkFrameBridge`, só a thread que dirige o core usa isto.
unsafe impl Send for VkReadback {}

impl VkReadback {
    /// `max_w`×`max_h`: o maior quadro do core (tamanho do anel a pedir).
    pub fn new(ctx: &VkContext, max_w: u32, max_h: u32) -> Result<Self, String> {
        let code = spirv()?;
        let device = ctx.device.clone();
        let mem_props = unsafe {
            ctx.instance
                .get_physical_device_memory_properties(ctx.physical_device)
        };
        // Monta peça por peça; o `Drop` destrói o que já existir (handles
        // nulos são ignorados pelos `vkDestroy*`).
        let mut me = Self {
            device,
            queue: ctx.queue,
            mem_props,
            pool: vk::CommandPool::null(),
            cmd: vk::CommandBuffer::null(),
            fence: vk::Fence::null(),
            shader: vk::ShaderModule::null(),
            set_layout: vk::DescriptorSetLayout::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            render_pass: vk::RenderPass::null(),
            pipeline: vk::Pipeline::null(),
            desc_pool: vk::DescriptorPool::null(),
            set: vk::DescriptorSet::null(),
            target: None,
            ring_pass: vk::RenderPass::null(),
            #[cfg(windows)]
            ring: None,
            queue_family: ctx.queue_family_index,
        };
        // SAFETY: device válido; tudo destruído no `Drop`.
        unsafe { me.build(ctx.queue_family_index, &code)? };
        // C3b: device na GPU do app com as extensões → pede o anel (o
        // core-host manda o `InteropRequest`; o app responde com as imagens).
        if cfg!(windows) && ctx.interop_ready && crate::gl_interop_wanted() {
            *crate::gl_context::INTEROP_REQUEST
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = Some((max_w.max(1), max_h.max(1), true));
            log::info!("Vulkan no processo filho: pedindo o anel compartilhado ({max_w}x{max_h})");
        }
        let _ = (max_w, max_h);
        Ok(me)
    }

    unsafe fn build(&mut self, queue_family: u32, code: &[u32]) -> Result<(), String> {
        let d = self.device.clone();
        self.pool = d
            .create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
            .map_err(|e| format!("vkCreateCommandPool: {e}"))?;
        self.cmd = d
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
            .map_err(|e| format!("vkAllocateCommandBuffers: {e}"))?[0];
        self.fence = d
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .map_err(|e| format!("vkCreateFence: {e}"))?;
        self.shader = d
            .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(code), None)
            .map_err(|e| format!("vkCreateShaderModule: {e}"))?;
        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        self.set_layout = d
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
            .map_err(|e| format!("vkCreateDescriptorSetLayout: {e}"))?;
        let set_layouts = [self.set_layout];
        self.pipeline_layout = d
            .create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts),
                None,
            )
            .map_err(|e| format!("vkCreatePipelineLayout: {e}"))?;

        // Alvo da cópia pra CPU: termina pronto pra cópia (TRANSFER_SRC).
        self.render_pass = make_render_pass(&d, vk::ImageLayout::TRANSFER_SRC_OPTIMAL)?;
        // Slot do anel (C3b): termina como attachment; o *release* depois do
        // passe faz a transição pra SHADER_READ_ONLY. Mesmo formato, subpasso
        // e dependências → compatível com o mesmo pipeline (só layouts
        // finais diferem, o que a compatibilidade de render pass ignora).
        self.ring_pass = make_render_pass(&d, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)?;

        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(self.shader)
                .name(c"vs_main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(self.shader)
                .name(c"fs_main"),
        ];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
        let input_asm = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend_attachments = [vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&blend_attachments);
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let pipeline_ci = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_asm)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&multisample)
            .color_blend_state(&blend)
            .dynamic_state(&dynamic)
            .layout(self.pipeline_layout)
            .render_pass(self.render_pass)
            .subpass(0);
        self.pipeline = d
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_ci], None)
            .map_err(|(_, e)| format!("vkCreateGraphicsPipelines: {e}"))?[0];

        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::SAMPLED_IMAGE)
            .descriptor_count(1)];
        self.desc_pool = d
            .create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
            .map_err(|e| format!("vkCreateDescriptorPool: {e}"))?;
        self.set = d
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(self.desc_pool)
                    .set_layouts(&set_layouts),
            )
            .map_err(|e| format!("vkAllocateDescriptorSets: {e}"))?[0];
        Ok(())
    }

    fn mem_type(&self, bits: u32, want: vk::MemoryPropertyFlags) -> Option<u32> {
        (0..self.mem_props.memory_type_count).find(|&t| {
            bits & (1 << t) != 0
                && self.mem_props.memory_types[t as usize]
                    .property_flags
                    .contains(want)
        })
    }

    /// Alvo `w`×`h`: recria só quando o tamanho muda.
    unsafe fn ensure_target(&mut self, w: u32, h: u32) -> Result<(), String> {
        if self.target.as_ref().is_some_and(|t| (t.w, t.h) == (w, h)) {
            return Ok(());
        }
        if let Some(t) = self.target.take() {
            self.destroy_target(t);
        }
        let d = self.device.clone();
        // Montado aos poucos; em erro, `destroy_target` solta o que existir.
        let mut t = Target {
            image: vk::Image::null(),
            image_mem: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            framebuffer: vk::Framebuffer::null(),
            buf: vk::Buffer::null(),
            buf_mem: vk::DeviceMemory::null(),
            ptr: std::ptr::null(),
            w,
            h,
        };
        let r = (|| -> Result<(), String> {
            t.image = d
                .create_image(
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
                        .usage(
                            vk::ImageUsageFlags::COLOR_ATTACHMENT
                                | vk::ImageUsageFlags::TRANSFER_SRC,
                        )
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED),
                    None,
                )
                .map_err(|e| format!("vkCreateImage: {e}"))?;
            let req = d.get_image_memory_requirements(t.image);
            let ty = self
                .mem_type(req.memory_type_bits, vk::MemoryPropertyFlags::DEVICE_LOCAL)
                .ok_or("sem memória DEVICE_LOCAL pro alvo RGBA8")?;
            t.image_mem = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(ty),
                    None,
                )
                .map_err(|e| format!("vkAllocateMemory (alvo): {e}"))?;
            d.bind_image_memory(t.image, t.image_mem, 0)
                .map_err(|e| format!("vkBindImageMemory: {e}"))?;
            t.view = d
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(t.image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
                .map_err(|e| format!("vkCreateImageView: {e}"))?;
            let views = [t.view];
            t.framebuffer = d
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(self.render_pass)
                        .attachments(&views)
                        .width(w)
                        .height(h)
                        .layers(1),
                    None,
                )
                .map_err(|e| format!("vkCreateFramebuffer: {e}"))?;
            t.buf = d
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(u64::from(w) * u64::from(h) * 4)
                        .usage(vk::BufferUsageFlags::TRANSFER_DST)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .map_err(|e| format!("vkCreateBuffer: {e}"))?;
            let breq = d.get_buffer_memory_requirements(t.buf);
            let host =
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let bty = self
                .mem_type(breq.memory_type_bits, host)
                .ok_or("sem memória HOST_VISIBLE|HOST_COHERENT")?;
            t.buf_mem = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(breq.size)
                        .memory_type_index(bty),
                    None,
                )
                .map_err(|e| format!("vkAllocateMemory (buffer): {e}"))?;
            d.bind_buffer_memory(t.buf, t.buf_mem, 0)
                .map_err(|e| format!("vkBindBufferMemory: {e}"))?;
            t.ptr = d
                .map_memory(t.buf_mem, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                .map_err(|e| format!("vkMapMemory: {e}"))? as *const u8;
            Ok(())
        })();
        match r {
            Ok(()) => {
                self.target = Some(t);
                Ok(())
            }
            Err(e) => {
                self.destroy_target(t);
                Err(e)
            }
        }
    }

    unsafe fn destroy_target(&self, t: Target) {
        let d = &self.device;
        if !t.ptr.is_null() {
            d.unmap_memory(t.buf_mem);
        }
        d.destroy_buffer(t.buf, None);
        d.free_memory(t.buf_mem, None);
        d.destroy_framebuffer(t.framebuffer, None);
        d.destroy_image_view(t.view, None);
        d.destroy_image(t.image, None);
        d.free_memory(t.image_mem, None);
    }

    /// Submete o trabalho do core + a cópia e devolve o quadro `w`×`h` em
    /// RGBA8 (origem no topo, como o Vulkan). Espera a fence (síncrono, como
    /// o readback do GL sem PBO). `None` = falha (o quadro é descartado).
    ///
    /// # Safety
    /// `ctx` é o contexto do bridge que produziu `frame`; mesma thread.
    pub unsafe fn present(
        &mut self,
        ctx: &VkContext,
        frame: &PendingFrame,
        w: u32,
        h: u32,
    ) -> Option<Presented> {
        let (w, h) = (w.max(1), h.max(1));
        #[cfg(windows)]
        if let Some(slot) = self.present_to_ring(ctx, frame, w, h) {
            return Some(Presented::Slot(slot));
        }
        self.read(ctx, frame, w, h).map(Presented::Rgba)
    }

    /// Cópia pra CPU (sem anel, ou sem slot livre).
    unsafe fn read(
        &mut self,
        ctx: &VkContext,
        frame: &PendingFrame,
        w: u32,
        h: u32,
    ) -> Option<Vec<u8>> {
        if let Err(e) = self.ensure_target(w, h) {
            log::warn!("Vulkan no processo filho: {e}");
            return None;
        }
        let t = self.target.as_ref()?;
        let d = &self.device;

        // A submissão anterior já terminou (fence esperada): pode reescrever
        // o descritor com a imagem deste quadro.
        let image_info = [vk::DescriptorImageInfo::default()
            .image_view(frame.view)
            .image_layout(frame.layout)];
        d.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&image_info)],
            &[],
        );

        d.reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty())
            .ok()?;
        d.begin_command_buffer(
            self.cmd,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )
        .ok()?;
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D {
                width: w,
                height: h,
            },
        };
        d.cmd_begin_render_pass(
            self.cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(t.framebuffer)
                .render_area(area),
            vk::SubpassContents::INLINE,
        );
        d.cmd_bind_pipeline(self.cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        d.cmd_set_viewport(
            self.cmd,
            0,
            &[vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: w as f32,
                height: h as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        d.cmd_set_scissor(self.cmd, 0, &[area]);
        d.cmd_bind_descriptor_sets(
            self.cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.set],
            &[],
        );
        d.cmd_draw(self.cmd, 3, 1, 0, 0);
        d.cmd_end_render_pass(self.cmd);

        let layers = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .layer_count(1);
        d.cmd_copy_image_to_buffer(
            self.cmd,
            t.image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            t.buf,
            &[vk::BufferImageCopy::default()
                .image_subresource(layers)
                .image_extent(vk::Extent3D {
                    width: w,
                    height: h,
                    depth: 1,
                })],
        );
        // Escrita da cópia visível pra leitura da CPU depois da fence.
        d.cmd_pipeline_barrier(
            self.cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::HOST,
            vk::DependencyFlags::empty(),
            &[],
            &[vk::BufferMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .buffer(t.buf)
                .size(vk::WHOLE_SIZE)],
            &[],
        );
        d.end_command_buffer(self.cmd).ok()?;

        // Os command buffers do core primeiro, a cópia depois, num lote só.
        // Sem eles, os semáforos do `set_image` (se houver) seguram a leitura
        // no fragment shader.
        let mut cmds = frame.cmd_buffers.clone();
        cmds.push(self.cmd);
        let stages = vec![vk::PipelineStageFlags::FRAGMENT_SHADER; frame.wait_semaphores.len()];
        let submit = vk::SubmitInfo::default()
            .wait_semaphores(&frame.wait_semaphores)
            .wait_dst_stage_mask(&stages)
            .command_buffers(&cmds);
        {
            // "lock_queue/unlock_queue" em volta de todo `vkQueueSubmit`.
            let _q = ctx.queue_lock.lock().unwrap_or_else(|p| p.into_inner());
            if let Err(e) = d.queue_submit(self.queue, &[submit], self.fence) {
                log::warn!("Vulkan no processo filho: vkQueueSubmit: {e}");
                return None;
            }
        }
        let _ = d.wait_for_fences(&[self.fence], true, u64::MAX);
        let _ = d.reset_fences(&[self.fence]);
        let len = (w * h * 4) as usize;
        Some(std::slice::from_raw_parts(t.ptr, len).to_vec())
    }
}

impl Drop for VkReadback {
    fn drop(&mut self) {
        // SAFETY: nada mais usa estes objetos; a GPU termina antes. Handles
        // nulos (montagem interrompida) são ignorados pelos `vkDestroy*`.
        unsafe {
            let d = &self.device;
            let _ = d.device_wait_idle();
            if let Some(t) = self.target.take() {
                self.destroy_target(t);
            }
            #[cfg(windows)]
            if let Some(r) = self.ring.take() {
                r.destroy(d);
            }
            d.destroy_descriptor_pool(self.desc_pool, None);
            d.destroy_pipeline(self.pipeline, None);
            d.destroy_render_pass(self.render_pass, None);
            d.destroy_render_pass(self.ring_pass, None);
            d.destroy_pipeline_layout(self.pipeline_layout, None);
            d.destroy_descriptor_set_layout(self.set_layout, None);
            d.destroy_shader_module(self.shader, None);
            d.destroy_fence(self.fence, None);
            d.destroy_command_pool(self.pool, None);
        }
    }
}

#[cfg(windows)]
impl VkReadback {
    /// Desenha o quadro num slot livre do anel (C3b). `None` = sem anel, sem
    /// slot livre ou falha — o chamador cai na cópia pra CPU.
    unsafe fn present_to_ring(
        &mut self,
        ctx: &VkContext,
        frame: &PendingFrame,
        w: u32,
        h: u32,
    ) -> Option<u32> {
        // Anel novo do app?
        let offer = crate::gl_context::OFFERED_RING
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(offer) = offer {
            let _ = self.device.device_wait_idle();
            if let Some(old) = self.ring.take() {
                old.destroy(&self.device);
            }
            match ring::VkRing::import(ctx, offer, self.ring_pass) {
                Ok(r) => {
                    log::info!(
                        "Vulkan no processo filho: anel importado ({} imagens {}x{}) — quadro sem cópia",
                        r.slots.len(),
                        r.w,
                        r.h
                    );
                    self.ring = Some(r);
                }
                Err(e) => {
                    log::warn!("Vulkan no processo filho: importar o anel: {e} — segue na cópia")
                }
            }
        }
        let ring = self.ring.as_mut()?;
        if w > ring.w || h > ring.h {
            return None;
        }
        // Slots que o app devolveu (o `free` já foi submetido lá).
        for s in std::mem::take(
            &mut *crate::gl_context::FREED_SLOTS
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
        ) {
            if let Some(slot) = ring.slots.get_mut(s as usize) {
                if slot.lent {
                    slot.lent = false;
                    slot.needs_wait = true;
                }
            }
        }
        let n = ring.slots.len();
        let idx = (0..n)
            .map(|k| (ring.write + k) % n)
            .find(|&i| !ring.slots[i].lent)?;
        ring.write = (idx + 1) % n;
        let (framebuffer, image, done, free, needs_wait) = {
            let s = &ring.slots[idx];
            (s.framebuffer, s.image, s.done, s.free, s.needs_wait)
        };
        let d = &self.device;

        let image_info = [vk::DescriptorImageInfo::default()
            .image_view(frame.view)
            .image_layout(frame.layout)];
        d.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .image_info(&image_info)],
            &[],
        );
        d.reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty())
            .ok()?;
        d.begin_command_buffer(
            self.cmd,
            &vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
        )
        .ok()?;
        let area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: vk::Extent2D {
                width: w,
                height: h,
            },
        };
        d.cmd_begin_render_pass(
            self.cmd,
            &vk::RenderPassBeginInfo::default()
                .render_pass(self.ring_pass)
                .framebuffer(framebuffer)
                .render_area(area),
            vk::SubpassContents::INLINE,
        );
        d.cmd_bind_pipeline(self.cmd, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        d.cmd_set_viewport(
            self.cmd,
            0,
            &[vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: w as f32,
                height: h as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }],
        );
        d.cmd_set_scissor(self.cmd, 0, &[area]);
        d.cmd_bind_descriptor_sets(
            self.cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.set],
            &[],
        );
        d.cmd_draw(self.cmd, 3, 1, 0, 0);
        d.cmd_end_render_pass(self.cmd);
        // *Release* pra `VK_QUEUE_FAMILY_EXTERNAL` com a transição
        // COLOR_ATTACHMENT → SHADER_READ_ONLY (o *acquire* do app repete os
        // mesmos layouts). "The destination access mask is ignored for such a
        // barrier" → 0.
        d.cmd_pipeline_barrier(
            self.cmd,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::empty())
                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(self.queue_family)
                .dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
                .image(image)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                )],
        );
        d.end_command_buffer(self.cmd).ok()?;

        let mut cmds = frame.cmd_buffers.clone();
        cmds.push(self.cmd);
        // Espera: os semáforos do core (`set_image`, no fragment shader) e,
        // se o slot já foi entregue, o `free` do app antes da escrita.
        let mut waits = frame.wait_semaphores.clone();
        let mut stages = vec![vk::PipelineStageFlags::FRAGMENT_SHADER; waits.len()];
        if needs_wait {
            waits.push(free);
            stages.push(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT);
        }
        let signals = [done];
        let submit = vk::SubmitInfo::default()
            .wait_semaphores(&waits)
            .wait_dst_stage_mask(&stages)
            .command_buffers(&cmds)
            .signal_semaphores(&signals);
        {
            let _q = ctx.queue_lock.lock().unwrap_or_else(|p| p.into_inner());
            if let Err(e) = d.queue_submit(self.queue, &[submit], self.fence) {
                log::warn!("Vulkan no processo filho: vkQueueSubmit (anel): {e}");
                return None;
            }
        }
        // A fence segura a liberação da imagem do core (`wait_sync_index`) até
        // a leitura dela terminar.
        let _ = d.wait_for_fences(&[self.fence], true, u64::MAX);
        let _ = d.reset_fences(&[self.fence]);
        let slot = &mut ring.slots[idx];
        slot.lent = true;
        slot.needs_wait = false;
        Some(idx as u32)
    }
}

/// O anel do app importado no device do filho (C3b, Windows).
#[cfg(windows)]
mod ring {
    use ash::vk;

    use crate::gl_context::InteropOffer;
    use crate::vk_context::VkContext;

    pub(super) struct Slot {
        pub image: vk::Image,
        memory: vk::DeviceMemory,
        view: vk::ImageView,
        pub framebuffer: vk::Framebuffer,
        pub done: vk::Semaphore,
        pub free: vk::Semaphore,
        /// Entregue ao app e ainda não devolvido (`InteropSlotFree`).
        pub lent: bool,
        /// Devolvido: o `free` foi submetido no app; esperar antes de escrever.
        pub needs_wait: bool,
    }

    pub(super) struct VkRing {
        /// Os handles NT ficam abertos até o anel cair: "Importing memory
        /// object payloads from Windows handles does not transfer ownership
        /// of the handle to the Vulkan implementation" (spec Vulkan).
        _offer: InteropOffer,
        pub slots: Vec<Slot>,
        pub write: usize,
        pub w: u32,
        pub h: u32,
    }

    impl VkRing {
        /// # Safety
        /// `ctx` vivo; `render_pass` é o `ring_pass` do apresentador.
        pub unsafe fn import(
            ctx: &VkContext,
            offer: InteropOffer,
            render_pass: vk::RenderPass,
        ) -> Result<Self, String> {
            let d = &ctx.device;
            let mem_win32 = ash::khr::external_memory_win32::Device::new(&ctx.instance, d);
            let sem_win32 = ash::khr::external_semaphore_win32::Device::new(&ctx.instance, d);
            let mut ring = VkRing {
                slots: Vec::new(),
                write: 0,
                w: offer.width,
                h: offer.height,
                _offer: offer,
            };
            let images: Vec<_> = ring._offer.images.clone();
            for img in images {
                match import_slot(d, &mem_win32, &sem_win32, &ring._offer, &img, render_pass) {
                    Ok(s) => ring.slots.push(s),
                    Err(e) => {
                        ring.destroy(d);
                        return Err(e);
                    }
                }
            }
            if ring.slots.is_empty() {
                return Err("anel vazio".into());
            }
            Ok(ring)
        }

        /// # Safety
        /// A GPU já largou o anel (`device_wait_idle` antes).
        pub unsafe fn destroy(self, d: &ash::Device) {
            for s in &self.slots {
                d.destroy_framebuffer(s.framebuffer, None);
                d.destroy_image_view(s.view, None);
                d.destroy_image(s.image, None);
                d.free_memory(s.memory, None);
                d.destroy_semaphore(s.done, None);
                d.destroy_semaphore(s.free, None);
            }
            // `_offer` cai aqui e fecha os handles.
        }
    }

    unsafe fn import_slot(
        d: &ash::Device,
        mem_win32: &ash::khr::external_memory_win32::Device,
        sem_win32: &ash::khr::external_semaphore_win32::Device,
        offer: &InteropOffer,
        img: &crate::gl_context::OfferedImage,
        render_pass: vk::RenderPass,
    ) -> Result<Slot, String> {
        let mut s = Slot {
            image: vk::Image::null(),
            memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            framebuffer: vk::Framebuffer::null(),
            done: vk::Semaphore::null(),
            free: vk::Semaphore::null(),
            lent: false,
            needs_wait: false,
        };
        let r = (|| -> Result<(), String> {
            // A imagem com os MESMOS parâmetros da do app (dedicada).
            let mut ext = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32);
            s.image = d
                .create_image(
                    &vk::ImageCreateInfo::default()
                        .flags(vk::ImageCreateFlags::from_raw(offer.image_flags))
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .extent(vk::Extent3D {
                            width: offer.width,
                            height: offer.height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::from_raw(offer.image_usage))
                        .sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .initial_layout(vk::ImageLayout::UNDEFINED)
                        .push_next(&mut ext),
                    None,
                )
                .map_err(|e| format!("vkCreateImage: {e}"))?;
            let mut import = vk::ImportMemoryWin32HandleInfoKHR::default()
                .handle_type(vk::ExternalMemoryHandleTypeFlags::OPAQUE_WIN32)
                .handle(img.memory as isize);
            let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(s.image);
            s.memory = d
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(img.alloc_size)
                        .memory_type_index(img.memory_type)
                        .push_next(&mut import)
                        .push_next(&mut dedicated),
                    None,
                )
                .map_err(|e| format!("vkAllocateMemory (import): {e}"))?;
            d.bind_image_memory(s.image, s.memory, 0)
                .map_err(|e| format!("vkBindImageMemory: {e}"))?;
            s.view = d
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(s.image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        ),
                    None,
                )
                .map_err(|e| format!("vkCreateImageView: {e}"))?;
            let views = [s.view];
            s.framebuffer = d
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(render_pass)
                        .attachments(&views)
                        .width(offer.width)
                        .height(offer.height)
                        .layers(1),
                    None,
                )
                .map_err(|e| format!("vkCreateFramebuffer: {e}"))?;
            for (sem, handle) in [(&mut s.done, img.done_sem), (&mut s.free, img.free_sem)] {
                *sem = d
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .map_err(|e| format!("vkCreateSemaphore: {e}"))?;
                sem_win32
                    .import_semaphore_win32_handle(
                        &vk::ImportSemaphoreWin32HandleInfoKHR::default()
                            .semaphore(*sem)
                            .handle_type(vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_WIN32)
                            .handle(handle as isize),
                    )
                    .map_err(|e| format!("vkImportSemaphoreWin32HandleKHR: {e}"))?;
            }
            let _ = mem_win32;
            Ok(())
        })();
        match r {
            Ok(()) => Ok(s),
            Err(e) => {
                d.destroy_framebuffer(s.framebuffer, None);
                d.destroy_image_view(s.view, None);
                d.destroy_image(s.image, None);
                d.free_memory(s.memory, None);
                d.destroy_semaphore(s.done, None);
                d.destroy_semaphore(s.free, None);
                Err(e)
            }
        }
    }
}
