//! Anel de shared memory (`memfd_create` + `mmap`) pro caminho de frame
//! software: o filho escreve os bytes crus no slot, manda só a metadata
//! (`FrameReady`) pelo canal — o pai lê direto da mesma memória (fd
//! compartilhado 1x via `SCM_RIGHTS` no `Loaded`). Evita levar o frame
//! inteiro pelo socket (tamanho variável, PS1/GBA em alta-res passariam do
//! limite de pacote confortável).
//!
//! N slots (não 1) pra não pisar no que o pai ainda não leu — mesma ideia do
//! `ReadbackRing` que já existe no caminho de vídeo (`gpu.rs`). Com 3 slots e
//! o pai copiando pra fora assim que recebe `FrameReady`, o risco de o filho
//! dar a volta no anel antes do pai terminar de ler é desprezível na prática
//! (mesmo trade-off já aceito ali).

use rustix::fd::{AsFd, BorrowedFd, OwnedFd};
use rustix::fs::{ftruncate, memfd_create, MemfdFlags};
use rustix::mm::{mmap, MapFlags, ProtFlags};
use std::io;

pub const SLOTS: usize = 3;

pub struct FrameRing {
    _fd: OwnedFd,
    ptr: *mut u8,
    slot_size: usize,
}

// SAFETY: os acessos a `ptr` são por índice de slot; o protocolo (o pai só lê
// um slot depois do `FrameReady` daquele índice, o filho só reescreve um slot
// depois de já ter mandado o `FrameReady` anterior dele) evita
// leitura/escrita concorrente do MESMO slot na prática — ver nota acima.
unsafe impl Send for FrameRing {}
unsafe impl Sync for FrameRing {}

impl FrameRing {
    /// Cria o anel (lado do filho): aloca `slot_size * SLOTS` bytes num
    /// memfd, mapeia RW. Chame `.fd()` depois pra mandar ao pai (1x, via
    /// `SCM_RIGHTS` — o `sendmsg` duplica no lado de quem recebe, não
    /// precisa dar `dup` aqui).
    pub fn create(slot_size: usize) -> io::Result<Self> {
        let fd = memfd_create("reemu-frame-ring", MemfdFlags::CLOEXEC).map_err(io::Error::from)?;
        let total = slot_size * SLOTS;
        ftruncate(&fd, total as u64).map_err(io::Error::from)?;
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                total,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                &fd,
                0,
            )
            .map_err(io::Error::from)?
        };
        Ok(Self {
            _fd: fd,
            ptr: ptr.cast(),
            slot_size,
        })
    }

    /// O fd do memfd (pra anexar via `SCM_RIGHTS` na resposta do `Loaded`).
    pub fn fd(&self) -> BorrowedFd<'_> {
        self._fd.as_fd()
    }

    /// Mapeia um anel recebido (lado do pai): RO, o pai nunca escreve nele.
    pub fn from_fd(fd: OwnedFd, slot_size: usize) -> io::Result<Self> {
        let total = slot_size * SLOTS;
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                total,
                ProtFlags::READ,
                MapFlags::SHARED,
                fd.as_fd(),
                0,
            )
            .map_err(io::Error::from)?
        };
        Ok(Self {
            _fd: fd,
            ptr: ptr.cast(),
            slot_size,
        })
    }

    pub fn slot_size(&self) -> usize {
        self.slot_size
    }

    /// Escreve `data` no slot `idx % SLOTS` (lado do filho). Trunca se
    /// `data` for maior que `slot_size` (não deveria acontecer — o slot é
    /// dimensionado por `max_width*max_height*4` no load).
    pub fn write_slot(&self, idx: usize, data: &[u8]) {
        let off = (idx % SLOTS) * self.slot_size;
        let n = data.len().min(self.slot_size);
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.add(off), n);
        }
    }

    /// Copia `len` bytes do slot `idx % SLOTS` pra um `Vec` novo (lado do
    /// pai) — a 1 cópia extra do caminho software em troca do processo
    /// descartável.
    pub fn read_slot_to_vec(&self, idx: usize, len: usize) -> Vec<u8> {
        let off = (idx % SLOTS) * self.slot_size;
        let n = len.min(self.slot_size);
        let mut out = vec![0u8; n];
        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr.add(off), out.as_mut_ptr(), n);
        }
        out
    }
}

impl Drop for FrameRing {
    fn drop(&mut self) {
        let total = self.slot_size * SLOTS;
        unsafe {
            let _ = rustix::mm::munmap(self.ptr.cast(), total);
        }
    }
}
