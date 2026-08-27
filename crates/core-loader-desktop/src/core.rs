//! `DesktopCore`: instância de core carregada. É a `FrameSource` (cada
//! `next_frame` roda um `retro_run`) e guarda a metadata técnica pós-load.

use crate::ffi_state::{self, CoreGuard};
use crate::raw::RawCore;
use domain::core_loader::{CoreRenderRequirements, LoadedCore, SystemAvInfo};
use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, FrameSource};

pub struct DesktopCore {
    raw: RawCore,
    av_info: SystemAvInfo,
    render_reqs: CoreRenderRequirements,
    /// Mantido vivo até o Drop: libera o slot global de "um core por processo".
    _guard: CoreGuard,
}

impl DesktopCore {
    pub(crate) fn new(
        raw: RawCore,
        av_info: SystemAvInfo,
        render_reqs: CoreRenderRequirements,
        guard: CoreGuard,
    ) -> Self {
        Self {
            raw,
            av_info,
            render_reqs,
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
        unsafe { (self.raw.run)() };

        let mut guard = ffi_state::lock();
        let st = guard.as_mut()?;
        if !st.had_new_frame {
            return None; // frame duplicado: compositor mantém o anterior
        }
        st.had_new_frame = false;
        let raw = st.last_frame.take()?;
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
                rotation_degrees: 0,
            },
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
            (self.raw.deinit)();
        }
    }
}
