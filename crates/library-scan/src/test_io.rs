//! Leitor que conta os bytes lidos, pros testes do NFR-5 (`serial`,
//! `identity`): prova quanto de um arquivo cada caminho lê.

use std::io::{Read, Result, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Soma em `read` todo byte que entrega.
pub(crate) struct Counting<R> {
    pub inner: R,
    pub read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.inner.seek(pos)
    }
}
