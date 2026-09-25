//! Contexto OpenGL offscreen pra cores libretro que pedem HW render
//! (`RETRO_ENVIRONMENT_SET_HW_RENDER`, context type GL/GLES).
//!
//! O core renderiza num FBO nosso; a camada global devolve o id desse FBO em
//! `get_current_framebuffer` e resolve símbolos GL em `get_proc_address`. O
//! frame renderizado sai por um de dois caminhos:
//!   - **interop zero-cópia** (slice 2): o color attachment é uma textura GL
//!     respaldada por memória Vulkan importada — o wgpu samplia direto.
//!   - **fallback `read_pixels`**: roundtrip de CPU pra um buffer RGBA8, que
//!     entra no pipeline como `SoftwareRawBuffer`.
//!
//! Afinidade de thread: tudo aqui roda na thread do core (`reemu-core-loop`) —
//! `create`, `make_current` e `read_pixels` sempre nela. Nunca compartilhado.

use glow::HasContext as _;
#[cfg(unix)]
use khronos_egl as egl;
use std::os::raw::{c_char, c_void};
#[cfg(unix)]
use std::sync::OnceLock;

#[cfg(unix)]
use crate::dmabuf::{DmabufAllocator, DmabufPlane, SharedBuffer};
use crate::sys;

/// `EGL_PLATFORM_SURFACELESS_MESA` — display headless sem servidor (CI, Mesa).
#[cfg(unix)]
const PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;

// --- EGL_EXT_image_dma_buf_import(_modifiers) ---
// Interop zero-cópia GL↔Vulkan via dma_buf: conceito Linux/DRM-only (o
// `dmabuf.rs` que aloca os buffers GBM já é `#[cfg(unix)]` em lib.rs). Sem
// equivalente Windows — lá o caminho GL sempre cai no readback via
// `glReadPixels` (ver `try_enable_interop`/`bind_write_slot`/
// `finish_write_slot` abaixo).
#[cfg(unix)]
const EGL_LINUX_DMA_BUF_EXT: egl::Enum = 0x3270;
#[cfg(unix)]
const EGL_LINUX_DRM_FOURCC_EXT: egl::Attrib = 0x3271;
#[cfg(unix)]
const EGL_DMA_BUF_PLANE0_FD_EXT: egl::Attrib = 0x3272;
#[cfg(unix)]
const EGL_DMA_BUF_PLANE0_OFFSET_EXT: egl::Attrib = 0x3273;
#[cfg(unix)]
const EGL_DMA_BUF_PLANE0_PITCH_EXT: egl::Attrib = 0x3274;
#[cfg(unix)]
const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: egl::Attrib = 0x3443;
#[cfg(unix)]
const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: egl::Attrib = 0x3444;
#[cfg(unix)]
const EGL_WIDTH: egl::Attrib = 0x3057;
#[cfg(unix)]
const EGL_HEIGHT: egl::Attrib = 0x3056;
#[cfg(unix)]
const DRM_FORMAT_MOD_INVALID: u64 = (1 << 56) - 1;
/// Quantos alvos no ring (core escreve N, wgpu lê N-1 no mesmo frame).
#[cfg(unix)]
const RING: usize = 2;

#[cfg(unix)]
type EglInstance = egl::DynamicInstance<egl::EGL1_5>;
#[cfg(unix)]
static EGL: OnceLock<EglInstance> = OnceLock::new();

/// Instância EGL do processo (carrega `libEGL` em runtime na 1ª chamada).
#[cfg(unix)]
fn egl() -> Result<&'static EglInstance, String> {
    if let Some(e) = EGL.get() {
        return Ok(e);
    }
    let inst = unsafe { EglInstance::load_required() }
        .map_err(|e| format!("carregar libEGL (cores GL ficam indisponíveis): {e}"))?;
    let _ = EGL.set(inst);
    EGL.get().ok_or_else(|| "EGL OnceLock".into())
}

/// Config pedida pelo core (subconjunto de `retro_hw_render_callback`).
#[derive(Clone, Copy)]
pub struct GlConfig {
    /// `RETRO_HW_CONTEXT_*`.
    pub context_type: std::os::raw::c_uint,
    pub version_major: u32,
    pub version_minor: u32,
    pub depth: bool,
    pub stencil: bool,
    /// Core renderiza com origem bottom-left (GL nativo) → `read_pixels` flipa.
    /// `false` = core já entrega top-left, sem flip.
    pub bottom_left_origin: bool,
}

/// Como o produtor GL espera o render terminar antes de entregar o `dma_buf`
/// pro consumidor (wgpu/Vulkan). `REEMU_GL_SYNC`:
/// - `finish` (default) — `glFinish`: stall de pipeline inteiro, sempre seguro.
/// - `fence` — `glFlush` + `glClientWaitSync` num fence do fim do frame: espera
///   só até o render do core, spec-correto (`GL_ARB_sync`).
/// - `flush` — só `glFlush`, confia no sync implícito do `dma_buf` (kernel
///   fencing na reservation object). Mais rápido; pode tearar em driver que
///   não faz implicit sync.
#[derive(Clone, Copy, PartialEq)]
enum SyncMode {
    Finish,
    Fence,
    Flush,
}

fn sync_mode() -> SyncMode {
    match std::env::var("REEMU_GL_SYNC").ok().as_deref() {
        Some("fence") => SyncMode::Fence,
        Some("flush") => SyncMode::Flush,
        _ => SyncMode::Finish,
    }
}

pub struct GlContext {
    /// Contexto da plataforma: EGL no Linux, WGL no Windows.
    plat: PlatCtx,
    gl: glow::Context,
    fbo: glow::Framebuffer,
    color: glow::Texture,
    depth_rbo: Option<glow::Renderbuffer>,
    max_w: u32,
    max_h: u32,
    /// `read_pixels` inverte as linhas (core bottom-left → canvas top-left).
    flip: bool,
    /// Ring de alvos `dma_buf` compartilhados com o wgpu. `None` = readback CPU.
    /// Linux/DRM-only — no Windows o caminho GL sempre usa o readback
    /// (`try_enable_interop`/`interop_active`/`bind_write_slot`/
    /// `finish_write_slot` têm um braço `#[cfg(windows)]` que nunca ativa isso).
    #[cfg(unix)]
    interop: Option<InteropRing>,
    // Só lido em `finish_write_slot` (caminho de interop, Unix).
    #[cfg_attr(windows, allow(dead_code))]
    sync: SyncMode,
}

