//! Buffer circular de rewind em RAM com deltas XOR comprimidos
//! (`docs/ai-context/17-cheats-rewind.md`, fase 3).
//!
//! Implementação 100% própria (MIT), escrita do zero seguindo apenas o
//! `libretro.h` (`retro_serialize_size`, `retro_serialize`, `retro_unserialize`,
//! `RETRO_ENVIRONMENT_GET_SAVESTATE_CONTEXT` e `SET_SERIALIZATION_QUIRKS`).
//!
//! Estrutura do histórico:
//! - `head_state` guarda sempre o estado mais recente completo em memória;
//! - `history` guarda deltas reversos comprimidos (`Deflate` rápido) da entrada
//!   mais antiga na frente (`front`) até a mais recente atrás (`back`);
//! - cada `CompressedDelta` permite reconstruir exatamente o estado anterior
//!   em cima de `head_state` (inclusive quando o tamanho muda entre quadros em
//!   cores com `CORE_VARIABLE_SIZE`), e descartar quadros antigos quando o
//!   orçamento de RAM enche resume-se a `pop_front()` em O(1).

use crate::core::DesktopCore;
use crate::ffi_state;
use domain::frame_source::Frame;
use domain::rewind::check_rewind_quirks;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::collections::VecDeque;
use std::io::{Read, Write};

const STREAM_CHUNK: usize = 8192;
const MAX_WARM_SCRATCH_CAP: usize = 64 * 1024;

struct CompressedDelta {
    prev_len: usize,
    compressed: Box<[u8]>,
}

/// Anel em memória de estados serializados com compressão XOR + Deflate e teto
/// estrito de bytes (incluindo estado-base, histórico e buffers de trabalho).
pub struct RewindBuffer {
    budget_bytes: usize,
    head_state: Option<Vec<u8>>,
    serialize_buf: Vec<u8>,
    scratch_compressed: Vec<u8>,
    history: VecDeque<CompressedDelta>,
    history_payload_bytes: usize,
}

