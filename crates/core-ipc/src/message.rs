//! Mensagens trocadas entre `emu-session` (pai) e `reemu-core-host` (filho).
//! Corpo serializado com `bincode`; fds (memfd do anel de frame, dma_buf de
//! interop) viajam FORA de banda como `SCM_RIGHTS` — nunca inline aqui.

use domain::core_loader::SystemAvInfo;
use domain::core_options::CoreOptionDefinition;
use domain::frame_source::{FrameMetadata, SoftwarePixelFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Pai → filho.
#[derive(Debug, Serialize, Deserialize)]
pub enum ToChild {
    /// Carrega o core + a ROM. Mesma convenção de `core_id`/`cores_dir` que
    /// `DesktopCoreLoader::resolve_path` já aceita (id bare resolvido contra
    /// `cores_dir`, ou caminho absoluto direto — os testes usam absoluto).
    /// `initial_option_values` são os valores salvos no DB (o core pede via
    /// `GET_VARIABLE` já durante o load).
    Load {
        core_id: String,
        rom_path: String,
        cores_dir: PathBuf,
        system_dir: PathBuf,
        save_dir: PathBuf,
        initial_option_values: HashMap<String, String>,
        /// Bytes da `.srm` do jogo, se existir — o pai lê o arquivo (ele é
        /// quem sabe o `save_dir`/convenção de nome) e manda junto pra o
        /// filho restaurar ANTES do 1º frame, mesma garantia de ordem que
        /// existia quando os dois lados eram o mesmo processo.
        initial_save_ram: Option<Vec<u8>>,
    },
    SetPaused(bool),
    SaveState,
    RestoreState(Vec<u8>),
    /// Save RAM (battery) — lida periodicamente pelo pai pra flush em disco.
    GetSaveRam,
    SetSaveRam(Vec<u8>),
    /// Snapshot completo do RetroPad (todas as portas) — enviado toda vez que
    /// o pai reamostra o input (~60Hz), não diffado (mensagem é minúscula).
    Input {
        ports: [PortInput; 4],
    },
    SetCoreOption {
        key: String,
        value: String,
    },
    GetCoreOptions,
    Shutdown,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PortInput {
    /// Bit N = `libretro_joypad_id` do botão N pressionado.
    pub joypad_mask: u16,
    /// `[esquerdo, direito]`, cada um `(x, y)` em `[-0x8000, 0x7fff]`.
    pub sticks: [(i16, i16); 2],
}

/// Filho → pai.
#[derive(Debug, Serialize, Deserialize)]
pub enum ToParent {
    /// Resposta do `Load`. O fd do anel de frame (memfd) viaja junto, fora de
    /// banda, só quando `Ok`.
    Loaded(Result<SystemAvInfo, String>),
    FrameReady {
        slot: u32,
        meta: FrameMetadata,
        kind: FrameKind,
    },
    AudioBatch {
        samples: Vec<i16>,
        sample_rate: u32,
    },
    /// O core mudou fps/sample_rate em runtime (`SET_SYSTEM_AV_INFO`).
    AvInfoChanged {
        fps: f64,
        sample_rate: f64,
    },
    SaveStateResult(Option<Vec<u8>>),
    RestoreStateResult(bool),
    SaveRamResult(Option<Vec<u8>>),
    /// Resultado de restaurar a `.srm` no load (`Some` só se havia arquivo).
    SaveRamRestored(Option<bool>),
    CoreOptionsSnapshot {
        schema: Vec<CoreOptionDefinition>,
        values: HashMap<String, String>,
    },
    SetCoreOptionResult(bool),
    /// Erro fatal que não impede o processo de seguir vivo (loga no pai).
    Warn(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FrameKind {
    Software {
        pitch: u32,
        format: SoftwarePixelFormat,
    },
    /// Frame de HW render já importado como `dma_buf`. `plane` vem junto (+ o
    /// fd fora de banda) só na 1ª vez que este `slot` aparece — depois o pai
    /// já tem a textura importada e cacheada.
    Hardware {
        flip_y: bool,
        plane: Option<HwPlaneMeta>,
    },
}

/// Espelha `domain::frame_source::DmabufPlaneInfo` sem o `fd` (que viaja fora
/// de banda via `SCM_RIGHTS`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HwPlaneMeta {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub offset: u32,
    pub modifier: u64,
    pub fourcc: u32,
}
