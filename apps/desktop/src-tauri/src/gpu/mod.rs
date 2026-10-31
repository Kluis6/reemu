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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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

static SHADER_ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Proporção da imagem pedida pelo usuário. O `libretro.h` deixa o frontend
/// ignorar o `aspect_ratio` do core ("A frontend may ignore this setting").
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AspectMode {
    /// A que o core declara (`retro_game_geometry`, `SET_GEOMETRY`).
    Core,
    /// Fixa (largura / altura).
    Ratio(f32),
    /// A janela inteira, esticando a imagem.
    Fill,
}

impl AspectMode {
    /// `core` | `4:3` | `16:9` | `fill` (coluna `aspect_assignments.aspect`).
    /// O que não reconhece vale como `core`.
    pub fn parse(s: &str) -> Self {
        match s {
            "4:3" => Self::Ratio(4.0 / 3.0),
            "16:9" => Self::Ratio(16.0 / 9.0),
            "fill" => Self::Fill,
            _ => Self::Core,
        }
    }
}

/// A moldura sai quando a imagem é larga (16:10 pra cima) E bem mais larga que
/// a janela da própria moldura (a sobra abaixo). Só o primeiro critério
/// tiraria as molduras feitas pra jogos 16:9, como as de PSP.
const WIDE_ASPECT: f32 = 1.6;
const WIDE_OVER_WINDOW: f32 = 1.15;

/// Retângulo do integer scaling, em metades da saída `(hw, hh)`: altura em
/// múltiplo inteiro da nativa e largura pela proporção de exibição — com a
/// largura `nativa × fator` os cores de pixel não quadrado saíam deformados e
/// o widescreen era ignorado. Se a largura não couber, o fator desce; nunca
/// abaixo de 1.
fn integer_scale_rect(dw: u32, dh: u32, nh: u32, aspect: f32) -> (f32, f32) {
    let (dw_f, nh) = (dw as f32, nh.max(1));
    let mut factor = (dh / nh).max(1);
    while factor > 1 && (nh * factor) as f32 * aspect > dw_f {
        factor -= 1;
    }
    let h = (nh * factor) as f32;
    ((h * aspect / dw_f).min(1.0), (h / dh as f32).min(1.0))
}

/// Quem avisa o app quando o shader falha na GPU no meio do jogo
/// (`shader_failure::report`), registrado no `setup`. É um ponteiro de função,
/// não uma chamada direta, pra que o código da GPU não alcance o runtime do
/// Tauri: o binário dos testes não tem o manifesto do comctl32 v6 que o
/// runtime pede (`TaskDialogIndirect`) e nem abria
/// (`STATUS_ENTRYPOINT_NOT_FOUND`). Sem gancho (testes), só o log.
static SHADER_FAILURE_HOOK: std::sync::OnceLock<fn(&str, String)> = std::sync::OnceLock::new();

/// Registra o gancho de [`SHADER_FAILURE_HOOK`]. Uma vez, no startup.
pub fn set_shader_failure_hook(hook: fn(&str, String)) {
    let _ = SHADER_FAILURE_HOOK.set(hook);
}

/// Avisa o app que a moldura saiu porque a imagem está larga — vai pro sino,
/// nunca por cima do jogo. Ponteiro pelo mesmo motivo do de cima.
static WIDE_BEZEL_HOOK: std::sync::OnceLock<fn()> = std::sync::OnceLock::new();

/// Registra o gancho de [`WIDE_BEZEL_HOOK`]. Uma vez, no startup.
pub fn set_wide_bezel_hook(hook: fn()) {
    let _ = WIDE_BEZEL_HOOK.set(hook);
}

/// Raiz do pacote `slang-shaders` (`<dados>/shaders/slang-shaders`), usada pra
/// resolver os presets `curated:<id>`. Setada uma vez no startup.
pub fn set_shader_root(dir: std::path::PathBuf) {
    let _ = SHADER_ROOT.set(dir);
}

static BUNDLED_SHADER_ROOT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Pasta dos shaders próprios (`bundled_shaders::dir`). Setada uma vez no
/// startup, depois de gravá-los.
pub fn set_bundled_shader_root(dir: std::path::PathBuf) {
    let _ = BUNDLED_SHADER_ROOT.set(dir);
}

