//! State machine de foco (GameFocused <-> MenuFocused) e ToastLayer.
//!
//! Decisões incorporadas aqui:
//! - Menu sempre sobreposto (nunca escondido durante MenuFocused)
//! - Entrar em MenuFocused pausa o core (emulação + áudio)
//! - Toast é uma camada independente: nunca captura input, nunca pausa,
//!   e não é um terceiro estado do enum de foco.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InputFocus {
    GameFocused,
    MenuFocused,
}

pub trait FocusManager: Send + Sync {
    fn current(&self) -> InputFocus;
    /// Alterna o estado; a implementação é responsável por também
    /// pausar/resumir o core e o AudioSink na transição correta.
    fn toggle(&mut self);
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToastVariant {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToastSource {
    System,
    Core,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToastItem {
    pub id: String,
    pub message: String,
    pub variant: ToastVariant,
    pub duration_ms: u32,
    pub source: ToastSource,
}

/// Publica toasts pro frontend (via evento Tauri, na implementação real).
/// Nunca interage com FocusManager — camada deliberadamente desacoplada.
pub trait ToastPublisher: Send + Sync {
    fn publish(&self, toast: ToastItem);
}
