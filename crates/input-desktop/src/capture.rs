//! Modo de captura de binding (etapa 05, fluxo de UI).
//!
//! Um flag global e simples: enquanto ligado, o `GamepadPoller` e o comando
//! `input_key` do shell param de rotear o evento pro jogo/hotkey e em vez
//! disso o entregam pro frontend (`emit("raw-input-captured", ...)`). A
//! janela hold+press (~300ms) que agrupa a combinação é responsabilidade do
//! frontend (estado transitório em Zustand) — aqui só decidimos "roteia
//! normal" vs "manda pra captura".

use std::sync::atomic::{AtomicBool, Ordering};

static CAPTURING: AtomicBool = AtomicBool::new(false);

/// Entra em modo de captura. Idempotente.
pub fn begin() {
    CAPTURING.store(true, Ordering::SeqCst);
}

/// Sai do modo de captura. Idempotente.
pub fn end() {
    CAPTURING.store(false, Ordering::SeqCst);
}

pub fn is_capturing() -> bool {
    CAPTURING.load(Ordering::SeqCst)
}
