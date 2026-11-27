//! Ponte segura sobre `rc_libretro_memory_regions_t` (`src/rc_libretro.h`).
//!
//! Mapeia os endereços virtuais que o RetroAchievements consulta para os
//! buffers reais do core libretro, usando `RETRO_ENVIRONMENT_SET_MEMORY_MAPS`
//! quando disponível e caindo para `RETRO_MEMORY_SYSTEM_RAM` + `SAVE_RAM`
//! quando o core não declara mapa.

use crate::ffi;
use std::cell::RefCell;
use std::ffi::CString;

/// Descritor de região de memória vindo do `RETRO_ENVIRONMENT_SET_MEMORY_MAPS`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryDescriptorInput {
    pub flags: u64,
    pub ptr: usize,
    pub offset: usize,
    pub start: usize,
    pub select: usize,
    pub disconnect: usize,
    pub len: usize,
    pub addrspace: Option<String>,
}

/// Regiões de memória resolvidas para leitura a cada quadro pelo `rc_client`.
pub struct LibretroMemory {
    raw: ffi::rc_libretro_memory_regions_t,
    has_valid_region: bool,
}

type CoreMemLookup<'a> = &'a mut dyn FnMut(u32) -> (*mut u8, usize);
type ActiveCoreMemSlot = (*mut (), unsafe fn(*mut (), u32) -> (*mut u8, usize));

thread_local! {
    static ACTIVE_CORE_MEM_LOOKUP: RefCell<Option<ActiveCoreMemSlot>> =
        const { RefCell::new(None) };
}

unsafe extern "C" fn get_core_memory_trampoline(
    id: u32,
    info: *mut ffi::rc_libretro_core_memory_info_t,
) {
    if info.is_null() {
        return;
    }
    let (data, size) = ACTIVE_CORE_MEM_LOOKUP.with(|slot| {
        if let Some((ctx, func)) = *slot.borrow() {
            unsafe { func(ctx, id) }
        } else {
            (std::ptr::null_mut(), 0)
        }
    });
    (*info).data = data;
    (*info).size = size;
}

unsafe fn call_closure(ctx: *mut (), id: u32) -> (*mut u8, usize) {
    let f = &mut *(ctx as *mut CoreMemLookup<'_>);
    f(id)
}

impl LibretroMemory {
    /// Inicializa o mapa de memória para `console_id`.
    ///
    /// # Safety
    /// Os ponteiros de memória do host fornecidos em `descriptors` e por
    /// `core_memory` devem permanecer válidos pelo tempo de vida deste
    /// `LibretroMemory` (ou seja, enquanto o core estiver carregado).
    pub unsafe fn init(
        console_id: u32,
        descriptors: &[MemoryDescriptorInput],
        mut core_memory: impl FnMut(u32) -> (*mut u8, usize),
    ) -> Self {
        let mut raw = ffi::rc_libretro_memory_regions_t::default();

        let mut addrspaces: Vec<Option<CString>> = Vec::with_capacity(descriptors.len());
        let mut raw_descs: Vec<ffi::retro_memory_descriptor> =
            Vec::with_capacity(descriptors.len());

        for d in descriptors {
            let cs = d.addrspace.as_deref().and_then(|s| CString::new(s).ok());
            let addrspace_ptr = cs.as_ref().map_or(std::ptr::null(), |c| c.as_ptr());
            addrspaces.push(cs);
            raw_descs.push(ffi::retro_memory_descriptor {
                flags: d.flags,
                ptr: d.ptr as *mut std::os::raw::c_void,
                offset: d.offset,
                start: d.start,
                select: d.select,
                disconnect: d.disconnect,
                len: d.len,
                addrspace: addrspace_ptr,
            });
        }

        let mmap = ffi::retro_memory_map {
            descriptors: if raw_descs.is_empty() {
                std::ptr::null()
            } else {
                raw_descs.as_ptr()
            },
            num_descriptors: raw_descs.len() as std::os::raw::c_uint,
        };
        let mmap_ptr = if raw_descs.is_empty() {
            std::ptr::null()
        } else {
            &mmap as *const ffi::retro_memory_map
        };

        let mut trait_obj: CoreMemLookup<'_> = &mut core_memory;
        let ctx_ptr = (&mut trait_obj as *mut CoreMemLookup<'_>).cast::<()>();

        struct ResetGuard;
        impl Drop for ResetGuard {
            fn drop(&mut self) {
                ACTIVE_CORE_MEM_LOOKUP.with(|slot| {
                    *slot.borrow_mut() = None;
                });
            }
        }

        ACTIVE_CORE_MEM_LOOKUP.with(|slot| {
            *slot.borrow_mut() = Some((ctx_ptr, call_closure));
        });
        let _guard = ResetGuard;

        let has_valid_region = ffi::rc_libretro_memory_init(
            &mut raw,
            mmap_ptr,
            Some(get_core_memory_trampoline),
            console_id,
        ) != 0;
        drop(addrspaces);

        Self {
            raw,
            has_valid_region,
        }
    }

    pub fn has_valid_region(&self) -> bool {
        self.has_valid_region
    }

    pub fn total_size(&self) -> usize {
        self.raw.total_size
    }

    pub fn region_count(&self) -> u32 {
        self.raw.count
    }

    /// Lê até `out.len()` bytes a partir do endereço virtual `address` do RA.
    pub fn read(&self, address: u32, out: &mut [u8]) -> u32 {
        if out.is_empty() || self.raw.count == 0 {
            return 0;
        }
        let len = u32::try_from(out.len()).unwrap_or(u32::MAX);
        unsafe { ffi::rc_libretro_memory_read(&self.raw, address, out.as_mut_ptr(), len) }
    }
}

impl Drop for LibretroMemory {
    fn drop(&mut self) {
        unsafe {
            ffi::rc_libretro_memory_destroy(&mut self.raw);
        }
    }
}