/// Um alvo compartilhado: BO do GBM + `EGLImage` + textura GL respaldada por ele.
#[cfg(unix)]
struct InteropSlot {
    buffer: SharedBuffer,
    image: egl::Image,
    tex: glow::Texture,
    /// O plano (fd) já foi entregue pro importador wgpu?
    handed: bool,
}

#[cfg(unix)]
struct InteropRing {
    _alloc: DmabufAllocator,
    slots: Vec<InteropSlot>,
    write: usize,
}

// SAFETY: criado e usado exclusivamente na thread do core. O `glow::Context` e
// os handles EGL nunca são tocados de outra thread (o `core_loop` guarda o
// `DesktopCore` numa var local da thread e o dropa na mesma thread).
unsafe impl Send for GlContext {}

impl GlContext {
    pub fn create(cfg: &GlConfig, max_w: u32, max_h: u32) -> Result<Self, String> {
        let (max_w, max_h) = (max_w.max(1), max_h.max(1));
        let (plat, mut gl) = create_platform(cfg)?;

        if std::env::var_os("REEMU_GL_DEBUG").is_some() {
            unsafe { enable_gl_debug(&mut gl) };
        }

        let (fbo, color, depth_rbo) =
            unsafe { build_fbo(&gl, max_w, max_h, cfg.depth || cfg.stencil)? };

        let sync = sync_mode();
        if sync != SyncMode::Finish {
            log::info!(
                "HW render GL: sync mode {:?}",
                match sync {
                    SyncMode::Fence => "fence (glClientWaitSync)",
                    SyncMode::Flush => "flush (implicit dma_buf sync)",
                    SyncMode::Finish => unreachable!(),
                }
            );
        }

        Ok(Self {
            plat,
            gl,
            fbo,
            color,
            depth_rbo,
            max_w,
            max_h,
            flip: cfg.bottom_left_origin,
            #[cfg(unix)]
            interop: None,
            sync,
        })
    }

    /// Id do FBO que o core deve renderizar (`get_current_framebuffer`).
    pub fn fbo(&self) -> u32 {
        self.fbo.0.get()
    }

    pub fn make_current(&self) -> Result<(), String> {
        self.plat.make_current()
    }

