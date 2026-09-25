//! WGSL fixo e construtores de pipeline (composição, rotação, flip, blit, passes).

use super::*;

pub(super) const COMP_WGSL: &str = r#"
struct Rect { c: vec4<f32> }; // c.xy = centro (clip), c.zw = meia-extensão (clip)
@group(0) @binding(0) var<uniform> R: Rect;
@group(0) @binding(1) var Tex: texture_2d<f32>;
@group(0) @binding(2) var Smp: sampler;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs(@location(0) p: vec4<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var o: VOut;
    let n = p.xy * 2.0 - vec2<f32>(1.0, 1.0);
    o.pos = vec4<f32>(R.c.xy + n * R.c.zw, 0.0, 1.0);
    o.uv = uv;
    return o;
}
@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> { return textureSample(Tex, Smp, v.uv); }
"#;

/// Rotaciona uma textura em múltiplos de 90° (`SET_ROTATION`). Reusa o
/// `comp.bgl` (uniforme no binding 0, textura 1, sampler 2). `R.c.xy` = (cos,
/// sin) da rotação aplicada em `uv - 0.5` (giro do UV = giro inverso da imagem).
pub(super) const ROT_WGSL: &str = r#"
struct Rot { c: vec4<f32> };
@group(0) @binding(0) var<uniform> R: Rot;
@group(0) @binding(1) var Tex: texture_2d<f32>;
@group(0) @binding(2) var Smp: sampler;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs(@location(0) p: vec4<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(p.xy * 2.0 - vec2<f32>(1.0, 1.0), 0.0, 1.0);
    // rotação linear do UV em volta do centro → per-vertex + interpolação é
    // exata. (o binding 0 aqui é lido só no vertex, igual ao COMP_WGSL.)
    let d = uv - vec2<f32>(0.5, 0.5);
    o.uv = vec2<f32>(d.x * R.c.x - d.y * R.c.y, d.x * R.c.y + d.y * R.c.x)
         + vec2<f32>(0.5, 0.5);
    return o;
}
@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> { return textureSample(Tex, Smp, v.uv); }
"#;

pub(super) fn rotate_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rot layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rot"),
        source: wgpu::ShaderSource::Wgsl(ROT_WGSL.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rot pipeline"),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: QUAD_STRIDE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 16,
                        shader_location: 1,
                    },
                ],
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: FMT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Recorta o quadro de um `dma_buf` de core GL e, se preciso, inverte o Y
/// (fullscreen triangle). O buffer tem o tamanho MÁXIMO do core
/// (`max_width`×`max_height`) e o quadro ocupa só as primeiras `h` linhas e
/// `w` colunas da memória (viewport do GL em (0,0)) — `P.xy` = `(w/tw,
/// h/th)`. `P.z` = 1 → core bottom-left (GL nativo): a linha 0 da memória vai
/// pra BAIXO da saída. Em WebGPU a NDC tem y pra cima e o uv (0,0) é a 1ª
/// linha da textura (spec WebGPU, "Coordinate Systems").
pub(super) const FLIP_WGSL: &str = r#"
@group(0) @binding(0) var T: texture_2d<f32>;
@group(0) @binding(1) var S: sampler;
@group(0) @binding(2) var<uniform> P: vec4<f32>;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: V;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    let up = (p[i].y + 1.0) * 0.5; // 0 embaixo, 1 em cima
    let v = select(1.0 - up, up, P.z > 0.5);
    o.uv = vec2<f32>((p[i].x + 1.0) * 0.5 * P.x, v * P.y);
    return o;
}
@fragment fn fs(v: V) -> @location(0) vec4<f32> { return textureSample(T, S, v.uv); }
"#;

pub(super) struct FlipPipe {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) bgl: wgpu::BindGroupLayout,
    /// `vec4(w/tw, h/th, flip, 0)` — reescrito a cada quadro de interop.
    pub(super) params: wgpu::Buffer,
}

