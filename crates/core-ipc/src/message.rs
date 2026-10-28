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
        /// Modificadores DRM que o Vulkan do pai importa (negociação do
        /// `VK_EXT_image_drm_format_modifier`) — o GBM do interop GL aloca
        /// só com eles. Vazio = pai sem Vulkan / não informou.
        #[serde(default)]
        dmabuf_modifiers: Vec<u64>,
        /// Identidade e extensões do Vulkan do pai pro interop GL → Vulkan
        /// no Windows (B1: o filho só detecta e registra). `None` = pai sem
        /// Vulkan / não informou.
        #[serde(default)]
        // `Box`: sem ele a variante `Load` fica grande demais perto das
        // outras (clippy::large_enum_variant).
        vk_interop: Option<Box<domain::gpu_interop::VkInteropId>>,
        /// C3: core Vulkan roda NESTE processo filho (device próprio), não
        /// in-process no pai — ver `DesktopCoreLoader::vulkan_in_child`.
        #[serde(default)]
        vulkan_in_child: bool,
    },
    SetPaused(bool),
    SaveState,
    RestoreState(Vec<u8>),
    /// Lista de cheats do jogo (doc 17): o filho troca a do core.
    SetCheats(Vec<domain::cheats::CheatCode>),
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
    /// Período do monitor medido pelo pai, mandado uma vez por segundo
    /// (tarefa A4 do TASKS). Se bater com o fps do core, o filho casa o
    /// ritmo do `Pacer` com ele (`RateMatch`).
    DisplayPeriod {
        period_ns: u64,
    },
    /// Resposta ao `ToParent::InteropRequest` (B2, Windows): o anel de
    /// imagens Vulkan exportáveis, com os handles NT JÁ duplicados pro
    /// processo do filho (o número vale lá). O filho importa no GL (B3) e
    /// fecha os handles.
    InteropImages {
        width: u32,
        height: u32,
        /// Alocação dedicada (`VkMemoryDedicatedAllocateInfo`): o GL pede
        /// `DEDICATED_MEMORY_OBJECT_EXT` no import.
        dedicated: bool,
        images: Vec<InteropImage>,
        /// `VkImageCreateFlags`/`VkImageUsageFlags` com que o app criou as
        /// imagens: um filho Vulkan recria a `VkImage` com os mesmos
        /// parâmetros pra importar a memória (C3b).
        #[serde(default)]
        image_flags: u32,
        #[serde(default)]
        image_usage: u32,
    },
    /// B5 (Windows): o app terminou de ler o slot `slot` do anel e já
    /// submeteu o `free` dele — o filho pode desenhar nele de novo (e espera
    /// esse `free` no GL antes). Sem este aviso, o GL esperaria um semáforo
    /// sem sinal enviado, o que a spec deixa indefinido.
    InteropSlotFree {
        slot: u32,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PortInput {
    /// Bit N = `libretro_joypad_id` do botão N pressionado.
    pub joypad_mask: u16,
    /// `[esquerdo, direito]`, cada um `(x, y)` em `[-0x8000, 0x7fff]`.
    pub sticks: [(i16, i16); 2],
    /// Pressão de `(L2, R2)` em `[0, 0x7fff]` (0 = solto ou sem analógico).
    pub triggers: (u16, u16),
}

/// Filho → pai.
#[derive(Debug, Serialize, Deserialize)]
pub enum ToParent {
    /// Resposta do `Load`. O fd do anel de frame (memfd) viaja junto, fora de
    /// banda, só quando `Ok`.
    Loaded(Result<SystemAvInfo, String>),
    /// Quirks de serialização do core (`domain::core_loader::serialization_quirk`),
    /// mandados logo antes do `Loaded`.
    SerializationQuirks(u64),
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
    /// Confirmação do `SetPaused`, mandada DEPOIS de aplicar. Tudo que o
    /// filho mandou antes (um `FrameReady` do frame que estava rodando) chega
    /// antes dela no canal — o pai espera por ela, então quando o
    /// `set_paused(true)` retorna não chega mais frame nenhum.
    PausedAck,
    /// Erro fatal que não impede o processo de seguir vivo (loga no pai).
    Warn(String),
    /// O contexto GL do filho é compatível com o Vulkan do pai e o interop
    /// está ligado (padrão; `REEMU_GL_INTEROP=0` desliga): o filho quer o anel de imagens
    /// (B2, Windows) do tamanho máximo do core.
    InteropRequest {
        max_w: u32,
        max_h: u32,
        /// C3b: quem vai escrever no anel é um core Vulkan no filho (não o
        /// GL) — o app faz o *acquire* de `VK_QUEUE_FAMILY_EXTERNAL` antes de
        /// amostrar.
        #[serde(default)]
        vulkan: bool,
    },
    /// O core leu o analógico pela 1ª vez (`RETRO_DEVICE_ANALOG`). Mandado
    /// uma vez: a partir daí o pai para de dobrar o stick esquerdo como
    /// d-pad (senão mexer o stick aperta/solta as setas junto).
    AnalogUsed,
    /// O core trocou uma opção dele (`RETRO_ENVIRONMENT_SET_VARIABLE`) — o
    /// pai guarda, pra valer nas próximas sessões.
    CoreOptionSet {
        key: String,
        value: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FrameKind {
    Software {
        pitch: u32,
        format: SoftwarePixelFormat,
    },
    /// Frame de HW render já importado como `dma_buf`. `plane` vem junto (+ o
    /// fd fora de banda) só na 1ª vez que este `slot` aparece — depois o pai
    /// já tem a textura importada e cacheada. No Windows (B5), `slot` é o
    /// índice do anel do interop (`InteropImages`), sem plano nem fence: o
    /// `done` do slot faz o papel da fence.
    Hardware {
        flip_y: bool,
        plane: Option<HwPlaneMeta>,
        /// Uma fence de fim de render (`sync_file`) veio junto, fora de
        /// banda, DEPOIS do fd do plano (se houver). O consumidor espera por
        /// ela antes de amostrar (no lugar do `glFinish` do produtor).
        #[serde(default)]
        sync: bool,
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

/// Uma imagem do anel do interop GL → Vulkan (ver `ToChild::InteropImages`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteropImage {
    /// Handle NT da memória, já válido no processo do filho.
    pub handle: u64,
    /// Tamanho da alocação (o `size` do `glImportMemoryWin32HandleEXT`).
    pub alloc_size: u64,
    /// Handle NT do semáforo que o GL sinaliza quando termina de desenhar
    /// nesta imagem (o app espera antes de amostrar) — B4.
    pub done_sem: u64,
    /// Handle NT do semáforo que o app sinaliza quando termina de ler esta
    /// imagem (o GL espera antes de desenhar de novo) — B4.
    pub free_sem: u64,
    /// `memoryTypeIndex` da alocação: o import Vulkan (C3b) tem que repetir
    /// (VUID-VkMemoryAllocateInfo-allocationSize-01743).
    #[serde(default)]
    pub memory_type: u32,
}