    /// Lê `w×h` do FBO como RGBA8 apertado, já flipado pra origem top-left.
    /// Fallback quando o interop não está ativo.
    pub fn read_pixels(&self, w: u32, h: u32) -> Vec<u8> {
        let (w, h) = (w.min(self.max_w).max(1), h.min(self.max_h).max(1));
        let mut buf = vec![0u8; (w * h * 4) as usize];
        unsafe {
            self.gl
                .bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.fbo));
            self.gl.read_buffer(glow::COLOR_ATTACHMENT0);
            self.gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
            self.gl.read_pixels(
                0,
                0,
                w as i32,
                h as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut buf)),
            );
        }
        if self.flip {
            flip_rows_in_place(&mut buf, w, h);
        }
        buf
    }

    pub fn finish(&self) {
        unsafe { self.gl.finish() };
    }

    #[cfg(unix)]
    pub fn interop_active(&self) -> bool {
        self.interop.is_some()
    }

    /// No Windows não existe interop `dma_buf` (GBM/DRM são conceitos Linux) —
    /// o caminho GL sempre usa o readback via `read_pixels`.
    #[cfg(windows)]
    pub fn interop_active(&self) -> bool {
        false
    }

    /// Tenta montar o ring de alvos `dma_buf` (GBM + EGLImage). `false` → segue
    /// no readback de CPU (qualquer falha é best-effort, nunca fatal).
    #[cfg(unix)]
    pub fn try_enable_interop(&mut self) -> bool {
        match self.build_interop() {
            Ok(ring) => {
                log::info!(
                    "interop dma_buf ativo ({RING} alvos {}x{})",
                    self.max_w,
                    self.max_h
                );
                self.interop = Some(ring);
                true
            }
            Err(e) => {
                log::warn!("interop dma_buf indisponível ({e}) — usando readback");
                false
            }
        }
    }

    /// Sem `dma_buf` no Windows — sempre cai no readback via `read_pixels`.
    /// `REEMU_GL_INTEROP=1` (opt-in Linux) não tem efeito aqui.
    #[cfg(windows)]
    pub fn try_enable_interop(&mut self) -> bool {
        log::warn!("interop dma_buf indisponível (sem suporte no Windows) — usando readback");
        false
    }

    #[cfg(unix)]
    fn build_interop(&self) -> Result<InteropRing, String> {
        let egl = egl()?;
        let exts = egl
            .query_string(Some(self.plat.display), egl::EXTENSIONS)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !exts.contains("EGL_EXT_image_dma_buf_import") {
            return Err("sem EGL_EXT_image_dma_buf_import".into());
        }
        let with_mod = exts.contains("EGL_EXT_image_dma_buf_import_modifiers");

        let target: unsafe extern "system" fn(u32, *const c_void) = unsafe {
            let p = egl
                .get_proc_address("glEGLImageTargetTexture2DOES")
                .ok_or("sem glEGLImageTargetTexture2DOES")?;
            std::mem::transmute(p)
        };

        let alloc = DmabufAllocator::open()?;
        let mut slots = Vec::with_capacity(RING);
        for _ in 0..RING {
            let buffer = alloc.alloc(self.max_w, self.max_h)?;
            let plane = buffer.plane()?;
            let image = self.make_egl_image(egl, &plane, with_mod)?;
            let tex = unsafe {
                let t = self
                    .gl
                    .create_texture()
                    .map_err(|e| format!("glGenTextures: {e}"))?;
                self.gl.bind_texture(glow::TEXTURE_2D, Some(t));
                target(glow::TEXTURE_2D, image.as_ptr());
                self.gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR as i32,
                );
                self.gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::LINEAR as i32,
                );
                t
            };
            slots.push(InteropSlot {
                buffer,
                image,
                tex,
                handed: false,
            });
        }
        Ok(InteropRing {
            _alloc: alloc,
            slots,
            write: 0,
        })
    }

    #[cfg(unix)]
    fn make_egl_image(
        &self,
        egl: &EglInstance,
        plane: &DmabufPlane,
        with_mod: bool,
    ) -> Result<egl::Image, String> {
        use std::os::fd::IntoRawFd as _;
        let fd = plane
            .fd
            .try_clone()
            .map_err(|e| format!("dup fd: {e}"))?
            .into_raw_fd();
        let mut attrs: Vec<egl::Attrib> = vec![
            EGL_WIDTH,
            plane.width as egl::Attrib,
            EGL_HEIGHT,
            plane.height as egl::Attrib,
            EGL_LINUX_DRM_FOURCC_EXT,
            plane.fourcc as egl::Attrib,
            EGL_DMA_BUF_PLANE0_FD_EXT,
            fd as egl::Attrib,
            EGL_DMA_BUF_PLANE0_OFFSET_EXT,
            plane.offset as egl::Attrib,
            EGL_DMA_BUF_PLANE0_PITCH_EXT,
            plane.stride as egl::Attrib,
        ];
        if with_mod && plane.modifier != DRM_FORMAT_MOD_INVALID {
            attrs.extend_from_slice(&[
                EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
                (plane.modifier & 0xFFFF_FFFF) as egl::Attrib,
                EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
                (plane.modifier >> 32) as egl::Attrib,
            ]);
        }
        attrs.push(egl::ATTRIB_NONE);
        // SAFETY: ctx = NO_CONTEXT, buffer = NULL — o contrato do dma_buf import.
        let img = unsafe {
            egl.create_image(
                self.plat.display,
                egl::Context::from_ptr(egl::NO_CONTEXT),
                EGL_LINUX_DMA_BUF_EXT,
                egl::ClientBuffer::from_ptr(std::ptr::null_mut()),
                &attrs,
            )
        };
        img.map_err(|e| {
            // SAFETY: o EGL não assumiu o fd (create_image falhou).
            unsafe { libc_close(fd) };
            format!("eglCreateImage(dma_buf): {e}")
        })
    }

    /// Antes do `retro_run`: aponta o FBO pro slot de escrita atual.
    #[cfg(unix)]
    pub fn bind_write_slot(&self) {
        let Some(ring) = &self.interop else { return };
        let slot = &ring.slots[ring.write];
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(slot.tex),
                0,
            );
        }
    }

    /// Sem interop no Windows — o FBO já aponta pra `self.color` (montado em
    /// `build_fbo`) e o frame sai por `read_pixels`. No-op, chamado todo frame.
    #[cfg(windows)]
    pub fn bind_write_slot(&self) {}

    /// O core renderiza com origem bottom-left → o consumidor (wgpu) inverte Y.
    pub fn flip_y(&self) -> bool {
        self.flip
    }

    /// Depois do `retro_run`: garante que o render do core terminou antes de
    /// entregar o `dma_buf`, e devolve o slot escrito + o plano (só na 1ª vez de
    /// cada slot). Modo de sync por `REEMU_GL_SYNC` (ver [`SyncMode`]).
    #[cfg(unix)]
    pub fn finish_write_slot(&mut self) -> Option<(u32, Option<DmabufPlane>)> {
        let sync = self.sync;
        let ring = self.interop.as_mut()?;
        unsafe {
            match sync {
                SyncMode::Finish => self.gl.finish(),
                SyncMode::Flush => self.gl.flush(),
                SyncMode::Fence => {
                    self.gl.flush();
                    if let Ok(f) = self.gl.fence_sync(glow::SYNC_GPU_COMMANDS_COMPLETE, 0) {
                        // 50 ms de teto — se estourar, entrega mesmo assim (pior
                        // caso: 1 frame com tearing, melhor que travar o vídeo).
                        self.gl
                            .client_wait_sync(f, glow::SYNC_FLUSH_COMMANDS_BIT, 50_000_000);
                        self.gl.delete_sync(f);
                    } else {
                        self.gl.finish();
                    }
                }
            }
        }
        let idx = ring.write;
        let slot = &mut ring.slots[idx];
        let plane = if slot.handed {
            None
        } else {
            match slot.buffer.plane() {
                Ok(p) => {
                    slot.handed = true;
                    Some(p)
                }
                Err(e) => {
                    log::warn!("plano do slot {idx} indisponível: {e}");
                    None
                }
            }
        };
        ring.write = (ring.write + 1) % RING;
        Some((idx as u32, plane))
    }

    /// Sem `dma_buf` no Windows — sempre `None` (o chamador cai no readback
    /// via `interop_active() == false`, este método só é alcançado se algum
    /// dia esse invariante mudar). O tipo do plano é `()` porque não existe
    /// um equivalente Windows do plano `dma_buf` pra carregar aqui.
    #[cfg(windows)]
    pub fn finish_write_slot(&mut self) -> Option<(u32, Option<()>)> {
        None
    }
}

/// `close(2)` sem puxar a crate `libc` — só pro caminho de erro do EGLImage.
#[cfg(unix)]
unsafe fn libc_close(fd: i32) {
    extern "C" {
        fn close(fd: i32) -> i32;
    }
    close(fd);
}

/// Handle de um frame de HW render entregue via `dma_buf`. O `DesktopCore`
/// devolve isso em `FrameOrigin::HardwareTexture`; o `poll_frame` (lado wgpu)
/// importa o plano uma vez por slot e depois referencia por índice.
#[cfg(unix)]
pub struct GlInteropHandle {
    slot: u32,
    flip_y: bool,
    plane: std::sync::Mutex<Option<DmabufPlane>>,
}

