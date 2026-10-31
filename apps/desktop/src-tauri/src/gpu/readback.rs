//! Anel de leitura de volta do quadro processado para a CPU.

use super::*;

/// Um staging buffer do readback + o estado do `map_async`.
pub(super) struct RbSlot {
    pub(super) buf: wgpu::Buffer,
    /// setado pelo callback do `map_async` quando o buffer está mapeável.
    pub(super) ready: Arc<AtomicBool>,
    /// `map_async` foi pedido e o resultado ainda não foi consumido.
    pub(super) inflight: bool,
    pub(super) w: u32,
    pub(super) h: u32,
    pub(super) padded: u32,
}

/// 2 staging buffers rodando em pipeline (ver campo `rb` do `FrameProcessor`).
#[derive(Default)]
pub(super) struct ReadbackRing {
    pub(super) slots: [Option<RbSlot>; 2],
    pub(super) write: usize,
    pub(super) dims: Option<(u32, u32)>,
}

impl ReadbackRing {
    /// Descarta os buffers (troca de preset / decoração / resize forçado).
    pub(super) fn invalidate(&mut self) {
        self.slots = [None, None];
        self.dims = None;
        self.write = 0;
    }

    /// Garante 2 slots do tamanho `(w, h)`. Recria (e reseta o pipeline) se mudou.
    pub(super) fn ensure(&mut self, device: &wgpu::Device, w: u32, h: u32) {
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
pub(super) fn unpad_rows(src: &[u8], w: u32, h: u32, padded: u32) -> Vec<u8> {
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
pub(super) fn unpad_rows_into(dst: &mut Vec<u8>, src: &[u8], w: u32, h: u32, padded: u32) {
    let row = (w * 4) as usize;
    let need = row * h as usize;
    if dst.len() != need {
        dst.resize(need, 0);
    }
    for y in 0..h as usize {
        dst[y * row..(y + 1) * row].copy_from_slice(&src[y * padded as usize..][..row]);
    }
}
