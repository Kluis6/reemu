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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use domain::frame_source::{Frame, FrameOrigin};
use shader_slang::{Scale, UniformFieldKind, UniformLayout};
use video_surface::to_rgba8;

/// `close(2)` cru — pro caminho de erro do import dma_buf (o fd ainda é nosso).
unsafe fn close_raw_fd(fd: i32) {
    extern "C" {
        fn close(fd: i32) -> i32;
    }
    close(fd);
}

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

/// Um staging buffer do readback + o estado do `map_async`.
struct RbSlot {
    buf: wgpu::Buffer,
    /// setado pelo callback do `map_async` quando o buffer está mapeável.
    ready: Arc<AtomicBool>,
    /// `map_async` foi pedido e o resultado ainda não foi consumido.
    inflight: bool,
    w: u32,
    h: u32,
    padded: u32,
}

/// 2 staging buffers rodando em pipeline (ver campo `rb` do `FrameProcessor`).
#[derive(Default)]
struct ReadbackRing {
    slots: [Option<RbSlot>; 2],
    write: usize,
    dims: Option<(u32, u32)>,
}

impl ReadbackRing {
    /// Descarta os buffers (troca de preset / decoração / resize forçado).
    fn invalidate(&mut self) {
        self.slots = [None, None];
        self.dims = None;
        self.write = 0;
    }

    /// Garante 2 slots do tamanho `(w, h)`. Recria (e reseta o pipeline) se mudou.
    fn ensure(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        if self.dims == Some((w, h)) {
            return;
        }
        let padded = (w * 4).next_multiple_of(ROW_ALIGN);
        let mk = || RbSlot {
            buf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("etapa04 readback"),
                size: (padded * h) as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            ready: Arc::new(AtomicBool::new(false)),
            inflight: false,
            w,
            h,
            padded,
        };
        self.slots = [Some(mk()), Some(mk())];
        self.dims = Some((w, h));
        self.write = 0;
    }
}

/// Copia as `h` linhas úteis (`w*4` bytes) de um buffer com stride `padded`.
fn unpad_rows(src: &[u8], w: u32, h: u32, padded: u32) -> Vec<u8> {
    let row = (w * 4) as usize;
    let mut out = vec![0u8; row * h as usize];
    for y in 0..h as usize {
        out[y * row..(y + 1) * row].copy_from_slice(&src[y * padded as usize..][..row]);
    }
    out
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
    /// Guardados pra configurar a surface nativa depois — a `wgpu::Surface`
    /// tem que sair da MESMA `Instance`/`Adapter` que criou o `device`.
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
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
    /// Interop zero-cópia: `dma_buf` do core (GL) importado como textura wgpu,
    /// um por slot do ring. `interop_ok` = o device tem a feature.
    interop_ok: bool,
    imported: Vec<Option<(wgpu::Texture, wgpu::TextureView)>>,
    /// View da textura importada a usar como entrada da chain neste frame
    /// (`Some` só em frames de HW render com interop).
    interop_view: Option<wgpu::TextureView>,
    /// Readback com pipeline: 2 staging buffers. O frame N copia a saída da GPU
    /// pro slot N%2 e lê o slot (N+1)%2 (submetido no frame anterior, já pronto)
    /// — sem `poll(wait)` bloqueante no caminho normal, CPU e GPU deixam de
    /// serializar. Fallback bloqueante só quando o slot ainda não mapeou.
    rb: ReadbackRing,
    frame_count: u64,
    /// Frames apresentados na surface nativa (só pra log de diagnóstico).
    surf_frames: u64,
    comp: Composite,
    decoration: Option<Decoration>,
    /// Surface nativa (etapa 03 — vídeo fora da webview). `Some` = a chain
    /// desenha direto nela em vez de fazer readback pro canvas.
    surface: Option<SurfaceOut>,
}

/// Alvo de apresentação nativo: a `wgpu::Surface` de uma `wl_subsurface` (ou da
/// janela, em Win/macOS) + o pipeline de blit pro formato dela.
struct SurfaceOut {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    blit_pipeline: wgpu::RenderPipeline,
    blit_rect: wgpu::Buffer,
}