/// Caminho absoluto do `.slangp` de um preset curado, se o arquivo existe
/// (o do pacote, ou o próprio gravado no startup).
pub fn curated_slangp(c: &Curated) -> Option<std::path::PathBuf> {
    let root = if c.bundled {
        BUNDLED_SHADER_ROOT.get()?
    } else {
        SHADER_ROOT.get()?
    };
    let p = root.join(c.relpath);
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
    /// A imagem em RGBA8 — o modo canvas manda pro WebView uma vez por jogo
    /// (`decoration_image`) em vez de compor na GPU a cada quadro.
    rgba: Vec<u8>,
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
    /// Anel do interop GL → Vulkan (B2, Windows): as imagens exportadas pro
    /// processo do core. Vivem aqui (a textura é dona da memória) até um
    /// anel novo substituir.
    #[cfg(windows)]
    interop_ring: Vec<win_interop::ExportedImage>,
    /// B5 (Windows): slots amostrados desde o último `flush_interop`, com o
    /// `done`/`free` já postos na fila do wgpu-hal.
    #[cfg(windows)]
    win_staged: Vec<u32>,
    /// C3b: quem escreve no anel é um core Vulkan no filho — antes de
    /// amostrar, o *acquire* de `VK_QUEUE_FAMILY_EXTERNAL` (ver
    /// `stage_win_slot`). `false` = o GL (B5).
    #[cfg(windows)]
    interop_vk_producer: bool,
    /// O *acquire* por slot quando o produtor é Vulkan (C3b).
    #[cfg(windows)]
    interop_acquire: Option<win_interop::AcquireCmds>,
    imported: Vec<Option<(wgpu::Texture, wgpu::TextureView)>>,
    /// Espera na GPU pela fence de fim de render do core GL (no lugar do
    /// `glFinish` dele) — `None` = device sem `VK_KHR_external_semaphore_fd`
    /// (espera pela CPU no fd). Ver `gpu/sync_fd.rs`.
    #[cfg(target_os = "linux")]
    sync_fd: Option<sync_fd::SyncFdImporter>,
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
    /// Buffer reusado do readback de `process()` (testes e preview de shader
    /// — o jogo usa `process_packed`, que escreve direto no `Vec` do IPC).
    readback_scratch: Vec<u8>,
    /// Frame de software convertido pra RGBA8 antes de subir pra GPU —
    /// reusado quadro a quadro (era um `Vec` novo por frame).
    rgba_scratch: Vec<u8>,
    /// Blit linear nível-a-nível pra gerar cadeia de mips de um `.target` de
    /// passe (`mipmap_input`) — wgpu não tem "generate mipmaps" embutido
    /// (ver `generate_mips`). Reusa a `bgl`/shader do `comp` (mesmo layout:
    /// rect uniforme + textura + sampler), só o pipeline é próprio porque o
    /// formato do alvo pode divergir do formato de composição. Um por formato
    /// de alvo de passe (`FMT`, `float_framebuffer` → `Rgba16Float`,
    /// `srgb_framebuffer` → `Rgba8UnormSrgb`, ver `specs.rs`): o WebGPU exige
    /// o formato do color attachment igual ao do pipeline. Só com o de `FMT`,
    /// o mip de um alvo float era erro de validação a cada quadro (presets do
    /// Mega Bezel; jogo sem imagem com o som tocando, 2026-10-29).
    mip_pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    /// Rect fixo `[0,0,1,1]` (quad cheio) — reusado em todo blit de mip.
    mip_rect: wgpu::Buffer,
    /// `(w, h, com_moldura)` da última chamada de `render_to_surface` — pra
    /// `capture_surface_frame` ler de volta a textura certa sem rodar a chain.
    last_surface_out: Option<(u32, u32, bool)>,
    /// A última apresentação na surface falhou (Outdated/Timeout…): o pump
    /// reapresenta mesmo sem quadro novo (`redraw_surface_if_pending`).
    surface_redraw_pending: bool,
    /// `get_current_texture` falhando em sequência (surface `Outdated`/`Lost`) —
    /// diagnóstico da "tela preta" (surface configurada num tamanho que o
    /// compositor não aceita, comum em 4K).
    surface_fail_streak: u32,
    /// Já apresentou ao menos 1 frame na surface nativa? (log de sanidade).
    surface_presented: bool,
    /// Tempo do último `blit_last`: `(get_current_texture, submit + present)`.
    /// O `REEMU_PERF` do pump separa isso do "render" (tarefa A4 do TASKS).
    last_present_timing: Option<(std::time::Duration, std::time::Duration)>,
    /// Período real do monitor, medido pelo `get_current_texture` (A4).
    vblank: VblankEstimator,
    /// Queue family do device adotado (§Beetle) — pro command pool do blit.
    adopted_queue_family: Option<u32>,
    /// Conversor `A1R5G5B5`/packed-16 → RGBA8 pra cores Vulkan que fazem scanout
    /// num formato que o wgpu não amostra (Beetle PSX HW com dither ligado, ou
    /// jogo em modo 16bpp). Lazy: só nasce no 1º frame packed.
    vk_blit: Option<VkBlit>,
    comp: Composite,
    decoration: Option<Decoration>,
    /// Muda a cada `set_decoration` — o WebView compara pra saber quando
    /// buscar a imagem nova da moldura. 0 = nunca houve.
    deco_gen: u32,
    /// Retângulo do jogo dentro da moldura no último `run_chain`, em NDC
    /// `[centro_x, centro_y, meia_largura, meia_altura]`.
    game_rect: [f32; 4],
    /// Zoom da moldura (centro da tela) no último `run_chain`: 1 fora do
    /// integer scaling; perto de 1 com ele (ver `chain.rs`).
    deco_zoom: f32,
    /// Modo canvas: não compõe a moldura na GPU — o readback sai só com o
    /// jogo e o WebView empilha a moldura por cima. Compor aqui mandava o
    /// quadro no tamanho da moldura (1920×1080 = 8 MB) pelo IPC a cada
    /// quadro, o que derrubava o desempenho no Windows (2026-09-25).
    pub split_decoration: bool,
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
    /// Saída HDR pedida (aplicada quando houver surface nativa).
    hdr: HdrSettings,
    /// Proporção pedida pelo usuário (`set_aspect_mode`).
    aspect_mode: AspectMode,
    /// A imagem está larga demais pra moldura atual: ela não é composta.
    deco_suppressed: bool,
    /// Geração da moldura cuja saída já foi avisada (um aviso por moldura).
    deco_notified_gen: u32,
    /// Último erro do wgpu fora de um escopo (ver [`install_gpu_error_handler`])
    /// — o `run_chain` volta pro preset `plain` quando ele aparece.
    gpu_error: Arc<GpuErrorSlot>,
}

