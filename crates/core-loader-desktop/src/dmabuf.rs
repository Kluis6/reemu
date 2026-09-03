//! Alocação de buffers `dma_buf` via GBM pro interop zero-cópia entre o core
//! (OpenGL) e a shader chain (wgpu/Vulkan).
//!
//! GBM aloca; os dois lados **importam** o mesmo fd: o GL via
//! `EGL_EXT_image_dma_buf_import` ([`crate::gl_context`]), o wgpu via
//! `Device::texture_from_dmabuf_fd` (lado Tauri). É o caminho que funciona na
//! NVIDIA — o driver dela importa `dma_buf` mas não exporta textura GL como tal.
//!
//! `libgbm` é aberto com `libloading` (só ~8 símbolos) — sem link estático,
//! sem `libgbm-dev`. Se faltar, o chamador cai pro readback de CPU.
//!
//! O modifier é escolhido pelo driver (NVIDIA recusa linear puro em RENDERING)
//! e propagado pros dois importadores (EGL e Vulkan aceitam modifier explícito).

use libloading::{Library, Symbol};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::raw::{c_int, c_void};
use std::sync::OnceLock;

const GBM_BO_USE_RENDERING: u32 = 1 << 2;
const GBM_BO_USE_LINEAR: u32 = 1 << 4;

/// fourcc de 4 bytes ASCII (little-endian), como o `<drm_fourcc.h>` define.
const fn fourcc(s: &[u8; 4]) -> u32 {
    (s[0] as u32) | ((s[1] as u32) << 8) | ((s[2] as u32) << 16) | ((s[3] as u32) << 24)
}
/// `DRM_FORMAT_ABGR8888` — `[31:0] A:B:G:R`, ou seja R,G,B,A em memória.
const DRM_FORMAT_ABGR8888: u32 = fourcc(b"AB24");

#[allow(non_camel_case_types)]
type gbm_device = c_void;
#[allow(non_camel_case_types)]
type gbm_bo = c_void;

struct Gbm {
    _lib: Library,
    create_device: unsafe extern "C" fn(c_int) -> *mut gbm_device,
    device_destroy: unsafe extern "C" fn(*mut gbm_device),
    bo_create: unsafe extern "C" fn(*mut gbm_device, u32, u32, u32, u32) -> *mut gbm_bo,
    bo_create_with_modifiers:
        unsafe extern "C" fn(*mut gbm_device, u32, u32, u32, *const u64, u32) -> *mut gbm_bo,
    bo_destroy: unsafe extern "C" fn(*mut gbm_bo),
    bo_get_fd: unsafe extern "C" fn(*mut gbm_bo) -> c_int,
    bo_get_stride: unsafe extern "C" fn(*mut gbm_bo) -> u32,
    bo_get_offset: unsafe extern "C" fn(*mut gbm_bo, c_int) -> u32,
    bo_get_modifier: unsafe extern "C" fn(*mut gbm_bo) -> u64,
    bo_get_width: unsafe extern "C" fn(*mut gbm_bo) -> u32,
    bo_get_height: unsafe extern "C" fn(*mut gbm_bo) -> u32,
}

// SAFETY: os ponteiros de função são estáveis pela vida do processo; o
// `gbm_device`/`gbm_bo` só são tocados atrás de `&self` na thread do core.
unsafe impl Send for Gbm {}
unsafe impl Sync for Gbm {}

static GBM: OnceLock<Option<Gbm>> = OnceLock::new();

fn gbm() -> Result<&'static Gbm, String> {
    GBM.get_or_init(|| unsafe {
        let lib = Library::new("libgbm.so.1")
            .or_else(|_| Library::new("libgbm.so"))
            .ok()?;
        macro_rules! sym {
            ($n:literal) => {{
                let s: Symbol<_> = lib.get($n).ok()?;
                *s
            }};
        }
        let g = Gbm {
            create_device: sym!(b"gbm_create_device"),
            device_destroy: sym!(b"gbm_device_destroy"),
            bo_create: sym!(b"gbm_bo_create"),
            bo_create_with_modifiers: sym!(b"gbm_bo_create_with_modifiers"),
            bo_destroy: sym!(b"gbm_bo_destroy"),
            bo_get_fd: sym!(b"gbm_bo_get_fd"),
            bo_get_stride: sym!(b"gbm_bo_get_stride"),
            bo_get_offset: sym!(b"gbm_bo_get_offset"),
            bo_get_modifier: sym!(b"gbm_bo_get_modifier"),
            bo_get_width: sym!(b"gbm_bo_get_width"),
            bo_get_height: sym!(b"gbm_bo_get_height"),
            _lib: lib,
        };
        Some(g)
    })
    .as_ref()
    .ok_or_else(|| "libgbm indisponível — interop dma_buf desligado".to_string())
}

/// Plano único de um `dma_buf` RGBA8. `fd` é uma cópia própria (fecha no drop).
pub struct DmabufPlane {
    pub fd: OwnedFd,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub offset: u32,
    /// `DRM_FORMAT_MOD_*` (0 = linear).
    pub modifier: u64,
    /// FourCC DRM (`DRM_FORMAT_ABGR8888`).
    pub fourcc: u32,
}