#[cfg(unix)]
impl GlInteropHandle {
    pub fn new(slot: u32, flip_y: bool, plane: Option<DmabufPlane>) -> Self {
        Self {
            slot,
            flip_y,
            plane: std::sync::Mutex::new(plane),
        }
    }
}

#[cfg(unix)]
impl domain::frame_source::GpuTextureHandle for GlInteropHandle {
    fn slot(&self) -> u32 {
        self.slot
    }

    fn flip_y(&self) -> bool {
        self.flip_y
    }

    fn take_plane(&self) -> Option<domain::frame_source::DmabufPlaneInfo> {
        use std::os::fd::IntoRawFd as _;
        let p = self
            .plane
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()?;
        Some(domain::frame_source::DmabufPlaneInfo {
            fd: p.fd.into_raw_fd(),
            width: p.width,
            height: p.height,
            stride: p.stride,
            offset: p.offset,
            modifier: p.modifier,
            fourcc: p.fourcc,
        })
    }
}

/// Windows não tem `dma_buf` — este handle nunca é de fato produzido (só
/// existe pra `DesktopCore::next_hw_frame` compilar em todas as plataformas;
/// `GlContext::interop_active()` é sempre `false` aqui, então o branch que
/// construiria isto nunca roda). `take_plane` devolve `None` sempre.
#[cfg(windows)]
pub struct GlInteropHandle {
    slot: u32,
    flip_y: bool,
}

#[cfg(windows)]
impl GlInteropHandle {
    pub fn new(slot: u32, flip_y: bool, _plane: Option<()>) -> Self {
        Self { slot, flip_y }
    }
}

#[cfg(windows)]
impl domain::frame_source::GpuTextureHandle for GlInteropHandle {
    fn slot(&self) -> u32 {
        self.slot
    }

    fn flip_y(&self) -> bool {
        self.flip_y
    }

    fn take_plane(&self) -> Option<domain::frame_source::DmabufPlaneInfo> {
        None
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(ring) = self.interop.take() {
            let egl = egl();
            for slot in ring.slots {
                unsafe { self.gl.delete_texture(slot.tex) };
                if let Ok(e) = egl {
                    let _ = e.destroy_image(self.plat.display, slot.image);
                }
                drop(slot.buffer);
            }
        }
        unsafe {
            if let Some(rb) = self.depth_rbo.take() {
                self.gl.delete_renderbuffer(rb);
            }
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_texture(self.color);
        }
        self.plat.destroy();
    }
}

#[cfg(unix)]
impl PlatCtx {
    fn make_current(&self) -> Result<(), String> {
        egl()?
            .make_current(self.display, self.surface, self.surface, Some(self.context))
            .map_err(|e| format!("eglMakeCurrent: {e}"))
    }

    fn destroy(&mut self) {
        if let Ok(egl) = egl() {
            let _ = egl.make_current(self.display, None, None, None);
            let _ = egl.destroy_context(self.display, self.context);
            if let Some(s) = self.surface.take() {
                let _ = egl.destroy_surface(self.display, s);
            }
            // NÃO chamar `eglTerminate`: `eglGetDisplay(EGL_DEFAULT_DISPLAY)`
            // devolve um handle COMPARTILHADO no processo — o WebKitGTK também
            // renderiza via EGL. Terminar aqui invalidava o display dele e o app
            // fechava sozinho quando a webview repintava depois do unload de um
            // core GL (N64). Só destruímos o que é nosso; o display fica vivo e
            // o próximo core reusa (`eglInitialize` é idempotente).
        }
    }
}

/// `get_proc_address` que a camada global entrega pro core.
///
/// # Safety
/// `sym` deve ser um ponteiro C válido pra string NUL-terminada (ou nulo).
#[cfg(unix)]
pub unsafe fn resolve_proc(sym: *const c_char) -> sys::retro_proc_address_t {
    if sym.is_null() {
        return None;
    }
    let name = std::ffi::CStr::from_ptr(sym).to_str().ok()?;
    let f = EGL.get()?.get_proc_address(name)?;
    // `extern "system"` == `extern "C"` no x86_64 Linux.
    Some(std::mem::transmute::<
        unsafe extern "system" fn(),
        unsafe extern "C" fn(),
    >(f))
}

/// `get_proc_address` que a camada global entrega pro core (WGL).
///
/// # Safety
/// `sym` deve ser um ponteiro C válido pra string NUL-terminada (ou nulo).
#[cfg(windows)]
pub unsafe fn resolve_proc(sym: *const c_char) -> sys::retro_proc_address_t {
    if sym.is_null() {
        return None;
    }
    let p = wgl::proc_address(std::ffi::CStr::from_ptr(sym));
    if p.is_null() {
        return None;
    }
    // x86_64 Windows: `extern "system"` == `extern "C"`.
    Some(std::mem::transmute::<*const c_void, unsafe extern "C" fn()>(p))
}

/// Contexto da plataforma — Linux: EGL (surfaceless ou pbuffer 1×1).
#[cfg(unix)]
struct PlatCtx {
    display: egl::Display,
    context: egl::Context,
    /// `Some` = fallback pbuffer (sem `EGL_KHR_surfaceless_context`).
    surface: Option<egl::Surface>,
}

