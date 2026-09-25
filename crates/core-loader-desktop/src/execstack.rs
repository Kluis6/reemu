//! Cores que pedem **pilha executável** (ex.: melonDS no buildbot).
//!
//! Desde a glibc 2.41 o `dlopen` recusa uma biblioteca cujo segmento
//! `PT_GNU_STACK` tem o bit `PF_X` ("cannot enable executable stack as
//! shared object requires"). A glibc 2.42 trouxe o modo de compatibilidade
//! `glibc.rtld.execstack=2`, que o manual descreve exatamente para programas
//! que carregam com `dlopen` módulos que exigem pilha executável (glibc NEWS
//! 2.41/2.42 e o manual, "Dynamic Linking Tunables"). Ele vale para o
//! processo inteiro e só na partida — então é ligado SÓ no `reemu-core-host`
//! que vai rodar um core desses, nunca no app.
//!
//! O cabeçalho é lido conforme o System V gABI (ELF64 little-endian, o
//! formato dos cores x86_64): `e_phoff` em 0x20, `e_phentsize` em 0x36,
//! `e_phnum` em 0x38; em cada program header, `p_type` no byte 0 e `p_flags`
//! no byte 4.

use std::path::{Path, PathBuf};

const PT_GNU_STACK: u32 = 0x6474_e551;
const PF_X: u32 = 0x1;

/// `true` se o ELF em `bytes` declara `PT_GNU_STACK` com `PF_X`.
fn elf_requests_exec_stack(bytes: &[u8]) -> bool {
    // \x7fELF, ELFCLASS64 (2), ELFDATA2LSB (1)
    if bytes.len() < 0x40 || &bytes[..4] != b"\x7fELF" || bytes[4] != 2 || bytes[5] != 1 {
        return false;
    }
    let u16_at = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]) as usize;
    let phoff = u64::from_le_bytes(bytes[0x20..0x28].try_into().unwrap()) as usize;
    let (phentsize, phnum) = (u16_at(0x36), u16_at(0x38));
    (0..phnum).any(|i| {
        let o = phoff + i * phentsize;
        let Some(h) = bytes.get(o..o + 8) else {
            return false;
        };
        let p_type = u32::from_le_bytes(h[0..4].try_into().unwrap());
        let p_flags = u32::from_le_bytes(h[4..8].try_into().unwrap());
        p_type == PT_GNU_STACK && p_flags & PF_X != 0
    })
}

/// Arquivo do core como o loader resolve (`<pasta>/<id>` ou
/// `<pasta>/<id>.<so|dll|dylib>`, ou o próprio id se for um caminho).
pub fn core_file(cores_dir: &Path, core_id: &str) -> Option<PathBuf> {
    crate::loader::resolve_core_file(cores_dir, core_id)
}

/// Variável de ambiente a pôr no processo que vai carregar este core, se
/// ele pedir pilha executável (Linux). Soma ao `GLIBC_TUNABLES` que já
/// existir. `None` = nada a fazer.
pub fn exec_stack_env(core_path: &Path) -> Option<(&'static str, String)> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    // Só o começo do arquivo: os program headers ficam logo após o ELF
    // header em qualquer .so gerado por ld/lld.
    let mut head = vec![0u8; 64 * 1024];
    let n = std::io::Read::read(&mut std::fs::File::open(core_path).ok()?, &mut head).ok()?;
    if !elf_requests_exec_stack(&head[..n]) {
        return None;
    }
    let tunable = "glibc.rtld.execstack=2";
    let value = match std::env::var("GLIBC_TUNABLES") {
        Ok(v) if !v.is_empty() => format!("{v}:{tunable}"),
        _ => tunable.to_string(),
    };
    Some(("GLIBC_TUNABLES", value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ELF64 LE mínimo com um program header.
    fn elf(p_type: u32, p_flags: u32) -> Vec<u8> {
        let mut b = vec![0u8; 0x40 + 56];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = 2;
        b[5] = 1;
        b[0x20..0x28].copy_from_slice(&0x40u64.to_le_bytes());
        b[0x36..0x38].copy_from_slice(&56u16.to_le_bytes());
        b[0x38..0x3a].copy_from_slice(&1u16.to_le_bytes());
        b[0x40..0x44].copy_from_slice(&p_type.to_le_bytes());
        b[0x44..0x48].copy_from_slice(&p_flags.to_le_bytes());
        b
    }

    #[test]
    fn detects_executable_gnu_stack() {
        assert!(elf_requests_exec_stack(&elf(PT_GNU_STACK, 0x6 | PF_X)));
        assert!(!elf_requests_exec_stack(&elf(PT_GNU_STACK, 0x6)));
        assert!(!elf_requests_exec_stack(&elf(1, PF_X))); // PT_LOAD
        assert!(!elf_requests_exec_stack(b"MZ nada de ELF aqui"));
    }

    #[test]
    fn test_core_does_not_request_it() {
        let p = Path::new(env!("REEMU_TESTCORE"));
        assert_eq!(exec_stack_env(p), None);
    }
}
