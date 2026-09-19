//! Transporte Windows: DOIS pipes nomeados unidirecionais (um por sentido) em
//! modo byte, com framing próprio (prefixo de tamanho `u32` LE + corpo
//! bincode). Sem equivalente a `SCM_RIGHTS`: pipe nomeado não passa handle
//! inline numa mensagem. Os dois casos que precisavam disso no lado Unix
//! foram redesenhados: o memfd do `FrameRing` virou memória compartilhada
//! NOMEADA (nome derivado do nome do canal, ver `shm_ring_win.rs`); o dma_buf
//! do caminho HW de interop (Linux/GBM) só existe em `#[cfg(unix)]`.
//!
//! Por que dois pipes e não um duplex: num handle SÍNCRONO, um `ReadFile`
//! bloqueado (a thread leitora fica parada nele o tempo todo) segura o lock do
//! objeto de arquivo e trava qualquer `WriteFile` de outra thread no MESMO
//! handle — deadlock certo com "uma thread lê, outra manda". Um handle por
//! sentido evita isso sem precisar de I/O sobreposto (OVERLAPPED).
//!
//! Por que modo byte + framing e não `PIPE_TYPE_MESSAGE`: o lado cliente de um
//! pipe nasce em `PIPE_READMODE_BYTE` (só o servidor escolhe o modo na
//! criação), então as fronteiras de mensagem não valeriam do lado cliente sem
//! um `SetNamedPipeHandleState` extra. Framing explícito não depende disso.
//!
//! Assinaturas/features do `windows-sys` 0.61.2 conferidas na fonte vendorizada
//! (`~/.cargo/registry`), não de memória — ver a convenção do projeto em
//! `docs/ai-context/REFERENCES.md`. Todos os tipos de flag são `u32` puro.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt;
use std::io;
use std::ptr;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::{
    CloseHandle, SetHandleInformation, ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
    ERROR_PIPE_NOT_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE, HANDLE_FLAG_INHERIT,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_NONE,
    OPEN_EXISTING, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
};

/// Teto de sanidade por mensagem — mesmo valor do lado Unix (save state de
/// N64/PSP).
const MAX_MSG: usize = 128 * 1024 * 1024;
/// Tamanho de buffer sugerido ao pipe — só um guia pro kernel, não um teto.
const PIPE_BUF_HINT: u32 = 4 * 1024 * 1024;
/// Teto por chamada de `ReadFile`/`WriteFile` (o tamanho é `u32`).
const IO_CHUNK: usize = 1 << 30;

fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `HANDLE` bruto com `Drop` — o compilador não confia num `*mut c_void`
/// sozinho como `Send`/`Sync`. Cada `RawPipe` só é usado num sentido (leitura
/// OU escrita), e a escrita é serializada por `Inner::wr_lock`.
struct RawPipe(HANDLE);
unsafe impl Send for RawPipe {}
unsafe impl Sync for RawPipe {}

impl Drop for RawPipe {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Inner {
    rd: RawPipe,
    wr: RawPipe,
    /// Serializa `send` de threads diferentes (cabeçalho + corpo são duas
    /// escritas — sem isto duas mensagens poderiam se intercalar).
    wr_lock: Mutex<()>,
    /// Nome base do canal — `shm_ring_win` deriva o nome da memória
    /// compartilhada do anel dele, e o filho recebe o mesmo nome via
    /// `ChannelArg`.
    name: String,
}

#[derive(Clone)]
pub struct Channel(Arc<Inner>);

/// O que o pai passa pro filho na linha de comando (`--fd <isto>`): os dois
/// handles herdados (mesmos valores numéricos no filho — garantia do
/// Windows pra handle herdável) + o nome base do canal. Serializa como
/// `"<rd>:<wr>:<nome>"`; o `ToString` é o que `emu-session` já chama no
/// número do fd no lado Unix, então o ponto de chamada não muda.
pub struct ChannelArg {
    rd: isize,
    wr: isize,
    name: String,
}

impl fmt::Display for ChannelArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.rd, self.wr, self.name)
    }
}