impl RewindBuffer {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            head_state: None,
            serialize_buf: Vec::new(),
            scratch_compressed: Vec::new(),
            history: VecDeque::new(),
            history_payload_bytes: 0,
        }
    }

    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Atualiza o orçamento máximo de RAM e descarta entradas antigas se preciso.
    pub fn set_budget(&mut self, budget_bytes: usize) {
        self.budget_bytes = budget_bytes;
        self.evict_to_budget();
    }

    /// Total de estados representados no buffer (`0` vazio; `1 + deltas` quando
    /// há estado-base).
    pub fn len(&self) -> usize {
        match &self.head_state {
            Some(_) => 1 + self.history.len(),
            None => 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head_state.is_none()
    }

    /// Memória total retida pelo buffer, somando estado-base, tabela e cargas
    /// comprimidas do histórico e buffers temporários reutilizáveis.
    pub fn memory_usage(&self) -> usize {
        let head = self.head_state.as_ref().map_or(0, Vec::capacity);
        let ser = self.serialize_buf.capacity();
        let scratch = self.scratch_compressed.capacity();
        let table = self.history.capacity() * std::mem::size_of::<CompressedDelta>();
        head + ser + scratch + table + self.history_payload_bytes
    }

    /// Descarta todo o histórico e libera os buffers temporários (chamado em
    /// `RestoreState` externo, `Reset`, troca/fechamento de jogo ou desligamento
    /// do recurso).
    pub fn clear(&mut self) {
        self.head_state = None;
        self.serialize_buf = Vec::new();
        self.scratch_compressed = Vec::new();
        self.history = VecDeque::new();
        self.history_payload_bytes = 0;
    }

    /// Acrescenta uma captura `state` respeitando os `quirks` de serialização e
    /// o teto `budget_bytes`. Devolve `false` quando recusado por quirk, vazio
    /// ou maior que o orçamento total.
    pub fn push(&mut self, state: &[u8], quirks: u64) -> bool {
        if check_rewind_quirks(quirks).is_err() {
            self.clear();
            return false;
        }
        if state.is_empty() || state.len() > self.budget_bytes {
            if state.len() > self.budget_bytes {
                self.clear();
            }
            return false;
        }

        // Garante espaço para o novo `head_state` sem ultrapassar o orçamento.
        let mut next_head = Vec::new();
        next_head.reserve_exact(state.len());
        next_head.extend_from_slice(state);

        self.commit_new_head(next_head)
    }

    /// Captura diretamente do `DesktopCore` reutilizando `serialize_buf` sob o
    /// contexto `RETRO_SAVESTATE_CONTEXT_RUNAHEAD_SAME_INSTANCE`.
    pub fn capture_from_core(&mut self, core: &mut DesktopCore, quirks: u64) -> bool {
        if check_rewind_quirks(quirks).is_err() {
            self.clear();
            return false;
        }
        let size = core.rewind_serialize_size();
        if size == 0 || size > self.budget_bytes {
            if size > self.budget_bytes {
                self.clear();
            }
            return false;
        }

        // Como `head_state` e `serialize_buf` coexistem durante a captura, se
        // `head + size` estourar o orçamento inteiro esvaziamos o que for
        // necessário ANTES de redimensionar `serialize_buf`.
        self.make_room_for_capture(size);
        let head_cap = self.head_state.as_ref().map_or(0, Vec::capacity);
        if head_cap + size > self.budget_bytes {
            // O orçamento comporta 1 estado (`size <= budget_bytes`), mas não 2
            // buffers completos ao mesmo tempo: descarta o anterior para nunca
            // passar do limite.
            self.clear();
        }

        resize_exact(&mut self.serialize_buf, size);
        if !core.serialize_rewind_into(&mut self.serialize_buf) {
            // `MUST_INITIALIZE` pode falhar nos primeiros quadros; mantém o que
            // já havia e tenta de novo nos próximos.
            self.evict_to_budget();
            return false;
        }

        let captured = std::mem::take(&mut self.serialize_buf);
        self.commit_new_head(captured)
    }

    fn make_room_for_capture(&mut self, incoming_size: usize) {
        if self.scratch_compressed.capacity() > MAX_WARM_SCRATCH_CAP {
            self.scratch_compressed = Vec::new();
        }
        while !self.history.is_empty() {
            let head_cap = self.head_state.as_ref().map_or(0, Vec::capacity);
            let scratch = self.scratch_compressed.capacity();
            let table = self.history.capacity() * std::mem::size_of::<CompressedDelta>();
            let projected = head_cap + incoming_size + scratch + table + self.history_payload_bytes;
            if projected <= self.budget_bytes {
                break;
            }
            if let Some(oldest) = self.history.pop_front() {
                self.history_payload_bytes = self
                    .history_payload_bytes
                    .saturating_sub(oldest.compressed.len());
            }
        }
        if self.history.is_empty() && self.history.capacity() > 0 {
            self.history.shrink_to_fit();
        }
    }

    fn commit_new_head(&mut self, next_head: Vec<u8>) -> bool {
        if let Some(prev_head) = self.head_state.take() {
            if let Some(compressed) =
                encode_reverse_delta(&prev_head, &next_head, &mut self.scratch_compressed)
            {
                let delta = CompressedDelta {
                    prev_len: prev_head.len(),
                    compressed,
                };
                self.history_payload_bytes += delta.compressed.len();
                self.history.push_back(delta);
                // Reaproveita a alocação do `prev_head` para a próxima captura se
                // couber no orçamento; `evict_to_budget` libera se apertar.
                if self.serialize_buf.capacity() == 0 {
                    self.serialize_buf = prev_head;
                    self.serialize_buf.clear();
                }
            }
        }
        if self.scratch_compressed.capacity() > MAX_WARM_SCRATCH_CAP {
            self.scratch_compressed = Vec::new();
        }
        self.head_state = Some(next_head);
        self.evict_to_budget();
        self.head_state.is_some()
    }

    fn evict_to_budget(&mut self) {
        if self.memory_usage() <= self.budget_bytes {
            return;
        }
        // 1. Solta primeiro os buffers temporários ociosos.
        if self.scratch_compressed.capacity() > 0 {
            self.scratch_compressed = Vec::new();
        }
        if self.memory_usage() > self.budget_bytes && self.serialize_buf.capacity() > 0 {
            self.serialize_buf = Vec::new();
        }
        // 2. Descarta os deltas mais antigos em O(1).
        while self.memory_usage() > self.budget_bytes {
            let Some(oldest) = self.history.pop_front() else {
                break;
            };
            self.history_payload_bytes = self
                .history_payload_bytes
                .saturating_sub(oldest.compressed.len());
        }
        if self.memory_usage() > self.budget_bytes && self.history.capacity() > 0 {
            self.history.shrink_to_fit();
        }
        // 3. Se nem o `head_state` sozinho cabe no novo orçamento, esvazia tudo.
        if self.memory_usage() > self.budget_bytes {
            self.clear();
        }
    }

    /// Retira e devolve o estado mais recente do buffer (LIFO), reconstruindo o
    /// anterior em `head_state`.
    pub fn pop(&mut self) -> Option<Vec<u8>> {
        let mut curr = self.head_state.take()?;
        let Some(delta) = self.history.pop_back() else {
            return Some(curr);
        };
        self.history_payload_bytes = self
            .history_payload_bytes
            .saturating_sub(delta.compressed.len());
        let out = curr.clone();
        if decode_reverse_delta_in_place(&mut curr, delta.prev_len, &delta.compressed) {
            self.head_state = Some(curr);
        } else {
            self.clear();
        }
        Some(out)
    }

    /// Passo usado a cada quadro com o atalho de rewind segurado:
    /// - se havia quadros intermediários desde a última captura (`stride > 1`),
    ///   volta primeiro para `head_state`;
    /// - caso contrário, recua um delta no histórico;
    /// - ao atingir o estado mais antigo do buffer, permanece nele sem o apagar
    ///   (segurar o botão no início do histórico mantém o jogo parado ali em
    ///   vez de voltar a avançar).
    pub fn step_back_for_frame(&mut self, untracked_frames: &mut u32) -> Option<&[u8]> {
        self.head_state.as_ref()?;
        if *untracked_frames > 0 {
            *untracked_frames = 0;
            return self.head_state.as_deref();
        }
        if let Some(delta) = self.history.pop_back() {
            self.history_payload_bytes = self
                .history_payload_bytes
                .saturating_sub(delta.compressed.len());
            let head = self.head_state.as_mut()?;
            if !decode_reverse_delta_in_place(head, delta.prev_len, &delta.compressed) {
                self.clear();
                return None;
            }
        }
        self.head_state.as_deref()
    }
}

