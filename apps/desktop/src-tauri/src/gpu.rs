//! Processamento GPU do frame do core (wgpu **offscreen** — sem surface, por
//! isso não conflita com o GTK; ver `video.rs`). Etapa 04 — cadeia de shader.
//!
//! Cada passe é um quad fullscreen amostrando a saída do passe anterior, com
//! escala/filtro por passe e os uniforms semânticos do libretro. Duas fontes
//! de passe:
//!   - **embutidos** (`plain`/`crt`/`lcd`) — fragmento WGSL fixo, uniforms de
//!     64 bytes (`source_size`/`output_size`/`orig_size`/`frame`);
//!   - **`.slangp`** — `shader_slang` parseia o preset, preprocessa e compila
//!     cada `.slang` (GLSL→WGSL via naga); o bloco uniforme é montado por
//!     reflection (`MVP`, `SourceSize`, `FrameCount`, parâmetros por nome).
//!
//! `REEMU_SHADER=plain|crt|lcd|/caminho/preset.slangp`. Qualquer falha → `None`
//! e o `poll_frame` cai no caminho CPU (`to_rgba8`).

use std::collections::HashMap;
use std::path::Path;

use domain::frame_source::{Frame, FrameOrigin};
use shader_slang::{Scale, UniformFieldKind, UniformLayout};
use video_surface::to_rgba8;

const FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ROW_ALIGN: u32 = 256;
const MAX_OUT_PIXELS: u32 = 8_000_000;

/// Quad [0,1]×[0,1] em triangle-strip: `vec4 Position` + `vec2 TexCoord`.
/// (0,0) de `TexCoord` = topo-esquerda, casando com a textura.
#[rustfmt::skip]
const QUAD: [f32; 24] = [
    0.0, 1.0, 0.0, 1.0,   0.0, 0.0,
    1.0, 1.0, 0.0, 1.0,   1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,   0.0, 1.0,
    1.0, 0.0, 0.0, 1.0,   1.0, 1.0,
];
const QUAD_STRIDE: u64 = 24;

const BUILTIN_VS: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn main(@location(0) position: vec4<f32>, @location(1) texcoord: vec2<f32>) -> VOut {
    var o: VOut;
    o.pos = vec4<f32>(position.xy * 2.0 - vec2<f32>(1.0, 1.0), 0.0, 1.0);
    o.uv = texcoord;
    return o;
}
"#;

const BUILTIN_FS_PRELUDE: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
struct Uniforms {
    source_size: vec4<f32>,
    output_size: vec4<f32>,
    orig_size: vec4<f32>,
    frame: vec4<f32>,
};
@group(0) @binding(0) var<uniform> U: Uniforms;
@group(0) @binding(1) var Source: texture_2d<f32>;
@group(0) @binding(2) var Samp: sampler;
"#;

struct BuiltinPass {
    scale: f32,
    linear: bool,
    fs: &'static str,
}
struct Builtin {
    name: &'static str,
    passes: &'static [BuiltinPass],
}

