//! `DesktopCore`: instância de core carregada. É a `FrameSource` (cada
//! `next_frame` roda um `retro_run`) e guarda a metadata técnica pós-load.

use crate::archive::ExtractedRom;
use crate::ffi_state::{self, CoreGuard};
use crate::gl_context::GlContext;
use crate::raw::RawCore;
use crate::sys;
use domain::core_loader::{CoreRenderRequirements, LoadedCore, SystemAvInfo};
use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, FrameSource, SoftwarePixelFormat};

pub struct DesktopCore {
    raw: RawCore,
    av_info: SystemAvInfo,
    render_reqs: CoreRenderRequirements,
    /// `Some` = core de HW render (GL). O frame sai do FBO deste contexto.
    gl: Option<GlContext>,
    /// ROM extraída de um `.zip` — apagada quando o core é dropado.
    _extracted: Option<ExtractedRom>,
    /// Mantido vivo até o Drop: libera o slot global de "um core por processo".
    _guard: CoreGuard,
}

impl DesktopCore {
    pub(crate) fn new(
        raw: RawCore,
        av_info: SystemAvInfo,
        render_reqs: CoreRenderRequirements,
        guard: CoreGuard,
        gl: Option<GlContext>,
        extracted: Option<ExtractedRom>,
    ) -> Self {
        Self {
            raw,
            av_info,
            render_reqs,
            gl,
            _extracted: extracted,
            _guard: guard,
        }
    }

    /// PCM interleaved (estéreo, i16, na `sample_rate` do core) acumulado
    /// desde a última chamada. Consumido pelo `AudioSink` (etapa 06).
    pub fn drain_audio(&mut self) -> Vec<i16> {
        ffi_state::lock()
            .as_mut()
            .map(|st| std::mem::take(&mut st.audio))
            .unwrap_or_default()
    }

    /// Timing novo (`fps`, `sample_rate`) se o core pediu `SET_SYSTEM_AV_INFO`
    /// desde a última chamada. A thread do core reconfigura o pacing e o
    /// resampler de áudio. Também atualiza o `av_info` interno.
    pub fn take_av_update(&mut self) -> Option<domain::core_loader::SystemTiming> {
        let (fps, sample_rate) = ffi_state::lock()
            .as_mut()
            .and_then(|st| st.av_update.take())?;
        self.av_info.timing.fps = fps;
        self.av_info.timing.sample_rate = sample_rate;
        Some(self.av_info.timing)
    }

    /// Marca que um save state deve ser capturado. O loop principal chama
    /// `poll_save_state` logo após o próximo `next_frame` (nunca no meio de
    /// um `retro_run`). Implementação completa do save state é a etapa 08 —
    /// aqui só o ponto de extensão.
    pub fn request_save_state(&self) {
        if let Some(st) = ffi_state::lock().as_mut() {
            st.save_pending = true;
        }
    }

    /// Se há um save pendente, serializa o estado do core e devolve os bytes
    /// (limpando a flag). Deve ser chamado entre frames.
    pub fn poll_save_state(&mut self) -> Option<Vec<u8>> {
        {
            let mut g = ffi_state::lock();
            let st = g.as_mut()?;
            if !st.save_pending {
                return None;
            }
            st.save_pending = false;
        }
        self.serialize_state()
    }

    /// Serializa incondicionalmente (`retro_serialize`). `None` se o core
    /// não suporta.
    pub fn serialize_state(&mut self) -> Option<Vec<u8>> {
        let size = unsafe { (self.raw.serialize_size)() };
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size];
        let ok = unsafe { (self.raw.serialize)(buf.as_mut_ptr().cast(), size) };
        ok.then_some(buf)
    }

    /// Restaura um estado previamente serializado (`retro_unserialize`).
    pub fn restore_state(&mut self, data: &[u8]) -> bool {
        unsafe { (self.raw.unserialize)(data.as_ptr().cast(), data.len()) }
    }

    /// Cópia da save RAM (SRAM de cartucho / battery save) do core. `None` se
    /// o jogo não tem — muitos jogos salvam só via save state.
    pub fn save_ram(&self) -> Option<Vec<u8>> {
        let size = unsafe { (self.raw.get_memory_size)(sys::RETRO_MEMORY_SAVE_RAM) };
        if size == 0 {
            return None;
        }
        let ptr = unsafe { (self.raw.get_memory_data)(sys::RETRO_MEMORY_SAVE_RAM) };
        if ptr.is_null() {
            return None;
        }
        Some(unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), size) }.to_vec())
    }

    /// Escreve `data` na região de save RAM do core. Deve rodar logo após o
    /// load, antes do primeiro `retro_run`. `false` se o tamanho não bate ou
    /// o jogo não tem SRAM.
    pub fn restore_save_ram(&mut self, data: &[u8]) -> bool {
        let size = unsafe { (self.raw.get_memory_size)(sys::RETRO_MEMORY_SAVE_RAM) };
        if size == 0 || size != data.len() {
            return false;
        }
        let ptr = unsafe { (self.raw.get_memory_data)(sys::RETRO_MEMORY_SAVE_RAM) };
        if ptr.is_null() {
            return false;
        }
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.cast::<u8>(), size) };
        true
    }

    fn aspect_ratio(&self, w: u32, h: u32) -> f32 {
        let declared = self.av_info.geometry.aspect_ratio;
        if declared > 0.0 {
            declared
        } else if h > 0 {
            w as f32 / h as f32
        } else {
            1.0
        }
    }
}

