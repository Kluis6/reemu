//! Protocolo + transporte entre `emu-session` (pai) e `reemu-core-host`
//! (processo filho, um por core carregado — descartável a cada troca).

mod message;
mod shm_ring;
mod transport;

pub use message::{FrameKind, HwPlaneMeta, PortInput, ToChild, ToParent};
pub use shm_ring::{FrameRing, SLOTS};
pub use transport::Channel;
