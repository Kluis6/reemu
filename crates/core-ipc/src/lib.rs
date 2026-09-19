//! Protocolo + transporte entre `emu-session` (pai) e `reemu-core-host`
//! (processo filho, um por core carregado — descartável a cada troca).
//!
//! Dois transportes, mesma API pública (`Channel`, `FrameRing`, `SLOTS`):
//! Unix usa socketpair `SOCK_SEQPACKET` + `SCM_RIGHTS`/`memfd_create`
//! (`transport.rs`/`shm_ring.rs`); Windows usa pipe nomeado + memória
//! compartilhada nomeada (`transport_win.rs`/`shm_ring_win.rs`), já que não
//! há equivalente a `SCM_RIGHTS` em pipe nomeado — ver os comentários de
//! cada módulo Windows pra como isso foi contornado.

mod message;

#[cfg(unix)]
mod shm_ring;
#[cfg(unix)]
mod transport;

#[cfg(windows)]
mod shm_ring_win;
#[cfg(windows)]
mod transport_win;

pub use message::{FrameKind, HwPlaneMeta, PortInput, ToChild, ToParent};

#[cfg(unix)]
pub use shm_ring::{FrameRing, SLOTS};
#[cfg(unix)]
pub use transport::Channel;

#[cfg(windows)]
pub use shm_ring_win::{FrameRing, SLOTS};
#[cfg(windows)]
pub use transport_win::{Channel, ChannelArg};

/// Handle "de passagem" que pode vir junto de um `recv` — no Unix é um fd
/// (`SCM_RIGHTS`). No Windows não existe (pipe nomeado não passa handle
/// inline), então é um enum vazio: nunca é construído, o `Vec` vem sempre
/// vazio, e o tipo continua `Send` (um `RawHandle` não seria — o
/// `InboundEvent` do `emu-session` cruza threads).
#[cfg(unix)]
pub type InlineHandle = rustix::fd::OwnedFd;
#[cfg(windows)]
pub enum InlineHandle {}