/// Cria o contexto EGL pedido pelo core, torna atual e carrega o GL.
#[cfg(unix)]
fn create_platform(cfg: &GlConfig) -> Result<(PlatCtx, glow::Context), String> {
    let egl = egl()?;

    let display = open_display(egl)?;
    let (major, minor) = egl
        .initialize(display)
        .map_err(|e| format!("eglInitialize: {e}"))?;
    log::info!("EGL {major}.{minor} pra HW render");

    let is_gles = matches!(
        cfg.context_type,
        sys::RETRO_HW_CONTEXT_OPENGLES2
            | sys::RETRO_HW_CONTEXT_OPENGLES3
            | sys::RETRO_HW_CONTEXT_OPENGLES_VERSION
    );
    let renderable = if is_gles {
        egl::OPENGL_ES3_BIT
    } else {
        egl::OPENGL_BIT
    };

    let config = {
        let attrs = [
            egl::SURFACE_TYPE,
            egl::PBUFFER_BIT,
            egl::RENDERABLE_TYPE,
            renderable,
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::NONE,
        ];
        egl.choose_first_config(display, &attrs)
            .map_err(|e| format!("eglChooseConfig: {e}"))?
            .ok_or_else(|| "nenhuma EGLConfig compatível".to_string())?
    };

    egl.bind_api(if is_gles {
        egl::OPENGL_ES_API
    } else {
        egl::OPENGL_API
    })
    .map_err(|e| format!("eglBindAPI: {e}"))?;

    let context = {
        let mut attrs = vec![
            egl::CONTEXT_MAJOR_VERSION,
            cfg.version_major.max(if is_gles { 2 } else { 3 }) as egl::Int,
            egl::CONTEXT_MINOR_VERSION,
            cfg.version_minor as egl::Int,
        ];
        if !is_gles {
            // `OPENGL_CORE` → core profile; senão compat.
            let core = cfg.context_type == sys::RETRO_HW_CONTEXT_OPENGL_CORE;
            attrs.push(egl::CONTEXT_OPENGL_PROFILE_MASK);
            attrs.push(if core {
                egl::CONTEXT_OPENGL_CORE_PROFILE_BIT
            } else {
                egl::CONTEXT_OPENGL_COMPATIBILITY_PROFILE_BIT
            });
        }
        attrs.push(egl::NONE);
        egl.create_context(display, config, None, &attrs)
            .map_err(|e| format!("eglCreateContext: {e}"))?
    };

    // `EGL_KHR_surfaceless_context` evita a pbuffer; senão cria uma 1×1.
    let surfaceless = egl
        .query_string(Some(display), egl::EXTENSIONS)
        .map(|s| s.to_string_lossy().contains("EGL_KHR_surfaceless_context"))
        .unwrap_or(false);
    let surface = if surfaceless {
        None
    } else {
        let attrs = [egl::WIDTH, 1, egl::HEIGHT, 1, egl::NONE];
        Some(
            egl.create_pbuffer_surface(display, config, &attrs)
                .map_err(|e| format!("eglCreatePbufferSurface: {e}"))?,
        )
    };

    egl.make_current(display, surface, surface, Some(context))
        .map_err(|e| format!("eglMakeCurrent: {e}"))?;

    let gl = unsafe {
        glow::Context::from_loader_function_cstr(|s| {
            egl.get_proc_address(s.to_str().unwrap_or_default())
                .map_or(std::ptr::null(), |f| f as *const c_void)
        })
    };

    Ok((
        PlatCtx {
            display,
            context,
            surface,
        },
        gl,
    ))
}

/// Contexto da plataforma — Windows: WGL numa janela oculta 1×1. O Windows
/// não tem EGL (sem `libEGL.dll`); antes disto todo core com render OpenGL
/// por hardware falhava lá (Beetle PSX HW, flycast, mupen64plus).
#[cfg(windows)]
struct PlatCtx(wgl::Wgl);

#[cfg(windows)]
impl PlatCtx {
    fn make_current(&self) -> Result<(), String> {
        self.0.make_current()
    }

    fn destroy(&mut self) {
        self.0.destroy();
    }
}

#[cfg(windows)]
fn create_platform(cfg: &GlConfig) -> Result<(PlatCtx, glow::Context), String> {
    let is_gles = matches!(
        cfg.context_type,
        sys::RETRO_HW_CONTEXT_OPENGLES2
            | sys::RETRO_HW_CONTEXT_OPENGLES3
            | sys::RETRO_HW_CONTEXT_OPENGLES_VERSION
    );
    let profile = if is_gles {
        wgl::Profile::Es
    } else if cfg.context_type == sys::RETRO_HW_CONTEXT_OPENGL_CORE {
        wgl::Profile::Core
    } else {
        wgl::Profile::Compat
    };
    let major = cfg.version_major.max(if is_gles { 2 } else { 3 });
    let ctx = wgl::Wgl::create(major, cfg.version_minor, profile)?;
    log::info!(
        "WGL {major}.{} ({profile:?}) pra HW render",
        cfg.version_minor
    );
    let gl = unsafe { glow::Context::from_loader_function_cstr(|s| wgl::proc_address(s)) };
    Ok((PlatCtx(ctx), gl))
}

#[cfg(windows)]
mod wgl {
    //! Contexto OpenGL do Windows via WGL, sem dependência nova: janela
    //! oculta (a WGL exige um HDC com pixel format), contexto temporário pra
    //! obter `wglCreateContextAttribsARB` e, com ele, o contexto na versão e
    //! perfil que o core pediu. Tudo na thread do core, como no EGL.
    use std::ffi::{c_void, CStr};
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{GetDC, ReleaseDC, HDC};
    use windows_sys::Win32::Graphics::OpenGL::{
        wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent, ChoosePixelFormat,
        SetPixelFormat, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
        PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
    };
    use windows_sys::Win32::System::LibraryLoader::{
        GetModuleHandleW, GetProcAddress, LoadLibraryA,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, CS_OWNDC, WNDCLASSW,
        WS_POPUP,
    };

    // WGL_ARB_create_context / _profile, WGL_EXT_create_context_es2_profile
    const WGL_CONTEXT_MAJOR_VERSION_ARB: i32 = 0x2091;
    const WGL_CONTEXT_MINOR_VERSION_ARB: i32 = 0x2092;
    const WGL_CONTEXT_PROFILE_MASK_ARB: i32 = 0x9126;
    const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: i32 = 0x1;
    const WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB: i32 = 0x2;
    const WGL_CONTEXT_ES2_PROFILE_BIT_EXT: i32 = 0x4;

