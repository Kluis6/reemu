//! `CpalAudioSink`: `domain::audio::AudioSink` via `cpal`, com resample
//! linear de razão dinâmica (Dynamic Rate Control). Construído e usado na
//! thread que dirige o core — a `cpal::Stream` é `!Send`.

use crate::rate_control::RateControl;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use domain::audio::{AudioConfig, AudioSink};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("nenhum dispositivo de saída de áudio")]
    NoDevice,
    #[error("formato de saída não suportado (esperado f32): {0:?}")]
    UnsupportedFormat(cpal::SampleFormat),
    #[error("cpal: {0}")]
    Cpal(String),
}

pub struct CpalAudioSink {
    stream: cpal::Stream,
    producer: HeapProd<f32>,
    capacity: usize,
    device_rate: u32,
    rate_control: RateControl,
    resampler: Resampler,
    scratch: Vec<f32>,
    /// Estimador da taxa real de entrada (frames estéreo/s medidos pelo
    /// relógio). Cores mentem um pouco no `sample_rate` declarado
    /// (parallel_n64: diz 26807, entrega ~27244) — medir corrige o resto.
    in_rate: RateEstimator,
    /// DRC do `AudioConfig` — pra reconstruir ao trocar de dispositivo.
    drc_delta: f32,
    drc_enabled: bool,
    /// Diagnóstico (`REEMU_AUDIO_DEBUG=1`): amostras zeradas pelo callback do
    /// cpal por falta de dados (underrun), acumulado. `None` = debug desligado.
    diag: Option<AudioDiag>,
}

/// Contadores de diagnóstico do caminho de áudio. Ativado por `REEMU_AUDIO_DEBUG=1`.
struct AudioDiag {
    /// Amostras `f32` que o callback teve que zerar (underrun). Compartilhado
    /// com a closure do stream.
    starved: Arc<AtomicU64>,
    last_report: Instant,
    pushes: u64,
    in_frames: u64,
    out_frames: u64,
    dropped: u64,
    fill_min: f32,
    fill_max: f32,
    fill_sum: f64,
    fill_n: u64,
    last_rate: u32,
    last_factor: f32,
}

impl AudioDiag {
    fn new(starved: Arc<AtomicU64>) -> Self {
        Self {
            starved,
            last_report: Instant::now(),
            pushes: 0,
            in_frames: 0,
            out_frames: 0,
            dropped: 0,
            fill_min: 1.0,
            fill_max: 0.0,
            fill_sum: 0.0,
            fill_n: 0,
            last_rate: 0,
            last_factor: 1.0,
        }
    }
}

impl CpalAudioSink {
    /// Usa `config` (dispositivo por nome, delta do DRC). Cai pro dispositivo
    /// padrão se o salvo não existir (o caller avisa via toast).
    pub fn new(config: &AudioConfig) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device =
            pick_device(&host, config.output_device_id.as_deref()).ok_or(AudioError::NoDevice)?;

        let supported = device
            .default_output_config()
            .map_err(|e| AudioError::Cpal(e.to_string()))?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(AudioError::UnsupportedFormat(supported.sample_format()));
        }
        let stream_config: cpal::StreamConfig = supported.config();
        let device_rate = stream_config.sample_rate;
        let channels = stream_config.channels.max(1) as usize;

        // ~250 ms de buffer.
        let capacity = (device_rate as usize * channels / 4).max(2048);
        let (producer, mut consumer): (HeapProd<f32>, HeapCons<f32>) =
            HeapRb::<f32>::new(capacity).split();

        let debug = std::env::var_os("REEMU_AUDIO_DEBUG").is_some();
        let starved = Arc::new(AtomicU64::new(0));
        let cb_starved = Arc::clone(&starved);

        let stream = device
            .build_output_stream(
                stream_config,
                move |data: &mut [f32], _: &_| {
                    let got = consumer.pop_slice(data);
                    data[got..].fill(0.0); // underrun -> silêncio
                    if debug && got < data.len() {
                        cb_starved.fetch_add((data.len() - got) as u64, Ordering::Relaxed);
                    }
                },
                move |err| log::warn!("stream de áudio: {err}"),
                None,
            )
            .map_err(|e| AudioError::Cpal(e.to_string()))?;
        stream.play().map_err(|e| AudioError::Cpal(e.to_string()))?;

        if debug {
            log::info!(
                "áudio: device_rate={device_rate} Hz, {channels} canais, buffer {capacity} amostras (~{} ms)",
                capacity as u64 * 1000 / (device_rate as u64 * channels as u64).max(1)
            );
        }

        Ok(Self {
            stream,
            producer,
            capacity,
            device_rate,
            rate_control: RateControl::new(config.rate_control_delta, config.rate_control_enabled),
            resampler: Resampler::default(),
            scratch: Vec::new(),
            in_rate: RateEstimator::new(),
            drc_delta: config.rate_control_delta,
            drc_enabled: config.rate_control_enabled,
            diag: debug.then(|| AudioDiag::new(starved)),
        })
    }

    fn fill(&self) -> f32 {
        self.producer.occupied_len() as f32 / self.capacity as f32
    }
}