fn resize_exact(buf: &mut Vec<u8>, len: usize) {
    if buf.len() > len {
        buf.truncate(len);
        if buf.capacity() > len.saturating_mul(2) {
            buf.shrink_to(len);
        }
    } else if buf.len() < len {
        let extra = len - buf.len();
        if buf.capacity() < len {
            buf.reserve_exact(extra);
        }
        buf.resize(len, 0);
    }
}

/// Codifica `prev` em relação a `curr` (`prev[i] ^ curr[i]` no trecho comum e
/// `prev[common..]` no excedente quando `prev.len() > curr.len()`).
fn encode_reverse_delta(prev: &[u8], curr: &[u8], scratch: &mut Vec<u8>) -> Option<Box<[u8]>> {
    scratch.clear();
    let mut enc = DeflateEncoder::new(&mut *scratch, Compression::fast());
    let common = prev.len().min(curr.len());
    let mut offset = 0usize;
    let mut chunk = [0u8; STREAM_CHUNK];
    while offset < common {
        let n = (common - offset).min(STREAM_CHUNK);
        for i in 0..n {
            chunk[i] = prev[offset + i] ^ curr[offset + i];
        }
        enc.write_all(&chunk[..n]).ok()?;
        offset += n;
    }
    if prev.len() > common {
        enc.write_all(&prev[common..]).ok()?;
    }
    enc.finish().ok()?;
    Some(scratch.as_slice().into())
}

