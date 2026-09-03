//! `Renderer`: sobe o frame do core numa textura wgpu e desenha um quad
//! centralizado com letterbox. Não é dono de `Device`/`Queue` — o shell (ou
//! o exemplo) cria o contexto e passa nas chamadas.

use domain::frame_source::{Frame, FrameOrigin};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    scale: [f32; 2],
    _pad: [f32; 2],
}

struct FrameTex {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    aspect: f32,
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    frame: Option<FrameTex>,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video-surface"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-surface bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video-surface layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video-surface pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("video-surface sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("video-surface uniforms"),
            contents: bytemuck::bytes_of(&Uniforms {
                scale: [1.0, 1.0],
                _pad: [0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            pipeline,
            bgl,
            sampler,
            uniform_buf,
            frame: None,
        }
    }

    /// True se há um frame carregado (algo pra desenhar).
    pub fn has_frame(&self) -> bool {
        self.frame.is_some()
    }

    /// Sobe um novo frame do core. `HardwareTexture` ainda não é suportado
    /// (passo 4 da etapa 02) — é ignorado.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame) {
        let (data, pitch, format) = match &frame.origin {
            FrameOrigin::SoftwareRawBuffer {
                data,
                pitch,
                format,
            } => (data, *pitch, *format),
            FrameOrigin::HardwareTexture(_) => {
                log::warn!("HardwareTexture ainda não suportado no renderer");
                return;
            }
        };

        let w = frame.metadata.native_width;
        let h = frame.metadata.native_height;
        if w == 0 || h == 0 {
            return;
        }

        let rgba = crate::to_rgba8(data, w, h, pitch, format);

        let needs_new = self
            .frame
            .as_ref()
            .map(|f| f.width != w || f.height != h)
            .unwrap_or(true);

        if needs_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("core frame"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("video-surface bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.frame = Some(FrameTex {
                texture,
                bind_group,
                width: w,
                height: h,
                aspect: 1.0,
            });
        }

        let ft = self.frame.as_mut().unwrap();
        ft.aspect = if frame.metadata.aspect_ratio > 0.0 {
            frame.metadata.aspect_ratio
        } else {
            w as f32 / h as f32
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &ft.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Desenha (com letterbox) no `view`. Limpa pra preto — no pause, o
    /// caller simplesmente para de chamar `upload` e o último frame fica.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        target_w: u32,
        target_h: u32,
    ) {
        if let Some(ft) = &self.frame {
            let scale = letterbox_scale(ft.aspect, target_w, target_h);
            queue.write_buffer(
                &self.uniform_buf,
                0,
                bytemuck::bytes_of(&Uniforms {
                    scale,
                    _pad: [0.0, 0.0],
                }),
            );
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("video"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("video pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            if let Some(ft) = &self.frame {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &ft.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
    }
}

fn letterbox_scale(img_aspect: f32, target_w: u32, target_h: u32) -> [f32; 2] {
    let target_aspect = target_w.max(1) as f32 / target_h.max(1) as f32;
    if img_aspect > target_aspect {
        [1.0, target_aspect / img_aspect]
    } else {
        [img_aspect / target_aspect, 1.0]
    }
}

/// Cria um `Device`/`Queue`. `compatible_surface` pra escolher um adapter que
/// saiba desenhar nela. `None` = qualquer adapter (headless/testes).
pub fn create_device(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Option<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    create_device_with(instance, compatible_surface, wgpu::Features::empty())
        .map(|(a, d, q, _)| (a, d, q))
}

/// Como [`create_device`], mas tenta habilitar `wanted` (features nativas
/// opcionais); o 4º elemento diz quais entraram. Cai pro conjunto vazio se o
/// adapter não suportar — nunca falha por causa de uma feature opcional.
pub fn create_device_with(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
    wanted: wgpu::Features,
) -> Option<(wgpu::Adapter, wgpu::Device, wgpu::Queue, wgpu::Features)> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface,
        ..Default::default()
    }))
    .ok()?;

    let granted = wanted & adapter.features();
    let mk = |feats| {
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("video-surface device"),
            required_limits: wgpu::Limits::downlevel_defaults(),
            required_features: feats,
            ..Default::default()
        }))
    };
    match mk(granted) {
        Ok((device, queue)) => Some((adapter, device, queue, granted)),
        Err(_) if !granted.is_empty() => {
            let (device, queue) = mk(wgpu::Features::empty()).ok()?;
            Some((adapter, device, queue, wgpu::Features::empty()))
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::frame_source::{FrameMetadata, SoftwarePixelFormat};

    fn solid_rgb565_frame(color: u16, w: u32, h: u32) -> Frame {
        let mut data = Vec::with_capacity((w * h * 2) as usize);
        for _ in 0..w * h {
            data.extend_from_slice(&color.to_le_bytes());
        }
        Frame {
            origin: FrameOrigin::SoftwareRawBuffer {
                data,
                pitch: w * 2,
                format: SoftwarePixelFormat::Rgb565,
            },
            metadata: FrameMetadata {
                native_width: w,
                native_height: h,
                aspect_ratio: w as f32 / h as f32,
                rotation_degrees: 0,
            },
        }
    }

    #[test]
    fn renders_frame_to_offscreen_target() {
        let instance = wgpu::Instance::default();
        let Some((_adapter, device, queue)) = create_device(&instance, None) else {
            eprintln!("sem adapter wgpu disponível — pulando teste de render");
            return;
        };

        let fmt = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = Renderer::new(&device, fmt);
        // frame verde puro, mesma proporção do alvo -> preenche a tela toda
        renderer.upload(&device, &queue, &solid_rgb565_frame(0x07E0, 8, 8));

        let (tw, th) = (8u32, 8u32);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width: tw,
                height: th,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: fmt,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        renderer.render(&device, &queue, &view, tw, th);

        // Readback: bytes_per_row tem que respeitar COPY_BYTES_PER_ROW_ALIGNMENT (256).
        let row_stride = tw * 4;
        let padded = row_stride.next_multiple_of(256);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * th) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(th),
                },
            },
            wgpu::Extent3d {
                width: tw,
                height: th,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);

        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = slice.get_mapped_range().unwrap();

        let center = (th / 2) as usize * padded as usize + (tw / 2) as usize * 4;
        let px = &data[center..center + 4];
        assert!(
            px[0] < 20 && px[1] > 200 && px[2] < 20,
            "esperava verde no centro, veio {px:?}"
        );
    }
}
