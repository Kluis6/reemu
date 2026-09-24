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
use domain::frame_source::to_rgba8_into;
use domain::frame_source::{Frame, FrameOrigin};
use shader_slang::{
    Scale, TextureBind, TextureSemantic, UniformFieldKind, UniformLayout, WrapMode,
};

/// `close(2)` cru — pro caminho de erro do import dma_buf (o fd ainda é nosso).
unsafe fn close_raw_fd(fd: i32) {
    extern "C" {
        fn close(fd: i32) -> i32;
    }
    close(fd);
}

const FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const ROW_ALIGN: u32 = 256;
// Teto de segurança contra shader chain que explode de tamanho passe a
// passe. 8_000_000 (antigo) já é MENOR que 4K puro (3840×2160=8_294_400) —
// qualquer shader cujo passe final escale pro `viewport` (comum em CRT/
// scanline, ex.: crt-guest-advanced) faz `run_chain` devolver `None` calado
// nessa checagem em qualquer tela 4K, virando tela preta sem log nenhum.
// 20_000_000 cobre 4K com folga (~2.4x) e a maior parte de 5K, mantendo a
// checagem como proteção real só contra upscales patológicos.
const MAX_OUT_PIXELS: u32 = 20_000_000;

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
    /// `mipmap_input<N>` do `.slangp` neste passe — o passe ANTERIOR precisa
    /// gerar a cadeia de mips na saída dele pra este aqui amostrar (mesma
    /// convenção do RetroArch: `next_pass->mipmap` decide o `max_levels` do
    /// passe corrente — ver `ensure_target`).
    mipmap_input: bool,
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
    /// `<name>_mipmap` do `.slangp` — gera a cadeia de mips no upload (feito
    /// na CPU em `realize`, já que o LUT só carrega 1× no load do preset).
    mipmap: bool,
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
    mipmap_input: bool,
    /// Quantos níveis de mip o `.target` atual tem (1 = sem cadeia) — decidido
    /// em `ensure_target` por `mipmap_input` do PRÓXIMO passe.
    target_mip_levels: u32,
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
    unpad_rows_into(&mut out, src, w, h, padded);
    out
}

