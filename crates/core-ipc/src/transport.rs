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
use rustix::fs::{ftruncate, memfd_create, MemfdFlags};
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

/// Teto de sanidade pra UMA mensagem (inline OU memfd). Save state de N64
/// (parallel_n64) passa de 16 MB; PSP pode passar de 30 MB.
const MAX_MSG: usize = 128 * 1024 * 1024;
/// Corpo bincode acima disto NÃO cabe confortável num datagrama `SEQPACKET`
/// (o kernel recusa acima do `SO_SNDBUF` efetivo, ~8 MB nesta máquina) —
/// então vai por **memfd** anexado via `SCM_RIGHTS`: o `send` escreve o corpo
/// num `memfd_create`, manda só um marcador + o fd; o `recv` lê o corpo de
/// volta da mesma memória. Save state de SNES (~800 KB) e menores continuam
/// inline (1 syscall, sem memfd).
const INLINE_MAX: usize = 3 * 1024 * 1024;
/// iov do datagrama quando o corpo real está no memfd anexado. Não colide com
/// bincode (nossos enums começam com um byte de variante pequeno).
const MEMFD_MARKER: &[u8] = b"\x00REEMU-IPC-MEMFD\x00";
/// Buffer fixo do `recv` — cobre qualquer mensagem inline com folga
/// (`INLINE_MAX` < isto). Reusado por thread.
const INLINE_CAP: usize = 4 * 1024 * 1024;

thread_local! {
    static RECV_BUF: std::cell::RefCell<Vec<u8>> =
        std::cell::RefCell::new(vec![0u8; INLINE_CAP]);
}
/// Buffer de socket pedido (o kernel dobra e depois clampa em
/// `net.core.{wmem,rmem}_max`).
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

