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

use domain::core_loader::VulkanSharedDevice;
use domain::frame_source::{Frame, FrameOrigin};
use shader_slang::{
    Scale, TextureBind, TextureSemantic, UniformFieldKind, UniformLayout, WrapMode,
};
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
@group(0) @binding(2) var Source: texture_2d<f32>;
@group(0) @binding(3) var Samp: sampler;
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

/// Preset "de 1 clique" que aponta pra um `.slangp` do pacote `slang-shaders`.
/// A UI mostra estes como opções fixas; ficam indisponíveis se o pacote não
/// foi baixado. Wire id = `curated:<id>` — resolvido por [`build_specs`].
pub struct Curated {
    pub id: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    /// Caminho relativo à raiz do `slang-shaders`.
    pub relpath: &'static str,
}

/// Só entram aqui presets confirmados no `docs/shaders/working-presets.txt`.
pub const CURATED: &[Curated] = &[
    Curated {
        id: "xbr",
        label: "Suavizar pixel art (xBR)",
        desc: "Deixa o 2D liso sem borrar — ideal pra 8/16-bit.",
        relpath: "edge-smoothing/xbr/other presets/xbr-lv2-standalone.slangp",
    },
    Curated {
        id: "scalefx",
        label: "Suavizar pixel art (ScaleFX)",
        desc: "Alternativa ao xBR; segura melhor os detalhes finos.",
        relpath: "edge-smoothing/scalefx/scalefx.slangp",
    },
    Curated {
        id: "super-xbr",
        label: "Suavizar 2D e 3D (Super-xBR)",
        desc: "Também serve pra consoles 3D (PS1/N64).",
        relpath: "edge-smoothing/xbr/super-xbr.slangp",
    },
    Curated {
        id: "ntsc",
        label: "Cor de TV antiga (NTSC)",
        desc: "Sangramento de vídeo composto — a cara certa de NES/Mega Drive.",
        relpath: "ntsc/ntsc-adaptive.slangp",
    },
    Curated {
        id: "crt-guest",
        label: "CRT avançado (Guest)",
        desc: "Tubo completo: máscara de fósforo, brilho e geometria.",
        relpath: "crt/crt-guest-advanced.slangp",
    },
];

pub fn curated_by_wire(wire: &str) -> Option<&'static Curated> {
    let id = wire.strip_prefix("curated:")?;
    CURATED.iter().find(|c| c.id == id)
}

static SHADER_ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Raiz do pacote `slang-shaders` (`<dados>/shaders/slang-shaders`), usada pra
/// resolver os presets `curated:<id>`. Setada uma vez no startup.
pub fn set_shader_root(dir: std::path::PathBuf) {
    let _ = SHADER_ROOT.set(dir);
}

/// Caminho absoluto do `.slangp` de um preset curado, se o pacote já existe.
pub fn curated_slangp(c: &Curated) -> Option<std::path::PathBuf> {
    let p = SHADER_ROOT.get()?.join(c.relpath);
    p.is_file().then_some(p)
}

/// Como os buffers uniformes do passe são preenchidos a cada frame.
/// Bindings: 0 = `Push`/params ou os 64 bytes fixos; 1 = `UBO`/global (slang).
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
    wrap: WrapMode,
    vs_wgsl: String,
    fs_wgsl: String,
    uniform: UniformMode,
    /// Samplers declarados (na ordem = ordem dos bindings). Vazio ⇒ builtin
    /// com um `Source` implícito (binding 2/3).
    textures: Vec<TextureBind>,
    fmt: wgpu::TextureFormat,
    frame_count_mod: u32,
    alias: Option<String>,
    /// A saída deste passe precisa ser guardada pro próximo frame (`*Feedback`).
    feedback: bool,
    // TODO(fase 2): `mipmap_input` — precisa gerar a cadeia de mips (wgpu não
    // faz automático). `slangp::Pass.mipmap_input` já é parseado.
}

/// Uma textura do usuário do `.slangp` (LUT/máscara), já decodificada — o
/// upload pra GPU acontece em `FrameProcessor` (que tem o device).
struct LutSpec {
    name: String,
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    linear: bool,
    wrap: WrapMode,
    // TODO(fase 2): `mipmap` (`TextureRef.mipmap` já é parseado).
}

/// Resultado de `build_specs`: preset resolvido pronto pra montar os passes.
struct BuiltSpecs {
    name: String,
    /// valor atual de cada parâmetro (default do `#pragma` + override do `.slangp`).
    params: HashMap<String, f32>,
    /// metadados dos `#pragma parameter` (label/min/max/step) pra UI.
    meta: Vec<shader_slang::Parameter>,
    passes: Vec<PassSpec>,
    luts: Vec<LutSpec>,
    /// `1 + maior índice de `OriginalHistory`` usado por qualquer passe.
    history_depth: usize,
}

type TexView = (wgpu::Texture, wgpu::TextureView, u32, u32);