    type CreateContextAttribs = unsafe extern "system" fn(HDC, HGLRC, *const i32) -> HGLRC;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Profile {
        Core,
        Compat,
        Es,
    }

    pub struct Wgl {
        hwnd: HWND,
        hdc: HDC,
        hglrc: HGLRC,
    }

    /// "ReEmuGlHidden\0" em UTF-16.
    fn class_name() -> &'static [u16] {
        static NAME: OnceLock<Vec<u16>> = OnceLock::new();
        NAME.get_or_init(|| "ReEmuGlHidden\0".encode_utf16().collect())
    }

    unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        DefWindowProcW(h, m, w, l)
    }

    /// Registra a classe da janela oculta uma vez por processo.
    fn register_class() -> Result<(), String> {
        static DONE: OnceLock<Result<(), String>> = OnceLock::new();
        DONE.get_or_init(|| unsafe {
            let wc = WNDCLASSW {
                style: CS_OWNDC,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: GetModuleHandleW(std::ptr::null()),
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name().as_ptr(),
            };
            if RegisterClassW(&wc) == 0 {
                Err("RegisterClassW da janela GL oculta falhou".into())
            } else {
                Ok(())
            }
        })
        .clone()
    }

    impl Wgl {
        pub fn create(major: u32, minor: u32, profile: Profile) -> Result<Self, String> {
            register_class()?;
            unsafe {
                let hwnd = CreateWindowExW(
                    0,
                    class_name().as_ptr(),
                    class_name().as_ptr(),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return Err("CreateWindowExW da janela GL oculta falhou".into());
                }
                let hdc = GetDC(hwnd);
                let fail = |msg: String| {
                    ReleaseDC(hwnd, hdc);
                    DestroyWindow(hwnd);
                    Err(msg)
                };
                if hdc.is_null() {
                    return fail("GetDC da janela GL oculta falhou".into());
                }

                let mut pfd: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
                pfd.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
                pfd.nVersion = 1;
                pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
                pfd.iPixelType = PFD_TYPE_RGBA;
                pfd.cColorBits = 32;
                pfd.cAlphaBits = 8;
                pfd.cDepthBits = 24;
                pfd.cStencilBits = 8;
                pfd.iLayerType = PFD_MAIN_PLANE as u8;
                let pf = ChoosePixelFormat(hdc, &pfd);
                if pf == 0 || SetPixelFormat(hdc, pf, &pfd) == 0 {
                    return fail("pixel format OpenGL indisponível (driver de vídeo?)".into());
                }

                // Contexto legado temporário: só ele dá acesso ao
                // `wglCreateContextAttribsARB`.
                let tmp = wglCreateContext(hdc);
                if tmp.is_null() || wglMakeCurrent(hdc, tmp) == 0 {
                    if !tmp.is_null() {
                        wglDeleteContext(tmp);
                    }
                    return fail("wglCreateContext falhou (driver OpenGL instalado?)".into());
                }
                let create_attribs: Option<CreateContextAttribs> =
                    wglGetProcAddress(c"wglCreateContextAttribsARB".as_ptr().cast())
                        .map(|f| std::mem::transmute::<_, CreateContextAttribs>(f));

                let hglrc = match create_attribs {
                    Some(create) => {
                        let bit = match profile {
                            Profile::Core => WGL_CONTEXT_CORE_PROFILE_BIT_ARB,
                            Profile::Compat => WGL_CONTEXT_COMPATIBILITY_PROFILE_BIT_ARB,
                            Profile::Es => WGL_CONTEXT_ES2_PROFILE_BIT_EXT,
                        };
                        let attrs = [
                            WGL_CONTEXT_MAJOR_VERSION_ARB,
                            major as i32,
                            WGL_CONTEXT_MINOR_VERSION_ARB,
                            minor as i32,
                            WGL_CONTEXT_PROFILE_MASK_ARB,
                            bit,
                            0,
                        ];
                        let ctx = create(hdc, std::ptr::null_mut(), attrs.as_ptr());
                        wglMakeCurrent(hdc, std::ptr::null_mut());
                        wglDeleteContext(tmp);
                        if ctx.is_null() {
                            return fail(format!(
                                "o driver não criou um contexto OpenGL {major}.{minor} ({profile:?})"
                            ));
                        }
                        ctx
                    }
                    // Driver antigo: só serve se o core aceita o contexto legado.
                    None if profile == Profile::Compat => tmp,
                    None => {
                        wglMakeCurrent(hdc, std::ptr::null_mut());
                        wglDeleteContext(tmp);
                        return fail(
                            "driver sem WGL_ARB_create_context — atualize o driver de vídeo".into(),
                        );
                    }
                };
                if wglMakeCurrent(hdc, hglrc) == 0 {
                    wglDeleteContext(hglrc);
                    return fail("wglMakeCurrent falhou".into());
                }
                Ok(Self { hwnd, hdc, hglrc })
            }
        }

        pub fn make_current(&self) -> Result<(), String> {
            if unsafe { wglMakeCurrent(self.hdc, self.hglrc) } == 0 {
                Err("wglMakeCurrent falhou".into())
            } else {
                Ok(())
            }
        }

        pub fn destroy(&mut self) {
            unsafe {
                wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
                if !self.hglrc.is_null() {
                    wglDeleteContext(self.hglrc);
                    self.hglrc = std::ptr::null_mut();
                }
                if !self.hwnd.is_null() {
                    ReleaseDC(self.hwnd, self.hdc);
                    DestroyWindow(self.hwnd);
                    self.hwnd = std::ptr::null_mut();
                }
            }
        }
    }

    /// Endereço de uma função GL. `wglGetProcAddress` só resolve extensões e
    /// GL > 1.1 (e devolve 0/1/2/3/-1 quando não acha); o núcleo 1.1 sai do
    /// próprio `opengl32.dll`.
    pub fn proc_address(name: &CStr) -> *const c_void {
        unsafe {
            let p = wglGetProcAddress(name.as_ptr().cast()).map_or(0usize, |f| f as usize);
            if !matches!(p, 0 | 1 | 2 | 3 | usize::MAX) {
                return p as *const c_void;
            }
            static OPENGL32: OnceLock<usize> = OnceLock::new();
            let module =
                *OPENGL32.get_or_init(|| LoadLibraryA(c"opengl32.dll".as_ptr().cast()) as usize);
            if module == 0 {
                return std::ptr::null();
            }
            GetProcAddress(module as *mut c_void, name.as_ptr().cast())
                .map_or(std::ptr::null(), |f| f as *const c_void)
        }
    }
}