/// `true` se o erro significa "a outra ponta sumiu" — trata como EOF, não erro.
fn peer_gone(e: rustix::io::Errno) -> bool {
    use rustix::io::Errno;
    matches!(
        e,
        Errno::CONNRESET | Errno::PIPE | Errno::CONNABORTED | Errno::NOTCONN
    )
}

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
        // Corpo grande sem fds próprios (save state de N64/PSP) → memfd. As
        // mensagens que carregam fd (`Loaded`, `FrameReady`) são pequenas.
        if fds.is_empty() && body.len() > INLINE_MAX {
            return self.send_via_memfd(&body);
        }
        let iov = [IoSlice::new(&body)];
        let mut space = [MaybeUninit::<u8>::uninit(); MAX_ANCILLARY];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() {
            control.push(SendAncillaryMessage::ScmRights(fds));
        }
        net::sendmsg(self.fd(), &iov, &mut control, SendFlags::empty()).map_err(|e| {
            if e == rustix::io::Errno::MSGSIZE {
                io::Error::other(format!(
                    "mensagem IPC de {} bytes não coube no datagrama SEQPACKET",
                    body.len()
                ))
            } else {
                io::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Escreve o corpo num `memfd` e manda só o marcador + o fd (`SCM_RIGHTS`).
    fn send_via_memfd(&self, body: &[u8]) -> io::Result<()> {
        let memfd = memfd_create("reemu-ipc-blob", MemfdFlags::CLOEXEC).map_err(io::Error::from)?;
        ftruncate(&memfd, body.len() as u64).map_err(io::Error::from)?;
        let mut off = 0u64;
        while (off as usize) < body.len() {
            let n =
                rustix::io::pwrite(&memfd, &body[off as usize..], off).map_err(io::Error::from)?;
            if n == 0 {
                return Err(io::Error::other("pwrite no memfd devolveu 0"));
            }
            off += n as u64;
        }
        let iov = [IoSlice::new(MEMFD_MARKER)];
        let mut space = [MaybeUninit::<u8>::uninit(); MAX_ANCILLARY];
        let mut control = SendAncillaryBuffer::new(&mut space);
        let mfd = [memfd.as_fd()];
        control.push(SendAncillaryMessage::ScmRights(&mfd));
        net::sendmsg(self.fd(), &iov, &mut control, SendFlags::empty()).map_err(io::Error::from)?;
        Ok(())
    }

    /// Bloqueia até a próxima mensagem. `Ok(None)` = o outro lado fechou o
    /// canal (o processo saiu) — encerra a leitura, não um erro.
    pub fn recv<T: DeserializeOwned>(&self) -> io::Result<Option<(T, Vec<OwnedFd>)>> {
        // Uma leitura só, num buffer FIXO reusado por thread (a thread leitora
        // chama isto em loop). Nada inline passa de `INLINE_MAX`; o que passa
        // (save state grande) veio por memfd e o datagrama aqui é só o
        // marcador + o fd — cabe folgado. Sem `MSG_PEEK` (que interage mal com
        // `SCM_RIGHTS`).
        RECV_BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            let mut iov = [IoSliceMut::new(&mut buf[..])];
            let mut space = [MaybeUninit::<u8>::uninit(); MAX_ANCILLARY];
            let mut control = RecvAncillaryBuffer::new(&mut space);
            let got = match net::recvmsg(self.fd(), &mut iov, &mut control, RecvFlags::TRUNC) {
                Ok(r) => r,
                // O filho foi morto (troca de ROM / unload / crash): o
                // socketpair pode devolver RST em vez de FIN. Igual a EOF.
                Err(e) if peer_gone(e) => return Ok(None),
                Err(e) => return Err(io::Error::from(e)),
            };
            if got.bytes == 0 {
                return Ok(None); // EOF
            }
            if got.flags.contains(ReturnFlags::TRUNC) || got.bytes > buf.len() {
                return Err(io::Error::other(format!(
                    "datagrama IPC de {} bytes não coube no buffer inline ({}) \
                     — deveria ter ido por memfd",
                    got.bytes,
                    buf.len()
                )));
            }
            let mut owned_fds = Vec::new();
            for msg in control.drain() {
                if let RecvAncillaryMessage::ScmRights(iter) = msg {
                    owned_fds.extend(iter);
                }
            }

            // Corpo grande veio por memfd (ver `send_via_memfd`): lê o bincode
            // de volta da mesma memória e decodifica dali.
            if got.bytes == MEMFD_MARKER.len()
                && &buf[..got.bytes] == MEMFD_MARKER
                && owned_fds.len() == 1
            {
                let fd = owned_fds.pop().unwrap();
                let size = rustix::fs::fstat(&fd).map_err(io::Error::from)?.st_size as usize;
                if size > MAX_MSG {
                    return Err(io::Error::other(format!(
                        "blob IPC de {size} bytes excede o teto de {MAX_MSG}"
                    )));
                }
                let mut blob = vec![0u8; size];
                let mut off = 0u64;
                while (off as usize) < size {
                    let n = rustix::io::pread(&fd, &mut blob[off as usize..], off)
                        .map_err(io::Error::from)?;
                    if n == 0 {
                        return Err(io::Error::other("pread no memfd devolveu 0 antes do fim"));
                    }
                    off += n as u64;
                }
                let (value, _) =
                    bincode::serde::decode_from_slice::<T, _>(&blob, bincode::config::standard())
                        .map_err(|e| io::Error::other(format!("bincode decode (memfd): {e}")))?;
                return Ok(Some((value, Vec::new())));
            }

            let (value, _) = bincode::serde::decode_from_slice::<T, _>(
                &buf[..got.bytes],
                bincode::config::standard(),
            )
            .map_err(|e| io::Error::other(format!("bincode decode: {e}")))?;
            Ok(Some((value, owned_fds)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::fd::AsFd;

    /// Datagrama grande (save state de SNES passa de 800KB) — o bug era o
    /// `recv` truncar em silêncio num buffer fixo de 512KB. 2MB fica no
    /// caminho inline (< `INLINE_MAX`).
    #[test]
    fn roundtrips_a_2mb_message_inline() {
        let (a, b) = Channel::pair().unwrap();
        let payload: Vec<u8> = (0..2_000_000u32).map(|i| i as u8).collect();
        let sent = payload.clone();
        let h = std::thread::spawn(move || {
            a.send::<Vec<u8>>(&sent, &[]).unwrap();
        });
        let (got, fds) = b.recv::<Vec<u8>>().unwrap().unwrap();
        h.join().unwrap();
        assert!(fds.is_empty());
        assert_eq!(got, payload);
    }

    /// Save state de N64 (parallel_n64) passa de 16MB — não cabe num datagrama
    /// SEQPACKET, vai por memfd anexado. 20MB > `INLINE_MAX`.
    #[test]
    fn roundtrips_a_20mb_message_via_memfd() {
        let (a, b) = Channel::pair().unwrap();
        let payload: Vec<u8> = (0..20_000_000u32).map(|i| (i ^ (i >> 7)) as u8).collect();
        let sent = payload.clone();
        let h = std::thread::spawn(move || {
            a.send::<Vec<u8>>(&sent, &[]).unwrap();
        });
        let (got, fds) = b.recv::<Vec<u8>>().unwrap().unwrap();
        h.join().unwrap();
        assert!(fds.is_empty(), "o memfd não vaza pro consumidor");
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

    /// fds do `SCM_RIGHTS` chegam junto no `recv` (mensagem pequena + fds).
    #[test]
    fn passes_fds_with_a_small_message() {
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
