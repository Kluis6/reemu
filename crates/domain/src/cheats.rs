//! Cheats por código (docs/ai-context/17-cheats-rewind.md, fase 1). Quem
//! entende o código é o core: o frontend só entrega o texto pelo
//! `retro_cheat_set(index, enabled, code)` (libretro.h). Game Genie, Action
//! Replay, GameShark, CWCheat… conforme o core.

use serde::{Deserialize, Serialize};

/// O que vai pro core: o código e se está ligado. A posição na lista é o
/// `index` do `retro_cheat_set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheatCode {
    pub code: String,
    pub enabled: bool,
}

/// Cheat salvo de um jogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cheat {
    pub id: i64,
    pub rom_id: String,
    pub description: String,
    pub code: String,
    pub enabled: bool,
    /// Ordem na lista (e o `index` do `retro_cheat_set`).
    pub position: i64,
}

impl Cheat {
    pub fn to_code(&self) -> CheatCode {
        CheatCode {
            code: self.code.clone(),
            enabled: self.enabled,
        }
    }
}