impl FromStr for ChannelArg {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let mut it = s.splitn(3, ':');
        let mut next = |what: &str| it.next().ok_or_else(|| format!("--fd sem {what}: {s:?}"));
        let rd = next("handle de leitura")?
            .parse()
            .map_err(|e| format!("--fd: handle de leitura inválido: {e}"))?;
        let wr = next("handle de escrita")?
            .parse()
            .map_err(|e| format!("--fd: handle de escrita inválido: {e}"))?;
        let name = next("nome")?.to_string();
        Ok(ChannelArg { rd, wr, name })
    }
}

/// Um pipe unidirecional já conectado dentro deste processo: servidor
/// (`server_access` = `PIPE_ACCESS_OUTBOUND` ou `INBOUND`) + cliente
/// (`client_access` = `GENERIC_READ` ou `GENERIC_WRITE`) no mesmo nome.
/// `ConnectNamedPipe` só fecha o handshake do lado servidor —
/// `ERROR_PIPE_CONNECTED` é sucesso (o cliente já tinha chegado).
fn connected_pipe(
    path: &str,
    server_access: u32,
    client_access: u32,
) -> io::Result<(RawPipe, RawPipe)> {
    let wpath = wide_null(path);
    let server = unsafe {
        CreateNamedPipeW(
            wpath.as_ptr(),
            server_access | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1, // ponto-a-ponto: uma instância só
            PIPE_BUF_HINT,
            PIPE_BUF_HINT,
            0,
            ptr::null(),
        )
    };
    if server == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let server = RawPipe(server);

    let client = unsafe {
        CreateFileW(
            wpath.as_ptr(),
            client_access,
            FILE_SHARE_NONE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if client == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let client = RawPipe(client);

    if unsafe { ConnectNamedPipe(server.0, ptr::null_mut()) } == 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32) {
            return Err(err);
        }
    }
    Ok((server, client))
}

impl Channel {
    /// Dois pipes nomeados únicos (nome com PID + contador + relógio), os dois
    /// lados de cada um conectados DENTRO deste processo — mesmo papel do
    /// `socketpair()` do lado Unix: devolve `(a, b)` já ligados, o chamador
    /// decide qual lado repassa pro processo filho. `a` escreve pra `b` pelo
    /// pipe `-ab`; `b` escreve pra `a` pelo `-ba`.
    pub fn pair() -> io::Result<(Channel, Channel)> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = format!("reemu-ipc-{}-{n}-{nanos}", std::process::id());

        let (a_wr, b_rd) = connected_pipe(
            &format!("\\\\.\\pipe\\{name}-ab"),
            PIPE_ACCESS_OUTBOUND,
            GENERIC_READ,
        )?;
        let (a_rd, b_wr) = connected_pipe(
            &format!("\\\\.\\pipe\\{name}-ba"),
            PIPE_ACCESS_INBOUND,
            GENERIC_WRITE,
        )?;