/// Erro do wgpu que nenhum `push_error_scope` pegou, guardado pelo handler do
/// device até o próximo quadro.
#[derive(Default)]
struct GpuErrorSlot {
    last: std::sync::Mutex<Option<String>>,
    /// Quantos já chegaram — o log para de repetir depois de alguns (um
    /// erro que se repete a cada quadro encheria o arquivo).
    count: AtomicU32,
}

/// Troca o handler de erro não capturado do device. O padrão do wgpu 30 é
/// fatal: `default_error_handler` faz `panic!("wgpu error: …")`
/// (wgpu-30.0.1/src/backend/wgpu_core.rs). Um erro de validação de um passe
/// de shader no meio do jogo derrubava a thread do vídeo
/// (`reemu-video-pump`) — o core e o áudio seguiam em outras threads, então o
/// jogo "sumia" com o som tocando (relato de usuários, 2026-10-29; no log
/// desta máquina, 2026-10-16 18:14:50, preset `ossc_slot`). Aqui o erro vai
/// pro log com a mensagem e fica guardado pro `run_chain` se recuperar.
/// `Device::on_uncaptured_error` e `set_device_lost_callback`: docs.rs/wgpu
/// 30.0.1, `Device`.
fn install_gpu_error_handler(device: &wgpu::Device) -> Arc<GpuErrorSlot> {
    let slot = Arc::new(GpuErrorSlot::default());
    let s = slot.clone();
    device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
        let n = s.count.fetch_add(1, Ordering::Relaxed);
        if n < 5 {
            log::error!("GPU: erro do wgpu fora de escopo: {e}");
        } else if n == 5 {
            log::error!("GPU: mais erros do wgpu — o log para de repetir");
        }
        *s.last.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.to_string());
    }));
    device.set_device_lost_callback(|reason, msg| {
        log::error!("GPU: device perdido ({reason:?}): {msg}");
    });
    slot
}