/// Aplica o delta reverso in-place em `curr`, transformando `curr` em `prev`.
fn decode_reverse_delta_in_place(curr: &mut Vec<u8>, prev_len: usize, compressed: &[u8]) -> bool {
    if prev_len == 0 {
        return false;
    }
    resize_exact(curr, prev_len);
    let mut dec = DeflateDecoder::new(compressed);
    let mut offset = 0usize;
    let mut chunk = [0u8; STREAM_CHUNK];
    while offset < prev_len {
        let want = (prev_len - offset).min(STREAM_CHUNK);
        match dec.read(&mut chunk[..want]) {
            Ok(0) => return false,
            Ok(n) => {
                for i in 0..n {
                    curr[offset + i] ^= chunk[i];
                }
                offset += n;
            }
            Err(_) => return false,
        }
    }
    true
}

/// Orquestra captura periódica e retrocesso quadro a quadro sobre um `DesktopCore`
/// (compartilhado entre `reemu-core-host` e `emu-session::LocalCore`).
pub struct RewindController {
    enabled: bool,
    stride: u32,
    rewinding: bool,
    frames_until_capture: u32,
    untracked_frames: u32,
    buffer: RewindBuffer,
}

/// Resultado de um passo do loop de quadro sob `RewindController`.
pub struct RewindStepOutcome {
    pub frame: Option<Frame>,
    /// Vazio quando `rewound == true` (áudio silenciado/descartado no retrocesso).
    pub audio: Vec<i16>,
    /// `true` quando este quadro veio de um `retro_unserialize` do rewind.
    pub rewound: bool,
}

impl Default for RewindController {
    fn default() -> Self {
        Self::new(
            false,
            (domain::rewind::DEFAULT_REWIND_BUDGET_MB as usize) * 1024 * 1024,
            1,
        )
    }
}

impl RewindController {
    pub fn new(enabled: bool, budget_bytes: usize, stride: u32) -> Self {
        Self {
            enabled: enabled && stride > 0,
            stride,
            rewinding: false,
            frames_until_capture: 0,
            untracked_frames: 0,
            buffer: RewindBuffer::new(budget_bytes),
        }
    }

    pub fn configure(&mut self, enabled: bool, budget_bytes: usize, stride: u32) {
        let active = enabled && stride > 0;
        self.stride = stride;
        self.buffer.set_budget(budget_bytes);
        if !active {
            self.enabled = false;
            self.rewinding = false;
            self.frames_until_capture = 0;
            self.untracked_frames = 0;
            self.buffer.clear();
            return;
        }
        if !self.enabled {
            self.frames_until_capture = 0;
            self.untracked_frames = 0;
        }
        self.enabled = true;
    }

    pub fn set_rewinding(&mut self, rewinding: bool) {
        if !self.enabled {
            self.rewinding = false;
            return;
        }
        if self.rewinding && !rewinding {
            // Retomando avanço normal a partir do ponto restaurado: captura no
            // próximo quadro para manter a continuidade imediata.
            self.frames_until_capture = 0;
            self.untracked_frames = 0;
        }
        self.rewinding = rewinding;
    }

    pub fn is_rewinding(&self) -> bool {
        self.rewinding && self.enabled
    }

    pub fn clear(&mut self) {
        self.rewinding = false;
        self.frames_until_capture = 0;
        self.untracked_frames = 0;
        self.buffer.clear();
    }