const PLAIN: Builtin = Builtin {
    name: "plain",
    passes: &[BuiltinPass {
        scale: 1.0,
        linear: false,
        fs: "@fragment fn main(v: VOut) -> @location(0) vec4<f32> { return textureSample(Source, Samp, v.uv); }",
    }],
};
const CRT: Builtin = Builtin {
    name: "crt",
    passes: &[
        BuiltinPass {
            scale: 1.0,
            linear: false,
            fs: r#"
@fragment fn main(v: VOut) -> @location(0) vec4<f32> {
    let dx = U.source_size.z;
    var c = textureSample(Source, Samp, v.uv).rgb * 0.5;
    c += textureSample(Source, Samp, v.uv + vec2<f32>(dx, 0.0)).rgb * 0.25;
    c += textureSample(Source, Samp, v.uv - vec2<f32>(dx, 0.0)).rgb * 0.25;
    return vec4<f32>(c, 1.0);
}"#,
        },
        BuiltinPass {
            scale: 3.0,
            linear: true,
            fs: r#"
@fragment fn main(v: VOut) -> @location(0) vec4<f32> {
    var col = textureSample(Source, Samp, v.uv).rgb;
    col = pow(col, vec3<f32>(1.15));
    let line = fract(v.uv.y * U.source_size.y);
    col *= 1.0 - 0.30 * pow(sin(line * 3.14159265), 2.0);
    let m = i32(v.uv.x * U.output_size.x) % 3;
    var mask = vec3<f32>(0.94, 0.94, 0.94);
    if (m == 0) { mask.r = 1.06; } else if (m == 1) { mask.g = 1.06; } else { mask.b = 1.06; }
    col *= mask;
    let d = v.uv - vec2<f32>(0.5, 0.5);
    col *= 1.0 - dot(d, d) * 0.35;
    col *= 1.25;
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}"#,
        },
    ],
};
const LCD: Builtin = Builtin {
    name: "lcd",
    passes: &[BuiltinPass {
        scale: 2.0,
        linear: false,
        fs: r#"
@fragment fn main(v: VOut) -> @location(0) vec4<f32> {
    var col = textureSample(Source, Samp, v.uv).rgb;
    let g = fract(v.uv * U.source_size.xy);
    let grid = smoothstep(0.0, 0.12, g.x) * smoothstep(0.0, 0.12, g.y);
    col *= mix(0.88, 1.0, grid);
    col = pow(col, vec3<f32>(0.95));
    return vec4<f32>(col * 1.05, 1.0);
}"#,
    }],
};
const BUILTINS: &[&Builtin] = &[&PLAIN, &CRT, &LCD];

pub fn builtin_preset_names() -> Vec<String> {
    BUILTINS.iter().map(|p| p.name.to_string()).collect()
}

/// Como os buffers uniformes do passe são preenchidos a cada frame.
/// Bindings: 0 = `Push`/params ou os 64 bytes fixos; 3 = `UBO`/global (slang).
enum UniformMode {
    /// 64 bytes: `source_size`, `output_size`, `orig_size`, `frame`.
    Fixed,
    /// `(binding, layout)` de cada bloco refletido do `.slang`.
    Slang(Vec<(u32, UniformLayout)>),
}

struct PassSpec {
    scale_x: Scale,
    scale_y: Scale,
    linear: bool,
    vs_wgsl: String,
    fs_wgsl: String,
    uniform: UniformMode,
}

/// Resultado de `build_specs`: preset resolvido pronto pra montar os passes.
struct BuiltSpecs {
    name: String,
    /// valor atual de cada parâmetro (default do `#pragma` + override do `.slangp`).
    params: HashMap<String, f32>,
    /// metadados dos `#pragma parameter` (label/min/max/step) pra UI.
    meta: Vec<shader_slang::Parameter>,
    passes: Vec<PassSpec>,
}

struct Pass {
    pipeline: wgpu::RenderPipeline,
    scale_x: Scale,
    scale_y: Scale,
    linear: bool,
    uniform: UniformMode,
    /// buffers uniformes: `[0]` = binding 0, `[1]` = binding 3.
    ubuf: [wgpu::Buffer; 2],
    target: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    bind_group: Option<wgpu::BindGroup>,
    bound: bool,
}

/// Retângulo do jogo (viewport) dentro da moldura, em pixels da imagem.
#[derive(Clone, Copy)]
pub struct DecoViewport {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

struct Decoration {
    view: wgpu::TextureView,
    w: u32,
    h: u32,
    /// `None` = viewport padrão (centralizado, altura cheia, proporção do core).
    vp: Option<DecoViewport>,
}

/// Pipelines do passe de composição da moldura (`game` sem blend, `bezel` com
/// alpha). Criados uma vez.
struct Composite {
    bgl: wgpu::BindGroupLayout,
    game_pipeline: wgpu::RenderPipeline,
    bezel_pipeline: wgpu::RenderPipeline,
    rect_game: wgpu::Buffer,
    rect_bezel: wgpu::Buffer,
    target: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
}

pub struct FrameProcessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    quad: wgpu::Buffer,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    preset_name: String,
    preset_source: String,
    /// Parâmetros globais do preset `.slangp` (nome → valor atual).
    params: HashMap<String, f32>,
    /// Metadados dos `#pragma parameter` (label/min/max/step) — pra UI.
    param_meta: Vec<shader_slang::Parameter>,
    passes: Vec<Pass>,
    core_tex: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    readback: Option<(wgpu::Buffer, u32, u32, u32)>,
    frame_count: u64,
    comp: Composite,
    decoration: Option<Decoration>,
}

