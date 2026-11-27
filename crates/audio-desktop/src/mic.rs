//! Gravador nativo de notas de audio (microfone -> WAV PCM 16-bit mono) via `cpal`.
//!
//! `cpal::Stream` nao implementa `Send` em todas as plataformas, entao o stream
//! vive inteiramente numa thread dedicada (`reemu-mic-recorder`) controlada por
//! canais sincronos.

use crate::tap::write_wav_header;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct MicRecordingDone {
    pub path: PathBuf,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub sample_rate: u32,
}

enum ControlCmd {
    Stop,
    Cancel,
}

pub struct MicRecorderHandle {
    cmd_tx: SyncSender<ControlCmd>,
    done_rx: Mutex<Receiver<Result<MicRecordingDone, String>>>,
    started_at: Instant,
    samples_written: Arc<AtomicU64>,
    sample_rate: u32,
}

impl MicRecorderHandle {
    pub fn elapsed_ms(&self) -> u64 {
        let by_samples = self
            .samples_written
            .load(Ordering::Relaxed)
            .saturating_mul(1000)
            / u64::from(self.sample_rate.max(1));
        by_samples.max(self.started_at.elapsed().as_millis() as u64)
    }

    pub fn stop(self) -> Result<MicRecordingDone, String> {
        let _ = self.cmd_tx.send(ControlCmd::Stop);
        self.done_rx
            .into_inner()
            .unwrap_or_else(|p| p.into_inner())
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "tempo esgotado ao finalizar gravacao do microfone".to_owned())?
    }

    pub fn cancel(self) {
        let _ = self.cmd_tx.send(ControlCmd::Cancel);
        if let Ok(rx) = self.done_rx.into_inner() {
            let _ = rx.recv_timeout(Duration::from_secs(3));
        }
    }
}

struct MonoWavSink {
    writer: BufWriter<File>,
    data_bytes: u32,
    max_samples: u64,
    samples_written: Arc<AtomicU64>,
}

impl MonoWavSink {
    fn push_mono_i16(&mut self, sample: i16) {
        let cur = self.samples_written.load(Ordering::Relaxed);
        if cur >= self.max_samples {
            return;
        }
        if self.data_bytes > u32::MAX - 4 {
            return;
        }
        if self.writer.write_all(&sample.to_le_bytes()).is_ok() {
            self.data_bytes = self.data_bytes.saturating_add(2);
            self.samples_written.store(cur + 1, Ordering::Relaxed);
        }
    }
}

fn f32_to_i16(v: f32) -> i16 {
    let clamped = v.clamp(-1.0, 1.0);
    (clamped * 32767.0) as i16
}

fn u16_to_i16(v: u16) -> i16 {
    (i32::from(v) - 32768).clamp(-32768, 32767) as i16
}

