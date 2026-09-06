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
    ReturnFlags, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::sync::Arc;

/// Teto de sanidade pro tamanho de UMA mensagem — não é o tamanho de nenhum
/// buffer (o `recv` aloca exatamente o que a mensagem pede, ver abaixo). Save
/// state de SNES já passa de 800 KB; N64/PSP passam de vários MB. O que limita
/// o envio de verdade é o `SO_SNDBUF` do socket (`SEQPACKET` = 1 datagrama por
/// `send`), levantado no `pair()`.
const MAX_MSG: usize = 32 * 1024 * 1024;
/// Buffer de socket pedido (o kernel dobra e depois clampa em
/// `net.core.{wmem,rmem}_max`). `SEQPACKET` recusa datagrama maior que isto.
const SOCK_BUF: usize = 16 * 1024 * 1024;
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
        // Buffer de socket o maior que o SO deixar — o padrão do Linux
        // (~208KB) trunca save state. `SEQPACKET` recusa datagrama maior que
        // o `SO_SNDBUF` efetivo, então isto é o teto real de tamanho de save
        // state que passa pelo canal inline (N64/PSP muito grandes ainda
        // ficariam de fora — nesse caso o send falha e o filho reporta).
        for fd in [&a, &b] {
            let _ = sockopt::set_socket_recv_buffer_size(fd, SOCK_BUF);
            let _ = sockopt::set_socket_send_buffer_size(fd, SOCK_BUF);
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
        net::sendmsg(self.fd(), &iov, &mut control, SendFlags::empty()).map_err(|e| {
            if e == rustix::io::Errno::MSGSIZE {
                io::Error::other(format!(
                    "mensagem IPC de {} bytes maior que o buffer do socket \
                     (save state grande demais pro canal inline)",
                    body.len()
                ))
            } else {
                io::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Bloqueia até a próxima mensagem. `Ok(None)` = o outro lado fechou o
    /// canal (o processo saiu) — encerra a leitura, não um erro.
    ///
    /// `SEQPACKET` trunca no silêncio um datagrama maior que o buffer passado
    /// (e descarta o resto), então primeiro fazemos um `recvmsg` com
    /// `MSG_PEEK | MSG_TRUNC` — que devolve o tamanho REAL do datagrama sem
    /// consumi-lo — e só então alocamos exatamente e lemos de verdade. Assim o
    /// buffer nunca é grande demais (áudio é ~4KB) nem pequeno demais (save
    /// state passa de 800KB).
    pub fn recv<T: DeserializeOwned>(&self) -> io::Result<Option<(T, Vec<OwnedFd>)>> {
        // 1. tamanho do próximo datagrama, sem consumir e sem tocar nos fds
        //    (buffer de controle vazio → SCM_RIGHTS fica na fila pro passo 2).
        let mut probe = [0u8; 64];
        let mut piov = [IoSliceMut::new(&mut probe)];
        let mut no_ctrl = RecvAncillaryBuffer::new(&mut []);
        let peek = net::recvmsg(
            self.fd(),
            &mut piov,
            &mut no_ctrl,
            RecvFlags::PEEK | RecvFlags::TRUNC,
        )
        .map_err(io::Error::from)?;
        let size = peek.bytes;
        if size == 0 {
            return Ok(None); // EOF — o outro lado fechou
        }
        // 2. leitura de verdade, buffer do tamanho exato — mas se a mensagem
        //    passa do teto, consome mesmo assim (truncando) pra não travar o
        //    canal e devolve erro (quem lê registra e segue).
        let capped = size.min(MAX_MSG);
        let mut buf = vec![0u8; capped];
        let mut iov = [IoSliceMut::new(&mut buf)];
        let mut space = [MaybeUninit::<u8>::uninit(); MAX_ANCILLARY];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let got = net::recvmsg(self.fd(), &mut iov, &mut control, RecvFlags::empty())
            .map_err(io::Error::from)?;
        if got.bytes == 0 {
            return Ok(None);
        }
        if size > MAX_MSG {
            return Err(io::Error::other(format!(
                "mensagem IPC de {size} bytes excede o teto de {MAX_MSG} — descartada"
            )));
        }
        if got.flags.contains(ReturnFlags::TRUNC) || got.bytes > buf.len() {
            return Err(io::Error::other(format!(
                "datagrama IPC truncado ({} de {size} bytes)",
                got.bytes
            )));
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::fd::AsFd;

    /// Datagrama grande (save state de SNES passa de 800KB) — o bug era o
    /// `recv` truncar em silêncio num buffer fixo de 512KB.
    #[test]
    fn roundtrips_a_message_bigger_than_the_old_512k_cap() {
        let (a, b) = Channel::pair().unwrap();
        let payload: Vec<u8> = (0..2_000_000u32).map(|i| i as u8).collect();
        let sent = payload.clone();
        let h = std::thread::spawn(move || {
            a.send::<Vec<u8>>(&sent, &[]).unwrap();
        });
        let (got, fds) = b.recv::<Vec<u8>>().unwrap().unwrap();
        h.join().unwrap();
        assert!(fds.is_empty());
        assert_eq!(got.len(), payload.len());
        assert_eq!(got, payload);
    }

    #[test]
    fn small_messages_and_eof() {
        let (a, b) = Channel::pair().unwrap();
        a.send::<(u32, String)>(&(7, "oi".into()), &[]).unwrap();
        let (got, _) = b.recv::<(u32, String)>().unwrap().unwrap();
        assert_eq!(got, (7, "oi".to_string()));
        drop(a);
        assert!(
            b.recv::<(u32, String)>().unwrap().is_none(),
            "EOF vira None"
        );
    }

    /// O `MSG_PEEK` do passo 1 não pode consumir os fds do `SCM_RIGHTS`.
    #[test]
    fn passes_fds_after_peeking_for_size() {
        let (a, b) = Channel::pair().unwrap();
        // dois fds quaisquer pra mandar via SCM_RIGHTS
        let (f1, f2) = net::socketpair(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let h = std::thread::spawn(move || {
            a.send::<u8>(&1, &[f1.as_fd(), f2.as_fd()]).unwrap();
        });
        let (got, fds) = b.recv::<u8>().unwrap().unwrap();
        h.join().unwrap();
        assert_eq!(got, 1);
        assert_eq!(fds.len(), 2, "os dois fds atravessaram o peek + recv");
    }
}