impl FrameProcessor {
    pub fn new() -> Option<Self> {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            log::info!("REEMU_NO_GPU: processamento de frame na GPU desligado");
            return None;
        }
        let instance = wgpu::Instance::default();
        let (adapter, device, queue) = video_surface::create_device(&instance, None)?;
        log::info!("GPU (etapa 04): {}", adapter.get_info().name);

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("etapa04 bgl"),
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
                // binding 3 = `UBO`/global do slang; dummy pros embutidos.
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("etapa04 layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let quad = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("etapa04 quad"),
            size: std::mem::size_of_val(&QUAD) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&quad, 0, f32s_bytes(&QUAD));

        let mk_sampler = |f| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("etapa04 sampler"),
                mag_filter: f,
                min_filter: f,
                ..Default::default()
            })
        };

        let want = std::env::var("REEMU_SHADER").unwrap_or_else(|_| "plain".into());
        let (built, source) = match build_specs(&want) {
            Ok(b) => (b, want.clone()),
            Err(e) => {
                log::warn!("shader '{want}': {e} — usando 'plain'");
                (build_specs("plain").ok()?, "plain".to_string())
            }
        };
        let BuiltSpecs {
            name: preset_name,
            params,
            meta: param_meta,
            passes: specs,
        } = built;
        log::info!(
            "shader: preset '{preset_name}' ({} passe(s), {} parâmetro(s))",
            specs.len(),
            param_meta.len()
        );

        let passes = specs
            .into_iter()
            .map(|s| build_pass(&device, &layout, s))
            .collect::<Option<Vec<_>>>()?;

        let comp = build_composite(&device);

        Some(Self {
            sampler_nearest: mk_sampler(wgpu::FilterMode::Nearest),
            sampler_linear: mk_sampler(wgpu::FilterMode::Linear),
            device,
            queue,
            bgl,
            layout,
            quad,
            preset_name,
            preset_source: source,
            params,
            param_meta,
            passes,
            core_tex: None,
            readback: None,
            frame_count: 0,
            comp,
            decoration: None,
        })
    }

    /// Proporção de exibição imposta pela moldura (`w/h` da imagem), se houver.
    pub fn decoration_aspect(&self) -> Option<f32> {
        self.decoration
            .as_ref()
            .map(|d| d.w as f32 / d.h.max(1) as f32)
    }

    /// Define (ou tira, com `None`) a moldura. `rgba` é a imagem RGBA8
    /// top-to-bottom; `vp` é o viewport do jogo em pixels da imagem (`.cfg`).
    pub fn set_decoration(&mut self, deco: Option<(Vec<u8>, u32, u32, Option<DecoViewport>)>) {
        let Some((rgba, w, h, vp)) = deco else {
            self.decoration = None;
            return;
        };
        if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
            self.decoration = None;
            return;
        }
        let (tex, view) = new_tex(
            &self.device,
            w,
            h,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
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
        self.readback = None;
        self.decoration = Some(Decoration { view, w, h, vp });
    }

    /// Nome curto pra exibição (builtin ou stem do `.slangp`).
    pub fn preset_name(&self) -> &str {
        &self.preset_name
    }

    /// O que foi passado pra `set_preset` (builtin ou caminho) — pra dedup.
    pub fn preset_source(&self) -> &str {
        &self.preset_source
    }

    pub fn set_preset(&mut self, name: &str) -> Result<(), String> {
        let BuiltSpecs {
            name: preset_name,
            params,
            meta,
            passes: specs,
        } = build_specs(name).map_err(|e| format!("shader: {e}"))?;
        let passes = specs
            .into_iter()
            .map(|s| build_pass(&self.device, &self.layout, s))
            .collect::<Option<Vec<_>>>()
            .ok_or("shader: falha ao criar os pipelines")?;
        self.passes = passes;
        self.params = params;
        self.param_meta = meta;
        self.preset_name = preset_name;
        self.preset_source = name.to_string();
        self.readback = None;
        log::info!("preset de shader → '{}'", self.preset_name);
        Ok(())
    }

    /// Metadados dos parâmetros do preset atual (`#pragma parameter`).
    pub fn shader_param_meta(&self) -> &[shader_slang::Parameter] {
        &self.param_meta
    }

    /// Valor atual de um parâmetro (default se não houver meta).
    pub fn shader_param_value(&self, name: &str) -> Option<f32> {
        self.params.get(name).copied()
    }

    /// Ajusta um parâmetro do preset em runtime (clampa em [min, max] do
    /// `#pragma`). Sem rebuild de pipeline — o valor entra no uniform buffer
    /// no próximo `process()`. Ignora nomes que o preset não declara.
    pub fn set_shader_param(&mut self, name: &str, value: f32) -> bool {
        let Some(m) = self.param_meta.iter().find(|p| p.name == name) else {
            return false;
        };
        let (lo, hi) = if m.min <= m.max {
            (m.min, m.max)
        } else {
            (m.max, m.min)
        };
        self.params.insert(name.to_string(), value.clamp(lo, hi));
        self.readback = None;
        true
    }

    pub fn process(&mut self, frame: &Frame) -> Option<(u32, u32, Vec<u8>)> {
        let nw = frame.metadata.native_width;
        let nh = frame.metadata.native_height;
        if nw == 0 || nh == 0 {
            return None;
        }
        let FrameOrigin::SoftwareRawBuffer {
            data,
            pitch,
            format,
        } = &frame.origin
        else {
            return None;
        };
        let rgba = to_rgba8(data, nw, nh, *pitch, *format);
        if rgba.len() != (nw * nh * 4) as usize {
            return None;
        }

        // dimensões de cada alvo
        let mut sizes = Vec::with_capacity(self.passes.len());
        let (mut cw, mut ch) = (nw, nh);
        for p in &self.passes {
            cw = axis_size(p.scale_x, cw, nw).max(1);
            ch = axis_size(p.scale_y, ch, nh).max(1);
            sizes.push((cw, ch));
        }
        let (fw, fh) = *sizes.last()?;
        if fw * fh > MAX_OUT_PIXELS {
            return None;
        }

        self.ensure_core_tex(nw, nh);
        self.upload_core(&rgba, nw, nh);
        self.frame_count = self.frame_count.wrapping_add(1);

        for (idx, (pw, ph)) in sizes.iter().copied().enumerate() {
            self.ensure_target(idx, pw, ph);
            let (in_w, in_h) = if idx == 0 { (nw, nh) } else { sizes[idx - 1] };
            match &self.passes[idx].uniform {
                UniformMode::Fixed => {
                    let mut b = vec![0u8; 64];
                    b[0..16].copy_from_slice(f32s_bytes(&size_vec(in_w, in_h)));
                    b[16..32].copy_from_slice(f32s_bytes(&size_vec(pw, ph)));
                    b[32..48].copy_from_slice(f32s_bytes(&size_vec(nw, nh)));
                    b[48..64].copy_from_slice(f32s_bytes(&[
                        self.frame_count as f32,
                        1.0,
                        0.0,
                        0.0,
                    ]));
                    self.queue.write_buffer(&self.passes[idx].ubuf[0], 0, &b);
                }
                UniformMode::Slang(blocks) => {
                    for (binding, layout) in blocks {
                        let b = fill_slang(
                            layout,
                            &self.params,
                            (in_w, in_h),
                            (pw, ph),
                            (nw, nh),
                            (fw, fh),
                            self.frame_count,
                        );
                        let slot = if *binding == 0 { 0 } else { 1 };
                        self.queue.write_buffer(&self.passes[idx].ubuf[slot], 0, &b);
                    }
                }
            }
        }

        for idx in 0..self.passes.len() {
            if self.passes[idx].bind_group.is_some() && self.passes[idx].bound {
                continue;
            }
            let sampler = if self.passes[idx].linear {
                &self.sampler_linear
            } else {
                &self.sampler_nearest
            };
            let input_view: &wgpu::TextureView = if idx == 0 {
                &self.core_tex.as_ref()?.1
            } else {
                &self.passes[idx - 1].target.as_ref()?.1
            };
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("etapa04 bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.passes[idx].ubuf[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.passes[idx].ubuf[1].as_entire_binding(),
                    },
                ],
            });
            self.passes[idx].bind_group = Some(bg);
            self.passes[idx].bound = true;
        }

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("etapa04"),
            });
        for idx in 0..self.passes.len() {
            let (_, view, _, _) = self.passes[idx].target.as_ref()?;
            let bg = self.passes[idx].bind_group.as_ref()?;
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("etapa04 pass"),
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
            rp.set_pipeline(&self.passes[idx].pipeline);
            rp.set_bind_group(0, bg, &[]);
            rp.set_vertex_buffer(0, self.quad.slice(..));
            rp.draw(0..4, 0..1);
        }

        // Composição da moldura (etapa 04 fatia 4), se houver uma.
        let (out_w, out_h, use_comp) = if let Some((dw, dh, vp)) =
            self.decoration.as_ref().map(|d| (d.w, d.h, d.vp))
        {
            let dar = if frame.metadata.aspect_ratio > 0.0 {
                frame.metadata.aspect_ratio
            } else {
                nw as f32 / nh.max(1) as f32
            };
            let (cx, cy, hw, hh) = match vp {
                Some(v) => (
                    (v.x + v.w / 2.0) / dw as f32 * 2.0 - 1.0,
                    1.0 - (v.y + v.h / 2.0) / dh as f32 * 2.0,
                    (v.w / dw as f32).clamp(0.0, 1.0),
                    (v.h / dh as f32).clamp(0.0, 1.0),
                ),
                None => {
                    let vw = (dh as f32 * dar).min(dw as f32);
                    (0.0, 0.0, vw / dw as f32, 1.0)
                }
            };
            self.queue
                .write_buffer(&self.comp.rect_game, 0, f32s_bytes(&[cx, cy, hw, hh]));
            self.queue
                .write_buffer(&self.comp.rect_bezel, 0, f32s_bytes(&[0.0, 0.0, 1.0, 1.0]));
            self.ensure_comp_target(dw, dh);

            let game_view = &self.passes.last()?.target.as_ref()?.1;
            let deco_view = &self.decoration.as_ref()?.view;
            let comp_view = &self.comp.target.as_ref()?.1;
            let mk_bg = |rect: &wgpu::Buffer, tex: &wgpu::TextureView| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("comp bg"),
                    layout: &self.comp.bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: rect.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(tex),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                        },
                    ],
                })
            };
            let game_bg = mk_bg(&self.comp.rect_game, game_view);
            let bezel_bg = mk_bg(&self.comp.rect_bezel, deco_view);
            {
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("comp pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: comp_view,
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
                rp.set_vertex_buffer(0, self.quad.slice(..));
                rp.set_pipeline(&self.comp.game_pipeline);
                rp.set_bind_group(0, &game_bg, &[]);
                rp.draw(0..4, 0..1);
                rp.set_pipeline(&self.comp.bezel_pipeline);
                rp.set_bind_group(0, &bezel_bg, &[]);
                rp.draw(0..4, 0..1);
            }
            (dw, dh, true)
        } else {
            (fw, fh, false)
        };

        self.ensure_readback(out_w, out_h);
        let (rb, _, _, padded) = self.readback.as_ref()?;
        let padded = *padded;
        let src_tex = if use_comp {
            &self.comp.target.as_ref()?.0
        } else {
            &self.passes.last()?.target.as_ref()?.0
        };
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: src_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: rb,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(out_h),
                },
            },
            wgpu::Extent3d {
                width: out_w,
                height: out_h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let slice = rb.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let mapped = slice.get_mapped_range().ok()?;
        let row = (out_w * 4) as usize;
        let mut out = vec![0u8; row * out_h as usize];
        for y in 0..out_h as usize {
            out[y * row..(y + 1) * row].copy_from_slice(&mapped[y * padded as usize..][..row]);
        }
        drop(mapped);
        rb.unmap();
        Some((out_w, out_h, out))
    }

    fn ensure_comp_target(&mut self, w: u32, h: u32) {
        if matches!(&self.comp.target, Some((_, _, tw, th)) if *tw == w && *th == h) {
            return;
        }
        let (t, v) = new_tex(
            &self.device,
            w,
            h,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        self.comp.target = Some((t, v, w, h));
    }

    fn ensure_core_tex(&mut self, w: u32, h: u32) {
        if matches!(&self.core_tex, Some((_, _, tw, th)) if *tw == w && *th == h) {
            return;
        }
        let (t, v) = new_tex(
            &self.device,
            w,
            h,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.core_tex = Some((t, v, w, h));
        for p in &mut self.passes {
            p.bound = false;
        }
    }

    fn upload_core(&self, rgba: &[u8], w: u32, h: u32) {
        let Some((tex, _, _, _)) = &self.core_tex else {
            return;
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
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

    fn ensure_target(&mut self, idx: usize, w: u32, h: u32) {
        if matches!(&self.passes[idx].target, Some((_, _, tw, th)) if *tw == w && *th == h) {
            return;
        }
        let (t, v) = new_tex(
            &self.device,
            w,
            h,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        self.passes[idx].target = Some((t, v, w, h));
        if let Some(next) = self.passes.get_mut(idx + 1) {
            next.bound = false;
        }
    }

    fn ensure_readback(&mut self, w: u32, h: u32) {
        if matches!(&self.readback, Some((_, rw, rh, _)) if *rw == w && *rh == h) {
            return;
        }
        let padded = (w * 4).next_multiple_of(ROW_ALIGN);
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("etapa04 readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.readback = Some((buf, w, h, padded));
    }
}

/// `plain|crt|lcd` ou um caminho `.slangp` → preset pronto (nome, params,
/// metadados dos parâmetros, passes).
fn build_specs(want: &str) -> Result<BuiltSpecs, String> {
    if let Some(b) = BUILTINS.iter().find(|b| b.name == want) {
        let passes = b
            .passes
            .iter()
            .map(|p| PassSpec {
                scale_x: Scale::Source(p.scale),
                scale_y: Scale::Source(p.scale),
                linear: p.linear,
                vs_wgsl: BUILTIN_VS.to_string(),
                fs_wgsl: format!("{BUILTIN_FS_PRELUDE}\n{}", p.fs),
                uniform: UniformMode::Fixed,
            })
            .collect();
        return Ok(BuiltSpecs {
            name: b.name.to_string(),
            params: HashMap::new(),
            meta: Vec::new(),
            passes,
        });
    }

    let path = Path::new(want);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "slangp" => {}
        "glslp" | "cgp" => {
            return Err(format!(
                "preset '.{ext}' (GLSL/Cg) não é suportado — use a pasta `shaders_slang` do RetroArch (.slangp)"
            ))
        }
        "slang" | "glsl" | "cg" => {
            return Err("escolha o arquivo de preset `.slangp`, não o `.slang`".into())
        }
        "" if path.is_dir() => return Err("escolha um arquivo `.slangp`, não uma pasta".into()),
        _ => {
            return Err(if ext.is_empty() {
                format!("'{}' não é um preset .slangp", path.display())
            } else {
                format!("extensão '.{ext}' não reconhecida (esperado .slangp)")
            })
        }
    }
    log::info!("carregando preset slang: {}", path.display());
    let preset = shader_slang::parse_slangp_file(path).map_err(|e| e.to_string())?;
    if preset.passes.is_empty() {
        return Err("preset sem passes".into());
    }

    let mut params: HashMap<String, f32> = HashMap::new();
    let mut meta: Vec<shader_slang::Parameter> = Vec::new();
    let mut specs = Vec::new();
    for (i, pass) in preset.passes.iter().enumerate() {
        let src = shader_slang::preprocess_file(&pass.shader_path).map_err(|e| e.to_string())?;
        for p in &src.parameters {
            if !params.contains_key(&p.name) {
                params.insert(p.name.clone(), p.default);
                meta.push(p.clone());
            }
        }
        let compiled = shader_slang::compile(&src)
            .map_err(|e| format!("passe {i} ({}): {e}", pass.shader_path.display()))?;
        specs.push(PassSpec {
            scale_x: pass.scale_x,
            scale_y: pass.scale_y,
            linear: pass.filter_linear,
            vs_wgsl: compiled.vertex_wgsl,
            fs_wgsl: compiled.fragment_wgsl,
            uniform: UniformMode::Slang(compiled.uniforms),
        });
    }
    // valores do `.slangp` sobrescrevem os defaults dos `#pragma parameter`
    for (k, v) in &preset.parameters {
        params.insert(k.clone(), *v);
    }
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("slangp")
        .to_string();
    Ok(BuiltSpecs {
        name,
        params,
        meta,
        passes: specs,
    })
}

const COMP_WGSL: &str = r#"
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

fn build_composite(device: &wgpu::Device) -> Composite {
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

fn build_pass(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    spec: PassSpec,
) -> Option<Pass> {
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
        layout: Some(layout),
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
    });
    // buffer 0 (binding 0) e buffer 1 (binding 3). Tamanho de cada bloco slang,
    // ou dummy de 16 bytes.
    let (mut size0, mut size3) = match &spec.uniform {
        UniformMode::Fixed => (64u64, 16u64),
        UniformMode::Slang(_) => (16u64, 16u64),
    };
    if let UniformMode::Slang(blocks) = &spec.uniform {
        for (b, l) in blocks {
            let s = (l.size as u64).max(16);
            if *b == 0 {
                size0 = s;
            } else {
                size3 = s;
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
        scale_x: spec.scale_x,
        scale_y: spec.scale_y,
        linear: spec.linear,
        uniform: spec.uniform,
        ubuf: [mk(size0), mk(size3)],
        target: None,
        bind_group: None,
        bound: false,
    })
}

fn axis_size(scale: Scale, cur: u32, native: u32) -> u32 {
    match scale {
        Scale::Source(m) => (cur as f32 * m).round() as u32,
        Scale::Absolute(px) => px,
        // Sem viewport real aqui (o canvas escala) — supersample ~3x.
        Scale::Viewport(m) => (native as f32 * 3.0 * m).round() as u32,
    }
}

/// std140: `w, h, 1/w, 1/h`.
fn size_vec(w: u32, h: u32) -> [f32; 4] {
    [
        w as f32,
        h as f32,
        1.0 / w.max(1) as f32,
        1.0 / h.max(1) as f32,
    ]
}

#[allow(clippy::too_many_arguments)]
fn fill_slang(
    layout: &UniformLayout,
    params: &HashMap<String, f32>,
    src: (u32, u32),
    out: (u32, u32),
    orig: (u32, u32),
    final_vp: (u32, u32),
    frame_count: u64,
) -> Vec<u8> {
    let mut b = vec![0u8; (layout.size as usize).max(16)];
    let put = |b: &mut [u8], off: usize, bytes: &[u8]| {
        if off + bytes.len() <= b.len() {
            b[off..off + bytes.len()].copy_from_slice(bytes);
        }
    };
    for f in &layout.fields {
        let o = f.offset as usize;
        match (f.name.as_str(), f.kind) {
            ("MVP", UniformFieldKind::Mat4) => {
                // ortho [0,1] → [-1,1], column-major
                #[rustfmt::skip]
                let m: [f32; 16] = [2.0,0.0,0.0,0.0, 0.0,2.0,0.0,0.0, 0.0,0.0,1.0,0.0, -1.0,-1.0,0.0,1.0];
                put(&mut b, o, f32s_bytes(&m));
            }
            ("SourceSize", _) => put(&mut b, o, f32s_bytes(&size_vec(src.0, src.1))),
            ("OriginalSize", _) => put(&mut b, o, f32s_bytes(&size_vec(orig.0, orig.1))),
            ("OutputSize", _) => put(&mut b, o, f32s_bytes(&size_vec(out.0, out.1))),
            ("FinalViewportSize", _) => {
                put(&mut b, o, f32s_bytes(&size_vec(final_vp.0, final_vp.1)))
            }
            ("FrameCount", UniformFieldKind::U32) => {
                put(&mut b, o, &(frame_count as u32).to_le_bytes())
            }
            ("FrameDirection", UniformFieldKind::I32) => put(&mut b, o, &1i32.to_le_bytes()),
            (name, UniformFieldKind::F32) => {
                if let Some(v) = params.get(name) {
                    put(&mut b, o, &v.to_le_bytes());
                }
            }
            _ => {}
        }
    }
    b
}

fn new_tex(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("etapa04 tex"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FMT,
        usage,
        view_formats: &[],
    });
    let v = t.create_view(&wgpu::TextureViewDescriptor::default());
    (t, v)
}

fn f32s_bytes(s: &[f32]) -> &[u8] {
    // SAFETY: `f32` não tem padding nem invariantes de bit.
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}
