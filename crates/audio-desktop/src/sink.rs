//! `CpalAudioSink`: `domain::audio::AudioSink` via `cpal`, com resample
//! linear de razão dinâmica (Dynamic Rate Control). Construído e usado na
//! thread que dirige o core — a `cpal::Stream` é `!Send`.

use crate::rate_control::RateControl;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use domain::audio::{AudioConfig, AudioSink};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

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
    /// DRC do `AudioConfig` — pra reconstruir ao trocar de dispositivo.
    drc_delta: f32,
    drc_enabled: bool,
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

        let stream = device
            .build_output_stream(
                stream_config,
                move |data: &mut [f32], _: &_| {
                    let got = consumer.pop_slice(data);
                    data[got..].fill(0.0); // underrun -> silêncio
                },
                move |err| log::warn!("stream de áudio: {err}"),
                None,
            )
            .map_err(|e| AudioError::Cpal(e.to_string()))?;
        stream.play().map_err(|e| AudioError::Cpal(e.to_string()))?;

        Ok(Self {
            stream,
            producer,
            capacity,
            device_rate,
            rate_control: RateControl::new(config.rate_control_delta, config.rate_control_enabled),
            resampler: Resampler::default(),
            scratch: Vec::new(),
            drc_delta: config.rate_control_delta,
            drc_enabled: config.rate_control_enabled,
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

        let factor = self.rate_control.factor(self.fill()) as f64;
        let ratio = (self.device_rate as f64 / core_sample_rate.max(1) as f64) * factor;

        self.scratch.clear();
        self.resampler.process(&in_frames, ratio, &mut self.scratch);
        // push_slice descarta o que não couber (overrun -> o DRC corrige)
        self.producer.push_slice(&self.scratch);
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