        let a = Channel(Arc::new(Inner {
            rd: a_rd,
            wr: a_wr,
            wr_lock: Mutex::new(()),
            name: name.clone(),
        }));
        let b = Channel(Arc::new(Inner {
            rd: b_rd,
            wr: b_wr,
            wr_lock: Mutex::new(()),
            name,
        }));
        Ok((a, b))
    }

    /// Marca os handles como herdáveis pelo processo filho. No Windows o
    /// padrão é NÃO herdável (o oposto do Unix, onde o padrão É herdável e o
    /// `O_CLOEXEC` precisa ser desligado) — `SetHandleInformation` liga
    /// `HANDLE_FLAG_INHERIT`. `std::process::Command` já cria o processo com
    /// `bInheritHandles=TRUE` (`inherit_handles: true` por padrão, conferido
    /// em `library/std/src/sys/process/windows.rs`), e um handle herdado
    /// aparece no filho com o MESMO valor numérico. O nome `clear_cloexec` só
    /// mantém o ponto de chamada em `emu-session` idêntico nos dois lados.
    pub fn clear_cloexec(&self) -> io::Result<()> {
        for h in [&self.0.rd, &self.0.wr] {
            let ok = unsafe { SetHandleInformation(h.0, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// O que vai na linha de comando pro filho reconstruir o canal (ver
    /// `ChannelArg`). Mesmo papel do `as_raw_fd()` do lado Unix.
    pub fn as_raw_fd(&self) -> ChannelArg {
        ChannelArg {
            rd: self.0.rd.0 as isize,
            wr: self.0.wr.0 as isize,
            name: self.0.name.clone(),
        }
    }

    /// Reconstrói o canal no processo filho a partir do que o pai passou.
    ///
    /// # Safety
    /// Os handles em `arg` precisam ser os herdados do pai (ver
    /// `clear_cloexec`), abertos, e cuja posse ninguém mais reivindica neste
    /// processo (chamado 1x no `main` do filho).
    pub unsafe fn from_inherited_fd(arg: ChannelArg) -> Channel {
        Channel(Arc::new(Inner {
            rd: RawPipe(arg.rd as HANDLE),
            wr: RawPipe(arg.wr as HANDLE),
            wr_lock: Mutex::new(()),
            name: arg.name,
        }))
    }

    /// Nome base do canal — `shm_ring_win` deriva o nome do anel dele.
    pub(crate) fn ipc_name(&self) -> &str {
        &self.0.name
    }

    #[cfg(debug_assertions)]
    pub fn assert_inheritable(&self) {
        // Sem consulta barata equivalente ao `fcntl(F_GETFD)` do lado Unix —
        // `SetHandleInformation` já teria devolvido erro em `clear_cloexec`.
    }

    /// `_handles` existe só pra manter a mesma assinatura do lado Unix nos
    /// pontos de chamada compartilhados — sempre vazio (o tipo é um enum
    /// vazio, ver `InlineHandle`).
    pub fn send<T: Serialize>(&self, msg: &T, _handles: &[super::InlineHandle]) -> io::Result<()> {
        let body = bincode::serde::encode_to_vec(msg, bincode::config::standard())
            .map_err(|e| io::Error::other(format!("bincode encode: {e}")))?;
        if body.len() > MAX_MSG {
            return Err(io::Error::other(format!(
                "mensagem IPC de {} bytes excede o teto de {MAX_MSG}",
                body.len()
            )));
        }
        let header = (body.len() as u32).to_le_bytes();
        let _guard = self.0.wr_lock.lock().unwrap_or_else(|p| p.into_inner());
        write_all(self.0.wr.0, &header)?;
        write_all(self.0.wr.0, &body)
    }

    /// Bloqueia até a próxima mensagem. `Ok(None)` = o outro lado fechou o
    /// canal (processo saiu) — encerra a leitura, não um erro.
    pub fn recv<T: DeserializeOwned>(&self) -> io::Result<Option<(T, Vec<super::InlineHandle>)>> {
        let mut header = [0u8; 4];
        if !read_exact(self.0.rd.0, &mut header)? {
            return Ok(None);
        }
        let len = u32::from_le_bytes(header) as usize;
        if len > MAX_MSG {
            return Err(io::Error::other(format!(
                "mensagem IPC de {len} bytes excede o teto de {MAX_MSG}"
            )));
        }
        let mut body = vec![0u8; len];
        if !read_exact(self.0.rd.0, &mut body)? && len != 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "canal fechado no meio de uma mensagem",
            ));
        }
        let (value, _) =
            bincode::serde::decode_from_slice::<T, _>(&body, bincode::config::standard())
                .map_err(|e| io::Error::other(format!("bincode decode: {e}")))?;
        Ok(Some((value, Vec::new())))
    }
}

/// `true` se o erro significa "a outra ponta sumiu" — trata como EOF, mesma
/// ideia do `peer_gone` do lado Unix.
fn peer_gone(e: &io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(c) if c == ERROR_BROKEN_PIPE as i32
            || c == ERROR_NO_DATA as i32
            || c == ERROR_PIPE_NOT_CONNECTED as i32
    )
}