fn pick_device(host: &cpal::Host, wanted_id: Option<&str>) -> Option<cpal::Device> {
    if let Some(id) = wanted_id {
        if let Ok(mut it) = host.output_devices() {
            if let Some(d) = it.find(|d| d.id().ok().map(|i| i.to_string()).as_deref() == Some(id))
            {
                return Some(d);
            }
            log::warn!("dispositivo de áudio id={id} não encontrado — usando o padrão do sistema");
        }
    }
    host.default_output_device()
}

impl AudioSink for CpalAudioSink {
    fn push_samples(&mut self, samples: &[i16], core_sample_rate: u32) {
        if samples.is_empty() {
            return;
        }
        let in_frames: Vec<[f32; 2]> = samples
            .chunks_exact(2)
            .map(|c| [c[0] as f32 / 32768.0, c[1] as f32 / 32768.0])
            .collect();

        self.in_rate.anchor_to(core_sample_rate);
        self.in_rate.observe(in_frames.len());

        let fill = self.fill();
        let factor = self.rate_control.factor(fill) as f64;
        let ratio = (self.device_rate as f64 / self.in_rate.rate()) * factor;

        self.scratch.clear();
        self.resampler.process(&in_frames, ratio, &mut self.scratch);
        // push_slice descarta o que não couber (overrun -> o DRC corrige)
        let pushed = self.producer.push_slice(&self.scratch);

        if let Some(d) = self.diag.as_mut() {
            d.pushes += 1;
            d.in_frames += in_frames.len() as u64;
            d.out_frames += (self.scratch.len() / 2) as u64;
            d.dropped += ((self.scratch.len() - pushed) / 2) as u64;
            d.fill_min = d.fill_min.min(fill);
            d.fill_max = d.fill_max.max(fill);
            d.fill_sum += fill as f64;
            d.fill_n += 1;
            d.last_rate = core_sample_rate;
            d.last_factor = factor as f32;
            if d.last_report.elapsed().as_secs_f32() >= 1.0 {
                let starved = d.starved.swap(0, Ordering::Relaxed);
                let avg_fill = d.fill_sum / d.fill_n.max(1) as f64;
                log::info!(
                    "áudio 1s: core_rate={} rate_medido={:.0} factor={:.4} pushes={} \
                     fill[min {:.2} avg {:.2} max {:.2}] \
                     in={} out={} descartadas={} STARVED={} amostras{}",
                    d.last_rate,
                    self.in_rate.rate(),
                    d.last_factor,
                    d.pushes,
                    d.fill_min,
                    avg_fill,
                    d.fill_max,
                    d.in_frames,
                    d.out_frames,
                    d.dropped,
                    starved,
                    if starved > 0 { " ⚠️" } else { "" },
                );
                d.last_report = Instant::now();
                d.pushes = 0;
                d.in_frames = 0;
                d.out_frames = 0;
                d.dropped = 0;
                d.fill_min = 1.0;
                d.fill_max = 0.0;
                d.fill_sum = 0.0;
                d.fill_n = 0;
            }
        }
    }

    fn pause(&mut self) {
        let _ = self.stream.pause();
    }

    fn resume(&mut self) {
        let _ = self.stream.play();
    }