/// Alvo de apresentação nativo: a `wgpu::Surface` de uma `wl_subsurface` (ou da
/// janela, em Win/macOS) + o pipeline de blit pro formato dela.
struct SurfaceOut {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    blit_pipeline: wgpu::RenderPipeline,
    /// 32 bytes: o retângulo do jogo (16) + os parâmetros do HDR (16).
    blit_rect: wgpu::Buffer,
    /// Formato SDR escolhido no `attach_surface` (volta pra ele sem HDR).
    sdr_format: wgpu::TextureFormat,
    /// A surface aceita `Rgba16Float` em `ExtendedSrgbLinear` (scRGB).
    hdr_capable: bool,
    /// Configurada em scRGB agora.
    hdr_active: bool,
    /// O que o sistema informa do monitor (`Surface::display_hdr_info`).
    display: wgpu::DisplayHdrInfo,
    /// Branco SDR e pico usados agora, em nits.
    hdr_used: (f32, f32),
}

/// Saída HDR (Configurações › Sistema › Vídeo) — ver
/// `domain::video::VideoConfig`. `0` nos nits = automático.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HdrSettings {
    pub enabled: bool,
    pub paper_white_nits: u32,
    pub highlights: bool,
    pub peak_nits: u32,
}

/// Estado do HDR pra interface.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "HdrStatus"))]
pub struct HdrStatus {
    /// Há uma janela de jogo nativa (o HDR vale só nela).
    pub surface: bool,
    /// A janela aceita scRGB (FP16).
    pub capable: bool,
    /// Saindo em HDR agora.
    pub active: bool,
    /// HDR ligado no sistema (Windows: "Usar HDR"), se o sistema informa.
    pub system_hdr: Option<bool>,
    /// Branco SDR do sistema, pico e brilho sustentado em tela cheia (nits).
    pub sdr_white_nits: Option<f32>,
    pub peak_nits: Option<f32>,
    pub full_frame_nits: Option<f32>,
    /// Valores em uso (automáticos ou escolhidos).
    pub paper_white_used: f32,
    pub peak_used: f32,
}