pub fn start_mic_recording(
    wav_path: PathBuf,
    max_duration: Duration,
) -> Result<MicRecorderHandle, String> {
    if let Some(parent) = wav_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("nao foi possivel criar pasta da nota de audio: {e}"))?;
    }

    let (init_tx, init_rx) = mpsc::sync_channel::<Result<u32, String>>(1);
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<ControlCmd>(2);
    let (done_tx, done_rx) = mpsc::sync_channel::<Result<MicRecordingDone, String>>(1);
    let samples_written = Arc::new(AtomicU64::new(0));
    let samples_thread = Arc::clone(&samples_written);

    std::thread::Builder::new()
        .name("reemu-mic-recorder".into())
        .spawn(move || {
            let host = cpal::default_host();
            let Some(device) = host.default_input_device() else {
                let _ = init_tx.send(Err(
                    "Nenhum microfone encontrado no sistema. Conecte um microfone e tente novamente."
                        .into(),
                ));
                return;
            };
            let supported = match device.default_input_config() {
                Ok(cfg) => cfg,
                Err(e) => {
                    let _ = init_tx.send(Err(format!(
                        "Nao foi possivel ler a configuracao do microfone: {e}"
                    )));
                    return;
                }
            };

            let sample_format = supported.sample_format();
            let config: cpal::StreamConfig = supported.config();
            let sample_rate = config.sample_rate.max(8_000);
            let channels = usize::from(config.channels.max(1));
            let max_samples = u64::from(sample_rate)
                .saturating_mul(max_duration.as_secs().max(1));

            let file = match File::create(&wav_path) {
                Ok(f) => f,
                Err(e) => {
                    let _ = init_tx.send(Err(format!(
                        "Nao foi possivel criar arquivo de audio: {e}"
                    )));
                    return;
                }
            };
            let mut writer = BufWriter::with_capacity(64 * 1024, file);
            if let Err(e) = write_wav_header(&mut writer, 1, sample_rate, 0) {
                let _ = init_tx.send(Err(format!("Falha ao iniciar arquivo WAV: {e}")));
                return;
            }

            let sink = Arc::new(Mutex::new(MonoWavSink {
                writer,
                data_bytes: 0,
                max_samples,
                samples_written: Arc::clone(&samples_thread),
            }));

            let err_fn = |err| log::warn!("stream de microfone: {err}");
            let sink_cb = Arc::clone(&sink);

            let stream_res = match sample_format {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    config,
                    move |data: &[f32], _: &_| {
                        if let Ok(mut s) = sink_cb.lock() {
                            for frame in data.chunks(channels) {
                                let sum: f32 = frame.iter().copied().sum();
                                s.push_mono_i16(f32_to_i16(sum / frame.len() as f32));
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    config,
                    move |data: &[i16], _: &_| {
                        if let Ok(mut s) = sink_cb.lock() {
                            for frame in data.chunks(channels) {
                                let sum: i32 = frame.iter().map(|&x| i32::from(x)).sum();
                                let avg = (sum / frame.len() as i32).clamp(-32768, 32767) as i16;
                                s.push_mono_i16(avg);
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => device.build_input_stream(
                    config,
                    move |data: &[u16], _: &_| {
                        if let Ok(mut s) = sink_cb.lock() {
                            for frame in data.chunks(channels) {
                                let sum: i32 =
                                    frame.iter().map(|&x| i32::from(u16_to_i16(x))).sum();
                                let avg = (sum / frame.len() as i32).clamp(-32768, 32767) as i16;
                                s.push_mono_i16(avg);
                            }
                        }
                    },
                    err_fn,
                    None,
                ),
                other => {
                    let _ = init_tx.send(Err(format!(
                        "Formato de microfone nao suportado: {other:?}"
                    )));
                    return;
                }
            };

            let stream = match stream_res {
                Ok(s) => s,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Falha ao abrir microfone: {e}")));
                    return;
                }
            };
            if let Err(e) = stream.play() {
                let _ = init_tx.send(Err(format!("Falha ao iniciar captura do microfone: {e}")));
                return;
            }

            let started = Instant::now();
            let _ = init_tx.send(Ok(sample_rate));

            let cmd = match cmd_rx.recv_timeout(max_duration) {
                Ok(c) => c,
                Err(_) => ControlCmd::Stop,
            };
            drop(stream);

            if matches!(cmd, ControlCmd::Cancel) {
                drop(sink);
                let _ = std::fs::remove_file(&wav_path);
                let _ = done_tx.send(Err("cancelled".into()));
                return;
            }

            let mut final_sink = match Arc::try_unwrap(sink) {
                Ok(m) => m.into_inner().unwrap_or_else(|p| p.into_inner()),
                Err(arc) => {
                    let mut g = arc.lock().unwrap_or_else(|p| p.into_inner());
                    let data_bytes = g.data_bytes;
                    let _ = g.writer.flush();
                    if g.writer.seek(SeekFrom::Start(0)).is_ok() {
                        let _ = write_wav_header(&mut g.writer, 1, sample_rate, data_bytes);
                        let _ = g.writer.flush();
                    }
                    let samples = g.samples_written.load(Ordering::Relaxed);
                    let duration_ms = samples
                        .saturating_mul(1000)
                        .checked_div(u64::from(sample_rate))
                        .unwrap_or_else(|| started.elapsed().as_millis() as u64)
                        .max(100);
                    let size_bytes = u64::from(g.data_bytes) + 44;
                    let _ = done_tx.send(Ok(MicRecordingDone {
                        path: wav_path,
                        duration_ms,
                        size_bytes,
                        sample_rate,
                    }));
                    return;
                }
            };

            let _ = final_sink.writer.flush();
            if final_sink.writer.seek(SeekFrom::Start(0)).is_ok() {
                let _ = write_wav_header(
                    &mut final_sink.writer,
                    1,
                    sample_rate,
                    final_sink.data_bytes,
                );
                let _ = final_sink.writer.flush();
            }
            drop(final_sink.writer);

            let samples = samples_thread.load(Ordering::Relaxed);
            if samples == 0 {
                let _ = std::fs::remove_file(&wav_path);
                let _ = done_tx.send(Err(
                    "Nenhum audio foi capturado pelo microfone.".into(),
                ));
                return;
            }
            let duration_ms = samples
                .saturating_mul(1000)
                .checked_div(u64::from(sample_rate))
                .unwrap_or_else(|| started.elapsed().as_millis() as u64)
                .max(100);
            let size_bytes = std::fs::metadata(&wav_path)
                .map(|m| m.len())
                .unwrap_or(u64::from(final_sink.data_bytes) + 44);
            let _ = done_tx.send(Ok(MicRecordingDone {
                path: wav_path,
                duration_ms,
                size_bytes,
                sample_rate,
            }));
        })
        .map_err(|e| format!("falha ao iniciar thread do microfone: {e}"))?;

    let sample_rate = init_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "tempo esgotado ao inicializar o microfone".to_owned())??;

    Ok(MicRecorderHandle {
        cmd_tx,
        done_rx: Mutex::new(done_rx),
        started_at: Instant::now(),
        samples_written,
        sample_rate,
    })
}