    fn set_output_device(&mut self, device_id: Option<&str>) -> Result<(), String> {
        let cfg = AudioConfig {
            output_device_id: device_id.map(str::to_owned),
            rate_control_enabled: self.drc_enabled,
            rate_control_delta: self.drc_delta,
            ..AudioConfig::default()
        };
        match CpalAudioSink::new(&cfg) {
            Ok(fresh) => {
                *self = fresh;
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Mede a taxa real de chegada do áudio (frames estéreo por segundo de relógio)
/// e a suaviza. Ancorada no `sample_rate` declarado pelo core: só corrige até
/// ±12% (uma hiccup no load não pode mandar a estimativa pro espaço).
struct RateEstimator {
    anchor: f64,
    est: f64,
    window_frames: f64,
    window_start: Instant,
}

impl RateEstimator {
    fn new() -> Self {
        Self {
            anchor: 0.0,
            est: 0.0,
            window_frames: 0.0,
            window_start: Instant::now(),
        }
    }

    /// (Re)ancora no rate declarado — chamado quando o core troca / muda o timing.
    fn anchor_to(&mut self, declared: u32) {
        let d = declared.max(1) as f64;
        if (d - self.anchor).abs() > 1.0 {
            self.anchor = d;
            self.est = d;
            self.window_frames = 0.0;
            self.window_start = Instant::now();
        }
    }

    fn observe(&mut self, frames: usize) {
        self.window_frames += frames as f64;
        let elapsed = self.window_start.elapsed().as_secs_f64();
        if elapsed >= 0.5 && self.window_frames > 0.0 {
            let inst = self.window_frames / elapsed;
            self.est = self.est * 0.75 + inst * 0.25;
            let lo = self.anchor * 0.88;
            let hi = self.anchor * 1.12;
            self.est = self.est.clamp(lo, hi);
            self.window_frames = 0.0;
            self.window_start = Instant::now();
        }
    }

    fn rate(&self) -> f64 {
        if self.est > 1000.0 {
            self.est
        } else {
            self.anchor.max(1.0)
        }
    }
}

/// Resample linear estéreo com razão variável, mantendo estado entre chamadas.
#[derive(Default)]
struct Resampler {
    input: Vec<[f32; 2]>,
    pos: f64,
}

impl Resampler {
    fn process(&mut self, new: &[[f32; 2]], ratio: f64, out: &mut Vec<f32>) {
        self.input.extend_from_slice(new);
        let step = 1.0 / ratio.max(1e-6);
        while self.pos + 1.0 < self.input.len() as f64 {
            let i = self.pos.floor() as usize;
            let frac = (self.pos - i as f64) as f32;
            let a = self.input[i];
            let b = self.input[i + 1];
            out.push(a[0] + (b[0] - a[0]) * frac);
            out.push(a[1] + (b[1] - a[1]) * frac);
            self.pos += step;
        }
        let consumed = (self.pos.floor() as usize).min(self.input.len());
        if consumed > 0 {
            self.input.drain(..consumed);
            self.pos -= consumed as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_upsamples_2x() {
        let mut r = Resampler::default();
        let input: Vec<[f32; 2]> = (0..10).map(|n| [n as f32, -(n as f32)]).collect();
        let mut out = Vec::new();
        // ratio 2.0 -> ~2 frames de saída por frame de entrada
        r.process(&input, 2.0, &mut out);
        let out_frames = out.len() / 2;
        assert!(
            (out_frames as i32 - 18).abs() <= 2,
            "esperava ~18 frames, veio {out_frames}"
        );
        // canais preservados (esquerda positiva, direita negativa)
        assert!(out[0] >= 0.0 && out[1] <= 0.0);
    }

    #[test]
    fn resampler_keeps_state_across_calls() {
        let mut r = Resampler::default();
        let mut out = Vec::new();
        for _ in 0..5 {
            let chunk: Vec<[f32; 2]> = vec![[1.0, 1.0]; 4];
            r.process(&chunk, 1.0, &mut out);
        }
        // ratio 1.0, 20 frames de entrada -> ~19 de saída (perde ~1 de borda)
        let frames = out.len() / 2;
        assert!((17..=20).contains(&frames), "{frames}");
    }
}