fn write_all(h: HANDLE, mut buf: &[u8]) -> io::Result<()> {
    while !buf.is_empty() {
        let n = buf.len().min(IO_CHUNK);
        let mut written = 0u32;
        let ok = unsafe { WriteFile(h, buf.as_ptr(), n as u32, &mut written, ptr::null_mut()) };
        if ok == 0 {
            let e = io::Error::last_os_error();
            return Err(if peer_gone(&e) {
                io::Error::new(io::ErrorKind::BrokenPipe, "canal fechado")
            } else {
                e
            });
        }
        if written == 0 {
            return Err(io::Error::other("WriteFile escreveu 0 bytes"));
        }
        buf = &buf[written as usize..];
    }
    Ok(())
}

/// Preenche `buf` inteiro. `Ok(false)` = EOF limpo ANTES do primeiro byte (o
/// outro lado fechou entre mensagens); EOF no meio do buffer é erro.
fn read_exact(h: HANDLE, buf: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0usize;
    while filled < buf.len() {
        let n = (buf.len() - filled).min(IO_CHUNK);
        let mut got = 0u32;
        let ok = unsafe {
            ReadFile(
                h,
                buf[filled..].as_mut_ptr(),
                n as u32,
                &mut got,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            let e = io::Error::last_os_error();
            if peer_gone(&e) {
                if filled == 0 {
                    return Ok(false);
                }
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "canal fechado no meio de uma leitura",
                ));
            }
            return Err(e);
        }
        if got == 0 {
            // Sucesso com 0 bytes num pipe byte = o outro lado fechou.
            if filled == 0 {
                return Ok(false);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "canal fechado no meio de uma leitura",
            ));
        }
        filled += got as usize;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_a_2mb_message() {
        let (a, b) = Channel::pair().unwrap();
        let payload: Vec<u8> = (0..2_000_000u32).map(|i| i as u8).collect();
        let sent = payload.clone();
        let h = std::thread::spawn(move || {
            a.send::<Vec<u8>>(&sent, &[]).unwrap();
        });
        let (got, handles) = b.recv::<Vec<u8>>().unwrap().unwrap();
        h.join().unwrap();
        assert!(handles.is_empty());
        assert_eq!(got, payload);
    }

    #[test]
    fn roundtrips_a_20mb_message() {
        let (a, b) = Channel::pair().unwrap();
        let payload: Vec<u8> = (0..20_000_000u32).map(|i| (i ^ (i >> 7)) as u8).collect();
        let sent = payload.clone();
        let h = std::thread::spawn(move || {
            a.send::<Vec<u8>>(&sent, &[]).unwrap();
        });
        let (got, _) = b.recv::<Vec<u8>>().unwrap().unwrap();
        h.join().unwrap();
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

    /// O cenário que um pipe duplex síncrono trava: uma thread bloqueada em
    /// `recv` no mesmo canal em que outra thread faz `send`.
    #[test]
    fn send_while_another_thread_is_blocked_in_recv() {
        let (a, b) = Channel::pair().unwrap();
        let a_reader = a.clone();
        let reader = std::thread::spawn(move || a_reader.recv::<u32>().unwrap().unwrap().0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        a.send::<u8>(&1, &[]).unwrap(); // não pode travar atrás do recv
        assert_eq!(b.recv::<u8>().unwrap().unwrap().0, 1);
        b.send::<u32>(&99, &[]).unwrap();
        assert_eq!(reader.join().unwrap(), 99);
    }

    #[test]
    fn channel_arg_roundtrips_through_a_string() {
        let (a, _b) = Channel::pair().unwrap();
        let s = a.as_raw_fd().to_string();
        let parsed: ChannelArg = s.parse().unwrap();
        assert_eq!(parsed.to_string(), s);
        assert_eq!(parsed.name, a.ipc_name());
    }
}