impl FrameProcessor {
    pub fn new() -> Option<Self> {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            log::info!("REEMU_NO_GPU: processamento de frame na GPU desligado");
            return None;
        }
        let instance = wgpu::Instance::default();
        let (adapter, device, queue, feats) = video_surface::create_device_with(
            &instance,
            None,
            wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF,
        )?;
        let interop_ok = feats.contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF);
        log::info!(
            "GPU (etapa 04): {} (interop dma_buf={interop_ok})",
            adapter.get_info().name
        );

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
            instance,
            adapter,
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
            interop_ok,
            imported: Vec::new(),
            interop_view: None,
            rb: ReadbackRing::default(),
            frame_count: 0,
            surf_frames: 0,
            comp,
            decoration: None,
            surface: None,
        })
    }

    /// Anexa uma surface nativa a partir de raw handles (a `wl_surface` de uma
    /// subsurface no Linux; a janela em Win/macOS). A chain passa a desenhar
    /// nela em vez de fazer readback. `false` = falhou (segue no canvas).
    ///
    /// # Safety
    /// `display`/`window` têm que continuar válidos enquanto a surface viver.
    pub unsafe fn attach_surface(
        &mut self,
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
        w: u32,
        h: u32,
    ) -> bool {
        let surface = match unsafe {
            self.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(display),
                    raw_window_handle: window,
                })
        } {
            Ok(s) => s,
            Err(e) => {
                log::warn!("create_surface: {e}");
                return false;
            }
        };
        let caps = surface.get_capabilities(&self.adapter);
        if caps.formats.is_empty() {
            log::warn!("adapter não desenha nessa surface");
            return false;
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Fifo,
        ]
        .into_iter()
        .find(|m| caps.present_modes.contains(m))
        .unwrap_or(wgpu::PresentMode::Fifo);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: w.max(1),
            height: h.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
        // SAFETY: o chamador garante os handles vivos; transmute pro 'static.
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

        let blit_pipeline = blit_pipeline(&self.device, &self.comp.bgl, format);
        let blit_rect = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blit rect"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        log::info!(
            "surface nativa: {w}x{h} {format:?} {present_mode:?} alpha={:?}",
            config.alpha_mode
        );
        self.surface = Some(SurfaceOut {
            surface,
            config,
            blit_pipeline,
            blit_rect,
        });
        true
    }

    pub fn resize_surface(&mut self, w: u32, h: u32) {
        if let Some(s) = &mut self.surface {
            s.config.width = w.max(1);
            s.config.height = h.max(1);
            s.surface.configure(&self.device, &s.config);
        }
    }

    /// Caminho da surface nativa: roda a chain e desenha o resultado (com
    /// letterbox) direto na surface, sem tocar a CPU. Sem frame novo é no-op —
    /// a `wl_surface` segura o último buffer apresentado (freeze no pause).
    pub fn render_to_surface(&mut self, frame: Option<&Frame>) {
        if self.surface.is_none() {
            return;
        }
        let Some(frame) = frame else { return };

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("surface"),
            });
        let Some((out_w, out_h, use_comp)) = self.run_chain(frame, &mut enc) else {
            log::warn!("render_to_surface: run_chain devolveu None");
            return;
        };
        let first = self.surf_frames == 0;
        self.surf_frames += 1;

        let s = self.surface.as_ref().unwrap();
        let (dw, dh) = (s.config.width.max(1), s.config.height.max(1));
        // letterbox: encaixa (out_w × out_h) em (dw × dh) mantendo a proporção.
        let ar_src = out_w as f32 / out_h.max(1) as f32;
        let ar_dst = dw as f32 / dh as f32;
        let (hw, hh) = if ar_src > ar_dst {
            (1.0, ar_dst / ar_src)
        } else {
            (ar_src / ar_dst, 1.0)
        };
        self.queue
            .write_buffer(&s.blit_rect, 0, f32s_bytes(&[0.0, 0.0, hw, hh]));

        let src_view = if use_comp {
            &self.comp.target.as_ref().unwrap().1
        } else {
            &self.passes.last().unwrap().target.as_ref().unwrap().1
        };
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit bg"),
            layout: &self.comp.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: s.blit_rect.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                },
            ],
        });

        let acquired = s.surface.get_current_texture();
        if first {
            log::info!(
                "render_to_surface: 1º frame chain {out_w}x{out_h} → surface {dw}x{dh}, acquire={}",
                match &acquired {
                    wgpu::CurrentSurfaceTexture::Success(_) => "Success",
                    wgpu::CurrentSurfaceTexture::Suboptimal(_) => "Suboptimal",
                    wgpu::CurrentSurfaceTexture::Timeout => "Timeout",
                    wgpu::CurrentSurfaceTexture::Occluded => "Occluded",
                    wgpu::CurrentSurfaceTexture::Outdated => "Outdated",
                    wgpu::CurrentSurfaceTexture::Lost => "Lost",
                    _ => "Validation/outro",
                }
            );
        }
        let frame_tex = match acquired {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                s.surface.configure(&self.device, &s.config);
                return;
            }
            _ => return,
        };
        let view = frame_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        // Magenta em DEBUG: se aparecer, a subsurface está
                        // visível e o problema é a chain/quad; se não, está
                        // escondida (transparência/z-order).
                        load: wgpu::LoadOp::Clear(
                            if std::env::var_os("REEMU_NATIVE_VIDEO_DEBUG").is_some() {
                                wgpu::Color {
                                    r: 1.0,
                                    g: 0.0,
                                    b: 1.0,
                                    a: 1.0,
                                }
                            } else {
                                wgpu::Color::BLACK
                            },
                        ),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&s.blit_pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.set_vertex_buffer(0, self.quad.slice(..));
            rp.draw(0..4, 0..1);
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(frame_tex);
        if first {
            log::info!("render_to_surface: 1º present feito");
        }
    }

    /// Apresenta um frame preto opaco na surface nativa — pra limpar o último
    /// frame do jogo quando ele é descarregado (senão fica preso atrás).
    pub fn clear_surface(&mut self) {
        let Some(s) = self.surface.as_ref() else {
            return;
        };
        let frame_tex = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return,
        };
        let view = frame_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear surface"),
            });
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
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
        self.queue.submit([enc.finish()]);
        self.queue.present(frame_tex);
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
        self.rb.invalidate();
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
        self.rb.invalidate();
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
        self.rb.invalidate();
        true
    }

    /// Roda a cadeia inteira (entrada + N passes + composição da moldura)
    /// gravando em `enc`. Devolve `(out_w, out_h, com_moldura)` — a textura
    /// final ainda não foi consumida (readback ou blit fica pro chamador).
    fn run_chain(
        &mut self,
        frame: &Frame,
        enc: &mut wgpu::CommandEncoder,
    ) -> Option<(u32, u32, bool)> {
        let nw = frame.metadata.native_width;
        let nh = frame.metadata.native_height;
        if nw == 0 || nh == 0 {
            return None;
        }
        // Entrada da chain: buffer cru (upload) ou textura dma_buf já na GPU.
        match &frame.origin {
            FrameOrigin::SoftwareRawBuffer {
                data,
                pitch,
                format,
            } => {
                let rgba = to_rgba8(data, nw, nh, *pitch, *format);
                if rgba.len() != (nw * nh * 4) as usize {
                    return None;
                }
                self.ensure_core_tex(nw, nh);
                self.upload_core(&rgba, nw, nh);
                self.interop_view = None;
            }
            FrameOrigin::HardwareTexture(handle) => {
                if !self.bind_interop_input(handle.as_ref(), nw, nh) {
                    return None;
                }
                // a entrada troca de slot a cada frame → rebuild do bind group 0
                if let Some(p) = self.passes.first_mut() {
                    p.bound = false;
                }
            }
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
                match &self.interop_view {
                    Some(v) => v,
                    None => &self.core_tex.as_ref()?.1,
                }
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
        Some((out_w, out_h, use_comp))
    }

    /// Caminho canvas: roda a chain e lê o resultado de volta pra CPU (RGBA8).
    pub fn process(&mut self, frame: &Frame) -> Option<(u32, u32, Vec<u8>)> {
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("etapa04"),
            });
        let (out_w, out_h, use_comp) = self.run_chain(frame, &mut enc)?;

        // --- readback com pipeline ---
        self.rb.ensure(&self.device, out_w, out_h);
        let write_i = self.rb.write;
        let read_i = write_i ^ 1;

        // 1. codifica a cópia GPU→buffer no slot de escrita e submete.
        {
            let ws = self.rb.slots[write_i].as_ref()?;
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
                    buffer: &ws.buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(ws.padded),
                        rows_per_image: Some(out_h),
                    },
                },
                wgpu::Extent3d {
                    width: out_w,
                    height: out_h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.queue.submit([enc.finish()]);

        // 2. pede o map do slot recém-escrito (o callback marca `ready`).
        {
            let ws = self.rb.slots[write_i].as_mut()?;
            ws.ready.store(false, Ordering::Relaxed);
            let ready = ws.ready.clone();
            ws.buf.slice(..).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    ready.store(true, Ordering::Relaxed);
                }
            });
            ws.inflight = true;
        }

        // 3. drena callbacks pendentes sem bloquear.
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.rb.write = read_i;

        // 4. lê o slot do frame anterior. Se ainda não mapeou (GPU atrasada),
        //    aí sim espera — raro, e melhor que perder o frame.
        let rs = self.rb.slots[read_i].as_mut()?;
        if !rs.inflight {
            return None; // 1º frame / logo após resize
        }
        if !rs.ready.load(Ordering::Relaxed) {
            self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        }
        let (rw, rh, rpad) = (rs.w, rs.h, rs.padded);
        let out = {
            let mapped = rs.buf.slice(..).get_mapped_range().ok()?;
            unpad_rows(&mapped, rw, rh, rpad)
        };
        rs.buf.unmap();
        rs.inflight = false;
        Some((rw, rh, out))
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

    /// Importa (1ª vez do slot) e seleciona a textura `dma_buf` como entrada da
    /// chain neste frame. `false` = interop indisponível / falhou → o chamador
    /// devolve `None` e o `poll_frame` cai no caminho de canvas vazio.
    fn bind_interop_input(
        &mut self,
        handle: &dyn domain::frame_source::GpuTextureHandle,
        w: u32,
        h: u32,
    ) -> bool {
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
        let Some((_, view)) = self.imported.get(slot).and_then(|s| s.as_ref()) else {
            return false;
        };
        self.interop_view = Some(view.clone());
        true
    }

    fn import_dmabuf(
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

/// Pipeline de blit (mesmo shader do composite: quad posicionado por um `Rect`
/// uniforme, sampla uma textura) pro formato de uma surface nativa — usa a
/// `bgl` do composite, então o bind group é o mesmo layout.
fn blit_pipeline(
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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, SoftwarePixelFormat};

    fn grey_frame(w: u32, h: u32, val: u8) -> Frame {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for px in data.chunks_mut(4) {
            px[0] = val; // B
            px[1] = val; // G
            px[2] = val; // R
            px[3] = 0xFF; // X
        }
        Frame {
            origin: FrameOrigin::SoftwareRawBuffer {
                data,
                pitch: w * 4,
                format: SoftwarePixelFormat::Xrgb8888,
            },
            metadata: FrameMetadata {
                native_width: w,
                native_height: h,
                aspect_ratio: w as f32 / h as f32,
                rotation_degrees: 0,
            },
        }
    }

    /// O readback com pipeline prima 1 frame e depois entrega o frame ANTERIOR
    /// (atraso de exatamente 1 frame). `plain` = passthrough, então a cor sai
    /// igual à que entrou 1 frame antes.
    #[test]
    fn pipelined_readback_has_one_frame_delay() {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando teste de readback");
            return;
        };

        assert!(
            fp.process(&grey_frame(64, 48, 0x40)).is_none(),
            "1º process deve primar o pipeline (None)"
        );

        let (w, h, d2) = fp
            .process(&grey_frame(64, 48, 0xC0))
            .expect("2º process entrega");
        assert_eq!((w, h), (64, 48));
        assert_eq!(d2.len(), 64 * 48 * 4);
        assert!(
            d2[0].abs_diff(0x40) <= 2,
            "esperava ~0x40, veio {:#x}",
            d2[0]
        );

        let (_, _, d3) = fp
            .process(&grey_frame(64, 48, 0x10))
            .expect("3º process entrega");
        assert!(
            d3[0].abs_diff(0xC0) <= 2,
            "esperava ~0xC0, veio {:#x}",
            d3[0]
        );
    }
}