/// Um alvo de render compartilhado (BO do GBM + metadados de plano).
pub struct SharedBuffer {
    bo: *mut gbm_bo,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub offset: u32,
    pub modifier: u64,
    pub fourcc: u32,
}

// SAFETY: só a thread do core mexe no BO.
unsafe impl Send for SharedBuffer {}

impl SharedBuffer {
    /// Novo fd (dup do kernel) pro plano 0 — um pro EGL, outro pro wgpu.
    pub fn plane(&self) -> Result<DmabufPlane, String> {
        let g = gbm()?;
        let raw = unsafe { (g.bo_get_fd)(self.bo) };
        if raw < 0 {
            return Err("gbm_bo_get_fd < 0".into());
        }
        Ok(DmabufPlane {
            fd: unsafe { OwnedFd::from_raw_fd(raw) },
            width: self.width,
            height: self.height,
            stride: self.stride,
            offset: self.offset,
            modifier: self.modifier,
            fourcc: self.fourcc,
        })
    }
}

impl Drop for SharedBuffer {
    fn drop(&mut self) {
        if let Ok(g) = gbm() {
            unsafe { (g.bo_destroy)(self.bo) };
        }
    }
}

/// Dono do device GBM (um render node DRM).
pub struct DmabufAllocator {
    dev: *mut gbm_device,
    _node: std::fs::File,
}

// SAFETY: idem — thread do core.
unsafe impl Send for DmabufAllocator {}

impl DmabufAllocator {
    /// Abre o primeiro render node DRM que aceita `gbm_create_device`.
    pub fn open() -> Result<Self, String> {
        let g = gbm()?;
        let mut last = String::from("nenhum /dev/dri/renderD*");
        for n in 128..136 {
            let path = format!("/dev/dri/renderD{n}");
            let node = match std::fs::File::options().read(true).write(true).open(&path) {
                Ok(f) => f,
                Err(e) => {
                    last = format!("{path}: {e}");
                    continue;
                }
            };
            let dev = unsafe { (g.create_device)(std::os::fd::AsRawFd::as_raw_fd(&node)) };
            if dev.is_null() {
                last = format!("{path}: gbm_create_device devolveu NULL");
                continue;
            }
            log::info!("GBM em {path}");
            return Ok(Self { dev, _node: node });
        }
        Err(last)
    }

    /// Aloca um buffer RGBA8 `w×h` pra render + amostragem. Deixa o driver
    /// escolher o modifier (NVIDIA costuma recusar linear puro em RENDERING);
    /// tenta com/sem modifiers explícitos e cai pra linear como último recurso.
    pub fn alloc(&self, w: u32, h: u32) -> Result<SharedBuffer, String> {
        let g = gbm()?;
        let (w, h) = (w.max(1), h.max(1));
        let fmt = DRM_FORMAT_ABGR8888;
        // DRM_FORMAT_MOD_LINEAR = 0; DRM_FORMAT_MOD_INVALID = u64::MAX.
        let with_mods = |mods: &[u64]| unsafe {
            (g.bo_create_with_modifiers)(self.dev, w, h, fmt, mods.as_ptr(), mods.len() as u32)
        };
        let bo = {
            let a = with_mods(&[u64::MAX]); // deixa o driver escolher
            if !a.is_null() {
                a
            } else {
                let b = unsafe { (g.bo_create)(self.dev, w, h, fmt, GBM_BO_USE_RENDERING) };
                if !b.is_null() {
                    b
                } else {
                    unsafe {
                        (g.bo_create)(
                            self.dev,
                            w,
                            h,
                            fmt,
                            GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR,
                        )
                    }
                }
            }
        };
        if bo.is_null() {
            return Err(format!("gbm_bo_create {w}x{h} falhou (todas as vias)"));
        }
        let modifier = unsafe { (g.bo_get_modifier)(bo) };
        log::info!("dma_buf {w}x{h} modifier 0x{modifier:x}");
        Ok(SharedBuffer {
            modifier,
            width: unsafe { (g.bo_get_width)(bo) },
            height: unsafe { (g.bo_get_height)(bo) },
            stride: unsafe { (g.bo_get_stride)(bo) },
            offset: unsafe { (g.bo_get_offset)(bo, 0) },
            fourcc: DRM_FORMAT_ABGR8888,
            bo,
        })
    }
}

impl Drop for DmabufAllocator {
    fn drop(&mut self) {
        if let Ok(g) = gbm() {
            unsafe { (g.device_destroy)(self.dev) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd as _;

    #[test]
    fn fourcc_abgr8888_matches_drm() {
        // DRM_FORMAT_ABGR8888 == 0x34324241
        assert_eq!(DRM_FORMAT_ABGR8888, 0x3432_4241);
    }

    #[test]
    #[ignore = "precisa de um render node DRM (/dev/dri/renderD128) + libgbm"]
    fn alloc_exports_a_valid_fd() {
        let alloc = DmabufAllocator::open().expect("abrir GBM");
        let buf = alloc.alloc(320, 240).expect("alocar BO");
        assert_eq!((buf.width, buf.height), (320, 240));
        assert!(buf.stride >= 320 * 4, "stride >= largura*4");
        let p = buf.plane().expect("exportar fd");
        assert!(p.fd.as_raw_fd() >= 0);
    }
}