    /// Executa um quadro do core:
    /// - em modo rewind, restaura o estado anterior do `RewindBuffer`, roda 1
    ///   quadro para gerar a imagem e descarta o áudio gerado;
    /// - em modo normal, captura antes do `retro_run` conforme `stride` e devolve
    ///   vídeo + áudio normais.
    pub fn step_realtime(&mut self, core: &mut DesktopCore) -> RewindStepOutcome {
        let quirks = ffi_state::serialization_quirks();
        if self.enabled && check_rewind_quirks(quirks).is_err() {
            log::info!("rewind: desativado pelos quirks de serialização do core (0x{quirks:x})");
            self.configure(false, self.buffer.budget_bytes(), self.stride);
        }

        if self.rewinding && self.enabled {
            if let Some(state_bytes) = self.buffer.step_back_for_frame(&mut self.untracked_frames) {
                let restored = core.restore_rewind_state(state_bytes);
                if restored {
                    core.reset_frame_time();
                    let frame = core.next_realtime_frame();
                    // Descarta o PCM do quadro rebobinado para não chiar ao contrário.
                    let _ = core.drain_audio();
                    return RewindStepOutcome {
                        frame,
                        audio: Vec::new(),
                        rewound: true,
                    };
                }
                // Core recusou `retro_unserialize`: descarta histórico inválido.
                self.clear();
            } else {
                // Histórico ainda vazio (ex.: apertou no 1º quadro): não há o que voltar.
                let frame = core.next_realtime_frame();
                let _ = core.drain_audio();
                return RewindStepOutcome {
                    frame,
                    audio: Vec::new(),
                    rewound: true,
                };
            }
        }

        if self.enabled && self.stride > 0 {
            if self.frames_until_capture == 0 {
                if self.buffer.capture_from_core(core, quirks) {
                    self.frames_until_capture = self.stride.saturating_sub(1);
                    self.untracked_frames = 0;
                }
            } else {
                self.frames_until_capture -= 1;
                self.untracked_frames += 1;
            }
        }

        let frame = core.next_realtime_frame();
        let audio = core.drain_audio();
        RewindStepOutcome {
            frame,
            audio,
            rewound: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::core_loader::serialization_quirk as q;

    fn synthetic_state(frame: usize, len: usize) -> Vec<u8> {
        let mut buf = vec![0u8; len];
        // Cabeçalho único por quadro + miolo esparso realista (RAM majoritariamente estável)
        let f_bytes = (frame as u64).to_le_bytes();
        buf[..8.min(len)].copy_from_slice(&f_bytes[..8.min(len)]);
        for (i, slot) in buf.iter_mut().enumerate().take(len).skip(8) {
            // A maior parte fica idêntica entre quadros consecutivos, mudando uma janela por frame
            if (i / 64) % 17 == (frame % 17) {
                *slot = ((i.wrapping_mul(31).wrapping_add(frame * 7)) & 0xFF) as u8;
            } else {
                *slot = ((i.wrapping_mul(13)) & 0xFF) as u8;
            }
        }
        buf
    }

    #[test]
    fn restores_exact_states_across_20_frames_in_reverse_order() {
        let mut rb = RewindBuffer::new(4 * 1024 * 1024);
        let expected: Vec<Vec<u8>> = (0..20).map(|f| synthetic_state(f, 16_384)).collect();

        for st in &expected {
            assert!(rb.push(st, 0));
            assert!(rb.memory_usage() <= rb.budget_bytes());
        }
        assert_eq!(rb.len(), 20);

        for f in (0..20).rev() {
            let got = rb.pop().expect("estado esperado no pop");
            assert_eq!(got, expected[f], "divergência ao restaurar quadro {f}");
        }
        assert!(rb.pop().is_none());
        assert!(rb.is_empty());
    }

    #[test]
    fn handles_variable_state_sizes_growing_and_shrinking() {
        let mut rb = RewindBuffer::new(2 * 1024 * 1024);
        let quirks = q::CORE_VARIABLE_SIZE | q::FRONT_VARIABLE_SIZE;
        let sizes = [1024usize, 1500, 4096, 900, 8192, 512, 2048];
        let states: Vec<Vec<u8>> = sizes
            .iter()
            .enumerate()
            .map(|(i, &sz)| synthetic_state(i + 100, sz))
            .collect();

        for st in &states {
            assert!(rb.push(st, quirks));
            assert!(rb.memory_usage() <= rb.budget_bytes());
        }
        assert_eq!(rb.len(), states.len());

        for (i, expected) in states.iter().enumerate().rev() {
            let got = rb.pop().unwrap();
            assert_eq!(&got, expected, "falha em tamanho variável no passo {i}");
        }
    }

    #[test]
    fn strictly_enforces_ram_budget_and_evicts_oldest_entries() {
        let budget = 48 * 1024; // 48 KiB
        let mut rb = RewindBuffer::new(budget);
        // Estados pseudo-aleatórios pouco compressíveis para forçar despejo real
        let mut states = Vec::new();
        let mut seed = 0x1234_5678u64;
        for f in 0..30 {
            let mut st = vec![0u8; 8 * 1024];
            for b in &mut st {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1 + f);
                *b = (seed >> 32) as u8;
            }
            assert!(rb.push(&st, 0));
            assert!(
                rb.memory_usage() <= budget,
                "uso de RAM ({}) excedeu o orçamento ({budget}) no quadro {f}",
                rb.memory_usage()
            );
            states.push(st);
        }

        // Deve ter descartado os mais antigos e preservado exatamente o sufixo mais novo.
        let kept = rb.len();
        assert!(kept >= 2 && kept < states.len(), "kept={kept}");
        for idx in (states.len() - kept..states.len()).rev() {
            assert_eq!(rb.pop().unwrap(), states[idx]);
        }
        assert!(rb.is_empty());

        // Estado único maior que o orçamento inteiro é recusado sem estourar o limite.
        let huge = vec![0xAAu8; budget + 1024];
        assert!(!rb.push(&huge, 0));
        assert!(rb.memory_usage() <= budget);
    }

    #[test]
    fn clear_discards_history_and_quirks_reject_cleanly() {
        let mut rb = RewindBuffer::new(512 * 1024);
        for f in 0..5 {
            assert!(rb.push(&synthetic_state(f, 4096), 0));
        }
        assert_eq!(rb.len(), 5);

        rb.clear();
        assert_eq!(rb.len(), 0);
        assert_eq!(rb.memory_usage(), 0);
        assert!(rb.pop().is_none());

        // `INCOMPLETE` e `SINGLE_SESSION` recusam e limpam o buffer.
        assert!(rb.push(&synthetic_state(1, 2048), q::MUST_INITIALIZE));
        assert_eq!(rb.len(), 1);
        assert!(!rb.push(&synthetic_state(2, 2048), q::INCOMPLETE));
        assert!(rb.is_empty());
        assert!(!rb.push(&synthetic_state(3, 2048), q::SINGLE_SESSION));
        assert!(rb.is_empty());
    }

    #[test]
    fn step_back_for_frame_holds_at_oldest_snapshot_without_underflow() {
        let mut rb = RewindBuffer::new(512 * 1024);
        let s0 = synthetic_state(0, 1024);
        let s1 = synthetic_state(1, 1024);
        let s2 = synthetic_state(2, 1024);
        rb.push(&s0, 0);
        rb.push(&s1, 0);
        rb.push(&s2, 0);

        let mut untracked = 1u32;
        assert_eq!(
            rb.step_back_for_frame(&mut untracked).unwrap(),
            s2.as_slice()
        );
        assert_eq!(untracked, 0);
        assert_eq!(
            rb.step_back_for_frame(&mut untracked).unwrap(),
            s1.as_slice()
        );
        assert_eq!(
            rb.step_back_for_frame(&mut untracked).unwrap(),
            s0.as_slice()
        );
        // Continua segurando no piso (`s0`) sem esvaziar nem avançar.
        assert_eq!(
            rb.step_back_for_frame(&mut untracked).unwrap(),
            s0.as_slice()
        );
        assert_eq!(rb.len(), 1);
    }
}