impl FrameSource for DesktopCore {
    fn next_frame(&mut self) -> Option<Frame> {
        if let Some(gl) = &self.gl {
            // Interop: aponta o FBO pro slot de escrita do ring antes do run.
            gl.bind_write_slot();
        }

        unsafe { (self.raw.run)() };

        if self.gl.is_some() {
            return self.next_hw_frame();
        }

        let mut guard = ffi_state::lock();
        let st = guard.as_mut()?;
        if !st.had_new_frame {
            return None; // frame duplicado: compositor mantém o anterior
        }
        st.had_new_frame = false;
        let raw = st.last_frame.take()?;
        let rotation_degrees = st.rotation_degrees;
        drop(guard);

        let ar = self.aspect_ratio(raw.width, raw.height);
        Some(Frame {
            origin: FrameOrigin::SoftwareRawBuffer {
                data: raw.data,
                pitch: raw.pitch,
                format: raw.format,
            },
            metadata: FrameMetadata {
                native_width: raw.width,
                native_height: raw.height,
                aspect_ratio: ar,
                rotation_degrees,
            },
        })
    }
}

impl DesktopCore {
    /// Frame de HW render: interop zero-cópia (`dma_buf` → `HardwareTexture`)
    /// quando ativo, senão readback CPU (`SoftwareRawBuffer` RGBA8).
    fn next_hw_frame(&mut self) -> Option<Frame> {
        let (w, h, rotation_degrees) = {
            let mut guard = ffi_state::lock();
            let st = guard.as_mut()?;
            if !st.had_new_frame {
                return None;
            }
            st.had_new_frame = false;
            let (w, h) = st.hw_frame.take()?;
            (w, h, st.rotation_degrees)
        };
        let ar = self.aspect_ratio(w, h);
        let meta = FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: ar,
            rotation_degrees,
        };

        let gl = self.gl.as_mut()?;
        if gl.interop_active() {
            let flip_y = gl.flip_y();
            let (slot, plane) = gl.finish_write_slot()?;
            return Some(Frame {
                origin: FrameOrigin::HardwareTexture(Box::new(
                    crate::gl_context::GlInteropHandle::new(slot, flip_y, plane),
                )),
                metadata: meta,
            });
        }

        gl.finish();
        let data = gl.read_pixels(w, h);
        Some(Frame {
            origin: FrameOrigin::SoftwareRawBuffer {
                data,
                pitch: w * 4,
                format: SoftwarePixelFormat::Rgba8888,
            },
            metadata: meta,
        })
    }
}

impl LoadedCore for DesktopCore {
    fn system_av_info(&self) -> SystemAvInfo {
        self.av_info
    }

    fn render_requirements(&self) -> CoreRenderRequirements {
        self.render_reqs.clone()
    }
}

impl Drop for DesktopCore {
    fn drop(&mut self) {
        // Sequência de teardown libretro; depois a lib descarrega e o
        // `_guard` zera o estado global.
        unsafe {
            (self.raw.unload_game)();

            // HW render: o core solta os recursos GL dele (`context_destroy`)
            // enquanto o contexto ainda está corrente; depois o `GlContext`
            // dropa e destrói o EGL.
            if let Some(gl) = &self.gl {
                let _ = gl.make_current();
                let destroy = ffi_state::lock()
                    .as_ref()
                    .and_then(|st| st.hw_render)
                    .and_then(|r| r.context_destroy);
                if let Some(f) = destroy {
                    f();
                }
            }
            self.gl = None;

            (self.raw.deinit)();
        }
    }
}
