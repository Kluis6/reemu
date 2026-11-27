//! Tap de audio PCM estereo do core em execucao para gravacao de video do Meu Log.
//!
//! Quando desligado, `push_samples` faz apenas um `AtomicBool::load(Relaxed)`.
//! Quando ligado, grava direto em um arquivo `.wav` temporario no disco e ajusta
//! o cabecalho RIFF/WAVE ao encerrar.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Limite de seguranca: ~30 min em 48 kHz estereo 16-bit (~345 MB).
const MAX_TAP_DATA_BYTES: u32 = 360 * 1024 * 1024;

static TAP_ACTIVE: AtomicBool = AtomicBool::new(false);
static TAP_WRITER: Mutex<Option<GameAudioWriter>> = Mutex::new(None);

struct GameAudioWriter {
    path: PathBuf,
    writer: BufWriter<File>,
    sample_rate: u32,
    rate_locked: bool,
    data_bytes: u32,
    frames: u64,
    byte_scratch: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct FinishedGameAudio {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub frames: u64,
    pub data_bytes: u32,
}

pub fn write_wav_header<W: Write>(
    out: &mut W,
    channels: u16,
    sample_rate: u32,
    data_bytes: u32,
) -> std::io::Result<()> {
    let bits_per_sample: u16 = 16;
    let block_align: u16 = channels.saturating_mul(bits_per_sample / 8);
    let byte_rate: u32 = sample_rate.saturating_mul(u32::from(block_align));
    let riff_size: u32 = 36u32.saturating_add(data_bytes);

    out.write_all(b"RIFF")?;
    out.write_all(&riff_size.to_le_bytes())?;
    out.write_all(b"WAVE")?;
    out.write_all(b"fmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?; // PCM
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&sample_rate.to_le_bytes())?;
    out.write_all(&byte_rate.to_le_bytes())?;
    out.write_all(&block_align.to_le_bytes())?;
    out.write_all(&bits_per_sample.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data_bytes.to_le_bytes())?;
    Ok(())
}

pub fn is_active() -> bool {
    TAP_ACTIVE.load(Ordering::Relaxed)
}

pub fn start_game_audio_capture(wav_path: &Path) -> Result<(), String> {
    if let Some(parent) = wav_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("criando pasta de audio: {e}"))?;
    }
    let file = File::create(wav_path).map_err(|e| format!("abrindo wav temporario: {e}"))?;
    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    write_wav_header(&mut writer, 2, 48_000, 0)
        .map_err(|e| format!("escrevendo cabecalho wav: {e}"))?;

    let mut slot = TAP_WRITER.lock().unwrap_or_else(|p| p.into_inner());
    *slot = Some(GameAudioWriter {
        path: wav_path.to_path_buf(),
        writer,
        sample_rate: 48_000,
        rate_locked: false,
        data_bytes: 0,
        frames: 0,
        byte_scratch: Vec::with_capacity(8192),
    });
    TAP_ACTIVE.store(true, Ordering::Release);
    Ok(())
}

pub fn push_game_samples(samples: &[i16], core_sample_rate: u32) {
    if !TAP_ACTIVE.load(Ordering::Relaxed) || samples.is_empty() {
        return;
    }
    let Ok(mut guard) = TAP_WRITER.try_lock() else {
        return;
    };
    let Some(st) = guard.as_mut() else {
        return;
    };
    if !st.rate_locked && core_sample_rate > 0 {
        st.sample_rate = core_sample_rate;
        st.rate_locked = true;
    }
    let usable_samples = samples.len() & !1;
    if usable_samples == 0 {
        return;
    }
    let add_bytes = (usable_samples * 2) as u32;
    if st.data_bytes.saturating_add(add_bytes) > MAX_TAP_DATA_BYTES {
        return;
    }
    st.byte_scratch.clear();
    st.byte_scratch.reserve(usable_samples * 2);
    for &s in &samples[..usable_samples] {
        st.byte_scratch.extend_from_slice(&s.to_le_bytes());
    }
    if st.writer.write_all(&st.byte_scratch).is_ok() {
        st.data_bytes = st.data_bytes.saturating_add(add_bytes);
        st.frames = st.frames.saturating_add((usable_samples / 2) as u64);
    }
}

pub fn stop_game_audio_capture() -> Option<FinishedGameAudio> {
    TAP_ACTIVE.store(false, Ordering::Release);
    let mut slot = TAP_WRITER.lock().unwrap_or_else(|p| p.into_inner());
    let mut st = slot.take()?;
    let _ = st.writer.flush();
    if st.writer.seek(SeekFrom::Start(0)).is_ok() {
        let _ = write_wav_header(&mut st.writer, 2, st.sample_rate.max(8_000), st.data_bytes);
        let _ = st.writer.flush();
    }
    Some(FinishedGameAudio {
        path: st.path,
        sample_rate: st.sample_rate.max(8_000),
        frames: st.frames,
        data_bytes: st.data_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_and_patches_valid_wav_header() {
        let path = std::env::temp_dir().join(format!("reemu-tap-test-{}.wav", std::process::id()));
        let _ = std::fs::remove_file(&path);

        start_game_audio_capture(&path).unwrap();
        push_game_samples(&[1000, -1000, 2000, -2000], 44_100);
        let done = stop_game_audio_capture().unwrap();
        assert_eq!(done.sample_rate, 44_100);
        assert_eq!(done.frames, 2);
        assert_eq!(done.data_bytes, 8);

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(bytes.len(), 44 + 8);
        let sr = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let data_sz = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        assert_eq!(sr, 44_100);
        assert_eq!(data_sz, 8);
        let _ = std::fs::remove_file(&path);
    }
}
