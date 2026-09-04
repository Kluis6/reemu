//! Transporte: um par de sockets Unix `SOCK_SEQPACKET` conectados
//! (`socketpair`, sem endereço/bind/listen). Cada `send`/`recv` é 1 syscall =
//! 1 mensagem (o kernel preserva limite de pacote em `SEQPACKET`, então fds
//! passados via `SCM_RIGHTS` ficam sem ambiguidade — nunca cortados entre
//! duas chamadas como podia acontecer com `SOCK_STREAM`).
//!
//! Referências consultadas na fonte vendorizada do `rustix` 1.1.4
//! (`~/.cargo/registry/src/.../rustix-1.1.4/src/net/{send_recv,socketpair}.rs`,
//! `src/io/fcntl.rs`) — ver `docs/ai-context/REFERENCES.md`.

use rustix::fd::{AsFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use rustix::io::{fcntl_getfd, fcntl_setfd, FdFlags};
use rustix::net::{
    self, sockopt, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::sync::Arc;

/// Maior mensagem que cabe num pacote. `AudioBatch` é a mais gorda (poucos KB
/// por `retro_run`); generoso o bastante pra nunca truncar.
const MAX_MSG: usize = 512 * 1024;
/// Espaço de controle: no máximo 1 fd por mensagem hoje (memfd do anel OU o
/// dma_buf de um slot de interop), nunca os dois juntos.
const MAX_ANCILLARY: usize = 128;

/// Barato de clonar (`Arc` por dentro) — o lado que lê roda numa thread
/// dedicada, o lado que manda roda em outra; `send`/`recv` em threads
/// diferentes sobre o mesmo socket é seguro (syscalls independentes, sem
/// estado mutável compartilhado do nosso lado).
#[derive(Clone)]
pub struct Channel(Arc<OwnedFd>);

impl Channel {
    /// Par conectado, ambos os lados com `CLOEXEC` (não vazam pra outros
    /// processos que este venha a `spawn`ar). Quem for entregar um lado pro
    /// processo filho chama `clear_cloexec()` nele antes do `spawn`.
    pub fn pair() -> io::Result<(Channel, Channel)> {
        let (a, b) = net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .map_err(io::Error::from)?;
        // Buffer de socket generoso — o padrão do Linux (~208KB) é curto pro
        // pior caso de `AudioBatch`.
        for fd in [&a, &b] {
            let _ = sockopt::set_socket_recv_buffer_size(fd, MAX_MSG);
            let _ = sockopt::set_socket_send_buffer_size(fd, MAX_MSG);
        }
        Ok((Channel(Arc::new(a)), Channel(Arc::new(b))))
    }

    /// Limpa `O_CLOEXEC` — o fd sobrevive ao `exec` do processo filho (que o
    /// herda no MESMO número, repassado via argv).
    pub fn clear_cloexec(&self) -> io::Result<()> {
        fcntl_setfd(self.fd(), FdFlags::empty()).map_err(io::Error::from)
    }

    pub fn as_raw_fd(&self) -> RawFd {
        use rustix::fd::AsRawFd;
        self.0.as_raw_fd()
    }

    /// Reconstrói o canal a partir de um fd herdado do pai (mesmo número em
    /// que o pai o deixou, sem `O_CLOEXEC`).
    ///
    /// # Safety
    /// `fd` precisa ser um fd de socket válido, aberto, e cuja posse ninguém
    /// mais reivindica neste processo (chamado 1x no `main` do filho).
    pub unsafe fn from_inherited_fd(fd: RawFd) -> Channel {
        Channel(Arc::new(OwnedFd::from_raw_fd(fd)))
    }

    fn fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }

    /// Confirma que o `CLOEXEC` está de fato limpo (diagnóstico — chame antes
    /// de montar o argv do filho).
    #[cfg(debug_assertions)]
    pub fn assert_inheritable(&self) {
        let flags = fcntl_getfd(self.fd()).unwrap_or(FdFlags::CLOEXEC);
        debug_assert!(
            !flags.contains(FdFlags::CLOEXEC),
            "fd do canal ainda tem CLOEXEC — o filho não vai herdar"
        );
    }

    pub fn send<T: Serialize>(&self, msg: &T, fds: &[BorrowedFd<'_>]) -> io::Result<()> {
        let body = bincode::serde::encode_to_vec(msg, bincode::config::standard())
            .map_err(|e| io::Error::other(format!("bincode encode: {e}")))?;
        let iov = [IoSlice::new(&body)];
        let mut space = [MaybeUninit::<u8>::uninit(); MAX_ANCILLARY];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() {
            control.push(SendAncillaryMessage::ScmRights(fds));
        }
        net::sendmsg(self.fd(), &iov, &mut control, SendFlags::empty()).map_err(io::Error::from)?;
        Ok(())
    }

    /// Bloqueia até a próxima mensagem. `Ok(None)` = o outro lado fechou o
    /// canal (o processo saiu) — encerra a leitura, não um erro.
    pub fn recv<T: DeserializeOwned>(&self) -> io::Result<Option<(T, Vec<OwnedFd>)>> {
        let mut buf = vec![0u8; MAX_MSG];
        let mut iov = [IoSliceMut::new(&mut buf)];
        let mut space = [MaybeUninit::<u8>::uninit(); MAX_ANCILLARY];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let got = net::recvmsg(self.fd(), &mut iov, &mut control, RecvFlags::empty())
            .map_err(io::Error::from)?;
        if got.bytes == 0 {
            return Ok(None); // EOF — o outro lado fechou
        }
        let mut owned_fds = Vec::new();
        for msg in control.drain() {
            if let RecvAncillaryMessage::ScmRights(iter) = msg {
                owned_fds.extend(iter);
            }
        }
        let (value, _) = bincode::serde::decode_from_slice::<T, _>(
            &buf[..got.bytes],
            bincode::config::standard(),
        )
        .map_err(|e| io::Error::other(format!("bincode decode: {e}")))?;
        Ok(Some((value, owned_fds)))
    }
}