struct Pass {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    scale_x: Scale,
    scale_y: Scale,
    linear: bool,
    wrap: WrapMode,
    uniform: UniformMode,
    /// buffers uniformes: `[0]` = binding 0 (`Push`), `[1]` = binding 1 (`UBO`).
    ubuf: [wgpu::Buffer; 2],
    /// Samplers declarados no `.slang` (ordem = ordem dos bindings).
    textures: Vec<TextureBind>,
    fmt: wgpu::TextureFormat,
    frame_count_mod: u32,
    feedback: bool,
    target: Option<TexView>,
    /// Cópia da saída do frame anterior (só quando `feedback`).
    feedback_target: Option<TexView>,
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

/// Handles crus de uma `VkInstance`/`VkDevice` **já criados por um core Vulkan**
/// (Beetle PSX HW cria o device na negociação de HW render — ver
/// `docs/ai-context/12-vulkan-hw-render-fase2.md` §Beetle). Passado pra
/// [`FrameProcessor::from_adopted_vulkan`], que envolve tudo no wgpu sem criar
/// device nenhum. O core é o dono — o `FrameProcessor` não destrói nada.
/// Handles crus de janela/display pra reanexar a surface nativa depois — o
/// `unsafe impl Send/Sync` é seguro porque os ponteiros wl vivem enquanto a
/// `VideoSurface` no `AppState` viver (o resto do app). Usado só pelo
/// negociador Vulkan §Beetle (D3), que roda numa thread do `emu-session`.
pub struct SendHandles {
    display: raw_window_handle::RawDisplayHandle,
    window: raw_window_handle::RawWindowHandle,
}
// SAFETY: ver doc acima.
unsafe impl Send for SendHandles {}
unsafe impl Sync for SendHandles {}
impl SendHandles {
    /// # Safety
    /// `display`/`window` têm que continuar válidos enquanto este valor viver.
    pub unsafe fn new(
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
    ) -> Self {
        Self { display, window }
    }
    pub fn display(&self) -> raw_window_handle::RawDisplayHandle {
        self.display
    }
    pub fn window(&self) -> raw_window_handle::RawWindowHandle {
        self.window
    }
}

/// `struct retro_vulkan_context` (libretro_vulkan.h) — o core preenche isto no
/// `create_device` da negociação. Espelha `core_loader_desktop::vk_sys`.
#[repr(C)]
#[derive(Default)]
struct RetroVulkanContext {
    gpu: ash::vk::PhysicalDevice,
    device: ash::vk::Device,
    queue: ash::vk::Queue,
    queue_family_index: u32,
    presentation_queue: ash::vk::Queue,
    presentation_queue_family_index: u32,
}

/// Extensões de device + `VkPhysicalDeviceFeatures` (core) que o `wgpu-hal 30`
/// quer pra adotar um device de `phys` desta `instance`. Constrói uma
/// `wgpu::hal::vulkan::Instance` DESCARTÁVEL (de clones, com `drop_callback`
/// no-op pra NÃO destruir a `VkInstance` real) só pra perguntar.
///
/// # Safety
/// `entry`/`instance` têm que ser válidos; `phys` tem que ser da `instance`.
unsafe fn wgpu_adopt_reqs(
    entry: &ash::Entry,
    instance: &ash::Instance,
    instance_exts: &[&'static std::ffi::CStr],
    api_version: u32,
    phys: ash::vk::PhysicalDevice,
) -> Option<(Vec<&'static std::ffi::CStr>, ash::vk::PhysicalDeviceFeatures)> {
    let hal = unsafe {
        wgpu::hal::vulkan::Instance::from_raw(
            entry.clone(),
            instance.clone(),
            api_version,
            0,
            None,
            instance_exts.to_vec(),
            wgpu::InstanceFlags::from_build_config().with_env(),
            wgpu::MemoryBudgetThresholds::default(),
            false,
            Some(Box::new(|| {})), // no-op: não destrói a VkInstance real
        )
    }
    .ok()?;
    let exposed = hal.expose_adapter(phys)?;
    let exts = exposed
        .adapter
        .required_device_extensions(wgpu::Features::empty());
    let feats = exposed
        .adapter
        .physical_device_features(&exts, wgpu::Features::empty())
        .get_core();
    Some((exts, feats))
}

// Consumido pelo `from_core_negotiation` (D2/D3) e pelo teste
// `from_adopted_vulkan_runs_the_chain`.
#[allow(dead_code)]
pub struct AdoptedVulkan {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical_device: ash::vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_family_index: u32,
    pub queue_index: u32,
    /// `apiVersion` do `VkApplicationInfo` usado ao criar a instância.
    pub instance_api_version: u32,
    /// Extensões de instância habilitadas (ex.: `VK_KHR_surface`,
    /// `VK_KHR_get_physical_device_properties2`).
    pub instance_extensions: Vec<&'static std::ffi::CStr>,
    /// Extensões de device habilitadas — TÊM que bater com o `device`.
    pub device_extensions: Vec<&'static std::ffi::CStr>,
    /// Features habilitadas no `device` (a validação do wgpu 30 é estrita).
    pub features: wgpu::Features,
}

pub struct FrameProcessor {
    /// Guardados pra configurar a surface nativa depois — a `wgpu::Surface`
    /// tem que sair da MESMA `Instance`/`Adapter` que criou o `device`.
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    quad: wgpu::Buffer,
    sampler_nearest: wgpu::Sampler,
    sampler_linear: wgpu::Sampler,
    /// Samplers por (filtro, wrap) — criados sob demanda (passes + LUTs).
    sampler_cache: HashMap<(bool, WrapMode), wgpu::Sampler>,
    preset_name: String,
    preset_source: String,
    /// Parâmetros globais do preset `.slangp` (nome → valor atual).
    params: HashMap<String, f32>,
    /// Metadados dos `#pragma parameter` (label/min/max/step) — pra UI.
    param_meta: Vec<shader_slang::Parameter>,
    passes: Vec<Pass>,
    /// Texturas do usuário (LUT/máscara) do `.slangp`, já na GPU.
    luts: HashMap<String, (wgpu::Texture, wgpu::TextureView, wgpu::Sampler, u32, u32)>,
    /// Alias de passe (`aliasN` no `.slangp`) → índice do passe.
    pass_alias: HashMap<String, usize>,
    /// Ring do frame do core: `[0]` = atual (`Original`), `[n]` = n frames atrás
    /// (`OriginalHistoryN`). Tamanho = `history_depth`.
    history: Vec<TexView>,
    history_depth: usize,
    /// Tamanho real da saída (surface nativa / comp target) — `scale_type =
    /// viewport` usa isto.
    viewport: (u32, u32),
    /// Interop zero-cópia: `dma_buf` do core (GL) importado como textura wgpu,
    /// um por slot do ring. `interop_ok` = o device tem a feature.
    interop_ok: bool,
    imported: Vec<Option<(wgpu::Texture, wgpu::TextureView)>>,
    /// HW render Vulkan (etapa 12): `VkImage` do core embrulhada com
    /// `texture_from_raw` no MESMO device, cacheada por `sync_index`
    /// (`(handle_da_VkImage, tex, view)`) — o core cicla um conjunto fixo de
    /// imagens. Zero cópia, sem `dma_buf`.
    vk_imported: Vec<Option<(u64, wgpu::Texture, wgpu::TextureView)>>,
    /// Alvo por slot pra inverter o Y do `dma_buf` (cores GL renderizam
    /// bottom-left) — só alocado quando `flip_y`.
    flip_tgt: Vec<Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>>,
    flip: FlipPipe,
    /// View da textura importada a usar como entrada da chain neste frame
    /// (`Some` só em frames de HW render com interop).
    interop_view: Option<wgpu::TextureView>,
    /// Rotação de tela (`SET_ROTATION`, jogos verticais de arcade): a saída da
    /// chain é redesenhada rotacionada AQUI antes da moldura/blit, então a
    /// moldura fica em pé e só o jogo gira. `rot_view` = `Some` quando o frame
    /// atual tem rotação.
    rot_pipeline: wgpu::RenderPipeline,
    rot_tgt: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    rot_view: Option<wgpu::TextureView>,
    /// Readback com pipeline: 2 staging buffers. O frame N copia a saída da GPU
    /// pro slot N%2 e lê o slot (N+1)%2 (submetido no frame anterior, já pronto)
    /// — sem `poll(wait)` bloqueante no caminho normal, CPU e GPU deixam de
    /// serializar. Fallback bloqueante só quando o slot ainda não mapeou.
    rb: ReadbackRing,
    frame_count: u64,
    /// `(w, h, com_moldura)` da última chamada de `render_to_surface` — pra
    /// `capture_surface_frame` ler de volta a textura certa sem rodar a chain.
    last_surface_out: Option<(u32, u32, bool)>,
    /// `get_current_texture` falhando em sequência (surface `Outdated`/`Lost`) —
    /// diagnóstico da "tela preta" (surface configurada num tamanho que o
    /// compositor não aceita, comum em 4K).
    surface_fail_streak: u32,
    /// Já apresentou ao menos 1 frame na surface nativa? (log de sanidade).
    surface_presented: bool,
    /// Queue family do device adotado (§Beetle) — pro command pool do blit.
    adopted_queue_family: Option<u32>,
    /// Conversor `A1R5G5B5`/packed-16 → RGBA8 pra cores Vulkan que fazem scanout
    /// num formato que o wgpu não amostra (Beetle PSX HW com dither ligado, ou
    /// jogo em modo 16bpp). Lazy: só nasce no 1º frame packed.
    vk_blit: Option<VkBlit>,
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

/// Um alvo RGBA8 do conversor packed→RGBA8, por slot do ring do core.
struct VkBlitTarget {
    image: ash::vk::Image,
    memory: ash::vk::DeviceMemory,
    w: u32,
    h: u32,
    /// Já foi transicionado pra `SHADER_READ_ONLY_OPTIMAL` ao menos uma vez
    /// (o 1º barrier usa `old_layout = UNDEFINED`).
    ready: bool,
    /// `wgpu::Texture` embrulhando `image` (external — wgpu não destrói).
    wrapped: Option<(wgpu::Texture, wgpu::TextureView)>,
}

/// Converte o scanout packed 16-bit de um core Vulkan (Beetle PSX HW com dither,
/// ou jogo em modo 16bpp — `VK_FORMAT_A1R5G5B5_UNORM_PACK16` etc.) pra RGBA8
/// via `vkCmdBlitImage` no device adotado, já que o wgpu não amostra esses
/// formatos. Recursos crus de `ash` — destruídos no `Drop` após
/// `device_wait_idle`.
struct VkBlit {
    device: ash::Device,
    queue: ash::vk::Queue,
    mem_props: ash::vk::PhysicalDeviceMemoryProperties,
    pool: ash::vk::CommandPool,
    cmd: ash::vk::CommandBuffer,
    fence: ash::vk::Fence,
    targets: Vec<Option<VkBlitTarget>>,
}

impl VkBlit {
    /// # Safety
    /// `device`/`queue` são do device Vulkan adotado (§Beetle); `qf` é a queue
    /// family da `queue`.
    unsafe fn new(device: &wgpu::Device, queue: &wgpu::Queue, qf: u32) -> Option<Self> {
        use ash::vk;
        let (raw_device, mem_props) = unsafe {
            let hd = device.as_hal::<wgpu::hal::api::Vulkan>()?;
            let phys = hd.raw_physical_device();
            let instance = hd.shared_instance().raw_instance().clone();
            (hd.raw_device().clone(), instance.get_physical_device_memory_properties(phys))
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
        let cmd = unsafe {
            raw_device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .ok()?[0];
        let fence = unsafe {
            raw_device.create_fence(&vk::FenceCreateInfo::default(), None)
        }
        .ok()?;

        Some(Self {
            device: raw_device,
            queue: raw_queue,
            mem_props,
            pool,
            cmd,
            fence,
            targets: Vec::new(),
        })
    }

    fn mem_type(&self, bits: u32, want: ash::vk::MemoryPropertyFlags) -> Option<u32> {
        (0..self.mem_props.memory_type_count).find(|&i| {
            (bits & (1 << i)) != 0
                && self.mem_props.memory_types[i as usize]
                    .property_flags
                    .contains(want)
        })
    }

    /// Garante um alvo RGBA8 `w×h` no `slot`. `false` = falhou.
    unsafe fn ensure_target(&mut self, _device: &wgpu::Device, slot: usize, w: u32, h: u32) -> bool {
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
                    .extent(vk::Extent3D { width: w, height: h, depth: 1 })
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
    unsafe fn run(
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
        let end = vk::Offset3D { x: w as i32, y: h as i32, z: 1 };
        let ignore = vk::QUEUE_FAMILY_IGNORED;

        let ok = unsafe {
            self.device
                .reset_command_buffer(self.cmd, vk::CommandBufferResetFlags::empty())
                .is_ok()
                && self
                    .device
                    .begin_command_buffer(
                        self.cmd,
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
                self.cmd,
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
                self.cmd,
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
                self.cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &to_shader,
            );

            if self.device.end_command_buffer(self.cmd).is_err() {
                return false;
            }
            let cmds = [self.cmd];
            let submit = vk::SubmitInfo::default().command_buffers(&cmds);
            let _ = self.device.reset_fences(&[self.fence]);
            if let Err(e) = self.device.queue_submit(self.queue, &[submit], self.fence) {
                log::error!("vk_blit: queue_submit: {e}");
                return false;
            }
            if self
                .device
                .wait_for_fences(&[self.fence], true, u64::MAX)
                .is_err()
            {
                return false;
            }
        }
        if let Some(t) = self.targets[slot].as_mut() {
            t.ready = true;
        }
        true
    }

    /// `wgpu::TextureView` do RGBA8 do `slot` (embrulha a `VkImage` uma vez).
    unsafe fn wgpu_view(&mut self, device: &wgpu::Device, slot: usize) -> Option<wgpu::TextureView> {
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

    unsafe fn destroy_target(&self, t: VkBlitTarget) {
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
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_command_pool(self.pool, None);
        }
    }
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

        Self::assemble(instance, adapter, device, queue, interop_ok)
    }

    /// Monta o `FrameProcessor` a partir de um `Device`/`Queue` wgpu já
    /// existentes — shaders, quad, samplers, pipelines de composição/flip. O
    /// `new()` (device criado por nós) e o `from_adopted_vulkan` (device de um
    /// core Vulkan, etapa 12 §Beetle) só diferem em COMO conseguem o quarteto.
    fn assemble(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        interop_ok: bool,
    ) -> Option<Self> {
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
        let r = realize(&device, &queue, built)?;
        log::info!(
            "shader: preset '{}' ({} passe(s), {} parâmetro(s), {} LUT(s), history {})",
            r.preset_name,
            r.passes.len(),
            r.param_meta.len(),
            r.luts.len(),
            r.history_depth
        );

        let comp = build_composite(&device);
        let flip = build_flip(&device);
        let rot_pipeline = rotate_pipeline(&device, &comp.bgl);

        Some(Self {
            sampler_nearest: mk_sampler(wgpu::FilterMode::Nearest),
            sampler_linear: mk_sampler(wgpu::FilterMode::Linear),
            sampler_cache: HashMap::new(),
            instance,
            adapter,
            device,
            queue,
            quad,
            preset_name: r.preset_name,
            preset_source: source,
            params: r.params,
            param_meta: r.param_meta,
            passes: r.passes,
            luts: r.luts,
            pass_alias: r.pass_alias,
            history: Vec::new(),
            history_depth: r.history_depth,
            viewport: (0, 0),
            interop_ok,
            imported: Vec::new(),
            vk_imported: Vec::new(),
            flip_tgt: Vec::new(),
            flip,
            interop_view: None,
            rb: ReadbackRing::default(),
            frame_count: 0,
            last_surface_out: None,
            surface_fail_streak: 0,
            surface_presented: false,
            adopted_queue_family: None,
            vk_blit: None,
            comp,
            rot_pipeline,
            rot_tgt: None,
            rot_view: None,
            decoration: None,
            surface: None,
        })
    }

    /// Constrói o `FrameProcessor` ADOTANDO uma `VkInstance`/`VkDevice` que já
    /// existem — o caso de um core Vulkan que EXIGE criar o device ele mesmo na
    /// negociação de HW render (Beetle PSX HW; ver
    /// `docs/ai-context/12-vulkan-hw-render-fase2.md` §Beetle, fatia D1). O wgpu
    /// não cria device nenhum: envolve o do core via `wgpu-hal`
    /// (`Instance::from_raw` → `expose_adapter` → `device_from_raw` →
    /// `wgpu::…::from_hal`).
    ///
    /// # Safety
    /// - `entry`/`instance`/`device` de `v` têm que continuar válidos por toda a
    ///   vida do `FrameProcessor` e ninguém mais pode destruí-los antes dele
    ///   (por isso os `drop_callback` são `None` — o dono é o core).
    /// - `queue_family_index`/`queue_index` apontam pra uma queue com
    ///   GRAPHICS+COMPUTE de `physical_device`.
    /// - `device_extensions` = EXATAMENTE as extensões habilitadas em `device`.
    /// - `features` = as features habilitadas em `device` (nem mais nem menos —
    ///   a validação do wgpu 30 é estrita).
    // Fiado no `loader`/`ffi_state` na fatia D2; hoje só o teste usa.
    #[allow(dead_code)]
    pub unsafe fn from_adopted_vulkan(v: AdoptedVulkan) -> Option<Self> {
        use wgpu::hal::api::Vulkan as Vk;

        let hal_instance = unsafe {
            wgpu::hal::vulkan::Instance::from_raw(
                v.entry,
                v.instance,
                v.instance_api_version,
                0, // android_sdk_version
                None,
                v.instance_extensions,
                wgpu::InstanceFlags::from_build_config().with_env(),
                wgpu::MemoryBudgetThresholds::default(),
                false, // has_nv_optimus
                // drop_callback no-op: a `VkInstance` é do core — o
                // `FrameProcessor` NÃO pode destruí-la ao dropar.
                Some(Box::new(|| {})),
            )
        }
        .inspect_err(|e| log::error!("adopt vk: Instance::from_raw: {e}"))
        .ok()?;

        let exposed = hal_instance.expose_adapter(v.physical_device)?;
        log::info!(
            "GPU (etapa 12 §Beetle): adotando {} do core",
            exposed.info.name
        );
        // NÃO usar `exposed.capabilities.limits` cru: a RTX 3060 reporta
        // `max_buffer_size > u32::MAX` e a validação de indirect draw do
        // wgpu-core dá `assert!(max_buffer_size <= u32::MAX)`. Base nos downlevel
        // defaults, mas sobe o teto de textura (downlevel trava em 2048 e a
        // surface 4K precisa de swapchain de 3840+ → tela do jogo preta).
        let al = &exposed.capabilities.limits;
        let limits = wgpu::Limits {
            max_texture_dimension_1d: al.max_texture_dimension_1d.min(16384),
            max_texture_dimension_2d: al.max_texture_dimension_2d.min(16384),
            ..wgpu::Limits::downlevel_defaults()
        };

        let open_device = unsafe {
            exposed.adapter.device_from_raw(
                v.device,
                // drop_callback no-op: o `VkDevice` é do core.
                Some(Box::new(|| {})),
                &v.device_extensions,
                v.features,
                &limits,
                &wgpu::MemoryHints::default(),
                v.queue_family_index,
                v.queue_index,
            )
        }
        .inspect_err(|e| log::error!("adopt vk: device_from_raw: {e}"))
        .ok()?;

        let wgpu_instance = unsafe { wgpu::Instance::from_hal::<Vk>(hal_instance) };
        let adapter = unsafe { wgpu_instance.create_adapter_from_hal::<Vk>(exposed) };
        let (device, queue) = unsafe {
            adapter.create_device_from_hal::<Vk>(
                open_device,
                &wgpu::DeviceDescriptor {
                    label: Some("etapa12 device adotado do core"),
                    required_features: v.features,
                    required_limits: limits,
                    ..Default::default()
                },
            )
        }
        .inspect_err(|e| log::error!("adopt vk: create_device_from_hal: {e}"))
        .ok()?;

        // `interop_ok` = false: o caminho Beetle é zero-cópia via
        // `texture_from_raw` no MESMO device, não usa `dma_buf`.
        let mut fp = Self::assemble(wgpu_instance, adapter, device, queue, false)?;
        fp.adopted_queue_family = Some(v.queue_family_index);
        Some(fp)
    }

    /// Etapa 12 §Beetle (D2/D3): um core Vulkan que EXIGE criar o `VkDevice`
    /// ele mesmo na negociação (Beetle PSX HW — o `context_reset` dele aborta
    /// se `context == NULL`). Constrói a `ash::Instance` com as extensões do
    /// `wgpu-hal`, chama o `create_device` do core passando as
    /// extensões/features que o `wgpu-hal` quer, e reconstrói o `FrameProcessor`
    /// sobre o device resultante. Devolve os handles pro `VkContext::adopt` do
    /// `core-loader-desktop` montar a ponte de frame.
    ///
    /// # Safety
    /// `neg.create_device` (e `neg.get_application_info`, se != 0) têm que
    /// apontar pros callbacks vivos da `.so` do core.
    pub unsafe fn from_core_negotiation(
        neg: domain::core_loader::VkNegotiation,
    ) -> Result<(Self, VulkanSharedDevice), String> {
        use ash::vk::{self, Handle as _};
        use std::os::raw::c_char;

        let entry =
            unsafe { ash::Entry::load() }.map_err(|e| format!("carregar loader Vulkan: {e}"))?;

        // apiVersion: o que o core pediu (Beetle manda VK_MAKE_VERSION(1,0,32)),
        // com **piso em 1.2** — o wgpu-hal 30 chama `vkWaitSemaphores` (timeline,
        // core em 1.2) no `wait_for_fence`; num device 1.1 esse ponteiro é nulo
        // e o `ash` faz `panic!("Unable to load wait_semaphores")` no video pump.
        // NÓS criamos a instância, então o app info 1.1 do Beetle não limita —
        // a instância nasce 1.2 e o `create_device` do core cria um device 1.2.
        let api_version = if neg.get_application_info != 0 {
            let f: unsafe extern "C" fn() -> *const vk::ApplicationInfo<'static> =
                unsafe { std::mem::transmute(neg.get_application_info) };
            let p = unsafe { f() };
            if p.is_null() {
                vk::API_VERSION_1_2
            } else {
                unsafe { (*p).api_version }.max(vk::API_VERSION_1_2)
            }
        } else {
            vk::API_VERSION_1_2
        };

        let flags = wgpu::InstanceFlags::from_build_config().with_env();
        let inst_exts =
            wgpu::hal::vulkan::Instance::desired_extensions(&entry, api_version, flags)
                .map_err(|e| format!("wgpu-hal desired_extensions: {e}"))?;
        let inst_exts_c: Vec<*const c_char> = inst_exts.iter().map(|e| e.as_ptr()).collect();

        let app = vk::ApplicationInfo::default().api_version(api_version);
        let ici = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_extension_names(&inst_exts_c);
        let instance = unsafe { entry.create_instance(&ici, None) }
            .map_err(|e| format!("vkCreateInstance: {e}"))?;

        let cleanup = |inst: &ash::Instance| unsafe { inst.destroy_instance(None) };

        let gpus = match unsafe { instance.enumerate_physical_devices() } {
            Ok(g) => g,
            Err(e) => {
                cleanup(&instance);
                return Err(format!("enumerate_physical_devices: {e}"));
            }
        };
        let gpu = gpus
            .iter()
            .copied()
            .find(|&g| {
                unsafe { instance.get_physical_device_properties(g) }.device_type
                    == vk::PhysicalDeviceType::DISCRETE_GPU
            })
            .or_else(|| gpus.first().copied());
        let Some(gpu) = gpu else {
            cleanup(&instance);
            return Err("nenhuma GPU Vulkan".into());
        };

        // Pergunta pro wgpu-hal (instância hal descartável, drop no-op) quais
        // device exts + core features ele precisa.
        let reqs = unsafe { wgpu_adopt_reqs(&entry, &instance, &inst_exts, api_version, gpu) };
        let Some((dev_exts, dev_feats)) = reqs else {
            cleanup(&instance);
            return Err("wgpu-hal não expôs a GPU".into());
        };
        let dev_exts_c: Vec<*const c_char> = dev_exts.iter().map(|e| e.as_ptr()).collect();

        let create_device: unsafe extern "C" fn(
            *mut RetroVulkanContext,
            vk::Instance,
            vk::PhysicalDevice,
            vk::SurfaceKHR,
            vk::PFN_vkGetInstanceProcAddr,
            *const *const c_char,
            u32,
            *const *const c_char,
            u32,
            *const vk::PhysicalDeviceFeatures,
        ) -> bool = unsafe { std::mem::transmute(neg.create_device) };
        let gipa = entry.static_fn().get_instance_proc_addr;
        let mut ctx = RetroVulkanContext::default();
        let ok = unsafe {
            create_device(
                &mut ctx,
                instance.handle(),
                gpu,
                vk::SurfaceKHR::null(),
                gipa,
                dev_exts_c.as_ptr(),
                dev_exts_c.len() as u32,
                std::ptr::null(),
                0,
                &dev_feats,
            )
        };
        if !ok || ctx.device.is_null() {
            cleanup(&instance);
            return Err("o create_device do core devolveu false".into());
        }
        let final_gpu = if ctx.gpu.is_null() { gpu } else { ctx.gpu };
        log::info!(
            "core criou o VkDevice (queue family {}) — wgpu vai adotar",
            ctx.queue_family_index
        );

        let device = unsafe { ash::Device::load(instance.fp_v1_0(), ctx.device) };
        let shared = VulkanSharedDevice {
            get_instance_proc_addr: gipa as usize,
            instance: instance.handle().as_raw() as usize,
            physical_device: final_gpu.as_raw() as usize,
            device: ctx.device.as_raw() as usize,
            queue: ctx.queue.as_raw() as usize,
            queue_family_index: ctx.queue_family_index,
        };

        let adopted = AdoptedVulkan {
            entry,
            instance,
            physical_device: final_gpu,
            device,
            queue_family_index: ctx.queue_family_index,
            queue_index: 0,
            instance_api_version: api_version,
            instance_extensions: inst_exts,
            device_extensions: dev_exts,
            features: wgpu::Features::empty(),
        };
        let fp = unsafe { Self::from_adopted_vulkan(adopted) }
            .ok_or("from_adopted_vulkan falhou no device do core")?;
        Ok((fp, shared))
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
        // Nunca configurar acima do teto de textura do device (senão o
        // swapchain não é criado e `get_current_texture` fica `Outdated`).
        let cap = self.device.limits().max_texture_dimension_2d;
        let (cw, ch) = (w.clamp(1, cap), h.clamp(1, cap));
        if (cw, ch) != (w, h) {
            log::warn!("surface nativa: {w}x{h} > teto {cap} — limitando a {cw}x{ch}");
        }
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: cw,
            height: ch,
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
        log::info!("surface nativa: {cw}x{ch} {format:?} {present_mode:?}");
        self.viewport = (cw, ch);
        self.surface = Some(SurfaceOut {
            surface,
            config,
            blit_pipeline,
            blit_rect,
        });
        true
    }

    pub fn resize_surface(&mut self, w: u32, h: u32) {
        let cap = self.device.limits().max_texture_dimension_2d;
        let (w, h) = (w.clamp(1, cap), h.clamp(1, cap));
        self.viewport = (w, h);
        if let Some(s) = &mut self.surface {
            if (s.config.width, s.config.height) != (w, h) {
                s.config.width = w;
                s.config.height = h;
                s.surface.configure(&self.device, &s.config);
                self.surface_fail_streak = 0;
            }
        }
        // `scale_type = viewport` mudou de tamanho → realoca alvos.
        for p in &mut self.passes {
            p.bound = false;
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
            return;
        };
        self.last_surface_out = Some((out_w, out_h, use_comp));

        // Proporção de exibição: a moldura impõe a sua; senão a AR declarada do
        // core (respeita PAR ≠ 1); só cai na proporção de pixels se não houver.
        // Com rotação de 90°/270° e sem moldura, a AR do core inverte — mais
        // simples usar as dimensões (já rotacionadas) da saída.
        let quarter = matches!(frame.metadata.rotation_degrees % 360, 90 | 270);
        let ar_src = self
            .decoration_aspect()
            .filter(|_| use_comp)
            .or(Some(frame.metadata.aspect_ratio)
                .filter(|a| *a > 0.0 && !quarter))
            .unwrap_or(out_w as f32 / out_h.max(1) as f32);

        let s = self.surface.as_ref().unwrap();
        let (dw, dh) = (s.config.width.max(1), s.config.height.max(1));
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
        } else if let Some(v) = &self.rot_view {
            v
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

        let frame_tex = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => {
                self.surface_fail_streak = 0;
                t
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                self.surface_fail_streak = 0;
                t
            }
            other => {
                self.surface_fail_streak += 1;
                if matches!(self.surface_fail_streak, 1 | 30 | 300) {
                    log::warn!(
                        "surface nativa: get_current_texture {:?} ({}x) — config {}x{}; reconfigurando",
                        std::mem::discriminant(&other),
                        self.surface_fail_streak,
                        s.config.width,
                        s.config.height,
                    );
                }
                if matches!(
                    other,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost
                ) {
                    s.surface.configure(&self.device, &s.config);
                }
                return;
            }
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
                        // letterbox = preto ao redor da imagem.
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
        if !self.surface_presented {
            self.surface_presented = true;
            let s = self.surface.as_ref().unwrap();
            log::info!(
                "surface nativa: 1º frame apresentado ({}x{})",
                s.config.width,
                s.config.height
            );
        }
    }

    /// Apresenta um frame preto opaco na surface nativa. Hoje o pump esconde a
    /// subsurface no idle em vez de pintar preto (o preto tapava a webview);
    /// mantido pra um possível "fade to black" antes de esconder.
    #[allow(dead_code)]
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

    /// Lê de volta o último frame que foi pra surface nativa (RGBA8 apertado) —
    /// pra usar de fundo do menu de pausa. Bloqueante; chamado 1× por pause,
    /// não no caminho de render.
    pub fn capture_surface_frame(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        let (w, h, use_comp) = self.last_surface_out?;
        let src = if use_comp {
            &self.comp.target.as_ref()?.0
        } else if let Some((t, _, _, _)) = self.rot_tgt.as_ref().filter(|_| self.rot_view.is_some()) {
            t
        } else {
            &self.passes.last()?.target.as_ref()?.0
        };
        let padded = w * 4 + (ROW_ALIGN - (w * 4) % ROW_ALIGN) % ROW_ALIGN;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("capture"),
            });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: src,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let rgba = {
            let mapped = buf.slice(..).get_mapped_range().ok()?;
            unpad_rows(&mapped, w, h, padded)
        };
        buf.unmap();
        Some((w, h, rgba))
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
    pub fn vulkan_shared_device(&self) -> Option<VulkanSharedDevice> {
        // SAFETY: só lemos handles; nada é destruído aqui. O guard do `as_hal`
        // mantém o device vivo durante a leitura, e os handles seguem válidos
        // enquanto o `FrameProcessor` viver (é ele o dono).
        unsafe {
            let hal = self.device.as_hal::<wgpu::hal::api::Vulkan>()?;
            let instance = hal.shared_instance().raw_instance();
            let physical_device = hal.raw_physical_device();
            let family = hal.queue_family_index();

            // A queue do wgpu precisa servir pro core (GRAPHICS+COMPUTE).
            let want = ash::vk::QueueFlags::GRAPHICS | ash::vk::QueueFlags::COMPUTE;
            let families = instance.get_physical_device_queue_family_properties(physical_device);
            let ok = families
                .get(family as usize)
                .is_some_and(|f| f.queue_flags.contains(want));
            if !ok {
                log::warn!(
                    "queue family {family} do wgpu não tem GRAPHICS+COMPUTE — \
                     HW render Vulkan indisponível"
                );
                return None;
            }

            Some(VulkanSharedDevice {
                get_instance_proc_addr: hal
                    .shared_instance()
                    .entry()
                    .static_fn()
                    .get_instance_proc_addr as usize,
                instance: std::mem::transmute::<ash::vk::Instance, usize>(instance.handle()),
                physical_device: std::mem::transmute::<ash::vk::PhysicalDevice, usize>(
                    physical_device,
                ),
                device: std::mem::transmute::<ash::vk::Device, usize>(hal.raw_device().handle()),
                queue: std::mem::transmute::<ash::vk::Queue, usize>(hal.raw_queue()),
                queue_family_index: family,
            })
        }
    }

    /// O que foi passado pra `set_preset` (builtin ou caminho) — pra dedup.
    pub fn preset_source(&self) -> &str {
        &self.preset_source
    }

    pub fn set_preset(&mut self, name: &str) -> Result<(), String> {
        let built = build_specs(name).map_err(|e| format!("shader: {e}"))?;
        let r = realize(&self.device, &self.queue, built)
            .ok_or("shader: falha ao criar os pipelines")?;
        self.passes = r.passes;
        self.params = r.params;
        self.param_meta = r.param_meta;
        self.preset_name = r.preset_name;
        self.preset_source = name.to_string();
        self.luts = r.luts;
        self.pass_alias = r.pass_alias;
        if r.history_depth != self.history_depth {
            self.history_depth = r.history_depth;
            self.history.clear();
        }
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
        // Entrada da chain: buffer cru (vai pro ring de history) ou textura
        // dma_buf já na GPU (interop; history fica como o frame atual).
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
                self.ensure_history(nw, nh);
                self.push_history(&rgba, nw, nh);
                self.interop_view = None;
            }
            FrameOrigin::HardwareTexture(handle) => {
                if !self.bind_interop_input(handle.as_ref(), nw, nh, enc) {
                    return None;
                }
                // a entrada troca de slot a cada frame → rebuild de todo bg
                for p in &mut self.passes {
                    p.bound = false;
                }
            }
            FrameOrigin::HardwareVulkanImage(handle) => {
                if !self.bind_vulkan_input(handle.as_ref()) {
                    return None;
                }
                for p in &mut self.passes {
                    p.bound = false;
                }
            }
        }

        // dimensões de cada alvo (`viewport` usa o tamanho real da saída).
        let vp = self.viewport;
        let mut sizes = Vec::with_capacity(self.passes.len());
        let (mut cw, mut ch) = (nw, nh);
        for p in &self.passes {
            cw = axis_size(p.scale_x, cw, nw, vp.0);
            ch = axis_size(p.scale_y, ch, nh, vp.1);
            sizes.push((cw, ch));
        }
        let (fw, fh) = *sizes.last()?;
        if fw * fh > MAX_OUT_PIXELS {
            return None;
        }
        let final_vp = if vp.0 > 0 { vp } else { (fw, fh) };

        self.frame_count = self.frame_count.wrapping_add(1);
        let fc = self.frame_count;

        for (idx, (pw, ph)) in sizes.iter().copied().enumerate() {
            self.ensure_target(idx, pw, ph);
            let (in_w, in_h) = if idx == 0 { (nw, nh) } else { sizes[idx - 1] };
            let fc_pass = {
                let m = self.passes[idx].frame_count_mod as u64;
                if m > 0 {
                    fc % m
                } else {
                    fc
                }
            };
            // nome-base de cada textura do passe → tamanho (pro `<Nome>Size`).
            let tex_sizes = self.tex_sizes_for(idx, (in_w, in_h), (nw, nh), &sizes);
            match &self.passes[idx].uniform {
                UniformMode::Fixed => {
                    let mut b = vec![0u8; 64];
                    b[0..16].copy_from_slice(f32s_bytes(&size_vec(in_w, in_h)));
                    b[16..32].copy_from_slice(f32s_bytes(&size_vec(pw, ph)));
                    b[32..48].copy_from_slice(f32s_bytes(&size_vec(nw, nh)));
                    b[48..64].copy_from_slice(f32s_bytes(&[fc_pass as f32, 1.0, 0.0, 0.0]));
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
                            final_vp,
                            fc_pass,
                            &tex_sizes,
                        );
                        let slot = if *binding == 0 { 0 } else { 1 };
                        self.queue.write_buffer(&self.passes[idx].ubuf[slot], 0, &b);
                    }
                }
            }
        }

        // Samplers de cada (passe, textura) — precisa de `&mut self` (cache), então
        // resolvemos antes da seção de bind groups (que só empresta `&self`).
        let samplers: Vec<Vec<wgpu::Sampler>> = (0..self.passes.len())
            .map(|idx| {
                let (linear, wrap) = (self.passes[idx].linear, self.passes[idx].wrap);
                (0..self.passes[idx].textures.len())
                    .map(|t| {
                        let sem = self.passes[idx].textures[t].semantic.clone();
                        if let TextureSemantic::Named {
                            name,
                            feedback: false,
                        } = &sem
                        {
                            if let Some((_, _, s, _, _)) = self.luts.get(name) {
                                return s.clone();
                            }
                        }
                        self.sampler_for(linear, wrap)
                    })
                    .collect()
            })
            .collect();

        #[allow(clippy::needless_range_loop)] // idx indexa passes (mut) + samplers
        for idx in 0..self.passes.len() {
            if self.passes[idx].bind_group.is_some() && self.passes[idx].bound {
                continue;
            }
            // Resolve as views (clona — `TextureView` é Arc barato) antes de
            // pegar `&mut self` pra gravar o bind group.
            let binds: Vec<(u32, u32, wgpu::TextureView)> = self.passes[idx]
                .textures
                .iter()
                .map(|b| {
                    self.resolve_tex_view(&b.semantic, idx)
                        .map(|v| (b.tex_binding, b.samp_binding, v.clone()))
                })
                .collect::<Option<_>>()?;

            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.passes[idx].ubuf[0].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.passes[idx].ubuf[1].as_entire_binding(),
                },
            ];
            for (t, (tb, sb, view)) in binds.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: *tb,
                    resource: wgpu::BindingResource::TextureView(view),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: *sb,
                    resource: wgpu::BindingResource::Sampler(&samplers[idx][t]),
                });
            }
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("etapa04 bg"),
                layout: &self.passes[idx].bgl,
                entries: &entries,
            });
            drop(entries);
            drop(binds);
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

        // Feedback: guarda a saída dos passes marcados pro próximo frame (o
        // sampler `*Feedback` lê esta cópia). Copiado depois de todos os passes
        // pra um passe poder ler o feedback dele mesmo.
        for idx in 0..self.passes.len() {
            if !self.passes[idx].feedback {
                continue;
            }
            let (Some((src, _, sw, sh)), Some((dst, _, _, _))) = (
                self.passes[idx].target.as_ref(),
                self.passes[idx].feedback_target.as_ref(),
            ) else {
                continue;
            };
            enc.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: src,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: dst,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: *sw,
                    height: *sh,
                    depth_or_array_layers: 1,
                },
            );
            self.passes[idx].bound = false; // o feedback_target mudou
        }

        // Rotação de tela (`SET_ROTATION`) — redesenha a saída da chain
        // rotacionada num alvo próprio; a moldura e o blit final leem daí, então
        // a moldura fica em pé. Faz ANTES da composição de propósito.
        self.rot_view = None;
        let (mut fw, mut fh) = (fw, fh);
        let deg = frame.metadata.rotation_degrees % 360;
        let quarter = deg == 90 || deg == 270;
        if deg != 0 {
            let (rw, rh) = if deg == 90 || deg == 270 {
                (fh, fw)
            } else {
                (fw, fh)
            };
            // giro do UV = -giro da imagem. `SET_ROTATION` do libretro é
            // anti-horário (turns×90° CCW): 90 → (0,1); 180 → (-1,0); 270 → (0,-1).
            let (cos, sin) = match deg {
                90 => (0.0f32, 1.0f32),
                180 => (-1.0, 0.0),
                270 => (0.0, -1.0),
                _ => (1.0, 0.0),
            };
            if self.ensure_rot_target(rw, rh) {
                let src = &self.passes.last()?.target.as_ref()?.1;
                let rot_view = self.rot_tgt.as_ref()?.1.clone();
                let ubuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rot u"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue
                    .write_buffer(&ubuf, 0, f32s_bytes(&[cos, sin, 0.0, 0.0]));
                let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("rot bg"),
                    layout: &self.comp.bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: ubuf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(src),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                        },
                    ],
                });
                {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("rot pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &rot_view,
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
                    rp.set_pipeline(&self.rot_pipeline);
                    rp.set_bind_group(0, &bg, &[]);
                    rp.set_vertex_buffer(0, self.quad.slice(..));
                    rp.draw(0..4, 0..1);
                }
                self.rot_view = Some(rot_view);
                fw = rw;
                fh = rh;
            }
        }

        // Composição da moldura (etapa 04 fatia 4), se houver uma.
        let (out_w, out_h, use_comp) = if let Some((dw, dh, vp)) =
            self.decoration.as_ref().map(|d| (d.w, d.h, d.vp))
        {
            let dar0 = if frame.metadata.aspect_ratio > 0.0 {
                frame.metadata.aspect_ratio
            } else {
                nw as f32 / nh.max(1) as f32
            };
            // com rotação de 90°/270° a AR de exibição inverte.
            let dar = if quarter && dar0 > 0.0 { 1.0 / dar0 } else { dar0 };
            let (cx, cy, hw, hh) = match vp {
                // Janela do jogo conhecida (do `.cfg` ou detectada pela
                // transparência da arte): o jogo PREENCHE a janela — sem
                // letterbox (a moldura foi desenhada pra esse retângulo).
                Some(v) if v.w > 0.0 && v.h > 0.0 => (
                    (v.x + v.w / 2.0) / dw as f32 * 2.0 - 1.0,
                    1.0 - (v.y + v.h / 2.0) / dh as f32 * 2.0,
                    (v.w / dw as f32).clamp(0.0, 1.0),
                    (v.h / dh as f32).clamp(0.0, 1.0),
                ),
                _ => {
                    let vw = (dh as f32 * dar).min(dw as f32);
                    (0.0, 0.0, vw / dw as f32, 1.0)
                }
            };
            self.queue
                .write_buffer(&self.comp.rect_game, 0, f32s_bytes(&[cx, cy, hw, hh]));
            self.queue
                .write_buffer(&self.comp.rect_bezel, 0, f32s_bytes(&[0.0, 0.0, 1.0, 1.0]));
            self.ensure_comp_target(dw, dh);

            let game_view = match &self.rot_view {
                Some(v) => v,
                None => &self.passes.last()?.target.as_ref()?.1,
            };
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
            } else if let Some((t, _, _, _)) = self.rot_tgt.as_ref().filter(|_| self.rot_view.is_some()) {
                t
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
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                // `render_to_surface` samplia esta textura no blit pra surface.
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        self.comp.target = Some((t, v, w, h));
    }

    /// Alvo pra o passe de rotação (`SET_ROTATION`). `false` se `w`/`h` for 0.
    fn ensure_rot_target(&mut self, w: u32, h: u32) -> bool {
        if w == 0 || h == 0 {
            return false;
        }
        if !matches!(&self.rot_tgt, Some((_, _, tw, th)) if *tw == w && *th == h) {
            let (t, v) = new_tex(
                &self.device,
                w,
                h,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            self.rot_tgt = Some((t, v, w, h));
        }
        true
    }

    /// Garante `history_depth` slots do frame do core, todos `w`×`h`. `[0]` é o
    /// frame atual (`Original`); `[n]`, n frames atrás (`OriginalHistoryN`).
    fn ensure_history(&mut self, w: u32, h: u32) {
        let ok = self.history.len() == self.history_depth
            && self
                .history
                .iter()
                .all(|(_, _, tw, th)| *tw == w && *th == h);
        if ok {
            return;
        }
        self.history = (0..self.history_depth)
            .map(|_| {
                let (t, v) = new_tex(
                    &self.device,
                    w,
                    h,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                (t, v, w, h)
            })
            .collect();
        for p in &mut self.passes {
            p.bound = false;
        }
    }

    /// Rotaciona o ring de history e grava o frame novo em `[0]`.
    fn push_history(&mut self, rgba: &[u8], w: u32, h: u32) {
        if self.history.is_empty() {
            return;
        }
        self.history.rotate_right(1);
        let (tex, _, _, _) = &self.history[0];
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
        for p in &mut self.passes {
            p.bound = false;
        }
    }

    /// HW render Vulkan (etapa 12): embrulha a `VkImage` que o core entregou
    /// com `texture_from_raw` (MESMO device, sem cópia) e a seleciona como
    /// entrada da chain. Cacheia por `sync_index` — o core cicla um conjunto
    /// fixo de imagens. `false` = falha → canvas vazio.
    fn bind_vulkan_input(&mut self, handle: &dyn domain::frame_source::VulkanImageHandle) -> bool {
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
    fn bind_via_blit(
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
    fn submit_vulkan_cmds(&self, handle: &dyn domain::frame_source::VulkanImageHandle) -> bool {
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
    fn wrap_vulkan_image(
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
    fn bind_interop_input(
        &mut self,
        handle: &dyn domain::frame_source::GpuTextureHandle,
        w: u32,
        h: u32,
        enc: &mut wgpu::CommandEncoder,
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
        let Some((imp_tex, imp_view)) = self.imported.get(slot).and_then(|s| s.as_ref()) else {
            return false;
        };

        if !handle.flip_y() {
            self.interop_view = Some(imp_view.clone());
            return true;
        }

        // Core GL renderiza bottom-left → inverte o Y num alvo próprio por slot.
        let (tw, th) = {
            let s = imp_tex.size();
            (s.width, s.height)
        };
        if self.flip_tgt.len() <= slot {
            self.flip_tgt.resize_with(slot + 1, || None);
        }
        if !matches!(&self.flip_tgt[slot], Some((_, _, w, h)) if *w == tw && *h == th) {
            let (t, v) = new_tex(
                &self.device,
                tw,
                th,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            self.flip_tgt[slot] = Some((t, v, tw, th));
        }
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
        let fmt = self.passes[idx].fmt;
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        let stale =
            !matches!(&self.passes[idx].target, Some((_, _, tw, th)) if *tw == w && *th == h);
        if stale {
            let (t, v) = new_tex_fmt(&self.device, w, h, fmt, usage);
            self.passes[idx].target = Some((t, v, w, h));
            // qualquer passe pode amostrar este (PassOutput/Source) → rebind todos
            for p in &mut self.passes {
                p.bound = false;
            }
        }
        // alvo de feedback (cópia do frame anterior) — mesmo tamanho/formato.
        if self.passes[idx].feedback {
            let fb_stale = !matches!(
                &self.passes[idx].feedback_target,
                Some((_, _, tw, th)) if *tw == w && *th == h
            );
            if fb_stale {
                let (t, v) = new_tex_fmt(&self.device, w, h, fmt, usage);
                self.passes[idx].feedback_target = Some((t, v, w, h));
                for p in &mut self.passes {
                    p.bound = false;
                }
            }
        }
    }

    /// Entrada da chain neste frame: interop (dma_buf) ou o topo do ring de
    /// history (frame do core).
    fn chain_input(&self) -> Option<&wgpu::TextureView> {
        self.interop_view
            .as_ref()
            .or_else(|| self.history.first().map(|(_, v, _, _)| v))
    }

    /// Resolve uma [`TextureSemantic`] pra `TextureView` deste frame.
    fn resolve_tex_view(
        &self,
        sem: &TextureSemantic,
        pass_idx: usize,
    ) -> Option<&wgpu::TextureView> {
        let out_of = |i: usize| {
            self.passes
                .get(i)
                .and_then(|p| p.target.as_ref())
                .map(|(_, v, _, _)| v)
        };
        let fb_of = |i: usize| {
            self.passes.get(i).and_then(|p| {
                p.feedback_target
                    .as_ref()
                    .or(p.target.as_ref())
                    .map(|(_, v, _, _)| v)
            })
        };
        match sem {
            TextureSemantic::Source => {
                if pass_idx == 0 {
                    self.chain_input()
                } else {
                    out_of(pass_idx - 1)
                }
            }
            TextureSemantic::Original => self.chain_input(),
            TextureSemantic::OriginalHistory(n) => {
                if self.interop_view.is_some() {
                    self.interop_view.as_ref()
                } else {
                    let last = self.history.len().saturating_sub(1);
                    self.history
                        .get((*n as usize).min(last))
                        .map(|(_, v, _, _)| v)
                }
            }
            TextureSemantic::PassOutput(n) => out_of(*n as usize),
            TextureSemantic::PassFeedback(n) => fb_of(*n as usize),
            TextureSemantic::Named { name, feedback } => {
                if let Some(&pi) = self.pass_alias.get(name) {
                    if *feedback {
                        fb_of(pi)
                    } else {
                        out_of(pi)
                    }
                } else {
                    self.luts.get(name).map(|(_, v, _, _, _)| v)
                }
            }
        }
    }

    /// Nome-base de cada textura do passe → tamanho (pros uniformes `<Nome>Size`).
    fn tex_sizes_for(
        &self,
        pass_idx: usize,
        input: (u32, u32),
        native: (u32, u32),
        sizes: &[(u32, u32)],
    ) -> HashMap<String, (u32, u32)> {
        let mut m = HashMap::new();
        m.insert("Source".to_string(), input);
        m.insert("Original".to_string(), native);
        for b in &self.passes[pass_idx].textures {
            let (base, sz) = match &b.semantic {
                TextureSemantic::Source => continue,
                TextureSemantic::Original => continue,
                TextureSemantic::OriginalHistory(k) => (format!("OriginalHistory{k}"), native),
                TextureSemantic::PassOutput(k) => (
                    format!("PassOutput{k}"),
                    sizes.get(*k as usize).copied().unwrap_or(native),
                ),
                TextureSemantic::PassFeedback(k) => (
                    format!("PassFeedback{k}"),
                    sizes.get(*k as usize).copied().unwrap_or(native),
                ),
                TextureSemantic::Named { name, .. } => {
                    let sz = if let Some(&pi) = self.pass_alias.get(name) {
                        sizes.get(pi).copied().unwrap_or(native)
                    } else if let Some((_, _, _, w, h)) = self.luts.get(name) {
                        (*w, *h)
                    } else {
                        native
                    };
                    (name.clone(), sz)
                }
            };
            m.insert(base, sz);
        }
        m
    }

    /// Sampler `(filtro, wrap)` do cache (cria na 1ª vez). Clona — samplers wgpu
    /// são handles baratos.
    fn sampler_for(&mut self, linear: bool, wrap: WrapMode) -> wgpu::Sampler {
        self.sampler_cache
            .entry((linear, wrap))
            .or_insert_with(|| {
                let f = if linear {
                    wgpu::FilterMode::Linear
                } else {
                    wgpu::FilterMode::Nearest
                };
                self.device.create_sampler(&sampler_desc(f, wrap))
            })
            .clone()
    }
}

/// `plain|crt|lcd` ou um caminho `.slangp` → preset pronto (nome, params,
/// metadados dos parâmetros, passes).
fn build_specs(want: &str) -> Result<BuiltSpecs, String> {
    if let Some(c) = curated_by_wire(want) {
        let p = curated_slangp(c).ok_or_else(|| {
            format!(
                "'{}' precisa do pacote de shaders — baixe em Config › Vídeo",
                c.label
            )
        })?;
        let s = p.to_string_lossy();
        return build_specs(&s);
    }
    if let Some(b) = BUILTINS.iter().find(|b| b.name == want) {
        let passes = b
            .passes
            .iter()
            .map(|p| PassSpec {
                scale_x: Scale::Source(p.scale),
                scale_y: Scale::Source(p.scale),
                linear: p.linear,
                wrap: WrapMode::ClampToEdge,
                vs_wgsl: BUILTIN_VS.to_string(),
                fs_wgsl: format!("{BUILTIN_FS_PRELUDE}\n{}", p.fs),
                uniform: UniformMode::Fixed,
                textures: Vec::new(),
                fmt: FMT,
                frame_count_mod: 0,
                alias: None,
                feedback: false,
            })
            .collect();
        return Ok(BuiltSpecs {
            name: b.name.to_string(),
            params: HashMap::new(),
            meta: Vec::new(),
            passes,
            luts: Vec::new(),
            history_depth: 1,
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
    let mut history_depth = 1usize;
    // aliases só ficam conhecidos depois de ler todos os passes (um passe pode
    // referenciar o alias de outro que vem depois? não — mas o alias pode não
    // ser o `#pragma name`; usamos o `aliasN` do `.slangp`).
    let aliases: Vec<Option<String>> = preset.passes.iter().map(|p| p.alias.clone()).collect();
    // um passe é fonte de feedback se `feedback_pass{i}` OU se algum passe
    // amostra `PassFeedback<i>` / `<aliasI>Feedback`.
    let mut needs_feedback = vec![false; preset.passes.len()];
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
        for t in &compiled.textures {
            match &t.semantic {
                TextureSemantic::OriginalHistory(n) => {
                    history_depth = history_depth.max(*n as usize + 1);
                }
                TextureSemantic::PassFeedback(n) => {
                    if let Some(f) = needs_feedback.get_mut(*n as usize) {
                        *f = true;
                    }
                }
                TextureSemantic::Named {
                    name,
                    feedback: true,
                } => {
                    if let Some(pi) = aliases.iter().position(|a| a.as_deref() == Some(name)) {
                        needs_feedback[pi] = true;
                    }
                }
                _ => {}
            }
        }
        if pass.feedback {
            needs_feedback[i] = true;
        }
        let fmt = if pass.float_framebuffer {
            wgpu::TextureFormat::Rgba16Float
        } else if pass.srgb_framebuffer {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            FMT
        };
        specs.push(PassSpec {
            scale_x: pass.scale_x,
            scale_y: pass.scale_y,
            linear: pass.filter_linear,
            wrap: pass.wrap_mode,
            vs_wgsl: compiled.vertex_wgsl,
            fs_wgsl: compiled.fragment_wgsl,
            uniform: UniformMode::Slang(compiled.uniforms),
            textures: compiled.textures,
            fmt,
            frame_count_mod: pass.frame_count_mod,
            alias: pass.alias.clone(),
            feedback: false, // preenchido abaixo
        });
    }
    for (spec, need) in specs.iter_mut().zip(needs_feedback) {
        spec.feedback = need;
    }
    // valores do `.slangp` sobrescrevem os defaults dos `#pragma parameter`
    for (k, v) in &preset.parameters {
        params.insert(k.clone(), *v);
    }

    // texturas do usuário (LUT/máscara) — decodifica agora, sobe pra GPU depois.
    let mut luts = Vec::new();
    for tex in &preset.textures {
        match crate::decoration::decode_png(&tex.path) {
            Ok((rgba, w, h)) => luts.push(LutSpec {
                name: tex.name.clone(),
                rgba,
                w,
                h,
                linear: tex.linear,
                wrap: tex.wrap_mode,
            }),
            Err(e) => log::warn!("LUT '{}' ({}): {e}", tex.name, tex.path.display()),
        }
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
        luts,
        history_depth,
    })
}

/// Preset já com os recursos wgpu (passes, LUTs) criados — o que `new` e
/// `set_preset` precisam.
struct Realized {
    preset_name: String,
    params: HashMap<String, f32>,
    param_meta: Vec<shader_slang::Parameter>,
    passes: Vec<Pass>,
    luts: HashMap<String, (wgpu::Texture, wgpu::TextureView, wgpu::Sampler, u32, u32)>,
    pass_alias: HashMap<String, usize>,
    history_depth: usize,
}

fn realize(device: &wgpu::Device, queue: &wgpu::Queue, built: BuiltSpecs) -> Option<Realized> {
    let BuiltSpecs {
        name,
        params,
        meta,
        passes: specs,
        luts: lut_specs,
        history_depth,
    } = built;

    let mut pass_alias = HashMap::new();
    for (i, s) in specs.iter().enumerate() {
        if let Some(a) = &s.alias {
            pass_alias.insert(a.clone(), i);
        }
    }
    let passes = specs
        .into_iter()
        .map(|s| build_pass(device, s))
        .collect::<Option<Vec<_>>>()?;

    let mut luts = HashMap::new();
    for l in lut_specs {
        let (tex, view) = new_tex(
            device,
            l.w,
            l.h,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &l.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(l.w * 4),
                rows_per_image: Some(l.h),
            },
            wgpu::Extent3d {
                width: l.w,
                height: l.h,
                depth_or_array_layers: 1,
            },
        );
        let filter = if l.linear {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };
        let sampler = device.create_sampler(&sampler_desc(filter, l.wrap));
        luts.insert(l.name, (tex, view, sampler, l.w, l.h));
    }

    Some(Realized {
        preset_name: name,
        params,
        param_meta: meta,
        passes,
        luts,
        pass_alias,
        history_depth,
    })
}

fn wrap_address(w: WrapMode) -> wgpu::AddressMode {
    match w {
        WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrapMode::ClampToBorder => wgpu::AddressMode::ClampToBorder,
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
        WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
    }
}

fn sampler_desc(filter: wgpu::FilterMode, wrap: WrapMode) -> wgpu::SamplerDescriptor<'static> {
    let a = wrap_address(wrap);
    wgpu::SamplerDescriptor {
        label: Some("etapa04 sampler"),
        address_mode_u: a,
        address_mode_v: a,
        address_mode_w: a,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: if filter == wgpu::FilterMode::Linear {
            wgpu::MipmapFilterMode::Linear
        } else {
            wgpu::MipmapFilterMode::Nearest
        },
        border_color: matches!(wrap, WrapMode::ClampToBorder)
            .then_some(wgpu::SamplerBorderColor::TransparentBlack),
        ..Default::default()
    }
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

/// Rotaciona uma textura em múltiplos de 90° (`SET_ROTATION`). Reusa o
/// `comp.bgl` (uniforme no binding 0, textura 1, sampler 2). `R.c.xy` = (cos,
/// sin) da rotação aplicada em `uv - 0.5` (giro do UV = giro inverso da imagem).
const ROT_WGSL: &str = r#"
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

fn rotate_pipeline(
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

/// Inverte o eixo Y de uma textura (fullscreen triangle). Pros `dma_buf` de
/// cores GL, que renderizam com origem bottom-left.
const FLIP_WGSL: &str = r#"
@group(0) @binding(0) var T: texture_2d<f32>;
@group(0) @binding(1) var S: sampler;
struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
    let p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: V;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    // uv.y sem a inversão usual = amostra de baixo pra cima (flip).
    o.uv = vec2<f32>((p[i].x + 1.0) * 0.5, (p[i].y + 1.0) * 0.5);
    return o;
}
@fragment fn fs(v: V) -> @location(0) vec4<f32> { return textureSample(T, S, v.uv); }
"#;

struct FlipPipe {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
}

fn build_flip(device: &wgpu::Device) -> FlipPipe {
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
        ],
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
    FlipPipe { pipeline, bgl }
}

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

/// Entradas da BGL de um passe: binding 0 e 1 (uniformes) + 1 par
/// textura/sampler pra cada sampler declarado (bindings `2+2i` / `3+2i`).
fn pass_bgl(device: &wgpu::Device, textures: &[TextureBind]) -> wgpu::BindGroupLayout {
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

fn build_pass(device: &wgpu::Device, spec: PassSpec) -> Option<Pass> {
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
    })
}

fn axis_size(scale: Scale, cur: u32, native: u32, viewport: u32) -> u32 {
    match scale {
        Scale::Source(m) => (cur as f32 * m).round().max(1.0) as u32,
        Scale::Absolute(px) => px.max(1),
        Scale::Viewport(m) => {
            let base = if viewport > 0 { viewport } else { native * 3 };
            (base as f32 * m).round().max(1.0) as u32
        }
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
    tex_sizes: &HashMap<String, (u32, u32)>,
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
            // `<Textura>Size` — history, PassOutput, feedback, LUTs, aliases.
            (name, _) if name.ends_with("Size") => {
                if let Some((w, h)) = tex_sizes.get(name.trim_end_matches("Size")) {
                    put(&mut b, o, f32s_bytes(&size_vec(*w, *h)));
                }
            }
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
    new_tex_fmt(device, w, h, FMT, usage)
}

fn new_tex_fmt(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("etapa04 tex"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let v = t.create_view(&wgpu::TextureViewDescriptor::default());
    (t, v)
}

/// `VkFormat` cru → `wgpu::TextureFormat` **amostrável direto** (`texture_from_raw`
/// só serve se a `VkImage` já é um formato que o wgpu conhece). `None` = precisa
/// de conversão antes (`VkBlit` faz `vkCmdBlitImage` pra RGBA8) — os formatos
/// packed 16-bit do PS1 (A1R5G5B5 = 8, R5G5B5A1 = 7, R5G6B5 = 4), que o wgpu
/// não tem.
fn vk_format_to_wgpu(vk_format: u32) -> Option<wgpu::TextureFormat> {
    Some(match vk_format {
        37 => wgpu::TextureFormat::Rgba8Unorm,     // R8G8B8A8_UNORM (vk_rendering, Beetle 32bpp)
        43 => wgpu::TextureFormat::Rgba8UnormSrgb, // R8G8B8A8_SRGB
        44 => wgpu::TextureFormat::Bgra8Unorm,     // B8G8R8A8_UNORM
        50 => wgpu::TextureFormat::Bgra8UnormSrgb, // B8G8R8A8_SRGB
        64 => wgpu::TextureFormat::Rgba16Float,    // R16G16B16A16_SFLOAT (Beetle HDR interno)
        97 => wgpu::TextureFormat::Rgba16Float,    // (alias observado em drivers)
        // Packed 16-bit → `None`: o `bind_via_blit` converte com `vkCmdBlitImage`.
        4 | 6 | 7 | 8 => return None,
        other => {
            log::warn!("VkFormat {other} inesperado no scanout do core — assumindo Rgba8Unorm");
            wgpu::TextureFormat::Rgba8Unorm
        }
    })
}

fn f32s_bytes(s: &[f32]) -> &[u8] {
    // SAFETY: `f32` não tem padding nem invariantes de bit.
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, SoftwarePixelFormat};

    #[test]
    fn curated_wire_ids_resolve_and_are_unique() {
        let mut ids: Vec<&str> = CURATED.iter().map(|c| c.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "id curado duplicado");
        assert!(curated_by_wire("curated:xbr").is_some());
        assert!(curated_by_wire("curated:desconhecido").is_none());
        assert!(curated_by_wire("xbr").is_none(), "exige o prefixo curated:");
        assert!(CURATED.iter().all(|c| c.relpath.ends_with(".slangp")));
    }

    #[test]
    fn curated_without_pack_errors_with_hint() {
        // SHADER_ROOT não setada neste processo de teste → sem pacote.
        let e = match build_specs("curated:ntsc") {
            Ok(_) => panic!("deveria falhar sem o pacote"),
            Err(e) => e,
        };
        assert!(e.contains("pacote de shaders"), "mensagem: {e}");
    }

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

    /// Etapa 12 §Beetle (fatia D1): o `FrameProcessor` ADOTA uma
    /// `VkInstance`/`VkDevice` criados fora do wgpu (aqui, à mão com `ash` —
    /// estruturalmente o que o `libretro_create_device` do Beetle devolve) e
    /// roda a chain de shader nela. Prova o caminho `wgpu-hal`
    /// `Instance::from_raw` → `expose_adapter` → `device_from_raw` →
    /// `wgpu::…::from_hal` antes de fiar o Beetle de verdade (D2..D5).
    ///
    /// `#[ignore]`: precisa de um ICD Vulkan (GPU ou lavapipe).
    #[test]
    #[ignore = "precisa de ICD Vulkan"]
    fn from_adopted_vulkan_runs_the_chain() {
        use ash::vk;

        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let Ok(entry) = (unsafe { ash::Entry::load() }) else {
            eprintln!("sem loader Vulkan — pulando");
            return;
        };
        let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_2);
        let Ok(instance) = (unsafe {
            entry.create_instance(&vk::InstanceCreateInfo::default().application_info(&app), None)
        }) else {
            eprintln!("sem ICD Vulkan — pulando");
            return;
        };

        let gpus = unsafe { instance.enumerate_physical_devices() }.unwrap_or_default();
        let Some(&gpu) = gpus.first() else {
            eprintln!("sem GPU Vulkan — pulando");
            unsafe { instance.destroy_instance(None) };
            return;
        };
        let fams = unsafe { instance.get_physical_device_queue_family_properties(gpu) };
        let Some(qfi) = fams.iter().position(|q| {
            q.queue_flags
                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
        }) else {
            eprintln!("sem queue GRAPHICS+COMPUTE — pulando");
            unsafe { instance.destroy_instance(None) };
            return;
        };
        let qfi = qfi as u32;

        let prio = [1.0f32];
        let qci = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(qfi)
            .queue_priorities(&prio)];
        let device = unsafe {
            instance.create_device(
                gpu,
                &vk::DeviceCreateInfo::default().queue_create_infos(&qci),
                None,
            )
        }
        .expect("create_device");

        let adopted = AdoptedVulkan {
            entry,
            instance,
            physical_device: gpu,
            device,
            queue_family_index: qfi,
            queue_index: 0,
            instance_api_version: vk::API_VERSION_1_2,
            instance_extensions: vec![],
            device_extensions: vec![],
            features: wgpu::Features::empty(),
        };
        let mut fp = unsafe { FrameProcessor::from_adopted_vulkan(adopted) }
            .expect("from_adopted_vulkan devolveu None");

        // readback com pipeline: 1º frame prima (pode vir None), 2º entrega.
        fp.process(&grey_frame(64, 48, 0xC0));
        let (w, h, rgba) = fp
            .process(&grey_frame(64, 48, 0xC0))
            .expect("a chain devia entregar um frame no device adotado");
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        assert!(
            rgba.chunks(4).any(|p| p[0] > 0x80 && p[2] > 0x80),
            "esperava pixel claro da chain 'plain' no device adotado"
        );
        // `fp` dropa aqui; a `VkInstance`/`VkDevice` vazam de propósito (o
        // `drop_callback` é `None` — quem seria o dono é o core; no teste o
        // processo sai logo).
    }

    /// Etapa 12 ponta-a-ponta (fase B): o core de teste `vk_rendering` adota o
    /// `VkDevice` do compositor, renderiza o triângulo numa `VkImage`, e o
    /// `FrameProcessor` a amostra com `texture_from_raw` (zero cópia) + roda a
    /// chain. Confere que sai pixel colorido (o core limpa pra 0.8,0.6,0.2).
    ///
    /// `#[ignore]`: precisa de GPU + `scripts/build-vk-test-core.sh`.
    #[test]
    #[ignore = "precisa de ICD Vulkan + scripts/build-vk-test-core.sh"]
    fn vk_hw_render_core_frame_reaches_the_chain() {
        use core_loader_desktop::DesktopCoreLoader;
        use domain::core_loader::CoreId;
        use domain::frame_source::FrameSource;

        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let core_path = {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../target/vk-test-core/testvulkan_libretro.so");
            if !root.is_file() {
                eprintln!("core de teste ausente — rode scripts/build-vk-test-core.sh");
                return;
            }
            root
        };
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let Some(shared) = fp.vulkan_shared_device() else {
            eprintln!("backend wgpu não-Vulkan — pulando");
            return;
        };

        let tmp = std::env::temp_dir();
        let rom = tmp.join(format!("reemu-vkchain-{}.bin", std::process::id()));
        std::fs::write(&rom, b"").unwrap();

        let mut core = DesktopCoreLoader::new(tmp.clone(), tmp.clone(), tmp)
            .with_vulkan_shared_device(shared)
            .open_core(
                &CoreId(core_path.to_string_lossy().into_owned()),
                rom.to_str().unwrap(),
            )
            .expect("carregar o core de teste Vulkan adotando o device do wgpu");

        // Alguns frames: o 1º prima o readback com pipeline (None), depois vem.
        let mut got_color = false;
        for i in 0..8 {
            let Some(frame) = core.next_frame() else {
                continue;
            };
            if let Some((w, h, rgba)) = fp.process(&frame) {
                assert_eq!(rgba.len(), (w * h * 4) as usize);
                // fundo do core = RGB (0.8, 0.6, 0.2) ≈ (204, 153, 51).
                let bright = rgba.chunks(4).any(|p| p[0] > 20 && p[1] > 20);
                if bright {
                    got_color = true;
                    eprintln!("frame {i}: {w}x{h}, 1º pixel = {:?}", &rgba[..4]);
                    break;
                }
            }
        }
        let _ = std::fs::remove_file(&rom);
        assert!(
            got_color,
            "a chain nunca recebeu um frame colorido do core Vulkan"
        );
    }

    /// Etapa 12: os handles Vulkan que exportamos pro core de HW render têm
    /// que ser REAIS — reconstrói `ash::Instance`/`Device` a partir deles
    /// (exatamente o que `VkContext::adopt` faz no `core-loader-desktop`) e
    /// chama a API pra provar que respondem.
    #[test]
    fn vulkan_shared_device_handles_are_usable() {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let Some(fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let Some(shared) = fp.vulkan_shared_device() else {
            eprintln!("backend wgpu não-Vulkan (ou queue sem GRAPHICS+COMPUTE) — pulando");
            return;
        };

        assert_ne!(shared.get_instance_proc_addr, 0);
        assert_ne!(shared.instance, 0);
        assert_ne!(shared.physical_device, 0);
        assert_ne!(shared.device, 0);
        assert_ne!(shared.queue, 0);

        // SAFETY: mesma reconstrução do `VkContext::adopt`; só lê propriedades.
        let name = unsafe {
            let gipa = std::mem::transmute::<usize, ash::vk::PFN_vkGetInstanceProcAddr>(
                shared.get_instance_proc_addr,
            );
            let static_fn = ash::StaticFn {
                get_instance_proc_addr: gipa,
            };
            let inst_handle = std::mem::transmute::<usize, ash::vk::Instance>(shared.instance);
            let instance = ash::Instance::load(&static_fn, inst_handle);
            let phd = std::mem::transmute::<usize, ash::vk::PhysicalDevice>(shared.physical_device);
            let props = instance.get_physical_device_properties(phd);
            // Provar que o VkDevice também responde: carregar a tabela dele e
            // pedir a queue declarada tem que devolver o MESMO handle.
            let dev_handle = std::mem::transmute::<usize, ash::vk::Device>(shared.device);
            let device = ash::Device::load(instance.fp_v1_0(), dev_handle);
            let q = device.get_device_queue(shared.queue_family_index, 0);
            assert_eq!(
                std::mem::transmute::<ash::vk::Queue, usize>(q),
                shared.queue,
                "a queue exportada tem que ser a (family, 0) do device"
            );
            std::ffi::CStr::from_ptr(props.device_name.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        assert!(
            !name.is_empty(),
            "vkGetPhysicalDeviceProperties devolveu nome vazio"
        );
        eprintln!(
            "device compartilhável: {name} (queue family {})",
            shared.queue_family_index
        );
    }

    /// Handle de teste pro lado consumidor do interop GL (`import_dmabuf`/
    /// `bind_interop_input`) — embrulha um `DmabufPlaneInfo` já pronto (vindo
    /// de `render_solid_rgba_to_dmabuf`, o lado produtor GL do
    /// `core-loader-desktop`) na mesma interface que um `GlInteropHandle` de
    /// verdade usaria.
    struct TestDmabufHandle {
        slot: u32,
        plane: std::sync::Mutex<Option<domain::frame_source::DmabufPlaneInfo>>,
    }

    impl domain::frame_source::GpuTextureHandle for TestDmabufHandle {
        fn slot(&self) -> u32 {
            self.slot
        }
        fn take_plane(&self) -> Option<domain::frame_source::DmabufPlaneInfo> {
            self.plane.lock().unwrap_or_else(|e| e.into_inner()).take()
        }
    }

    /// Etapa 02 (backlog): valida o lado CONSUMIDOR do interop dma_buf —
    /// `import_dmabuf`/`bind_interop_input` (`gpu.rs`) — contra um `dma_buf`
    /// produzido de verdade pelo lado GL do `core-loader-desktop`
    /// (`render_solid_rgba_to_dmabuf`, `EGL_EXT_image_dma_buf_import` + GBM),
    /// não um mock. Fecha a ponta que faltava: o produtor (GL) já tinha teste
    /// próprio (`core-loader-desktop::gl_context::tests::
    /// interop_ring_renders_into_dmabuf_backed_texture`); este prova que o
    /// wgpu do lado do compositor importa e amostra esse MESMO `dma_buf`
    /// corretamente, ponta a ponta entre os dois crates.
    #[test]
    #[ignore = "precisa de EGL+GBM+Vulkan em hardware real (render node DRM)"]
    fn dmabuf_from_gl_producer_imports_correctly_into_wgpu() {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let plane = core_loader_desktop::render_solid_rgba_to_dmabuf([220, 40, 10, 255], 64, 64)
            .expect("renderizar dma_buf de teste via GL (core-loader-desktop)");
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        assert!(
            fp.interop_ok,
            "device wgpu sem VULKAN_EXTERNAL_MEMORY_DMA_BUF — não dá pra validar interop aqui"
        );

        let frame = Frame {
            origin: FrameOrigin::HardwareTexture(Box::new(TestDmabufHandle {
                slot: 0,
                plane: std::sync::Mutex::new(Some(plane)),
            })),
            metadata: FrameMetadata {
                native_width: 64,
                native_height: 64,
                aspect_ratio: 1.0,
                rotation_degrees: 0,
            },
        };

        assert!(
            fp.process(&frame).is_none(),
            "1º process deve primar o pipeline (None)"
        );
        let (w, h, data) = fp.process(&frame).expect("2º process entrega");
        assert_eq!((w, h), (64, 64));
        assert_eq!(data.len(), 64 * 64 * 4);
        // `plain` (default) é passthrough — a cor tem que sair igual à que
        // foi renderizada no dma_buf do lado GL (R,G,B,A na mesma ordem de
        // memória do fourcc ABGR8888 usado pelo produtor).
        assert!(
            data[0].abs_diff(220) <= 2 && data[1].abs_diff(40) <= 2 && data[2].abs_diff(10) <= 2,
            "esperava ~[220,40,10,..], veio {:?}",
            &data[0..4]
        );
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

    /// Preset slang de 2 passes: o 2º amostra `Source` + `Original` +
    /// `OriginalHistory1` + o feedback do 1º (via alias). Exercita a BGL
    /// dinâmica, `resolve_tex_view`, o ring de history e a cópia de feedback.
    #[test]
    fn multipass_slang_with_history_and_feedback_runs() {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let dir = std::env::temp_dir().join("reemu_gpu_phase2");
        std::fs::create_dir_all(&dir).unwrap();
        let pass0 = dir.join("p0.slang");
        let pass1 = dir.join("p1.slang");
        let slangp = dir.join("chain.slangp");
        let vs = concat!(
            "#pragma stage vertex\n",
            "layout(location=0) in vec4 Position; layout(location=1) in vec2 TexCoord;\n",
            "layout(location=0) out vec2 vUV;\n",
            "layout(std140, set=0, binding=0) uniform UBO { mat4 MVP; } g;\n",
            "void main(){ gl_Position = g.MVP * Position; vUV = TexCoord; }\n",
        );
        std::fs::write(
            &pass0,
            [
                "#version 450\n#pragma name First\n",
                vs,
                "#pragma stage fragment\nlayout(location=0) in vec2 vUV;\n",
                "layout(location=0) out vec4 c;\n",
                "layout(set=0,binding=2) uniform sampler2D Source;\n",
                "void main(){ c = texture(Source, vUV); }\n",
            ]
            .concat(),
        )
        .unwrap();
        std::fs::write(
            &pass1,
            [
                "#version 450\n",
                vs,
                "#pragma stage fragment\nlayout(location=0) in vec2 vUV;\n",
                "layout(location=0) out vec4 c;\n",
                "layout(set=0,binding=2) uniform sampler2D Source;\n",
                "layout(set=0,binding=3) uniform sampler2D Original;\n",
                "layout(set=0,binding=4) uniform sampler2D OriginalHistory1;\n",
                "layout(set=0,binding=5) uniform sampler2D FirstFeedback;\n",
                "void main(){ c = 0.25*(texture(Source,vUV)+texture(Original,vUV)",
                "+texture(OriginalHistory1,vUV)+texture(FirstFeedback,vUV)); }\n",
            ]
            .concat(),
        )
        .unwrap();
        std::fs::write(
            &slangp,
            "shaders = 2\nshader0 = p0.slang\nalias0 = First\nfeedback_pass0 = true\nshader1 = p1.slang\n",
        )
        .unwrap();

        fp.set_preset(slangp.to_str().unwrap())
            .expect("preset multi-passe deve montar");
        assert_eq!(fp.passes.len(), 2);
        assert_eq!(fp.history_depth, 2, "OriginalHistory1 ⇒ 2 slots");
        assert!(fp.passes[0].feedback, "feedback_pass0 + FirstFeedback");
        assert!(
            fp.passes[0].feedback_target.is_none(),
            "alocado sob demanda"
        );

        fp.process(&grey_frame(32, 32, 0x20));
        let (w, h, d) = fp
            .process(&grey_frame(32, 32, 0x80))
            .expect("2º process entrega um frame");
        assert_eq!((w, h), (32, 32));
        assert_eq!(d.len(), 32 * 32 * 4);
        assert!(
            fp.passes[0].feedback_target.is_some(),
            "feedback alocado ao rodar"
        );
        let _ = fp.process(&grey_frame(32, 32, 0x40));

        std::fs::remove_dir_all(&dir).ok();
    }

    fn collect_slangp(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_slangp(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("slangp") {
                out.push(p);
            }
        }
    }

    /// Roda `build_specs` (parse + glslang→SPIR-V→naga por passe) em cada
    /// `.slangp` de uma pasta e tabula. Ignorado por padrão — é validação de
    /// campo, não CI. Rode com:
    ///   REEMU_SHADER_DIR=~/.local/share/com.reemu.desktop/shaders/slang-shaders \
    ///   cargo test -p reemu-desktop --lib field_validate_real_presets -- --ignored --nocapture
    #[test]
    #[ignore]
    fn field_validate_real_presets() {
        let dir = std::env::var("REEMU_SHADER_DIR").unwrap_or_else(|_| {
            format!(
                "{}/.local/share/com.reemu.desktop/shaders/slang-shaders",
                std::env::var("HOME").unwrap_or_default()
            )
        });
        let root = std::path::Path::new(&dir);
        let mut presets = Vec::new();
        collect_slangp(root, &mut presets);
        presets.sort();
        assert!(!presets.is_empty(), "nenhum .slangp em {dir}");
        eprintln!("validando {} presets em {dir}\n", presets.len());

        let limit: usize = std::env::var("REEMU_SHADER_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(usize::MAX);

        let (mut ok, mut err) = (0usize, 0usize);
        // tally por categoria (1º componente do caminho; `bezel/X` usa 2 —
        // os mega-pacotes são projetos distintos)
        let mut by_dir: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        let cat = |rel: &str| -> String {
            let parts: Vec<&str> = rel.split('/').collect();
            match parts.as_slice() {
                ["bezel", sub, ..] => format!("bezel/{sub}"),
                [top, _, ..] => top.to_string(),
                _ => "(raiz)".to_string(),
            }
        };
        let mut by_reason: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        // erro completo do 1º caso de cada grupo (a chave é truncada)
        let mut full: std::collections::BTreeMap<String, String> = Default::default();
        // `REEMU_SHADER_OK_LIST=/caminho` → grava os `.slangp` que compilam
        // (lista curada pro pacote de shaders).
        let mut ok_list: Vec<String> = Vec::new();
        for p in presets.iter().take(limit) {
            let rel = p.strip_prefix(root).unwrap_or(p).display().to_string();
            let entry = by_dir.entry(cat(&rel)).or_default();
            match build_specs(p.to_str().unwrap()) {
                Ok(b) => {
                    ok += 1;
                    entry.0 += 1;
                    ok_list.push(rel.clone());
                    let _ = b;
                }
                Err(e) => {
                    err += 1;
                    entry.1 += 1;
                    // agrupa pela causa: tira os caminhos absolutos (ruído) e
                    // corta na 1ª linha, senão cada erro do glslang vira um grupo.
                    let cleaned = e.replace(&dir, "…").replace('\n', " ");
                    let key: String = cleaned.chars().take(140).collect();
                    full.entry(key.clone()).or_insert(cleaned);
                    by_reason.entry(key).or_default().push(rel);
                }
            }
        }
        if let Ok(path) = std::env::var("REEMU_SHADER_OK_LIST") {
            ok_list.sort();
            let _ = std::fs::write(&path, ok_list.join("\n") + "\n");
            eprintln!("lista de {} presets ok gravada em {path}", ok_list.len());
        }
        eprintln!("\n=== {ok} ok · {err} falharam ===\n");
        let mut reasons: Vec<_> = by_reason.into_iter().collect();
        reasons.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (reason, files) in &reasons {
            eprintln!("[{}×] {}", files.len(), reason);
            if std::env::var_os("REEMU_SHADER_FULL_ERR").is_some() {
                if let Some(f) = full.get(reason) {
                    eprintln!("  ↳ {f}");
                }
            }
            for f in files.iter().take(4) {
                eprintln!("      {f}");
            }
            if files.len() > 4 {
                eprintln!("      … +{}", files.len() - 4);
            }
        }
        eprintln!("\n=== por categoria (ok/total) ===");
        let mut cats: Vec<_> = by_dir.into_iter().collect();
        cats.sort_by_key(|(_, (o, e))| std::cmp::Reverse(o + e));
        for (c, (o, e)) in &cats {
            let t = o + e;
            eprintln!(
                "  {:>4}/{:<4} {:>5.1}%  {c}",
                o,
                t,
                100.0 * *o as f64 / t as f64
            );
        }
        eprintln!(
            "\ntaxa de sucesso: {:.1}%",
            100.0 * ok as f64 / (ok + err).max(1) as f64
        );
    }

    /// Regressão: shaders slang saíam de cabeça pra baixo depois que o frontend
    /// virou glslang→SPIR-V→naga (o naga negava o Y de `gl_Position` por
    /// padrão). Frame com a metade de CIMA branca, metade de baixo preta →
    /// depois de um passthrough slang, o topo da saída tem que continuar branco.
    #[test]
    fn slang_passthrough_keeps_orientation() {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let dir = std::env::temp_dir().join("reemu_gpu_orient");
        std::fs::create_dir_all(&dir).unwrap();
        let sp = dir.join("pass.slangp");
        let sl = dir.join("pass.slang");
        std::fs::write(
            &sl,
            concat!(
                "#version 450\n",
                "#pragma stage vertex\n",
                "layout(location=0) in vec4 Position; layout(location=1) in vec2 TexCoord;\n",
                "layout(location=0) out vec2 vUV;\n",
                "layout(std140, set=0, binding=0) uniform UBO { mat4 MVP; } g;\n",
                "void main(){ gl_Position = g.MVP * Position; vUV = TexCoord; }\n",
                "#pragma stage fragment\n",
                "layout(location=0) in vec2 vUV;\n",
                "layout(location=0) out vec4 c;\n",
                "layout(set=0,binding=2) uniform sampler2D Source;\n",
                "void main(){ c = texture(Source, vUV); }\n",
            ),
        )
        .unwrap();
        std::fs::write(
            &sp,
            "shaders = 1\nshader0 = pass.slang\nscale_type0 = source\nscale0 = 1.0\n",
        )
        .unwrap();
        fp.set_preset(sp.to_str().unwrap())
            .expect("preset passthrough");

        let (w, h) = (32u32, 32u32);
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            let v = if y < h / 2 { 0xFF } else { 0x00 };
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[v, v, v, 0xFF]);
            }
        }
        let mk = || Frame {
            origin: FrameOrigin::SoftwareRawBuffer {
                data: data.clone(),
                pitch: w * 4,
                format: SoftwarePixelFormat::Xrgb8888,
            },
            metadata: FrameMetadata {
                native_width: w,
                native_height: h,
                aspect_ratio: 1.0,
                rotation_degrees: 0,
            },
        };
        fp.process(&mk());
        let (ow, oh, out) = fp.process(&mk()).expect("2º process entrega");
        let px = |x: u32, y: u32| out[((y * ow + x) * 4) as usize];
        assert!(
            px(ow / 2, 1) > 0xC0,
            "topo devia estar branco, veio {:#x}",
            px(ow / 2, 1)
        );
        assert!(
            px(ow / 2, oh - 2) < 0x40,
            "base devia estar preta, veio {:#x}",
            px(ow / 2, oh - 2)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Etapa 12 B3b ponta-a-ponta: com `REEMU_HW=vulkan` + o device do
    /// compositor publicado, `EmuSession` roda o core de teste `vk_rendering`
    /// **in-process** (nunca sobe o `reemu-core-host`), e o `Frame` que sai
    /// pela API de sempre (`take_latest_frame`) é uma `HardwareVulkanImage`
    /// que a chain amostra e devolve pixel colorido.
    ///
    /// `#[ignore]`: precisa de GPU Vulkan + `scripts/build-vk-test-core.sh`.
    /// Rode isolado: `-- --ignored --test-threads=1` (mexe no env `REEMU_HW`).
    #[test]
    #[ignore = "precisa de ICD Vulkan + scripts/build-vk-test-core.sh"]
    fn emu_session_routes_vulkan_core_in_process() {
        use emu_session::{EmuSession, SessionConfig};
        use std::collections::HashMap;
        use std::time::{Duration, Instant};

        if std::env::var_os("REEMU_NO_GPU").is_some() {
            return;
        }
        let core_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/vk-test-core/testvulkan_libretro.so");
        if !core_path.is_file() {
            eprintln!("core de teste ausente — rode scripts/build-vk-test-core.sh");
            return;
        }
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let Some(shared) = fp.vulkan_shared_device() else {
            eprintln!("backend wgpu não-Vulkan — pulando");
            return;
        };

        // Opt-in: sem isso o `GET_PREFERRED_HW_RENDER` não devolve Vulkan e o
        // roteamento local nem é tentado.
        std::env::set_var("REEMU_HW", "vulkan");

        let tmp = std::env::temp_dir();
        let rom = tmp.join(format!("reemu-b3b-{}.bin", std::process::id()));
        std::fs::write(&rom, b"").unwrap();

        let session =
            EmuSession::spawn(SessionConfig::new(tmp.clone(), tmp.clone(), tmp.clone()));
        session.attach_vulkan_device(shared);
        session
            .load(
                core_path.to_str().unwrap(),
                rom.to_str().unwrap(),
                HashMap::new(),
            )
            .expect("carregar o core Vulkan pela sessão");

        assert!(
            session.debug_child_pid().is_none(),
            "core Vulkan foi pro processo filho — devia rodar in-process"
        );

        // B3b/D4: o core Vulkan local é dirigido por ESTA thread (o papel do
        // video pump), a mesma que roda a chain — `step_vk_local` faz o
        // `retro_run` e devolve o `Frame`.
        let mut got_color = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && !got_color {
            let Some(frame) = session.step_vk_local() else {
                std::thread::sleep(Duration::from_millis(15));
                continue;
            };
            assert!(
                matches!(frame.origin, FrameOrigin::HardwareVulkanImage(_)),
                "esperava HardwareVulkanImage da sessão"
            );
            if let Some((w, h, rgba)) = fp.process(&frame) {
                assert_eq!(rgba.len(), (w * h * 4) as usize);
                if rgba.chunks(4).any(|p| p[0] > 20 && p[1] > 20) {
                    got_color = true;
                    eprintln!("B3b frame {w}x{h}, 1º pixel = {:?}", &rgba[..4]);
                }
            }
        }

        session.unload().ok();
        let _ = std::fs::remove_file(&rom);
        std::env::remove_var("REEMU_HW");
        assert!(got_color, "a chain nunca recebeu frame colorido pela sessão");
    }
}