#[cfg(unix)]
fn open_display(egl: &EglInstance) -> Result<egl::Display, String> {
    // 1) surfaceless Mesa (headless puro — CI, servidores). 2) display default
    // (NVIDIA / desktop com servidor rodando).
    // SAFETY: `DEFAULT_DISPLAY` é o argumento canônico; sem native handle cru.
    unsafe {
        if let Ok(d) = egl.get_platform_display(
            PLATFORM_SURFACELESS_MESA,
            egl::DEFAULT_DISPLAY,
            &[egl::ATTRIB_NONE],
        ) {
            return Ok(d);
        }
        egl.get_display(egl::DEFAULT_DISPLAY)
            .ok_or_else(|| "eglGetDisplay(EGL_DEFAULT_DISPLAY) devolveu NO_DISPLAY".into())
    }
}

/// `GL_KHR_debug`: manda as mensagens do driver pro `log` (síncrono, então o
/// backtrace bate com a chamada culpada). Ligado por `REEMU_GL_DEBUG` — o
/// diagnóstico de "tela preta" num core GL no hardware do usuário. Spec:
/// registry.khronos.org/OpenGL, extensão `KHR_debug`.
unsafe fn enable_gl_debug(gl: &mut glow::Context) {
    let exts = gl.supported_extensions().clone();
    if !exts.contains("GL_KHR_debug") {
        log::warn!("REEMU_GL_DEBUG: contexto sem GL_KHR_debug — sem debug output");
        return;
    }
    gl.enable(glow::DEBUG_OUTPUT);
    gl.enable(glow::DEBUG_OUTPUT_SYNCHRONOUS);
    gl.debug_message_callback(|source, gltype, id, severity, msg| {
        let sev = match severity {
            glow::DEBUG_SEVERITY_HIGH => "HIGH",
            glow::DEBUG_SEVERITY_MEDIUM => "MED",
            glow::DEBUG_SEVERITY_LOW => "LOW",
            _ => "NOTE",
        };
        if severity == glow::DEBUG_SEVERITY_HIGH {
            log::error!("GL[{sev}] src=0x{source:x} type=0x{gltype:x} id={id}: {msg}");
        } else {
            log::debug!("GL[{sev}] src=0x{source:x} type=0x{gltype:x} id={id}: {msg}");
        }
    });
    log::info!("REEMU_GL_DEBUG: GL_KHR_debug ativo (síncrono)");
}

unsafe fn build_fbo(
    gl: &glow::Context,
    w: u32,
    h: u32,
    depth_stencil: bool,
) -> Result<(glow::Framebuffer, glow::Texture, Option<glow::Renderbuffer>), String> {
    let color = gl
        .create_texture()
        .map_err(|e| format!("glGenTextures: {e}"))?;
    gl.bind_texture(glow::TEXTURE_2D, Some(color));
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        glow::RGBA8 as i32,
        w as i32,
        h as i32,
        0,
        glow::RGBA,
        glow::UNSIGNED_BYTE,
        glow::PixelUnpackData::Slice(None),
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_MIN_FILTER,
        glow::LINEAR as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_MAG_FILTER,
        glow::LINEAR as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_S,
        glow::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameter_i32(
        glow::TEXTURE_2D,
        glow::TEXTURE_WRAP_T,
        glow::CLAMP_TO_EDGE as i32,
    );

    let fbo = gl
        .create_framebuffer()
        .map_err(|e| format!("glGenFramebuffers: {e}"))?;
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER,
        glow::COLOR_ATTACHMENT0,
        glow::TEXTURE_2D,
        Some(color),
        0,
    );

    let depth_rbo = if depth_stencil {
        let rb = gl
            .create_renderbuffer()
            .map_err(|e| format!("glGenRenderbuffers: {e}"))?;
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(rb));
        gl.renderbuffer_storage(
            glow::RENDERBUFFER,
            glow::DEPTH24_STENCIL8,
            w as i32,
            h as i32,
        );
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_STENCIL_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(rb),
        );
        Some(rb)
    } else {
        None
    };

    let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
    if status != glow::FRAMEBUFFER_COMPLETE {
        return Err(format!("FBO incompleto: 0x{status:x}"));
    }
    Ok((fbo, color, depth_rbo))
}

/// Inverte as linhas de um buffer RGBA8 `w×h` no lugar (GL bottom-left → top-left).
fn flip_rows_in_place(buf: &mut [u8], w: u32, h: u32) {
    let row = (w * 4) as usize;
    if row == 0 || buf.len() < row * h as usize {
        return;
    }
    let mut tmp = vec![0u8; row];
    for y in 0..(h as usize) / 2 {
        let top = y * row;
        let bot = (h as usize - 1 - y) * row;
        tmp.copy_from_slice(&buf[top..top + row]);
        buf.copy_within(bot..bot + row, top);
        buf[bot..bot + row].copy_from_slice(&tmp);
    }
}

