//! `audio-desktop`: `domain::audio::AudioSink` via `cpal`, com Dynamic Rate
//! Control (`rate_control` — lógica pura testável).
//!
//! Construção: a `cpal::Stream` é `!Send`, então `CpalAudioSink` é criado
//! **dentro** da thread do core (`emu-session` recebe uma factory).

pub mod mic;
mod rate_control;
mod sink;
pub mod tap;

pub use rate_control::RateControl;
pub use sink::{AudioError, CpalAudioSink};