pub(super) fn build_flip(device: &wgpu::Device) -> FlipPipe {
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("flip bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(16),
                },
                count: None,
            },
        ],
    });
    let params = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("flip params"),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("flip layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("flip"),
        source: wgpu::ShaderSource::Wgsl(FLIP_WGSL.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("flip pipeline"),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &m,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &m,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: FMT,
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
    FlipPipe {
        pipeline,
        bgl,
        params,
    }
}

/// Pipeline de blit (mesmo shader do composite: quad posicionado por um `Rect`
/// uniforme, sampla uma textura) pro formato de uma surface nativa — usa a
/// `bgl` do composite, então o bind group é o mesmo layout.
pub(super) fn blit_pipeline(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("blit layout"),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blit"),
        source: wgpu::ShaderSource::Wgsl(COMP_WGSL.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit pipeline"),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: QUAD_STRIDE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 16,
                        shader_location: 1,
                    },
                ],
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn build_composite(device: &wgpu::Device) -> Composite {
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("comp bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
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
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("comp layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("comp"),
        source: wgpu::ShaderSource::Wgsl(COMP_WGSL.into()),
    });
    let mk = |blend: Option<wgpu::BlendState>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("comp pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: QUAD_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 16,
                            shader_location: 1,
                        },
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FMT,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    let rect_buf = || {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comp rect"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    Composite {
        game_pipeline: mk(None),
        bezel_pipeline: mk(Some(wgpu::BlendState::ALPHA_BLENDING)),
        bgl,
        rect_game: rect_buf(),
        rect_bezel: rect_buf(),
        target: None,
    }
}

/// Entradas da BGL de um passe: binding 0 e 1 (uniformes) + 1 par
/// textura/sampler pra cada sampler declarado (bindings `2+2i` / `3+2i`).
pub(super) fn pass_bgl(device: &wgpu::Device, textures: &[TextureBind]) -> wgpu::BindGroupLayout {
    let buf = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    let mut entries = vec![buf(0), buf(1)];
    for t in textures {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: t.tex_binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: t.samp_binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("etapa04 bgl"),
        entries: &entries,
    })
}

pub(super) fn build_pass(device: &wgpu::Device, spec: PassSpec) -> Option<Pass> {
    // Builtins não listam texturas → um `Source` implícito em 2/3.
    let textures = if spec.textures.is_empty() {
        vec![TextureBind {
            name: "Source".into(),
            semantic: TextureSemantic::Source,
            tex_binding: 2,
            samp_binding: 3,
        }]
    } else {
        spec.textures
    };
    let bgl = pass_bgl(device, &textures);
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("etapa04 layout"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let vs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("etapa04 vs"),
        source: wgpu::ShaderSource::Wgsl(spec.vs_wgsl.into()),
    });
    let fs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("etapa04 fs"),
        source: wgpu::ShaderSource::Wgsl(spec.fs_wgsl.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("etapa04 pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &vs,
            entry_point: Some("main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: QUAD_STRIDE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 16,
                        shader_location: 1,
                    },
                ],
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &fs,
            entry_point: Some("main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: spec.fmt,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    // buffer 0 (binding 0 = `Push`) e buffer 1 (binding 1 = `UBO`). Tamanho do
    // bloco slang refletido, ou dummy de 16 bytes.
    let (mut size0, mut size1) = match &spec.uniform {
        UniformMode::Fixed => (64u64, 16u64),
        UniformMode::Slang(_) => (16u64, 16u64),
    };
    if let UniformMode::Slang(blocks) = &spec.uniform {
        for (b, l) in blocks {
            let s = (l.size as u64).max(16);
            if *b == 0 {
                size0 = s;
            } else {
                size1 = s;
            }
        }
    }
    let mk = |size| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("etapa04 uniform"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    };
    Some(Pass {
        pipeline,
        bgl,
        scale_x: spec.scale_x,
        scale_y: spec.scale_y,
        linear: spec.linear,
        wrap: spec.wrap,
        uniform: spec.uniform,
        ubuf: [mk(size0), mk(size1)],
        textures,
        fmt: spec.fmt,
        frame_count_mod: spec.frame_count_mod,
        feedback: spec.feedback,
        target: None,
        feedback_target: None,
        bind_group: None,
        bound: false,
        mipmap_input: spec.mipmap_input,
        target_mip_levels: 1,
    })
}