/// Renderiza uma cor sólida num `dma_buf` real (GBM + EGLImage + GL, o mesmo
/// caminho de `try_enable_interop`/`bind_write_slot`/`finish_write_slot` que
/// um core de HW render usa) e devolve o plano já no formato público de
/// `domain` — pra provar, num teste de `reemu-desktop` (o outro lado do
/// interop, o import pelo wgpu em `gpu.rs`), que um `dma_buf` produzido de
/// verdade por este crate importa e amostra certo do lado de lá. Só disponível
/// com a feature `test-fixtures` (mesmo padrão do `testcore_path`).
///
/// O `GlContext` pode ser dropado logo depois — o fd do `dma_buf` exportado
/// (`gbm_bo_get_fd`) é uma referência do kernel independente do `gbm_bo` de
/// origem, então o conteúdo continua válido.
#[cfg(all(unix, feature = "test-fixtures"))]
pub fn render_solid_rgba_to_dmabuf(
    rgba: [u8; 4],
    w: u32,
    h: u32,
) -> Result<domain::frame_source::DmabufPlaneInfo, String> {
    use std::os::fd::IntoRawFd as _;
    let cfg = GlConfig {
        context_type: sys::RETRO_HW_CONTEXT_OPENGL,
        version_major: 3,
        version_minor: 3,
        depth: false,
        stencil: false,
        bottom_left_origin: false,
    };
    let mut ctx = GlContext::create(&cfg, w, h)?;
    if !ctx.try_enable_interop() {
        return Err("interop dma_buf indisponível nesta máquina".into());
    }
    ctx.bind_write_slot();
    unsafe {
        ctx.gl.clear_color(
            rgba[0] as f32 / 255.0,
            rgba[1] as f32 / 255.0,
            rgba[2] as f32 / 255.0,
            rgba[3] as f32 / 255.0,
        );
        ctx.gl.clear(glow::COLOR_BUFFER_BIT);
    }
    let (_, plane) = ctx
        .finish_write_slot()
        .ok_or("finish_write_slot devolveu None (interop não ativo?)")?;
    let plane = plane.ok_or("1ª entrega do slot deveria mandar o fd (handed=false)")?;
    Ok(domain::frame_source::DmabufPlaneInfo {
        fd: plane.fd.into_raw_fd(),
        width: plane.width,
        height: plane.height,
        stride: plane.stride,
        offset: plane.offset,
        modifier: plane.modifier,
        fourcc: plane.fourcc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "precisa de EGL em runtime (libEGL + Mesa surfaceless ou $DISPLAY)"]
    fn fbo_clear_and_readback() {
        let cfg = GlConfig {
            context_type: sys::RETRO_HW_CONTEXT_OPENGL,
            version_major: 3,
            version_minor: 3,
            depth: true,
            stencil: false,
            bottom_left_origin: true,
        };
        let ctx = GlContext::create(&cfg, 64, 64).expect("criar contexto GL");
        unsafe {
            ctx.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(ctx.fbo));
            ctx.gl.viewport(0, 0, 8, 8);
            ctx.gl.clear_color(0.0, 1.0, 0.0, 1.0);
            ctx.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            ctx.gl.finish();
        }
        let px = ctx.read_pixels(8, 8);
        assert_eq!(&px[0..4], &[0, 255, 0, 255], "pixel verde do glClear");
    }

    #[test]
    #[ignore = "precisa de EGL+GBM em hardware real (render node DRM) — interop dma_buf"]
    fn interop_ring_renders_into_dmabuf_backed_texture() {
        let cfg = GlConfig {
            context_type: sys::RETRO_HW_CONTEXT_OPENGL,
            version_major: 3,
            version_minor: 3,
            depth: false,
            stencil: false,
            bottom_left_origin: true,
        };
        let mut ctx = GlContext::create(&cfg, 64, 64).expect("criar contexto GL");
        assert!(
            ctx.try_enable_interop(),
            "interop deveria estar disponível nesta máquina (GBM + EGL_EXT_image_dma_buf_import)"
        );

        // 1º frame no slot 0: clear vermelho, lê de volta a MESMA textura que o
        // dma_buf respalda (bind_write_slot já a deixou como COLOR_ATTACHMENT0)
        // — prova que o driver realmente escreveu no buffer compartilhado, não
        // só numa textura GL comum.
        ctx.bind_write_slot();
        unsafe {
            ctx.gl.clear_color(1.0, 0.0, 0.0, 1.0);
            ctx.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        let (slot0, plane0) = ctx.finish_write_slot().expect("slot 0 sempre entrega Some");
        assert_eq!(slot0, 0);
        assert!(
            plane0.is_some(),
            "1ª entrega do slot manda o fd (handed=false)"
        );
        assert_eq!(
            &ctx.read_pixels(8, 8)[0..4],
            &[255, 0, 0, 255],
            "clear vermelho no alvo dma_buf do slot 0"
        );

        // 2º frame: ring de 2 avança pro slot 1.
        ctx.bind_write_slot();
        unsafe {
            ctx.gl.clear_color(0.0, 1.0, 0.0, 1.0);
            ctx.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        let (slot1, plane1) = ctx.finish_write_slot().expect("slot 1 entrega Some");
        assert_eq!(slot1, 1);
        assert!(plane1.is_some());

        // 3º frame: RING=2 volta pro slot 0 — reaproveita o MESMO dma_buf já
        // entregue antes, então não manda o fd de novo (`handed=true`).
        ctx.bind_write_slot();
        unsafe {
            ctx.gl.clear_color(0.0, 0.0, 1.0, 1.0);
            ctx.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        let (slot0_again, plane0_again) = ctx.finish_write_slot().expect("slot 0 de novo");
        assert_eq!(slot0_again, 0);
        assert!(
            plane0_again.is_none(),
            "2ª vez do MESMO slot não reenvia o fd — já foi entregue (`handed`)"
        );
        assert_eq!(
            &ctx.read_pixels(8, 8)[0..4],
            &[0, 0, 255, 255],
            "clear azul no slot 0 reaproveitado"
        );
    }

    #[test]
    fn flip_rows_swaps_top_bottom() {
        // 1×2, linha 0 = 0xAA, linha 1 = 0xBB
        let mut b = vec![0xAA, 0xAA, 0xAA, 0xAA, 0xBB, 0xBB, 0xBB, 0xBB];
        flip_rows_in_place(&mut b, 1, 2);
        assert_eq!(b, vec![0xBB, 0xBB, 0xBB, 0xBB, 0xAA, 0xAA, 0xAA, 0xAA]);
    }
}