/// Como `unpad_rows`, mas escreve em `dst` reaproveitando a alocação (só
/// redimensiona se o tamanho mudou — resolução do jogo é estável quadro a
/// quadro). Usado no readback por quadro (`process`, hot path a 60fps); sem
/// isto, era um `vec![0u8; ...]` novo (alocação + zero-fill) a cada frame só
/// pra ser copiado de novo logo em seguida no `pack_frame` do IPC.
fn unpad_rows_into(dst: &mut Vec<u8>, src: &[u8], w: u32, h: u32, padded: u32) {
    let row = (w * 4) as usize;
    let need = row * h as usize;
    if dst.len() != need {
        dst.resize(need, 0);
    }
    for y in 0..h as usize {
        dst[y * row..(y + 1) * row].copy_from_slice(&src[y * padded as usize..][..row]);
    }
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
) -> Option<(
    Vec<&'static std::ffi::CStr>,
    ash::vk::PhysicalDeviceFeatures,
)> {
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
    /// Buffer reusado do readback de `process()` (só nos testes — o app usa
    /// `process_packed`, que escreve direto no `Vec` da resposta IPC).
    #[cfg(test)]
    readback_scratch: Vec<u8>,
    /// Frame de software convertido pra RGBA8 antes de subir pra GPU —
    /// reusado quadro a quadro (era um `Vec` novo por frame).
    rgba_scratch: Vec<u8>,
    /// Blit linear nível-a-nível pra gerar cadeia de mips de um `.target` de
    /// passe (`mipmap_input`) — wgpu não tem "generate mipmaps" embutido
    /// (ver `generate_mips`). Reusa a `bgl`/shader do `comp` (mesmo layout:
    /// rect uniforme + textura + sampler), só o pipeline é próprio porque o
    /// formato do alvo pode divergir do formato de composição.
    mip_pipeline: wgpu::RenderPipeline,
    /// Rect fixo `[0,0,1,1]` (quad cheio) — reusado em todo blit de mip.
    mip_rect: wgpu::Buffer,
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
    /// Integer scaling (backlog): trava o retângulo final do jogo (caminho
    /// surface nativa, `render_to_surface`) num múltiplo INTEIRO da
    /// resolução nativa do core, em vez do letterbox fracionário livre. Não
    /// se aplica quando a moldura define um viewport explícito (o bezel foi
    /// desenhado pra aquele retângulo exato). `false` por padrão — muda ao
    /// vivo via `set_integer_scaling` (chamado por `update_video_config`).
    integer_scaling: bool,
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
        let mip_pipeline = blit_pipeline(&device, &comp.bgl, FMT);
        let mip_rect = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mip rect"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&mip_rect, 0, f32s_bytes(&[0.0, 0.0, 1.0, 1.0]));

        Some(Self {
            sampler_nearest: mk_sampler(wgpu::FilterMode::Nearest),
            sampler_linear: mk_sampler(wgpu::FilterMode::Linear),
            sampler_cache: HashMap::new(),
            instance,
            adapter,
            device,
            queue,
            quad,
            mip_pipeline,
            mip_rect,
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
            #[cfg(test)]
            readback_scratch: Vec::new(),
            rgba_scratch: Vec::new(),
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
            integer_scaling: false,
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
        let inst_exts = wgpu::hal::vulkan::Instance::desired_extensions(&entry, api_version, flags)
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

    /// Liga/desliga integer scaling ao vivo (`update_video_config`) — só
    /// precisa marcar a flag, o próximo `render_to_surface` já lê o valor
    /// novo (sem precisar recarregar o jogo).
    pub fn set_integer_scaling(&mut self, on: bool) {
        self.integer_scaling = on;
    }

    /// Pra carregar o valor atual num `FrameProcessor` NOVO que substitui
    /// este (§Beetle — troca de device Vulkan em runtime, ver `lib.rs`) —
    /// sem isto, o FP novo nasceria sempre com integer scaling desligado.
    pub fn integer_scaling(&self) -> bool {
        self.integer_scaling
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
            .or(Some(frame.metadata.aspect_ratio).filter(|a| *a > 0.0 && !quarter))
            .unwrap_or(out_w as f32 / out_h.max(1) as f32);

        let s = self.surface.as_ref().unwrap();
        let (dw, dh) = (s.config.width.max(1), s.config.height.max(1));
        let ar_dst = dw as f32 / dh as f32;
        // Integer scaling (backlog): só se aplica ao pixel CRU do core, sem
        // moldura (`!use_comp`) — uma moldura/bezel é foto, não pixel art, e
        // já foi desenhada pro retângulo dela (`decoration_aspect()` acima).
        // `native_width/height` não giram sozinhos com `SET_ROTATION`; troca
        // W↔H no giro de 90/270° igual ao `ar_src` acima.
        let (hw, hh) = if self.integer_scaling && !use_comp {
            let (nw, nh) = if quarter {
                (
                    frame.metadata.native_height.max(1),
                    frame.metadata.native_width.max(1),
                )
            } else {
                (
                    frame.metadata.native_width.max(1),
                    frame.metadata.native_height.max(1),
                )
            };
            let factor = (dw / nw).min(dh / nh).max(1);
            let (out_w, out_h) = ((nw * factor) as f32, (nh * factor) as f32);
            (out_w / dw as f32, out_h / dh as f32)
        } else if ar_src > ar_dst {
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
        } else if let Some((t, _, _, _)) = self.rot_tgt.as_ref().filter(|_| self.rot_view.is_some())
        {
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
}

impl FrameProcessor {
    /// Roda a chain e lê o resultado de volta pra CPU (RGBA8, sem cabeçalho).
    /// `None` se não há frame novo pronto ainda (readback com 1 quadro de
    /// atraso — ver o comentário de `rb`). O `&[u8]` é emprestado de
    /// `self.readback_scratch`. Só os testes usam; o canvas usa
    /// `process_packed`.
    #[cfg(test)]
    pub fn process(&mut self, frame: &Frame) -> Option<(u32, u32, &[u8])> {
        let (slot, w, h, padded) = self.readback_frame(frame)?;
        {
            let mapped = self.rb.slots[slot]
                .as_ref()?
                .buf
                .slice(..)
                .get_mapped_range()
                .ok()?;
            unpad_rows_into(&mut self.readback_scratch, &mapped, w, h, padded);
        }
        self.release_readback(slot);
        Some((w, h, self.readback_scratch.as_slice()))
    }

    /// Igual ao `process`, mas já no formato do `poll_frame`: `[w u32 LE]
    /// [h u32 LE][RGBA…]` num `Vec` só, pronto pra virar a resposta IPC. O
    /// `process` + montar o cabeçalho copiava o frame inteiro de novo
    /// (8 MB por frame em 1080p); aqui o readback escreve direto no destino.
    pub fn process_packed(&mut self, frame: &Frame) -> Option<Vec<u8>> {
        let (slot, w, h, padded) = self.readback_frame(frame)?;
        let out = {
            let mapped = self.rb.slots[slot]
                .as_ref()?
                .buf
                .slice(..)
                .get_mapped_range()
                .ok()?;
            let row = (w * 4) as usize;
            let mut out = Vec::with_capacity(8 + row * h as usize);
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            for y in 0..h as usize {
                out.extend_from_slice(&mapped[y * padded as usize..][..row]);
            }
            out
        };
        self.release_readback(slot);
        Some(out)
    }

    /// Roda a chain, pede o readback deste frame e deixa MAPEADO o slot do
    /// frame anterior (pipeline de 1 quadro — ver `rb`). Devolve
    /// `(slot, w, h, bytes_por_linha_com_padding)`; o chamador copia do
    /// mapeamento e chama `release_readback(slot)`.
    fn readback_frame(&mut self, frame: &Frame) -> Option<(usize, u32, u32, u32)> {
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
            } else if let Some((t, _, _, _)) =
                self.rot_tgt.as_ref().filter(|_| self.rot_view.is_some())
            {
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
        Some((read_i, rs.w, rs.h, rs.padded))
    }

    fn release_readback(&mut self, slot: usize) {
        if let Some(rs) = self.rb.slots[slot].as_mut() {
            rs.buf.unmap();
            rs.inflight = false;
        }
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
}

mod chain;
mod input;
mod pipelines;
mod specs;
#[cfg(test)]
mod tests;
mod textures;
mod vk_blit;

use pipelines::*;
use specs::*;
use textures::*;
use vk_blit::*;
