//! Anel de shared memory pro caminho de frame software — versão Windows.
//! Mesma ideia do `shm_ring.rs` (Unix, `memfd_create`+`mmap`+`SCM_RIGHTS`),
//! mas pipe nomeado não passa handle inline (ver `transport_win.rs`), então
//! em vez de criar a memória sem nome e mandar o handle pelo canal, ela é
//! criada com um NOME (`CreateFileMappingW`) derivado do nome do próprio
//! pipe (`Channel::ipc_name()` + sufixo) — os dois lados já conhecem esse
//! nome (quem criou o pipe gerou o nome; o filho recebeu via argv), então
//! não precisa de nenhuma troca de handle pelo canal pra abrir o anel.
//!
//! Assinaturas verificadas na doc (docs.rs, `windows-sys` 0.61.2) antes de
//! escrever: `CreateFileMappingW`, `OpenFileMappingW`, `MapViewOfFile`
//! (devolve `MEMORY_MAPPED_VIEW_ADDRESS`, não ponteiro cru), `UnmapViewOfFile`,
//! `CloseHandle`.

use std::io;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    FILE_MAP_READ, PAGE_READWRITE,
};

pub const SLOTS: usize = 3;

fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Sufixo fixo — o nome do pipe já é único por sessão de core
/// (`Channel::pair`), então só precisa distinguir "o pipe" de "o anel"
/// derivados do mesmo nome base.
fn ring_name(ipc_name: &str) -> String {
    format!("Local\\{ipc_name}-ring")
}

pub struct FrameRing {
    mapping: HANDLE,
    ptr: *mut u8,
    slot_size: usize,
}

// SAFETY: mesmo protocolo documentado no `shm_ring.rs` Unix — o pai só lê um
// slot depois do `FrameReady` daquele índice, o filho só reescreve um slot
// depois de já ter mandado o `FrameReady` anterior dele.
unsafe impl Send for FrameRing {}
unsafe impl Sync for FrameRing {}

impl FrameRing {
    /// Cria o anel nomeado (lado do filho) — nome derivado do nome do pipe
    /// que `channel` já carrega (`Channel::ipc_name`), mapeado RW.
    pub fn create(channel: &super::Channel, slot_size: usize) -> io::Result<Self> {
        let total = (slot_size * SLOTS) as u32;
        let wname = wide_null(&ring_name(channel.ipc_name()));
        let mapping = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                0,
                total,
                wname.as_ptr(),
            )
        };
        if mapping.is_null() {
            return Err(io::Error::last_os_error());
        }
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, total as usize) };
        if view.Value.is_null() {
            let e = io::Error::last_os_error();
            unsafe { CloseHandle(mapping) };
            return Err(e);
        }
        Ok(Self {
            mapping,
            ptr: view.Value.cast(),
            slot_size,
        })
    }

    /// Abre o anel pelo nome derivado de `channel` (lado do pai) — RO, o pai
    /// nunca escreve nele.
    pub fn open(channel: &super::Channel, slot_size: usize) -> io::Result<Self> {
        let total = (slot_size * SLOTS) as u32;
        let wname = wide_null(&ring_name(channel.ipc_name()));
        let mapping = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, wname.as_ptr()) };
        if mapping.is_null() {
            return Err(io::Error::last_os_error());
        }
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, total as usize) };
        if view.Value.is_null() {
            let e = io::Error::last_os_error();
            unsafe { CloseHandle(mapping) };
            return Err(e);
        }
        Ok(Self {
            mapping,
            ptr: view.Value.cast(),
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
    /// pai).
    pub fn read_slot_to_vec(&self, idx: usize, len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        self.read_slot_into(idx, len, &mut out);
        out
    }

    /// Igual a `read_slot_to_vec`, mas reaproveitando `out` — com o buffer
    /// de um quadro anterior (mesmo tamanho) não há alocação nem zeragem.
    pub fn read_slot_into(&self, idx: usize, len: usize, out: &mut Vec<u8>) {
        let off = (idx % SLOTS) * self.slot_size;
        let n = len.min(self.slot_size);
        out.resize(n, 0);
        unsafe {
            std::ptr::copy_nonoverlapping(self.ptr.add(off), out.as_mut_ptr(), n);
        }
    }
}

impl Drop for FrameRing {
    fn drop(&mut self) {
        unsafe {
            use windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS;
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr.cast(),
            });
            CloseHandle(self.mapping);
        }
    }
}
