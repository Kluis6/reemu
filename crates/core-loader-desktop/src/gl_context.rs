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
use khronos_egl as egl;
use std::os::raw::{c_char, c_void};
use std::sync::OnceLock;

use crate::dmabuf::{DmabufAllocator, DmabufPlane, SharedBuffer};
use crate::sys;

/// `EGL_PLATFORM_SURFACELESS_MESA` — display headless sem servidor (CI, Mesa).
const PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;

// --- EGL_EXT_image_dma_buf_import(_modifiers) ---
const EGL_LINUX_DMA_BUF_EXT: egl::Enum = 0x3270;
const EGL_LINUX_DRM_FOURCC_EXT: egl::Attrib = 0x3271;
const EGL_DMA_BUF_PLANE0_FD_EXT: egl::Attrib = 0x3272;
const EGL_DMA_BUF_PLANE0_OFFSET_EXT: egl::Attrib = 0x3273;
const EGL_DMA_BUF_PLANE0_PITCH_EXT: egl::Attrib = 0x3274;
const EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT: egl::Attrib = 0x3443;
const EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT: egl::Attrib = 0x3444;
const EGL_WIDTH: egl::Attrib = 0x3057;
const EGL_HEIGHT: egl::Attrib = 0x3056;
const DRM_FORMAT_MOD_INVALID: u64 = (1 << 56) - 1;
/// Quantos alvos no ring (core escreve N, wgpu lê N-1 no mesmo frame).
const RING: usize = 2;

type EglInstance = egl::DynamicInstance<egl::EGL1_5>;
static EGL: OnceLock<EglInstance> = OnceLock::new();

/// Instância EGL do processo (carrega `libEGL` em runtime na 1ª chamada).
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

pub struct GlContext {
    display: egl::Display,
    context: egl::Context,
    /// `Some` = fallback pbuffer (sem `EGL_KHR_surfaceless_context`).
    surface: Option<egl::Surface>,
    gl: glow::Context,
    fbo: glow::Framebuffer,
    color: glow::Texture,
    depth_rbo: Option<glow::Renderbuffer>,
    max_w: u32,
    max_h: u32,
    /// `read_pixels` inverte as linhas (core bottom-left → canvas top-left).
    flip: bool,
    /// Ring de alvos `dma_buf` compartilhados com o wgpu. `None` = readback CPU.
    interop: Option<InteropRing>,
}

/// Um alvo compartilhado: BO do GBM + `EGLImage` + textura GL respaldada por ele.
struct InteropSlot {
    buffer: SharedBuffer,
    image: egl::Image,
    tex: glow::Texture,
    /// O plano (fd) já foi entregue pro importador wgpu?
    handed: bool,
}

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

        let (fbo, color, depth_rbo) =
            unsafe { build_fbo(&gl, max_w, max_h, cfg.depth || cfg.stencil)? };

        Ok(Self {
            display,
            context,
            surface,
            gl,
            fbo,
            color,
            depth_rbo,
            max_w,
            max_h,
            flip: cfg.bottom_left_origin,
            interop: None,
        })
    }

    /// Id do FBO que o core deve renderizar (`get_current_framebuffer`).
    pub fn fbo(&self) -> u32 {
        self.fbo.0.get()
    }

    pub fn make_current(&self) -> Result<(), String> {
        egl()?
            .make_current(self.display, self.surface, self.surface, Some(self.context))
            .map_err(|e| format!("eglMakeCurrent: {e}"))
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

    pub fn interop_active(&self) -> bool {
        self.interop.is_some()
    }

    /// Tenta montar o ring de alvos `dma_buf` (GBM + EGLImage). `false` → segue
    /// no readback de CPU (qualquer falha é best-effort, nunca fatal).
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

    fn build_interop(&self) -> Result<InteropRing, String> {
        let egl = egl()?;
        let exts = egl
            .query_string(Some(self.display), egl::EXTENSIONS)
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
                self.display,
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

    /// O core renderiza com origem bottom-left → o consumidor (wgpu) inverte Y.
    pub fn flip_y(&self) -> bool {
        self.flip
    }

    /// Depois do `retro_run`: garante o render (sync grosso) e devolve o slot
    /// escrito + o plano `dma_buf` (só na 1ª vez de cada slot).
    pub fn finish_write_slot(&mut self) -> Option<(u32, Option<DmabufPlane>)> {
        let ring = self.interop.as_mut()?;
        unsafe { self.gl.finish() };
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
}

/// `close(2)` sem puxar a crate `libc` — só pro caminho de erro do EGLImage.
unsafe fn libc_close(fd: i32) {
    extern "C" {
        fn close(fd: i32) -> i32;
    }
    close(fd);
}

/// Handle de um frame de HW render entregue via `dma_buf`. O `DesktopCore`
/// devolve isso em `FrameOrigin::HardwareTexture`; o `poll_frame` (lado wgpu)
/// importa o plano uma vez por slot e depois referencia por índice.
pub struct GlInteropHandle {
    slot: u32,
    flip_y: bool,
    plane: std::sync::Mutex<Option<DmabufPlane>>,
}

impl GlInteropHandle {
    pub fn new(slot: u32, flip_y: bool, plane: Option<DmabufPlane>) -> Self {
        Self {
            slot,
            flip_y,
            plane: std::sync::Mutex::new(plane),
        }
    }
}

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

impl Drop for GlContext {
    fn drop(&mut self) {
        let egl = egl();
        if let Some(ring) = self.interop.take() {
            for slot in ring.slots {
                unsafe { self.gl.delete_texture(slot.tex) };
                if let Ok(e) = egl {
                    let _ = e.destroy_image(self.display, slot.image);
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
        if let Ok(egl) = egl {
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
    fn flip_rows_swaps_top_bottom() {
        // 1×2, linha 0 = 0xAA, linha 1 = 0xBB
        let mut b = vec![0xAA, 0xAA, 0xAA, 0xAA, 0xBB, 0xBB, 0xBB, 0xBB];
        flip_rows_in_place(&mut b, 1, 2);
        assert_eq!(b, vec![0xBB, 0xBB, 0xBB, 0xBB, 0xAA, 0xAA, 0xAA, 0xAA]);
    }
}