impl FrameProcessor {
    pub fn new() -> Option<Self> {
        if std::env::var_os("REEMU_NO_GPU").is_some() {
            log::info!("REEMU_NO_GPU: processamento de frame na GPU desligado");
            return None;
        }
        let instance = wgpu::Instance::default();
        let wanted = wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF;
        // Linux: tenta abrir já com `VK_KHR_external_semaphore_fd` (fence
        // do core GL como semáforo); senão, o device de sempre.
        #[cfg(target_os = "linux")]
        let with_sync = sync_fd::create_device(&instance, wanted);
        // Windows: o device já abre com as extensões Win32 do interop
        // GL → Vulkan (`win_interop`; `REEMU_GL_INTEROP=0` desliga).
        #[cfg(windows)]
        let with_sync =
            win_interop::create_device(&instance, wanted).map(|(a, d, q, f)| (a, d, q, f, false));
        #[cfg(not(any(target_os = "linux", windows)))]
        let with_sync: Option<(
            wgpu::Adapter,
            wgpu::Device,
            wgpu::Queue,
            wgpu::Features,
            bool,
        )> = None;
        let (adapter, device, queue, feats, sync_ok) = match with_sync {
            Some(t) => t,
            None => {
                let (a, d, q, f) = video_surface::create_device_with(&instance, None, wanted)?;
                (a, d, q, f, false)
            }
        };
        let interop_ok = feats.contains(wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF);
        log::info!(
            "GPU (etapa 04): {} (interop dma_buf={interop_ok}, sync_file→semáforo={sync_ok})",
            adapter.get_info().name
        );

        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
        let mut fp = Self::assemble(instance, adapter, device, queue, interop_ok)?;
        #[cfg(target_os = "linux")]
        if sync_ok {
            fp.sync_fd = sync_fd::SyncFdImporter::new(&fp.device);
        }
        Some(fp)
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

        let gpu_error = install_gpu_error_handler(&device);
        let comp = build_composite(&device);
        let flip = build_flip(&device);
        let rot_pipeline = rotate_pipeline(&device, &comp.bgl);
        let mip_pipelines = [
            FMT,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ]
        .into_iter()
        .map(|f| (f, mip_blit_pipeline(&device, &comp.bgl, f)))
        .collect();
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
            mip_pipelines,
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
            #[cfg(windows)]
            interop_ring: Vec::new(),
            #[cfg(windows)]
            win_staged: Vec::new(),
            #[cfg(windows)]
            interop_vk_producer: false,
            #[cfg(windows)]
            interop_acquire: None,
            #[cfg(target_os = "linux")]
            sync_fd: None,
            imported: Vec::new(),
            vk_imported: Vec::new(),
            flip_tgt: Vec::new(),
            flip,
            interop_view: None,
            rb: ReadbackRing::default(),
            readback_scratch: Vec::new(),
            rgba_scratch: Vec::new(),
            frame_count: 0,
            last_surface_out: None,
            surface_redraw_pending: false,
            surface_fail_streak: 0,
            surface_presented: false,
            last_present_timing: None,
            vblank: VblankEstimator::default(),
            adopted_queue_family: None,
            vk_blit: None,
            comp,
            rot_pipeline,
            rot_tgt: None,
            rot_view: None,
            decoration: None,
            deco_gen: 0,
            game_rect: [0.0, 0.0, 1.0, 1.0],
            deco_zoom: 1.0,
            split_decoration: false,
            surface: None,
            integer_scaling: false,
            hdr: HdrSettings::default(),
            aspect_mode: AspectMode::Core,
            deco_suppressed: false,
            deco_notified_gen: 0,
            gpu_error,
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
        // com piso na versão com que o wgpu cria as instâncias dele — 1.3 se o
        // loader suporta (`Instance::init` do wgpu-hal 30: "the max Vulkan API
        // version supported by wgpu-hal"), e nunca menos que 1.2.
        //
        // - 1.2: o wgpu-hal 30 chama `vkWaitSemaphores` (timeline, core em 1.2)
        //   no `wait_for_fence`; num device 1.1 esse ponteiro é nulo e o `ash`
        //   faz `panic!("Unable to load wait_semaphores")` no video pump.
        // - 1.3: o wgpu-hal escolhe a versão do SPIR-V pela versão do device
        //   físico (`device_api_version = properties.api_version`; ≥ 1.3 gera
        //   SPIR-V 1.6). Funcionalidade nova do device só vale quando a do
        //   device E a `apiVersion` da instância chegam lá ("Extending Physical
        //   Device Core Functionality", spec Vulkan, cap. Initialization). Com a
        //   instância em 1.2, os shaders 1.6 do wgpu davam
        //   `VUID-VkShaderModuleCreateInfo-pCode-08737` na validação (mupen
        //   com ParaLLEl-RDP, 2026-10-02).
        //
        // NÓS criamos a instância, então o app info antigo do core não limita.
        let loader_version = unsafe { entry.try_enumerate_instance_version() }
            .ok()
            .flatten()
            .unwrap_or(vk::API_VERSION_1_0);
        let wgpu_version = if loader_version >= vk::API_VERSION_1_3 {
            vk::API_VERSION_1_3
        } else {
            vk::API_VERSION_1_2
        };
        let core_version = if neg.get_application_info != 0 {
            let f: unsafe extern "C" fn() -> *const vk::ApplicationInfo<'static> =
                unsafe { std::mem::transmute(neg.get_application_info) };
            let p = unsafe { f() };
            if p.is_null() {
                0
            } else {
                unsafe { (*p).api_version }
            }
        } else {
            0
        };
        let api_version = core_version.max(wgpu_version);

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
        // O core recebe o NOSSO `vkGetInstanceProcAddr`: o `vkCreateDevice`
        // que ele carregar passa pelo hook, que garante as extensões/features
        // do wgpu e registra a lista final (ver `device_hook`).
        device_hook::arm(gipa, &dev_exts, dev_feats);
        let mut ctx = RetroVulkanContext::default();
        let ok = unsafe {
            create_device(
                &mut ctx,
                instance.handle(),
                gpu,
                vk::SurfaceKHR::null(),
                device_hook::get_instance_proc_addr,
                dev_exts_c.as_ptr(),
                dev_exts_c.len() as u32,
                std::ptr::null(),
                0,
                &dev_feats,
            )
        };
        let enabled_exts = device_hook::disarm();
        if !ok || ctx.device.is_null() {
            cleanup(&instance);
            return Err("o create_device do core devolveu false".into());
        }
        // O que foi LIGADO no device (o flycast ignora o
        // `required_device_extensions` e escolhe as dele; o hook somou as do
        // wgpu). Sem o hook ter visto o `vkCreateDevice` (core que usou outro
        // loader), cai no que foi pedido — o caso do Beetle, que honra a lista.
        let dev_exts = match enabled_exts {
            Some(list) => {
                log::info!("VkDevice do core: {} extensões ligadas", list.len());
                list
            }
            None => dev_exts,
        };
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

    /// Proporção de exibição imposta pela moldura (`w/h` da imagem), se houver.
    pub fn decoration_aspect(&self) -> Option<f32> {
        self.decoration
            .as_ref()
            .filter(|_| !self.deco_suppressed)
            .map(|d| d.w as f32 / d.h.max(1) as f32)
    }

    /// Proporção pedida pelo usuário. Vale no próximo quadro.
    pub fn set_aspect_mode(&mut self, mode: AspectMode) {
        self.aspect_mode = mode;
    }

    /// A proporção de exibição do quadro: a do core, ou a pedida pelo
    /// usuário. Na tela cheia, a da saída (surface; no canvas, o viewport).
    pub(crate) fn effective_aspect(&self, core: f32) -> f32 {
        match self.aspect_mode {
            AspectMode::Core => core,
            AspectMode::Ratio(r) => r,
            AspectMode::Fill => {
                let (w, h) = self
                    .surface
                    .as_ref()
                    .map(|s| (s.config.width, s.config.height))
                    .unwrap_or(self.viewport);
                if w > 0 && h > 0 {
                    w as f32 / h as f32
                } else {
                    core
                }
            }
        }
    }

    /// Decide se a moldura entra neste quadro: fora, se a imagem é larga demais
    /// pra janela dela (ver [`WIDE_ASPECT`]) — espremeria a cena. Avisa uma vez
    /// por moldura, pelo sino.
    pub(crate) fn update_deco_suppressed(&mut self, frame: &Frame) {
        let ar = self.effective_aspect(frame.metadata.aspect_ratio);
        let wide = self.decoration.as_ref().is_some_and(|d| {
            let window = match d.vp {
                Some(v) => v.w / v.h.max(1.0),
                None => d.w as f32 / d.h.max(1) as f32,
            };
            ar >= WIDE_ASPECT && ar > window * WIDE_OVER_WINDOW
        });
        if wide && self.deco_notified_gen != self.deco_gen {
            self.deco_notified_gen = self.deco_gen;
            if let Some(hook) = WIDE_BEZEL_HOOK.get() {
                hook();
            }
        }
        self.deco_suppressed = wide;
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
        self.deco_gen = self.deco_gen.wrapping_add(1).max(1);
        self.decoration = Some(Decoration {
            view,
            rgba,
            w,
            h,
            vp,
        });
    }

    /// Placa de vídeo em uso (Configurações › Sistema).
    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }

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
        // Erro de validação do WebGPU ao montar os pipelines (ex.: o fragment
        // lê uma entrada que o vertex não escreve — o Vulkan aceita, o WebGPU
        // não) vira `Err` aqui, e o preset atual continua. Sem o escopo ele ia
        // pro handler de erro não capturado do wgpu, que entra em pânico.
        // `push_error_scope`/`pop` do wgpu 30 (docs.rs/wgpu, `Device`); o
        // `pop` resolve na hora nos backends nativos.
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let r = realize(&self.device, &self.queue, built);
        if let Some(e) = pollster::block_on(scope.pop()) {
            log::warn!("preset de shader '{name}' recusado pelo WebGPU: {e}");
            return Err(format!("shader: {e}"));
        }
        let r = r.ok_or("shader: falha ao criar os pipelines")?;
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

mod chain;
mod device_hook;
mod input;
mod interop;
mod known_issues;
mod pipelines;
mod presets;
mod process;
mod readback;
mod specs;
mod surface;
#[cfg(target_os = "linux")]
mod sync_fd;
#[cfg(test)]
mod tests;
mod textures;
mod vblank;
mod vk_blit;
#[cfg(windows)]
pub(crate) mod win_interop;

pub use known_issues::{shader_issue, ShaderIssue};
use pipelines::*;
pub use presets::*;
use readback::*;
use specs::*;
use textures::*;
use vblank::*;
use vk_blit::*;
