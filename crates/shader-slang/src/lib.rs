//! `shader-slang`: leitura e compilação do formato **slang** do RetroArch
//! (`.slangp` + fontes `.slang`).
//!
//! Módulo deliberadamente **isolado**: é código que parseia/compila conteúdo
//! não confiável vindo de fora do projeto (presets baixados). Nada de lógica
//! de shader de terceiros vaza pro resto do código — o shell consome só os
//! tipos daqui.
//!
//! - [`parse_slangp`] / [`parse_slangp_file`] — o `.slangp` (lista de passes).
//! - [`preprocess_file`] / [`preprocess_str`] — o `.slang` (`#include`, split
//!   de estágios, `#pragma parameter`).
//! - [`compile`] — GLSL Vulkan → WGSL via `naga` (cobre CRT/scanline de
//!   arquivo único; Mega Bezel completo ainda não — ver `compile.rs`).

mod compile;
mod preprocess;
mod slangp;

pub use compile::{
    compile, CompileError, CompiledSlang, UniformField, UniformFieldKind, UniformLayout,
};
pub use preprocess::{preprocess_file, preprocess_str, Parameter, SlangSource};
pub use slangp::{
    parse_slangp, parse_slangp_file, Pass, Preset, Scale, SlangError, TextureRef, WrapMode,
};
