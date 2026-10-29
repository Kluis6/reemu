//! Servidor de uma conexão só em `127.0.0.1`, para onde o navegador volta
//! depois do consentimento (RFC 8252, seção 7.3).
//!
//! Só `127.0.0.1` e porta escolhida pelo SO. Uma requisição com `state`
//! errado, de outro caminho ou malformada não encerra o fluxo: leva uma
//! resposta de erro e o servidor continua esperando até o prazo. Termina com
//! exatamente um callback válido (code ou `access_denied`).

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Tempo total que uma conexão tem para mandar a requisição.
const CONNECTION_BUDGET: Duration = Duration::from_secs(2);

/// Idioma da página que o navegador mostra no fim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    PtBr,
    En,
    Es,
}

impl Lang {
    /// Aceita a tag do app (`pt-BR`, `en`, `es`, também `en-US`...); o que não
    /// reconhecer cai no pt-BR, como o resto do app.
    pub fn from_tag(tag: &str) -> Self {
        match tag.trim().to_ascii_lowercase().split(['-', '_']).next() {
            Some("en") => Self::En,
            Some("es") => Self::Es,
            _ => Self::PtBr,
        }
    }

    fn html_lang(self) -> &'static str {
        match self {
            Self::PtBr => "pt-BR",
            Self::En => "en",
            Self::Es => "es",
        }
    }

    fn done(self) -> &'static str {
        match self {
            Self::PtBr => "Pode fechar esta janela e voltar ao ReEmu.",
            Self::En => "You can close this window and go back to ReEmu.",
            Self::Es => "Ya puedes cerrar esta ventana y volver a ReEmu.",
        }
    }

    fn cancelled(self) -> &'static str {
        match self {
            Self::PtBr => "Conexão cancelada. Pode fechar esta janela e voltar ao ReEmu.",
            Self::En => "Connection cancelled. You can close this window and go back to ReEmu.",
            Self::Es => "Conexión cancelada. Ya puedes cerrar esta ventana y volver a ReEmu.",
        }
    }

    fn failed(self) -> &'static str {
        match self {
            Self::PtBr => {
                "Não foi possível conectar. Pode fechar esta janela e tentar de novo no ReEmu."
            }
            Self::En => "Could not connect. You can close this window and try again in ReEmu.",
            Self::Es => {
                "No se pudo conectar. Ya puedes cerrar esta ventana e intentarlo de nuevo en ReEmu."
            }
        }
    }

    fn bad_request(self) -> &'static str {
        match self {
            Self::PtBr => "Pedido inválido. Volte ao ReEmu e tente conectar de novo.",
            Self::En => "Invalid request. Go back to ReEmu and try connecting again.",
            Self::Es => "Solicitud inválida. Vuelve a ReEmu e intenta conectar de nuevo.",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoopbackError {
    /// O usuário tocou em Cancelar na página de consentimento.
    Denied,
    /// O Alvanista devolveu outro erro (o código, ex.: `server_error`).
    Provider(String),
    /// Ninguém voltou a tempo.
    Timeout,
    /// Quem conectou desistiu (Cancelar, ou outra tentativa começou).
    Cancelled,
    Io(String),
}

pub struct Loopback {
    listener: TcpListener,
    pub port: u16,
    lang: Lang,
}

/// Abre o servidor em `127.0.0.1:0` (porta livre escolhida pelo SO).
pub fn listen(lang: Lang) -> io::Result<Loopback> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    Ok(Loopback {
        listener,
        port,
        lang,
    })
}

/// O que fazer com uma requisição.
enum Verdict {
    Code(String),
    Fail(LoopbackError),
    /// Responde com este status e segue esperando.
    Keep(u16),
}

impl Loopback {
    /// Bloqueia até o callback certo ou o prazo. Consome o servidor: ao
    /// voltar (com qualquer resultado) a porta já está livre.
    #[cfg(test)]
    pub fn wait(self, expected_state: &str, timeout: Duration) -> Result<String, LoopbackError> {
        self.wait_unless(expected_state, timeout, || false)
    }

    /// Como `wait`, mas `cancelled` é consultado a cada volta (a cada 20 ms
    /// ocioso): se virar `true`, encerra com `Cancelled` e libera a porta.
    pub fn wait_unless(
        self,
        expected_state: &str,
        timeout: Duration,
        cancelled: impl Fn() -> bool,
    ) -> Result<String, LoopbackError> {
        let deadline = Instant::now() + timeout;
        loop {
            if cancelled() {
                return Err(LoopbackError::Cancelled);
            }
            // A cada volta, não só quando está ocioso: um fluxo de conexões
            // não pode empurrar o prazo.
            if Instant::now() >= deadline {
                return Err(LoopbackError::Timeout);
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(result) = self.serve(stream, expected_state, deadline) {
                        return result;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                // Conexão que caiu antes do accept: não é problema do servidor.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(LoopbackError::Io(e.to_string())),
            }
        }
    }

    /// Atende uma conexão. `Some` encerra o fluxo; `None` segue esperando.
    fn serve(
        &self,
        mut stream: TcpStream,
        expected_state: &str,
        outer_deadline: Instant,
    ) -> Option<Result<String, LoopbackError>> {
        // O accept herda o modo não bloqueante do listener em alguns SOs.
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_write_timeout(Some(CONNECTION_BUDGET));
        // Um prazo só para ler a requisição inteira (um cliente que pinga um
        // byte por vez não segura o servidor), limitado pelo prazo do fluxo.
        let deadline = (Instant::now() + CONNECTION_BUDGET).min(outer_deadline);
        let target = read_request_target(&mut stream, deadline)?;
        match judge(&target, expected_state) {
            Verdict::Keep(status) => {
                respond(&mut stream, status, self.lang, self.lang.bad_request());
                None
            }
            Verdict::Code(code) => {
                respond(&mut stream, 200, self.lang, self.lang.done());
                Some(Ok(code))
            }
            Verdict::Fail(err) => {
                let message = match err {
                    LoopbackError::Denied => self.lang.cancelled(),
                    _ => self.lang.failed(),
                };
                respond(&mut stream, 200, self.lang, message);
                Some(Err(err))
            }
        }
    }
}

/// Decide o que fazer com `target` (caminho + query da linha de requisição).
fn judge(target: &str, expected_state: &str) -> Verdict {
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Verdict::Keep(400);
    };
    if url.path() != "/callback" {
        return Verdict::Keep(404);
    }
    let mut code = None;
    let mut error = None;
    let mut state = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            _ => {}
        }
    }
    // Sem o state certo nada vale: pode ser requisição alheia ou forjada.
    if state.as_deref() != Some(expected_state) {
        return Verdict::Keep(400);
    }
    if let Some(error) = error {
        return Verdict::Fail(if error == "access_denied" {
            LoopbackError::Denied
        } else {
            LoopbackError::Provider(error)
        });
    }
    match code {
        Some(code) if !code.is_empty() => Verdict::Code(code),
        _ => Verdict::Keep(400),
    }
}

/// Lê o cabeçalho (até 8 KB) e devolve o alvo de um `GET`; `None` para
/// qualquer outra coisa (a conexão é só fechada).
fn read_request_target(stream: &mut TcpStream, deadline: Instant) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 8192 {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        let _ = stream.set_read_timeout(Some(left));
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    // Requisição cortada (sem a linha em branco do fim) não vale: nada é
    // aceito de um pedido que não chegou inteiro.
    if !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        return None;
    }
    let head = String::from_utf8_lossy(&buf);
    let mut parts = head.lines().next()?.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    parts.next().map(str::to_owned)
}

fn respond(stream: &mut TcpStream, status: u16, lang: Lang, message: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    // O 404 de outro caminho vai sem corpo.
    let body = if status == 404 {
        String::new()
    } else {
        page(lang, message)
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

/// HTML curto, sem recurso externo.
fn page(lang: Lang, message: &str) -> String {
    format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>ReEmu</title><style>body{{font-family:system-ui,sans-serif;\
         background:#14161a;color:#e8eaed;display:flex;align-items:center;\
         justify-content:center;min-height:100vh;margin:0;text-align:center}}\
         main{{max-width:28rem;padding:1.5rem}}</style></head>\
         <body><main><h1>ReEmu</h1><p>{message}</p></main></body></html>",
        lang.html_lang()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GET cru; devolve a resposta inteira (a conexão fecha no fim).
    fn get(port: u16, target: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    fn wait_in_thread(
        lb: Loopback,
        state: &'static str,
        timeout: Duration,
    ) -> std::thread::JoinHandle<Result<String, LoopbackError>> {
        std::thread::spawn(move || lb.wait(state, timeout))
    }

    #[test]
    fn loopback_accepts_callback_with_matching_state() {
        let lb = listen(Lang::PtBr).unwrap();
        let port = lb.port;
        let h = wait_in_thread(lb, "abc", Duration::from_secs(5));
        let resp = get(port, "/callback?code=the-code&state=abc");
        assert!(resp.starts_with("HTTP/1.1 200"));
        assert!(resp.contains("Pode fechar esta janela e voltar ao ReEmu"));
        assert!(!resp.contains("http://") && !resp.contains("https://"));
        assert_eq!(h.join().unwrap(), Ok("the-code".to_owned()));
    }

    #[test]
    fn loopback_rejects_wrong_state() {
        let lb = listen(Lang::En).unwrap();
        let port = lb.port;
        let h = wait_in_thread(lb, "abc", Duration::from_secs(5));
        // Estado errado, ausente, code ausente e outro caminho: nada encerra.
        assert!(get(port, "/callback?code=evil&state=nope").starts_with("HTTP/1.1 400"));
        assert!(get(port, "/callback?code=evil").starts_with("HTTP/1.1 400"));
        assert!(get(port, "/callback?state=abc").starts_with("HTTP/1.1 400"));
        assert!(get(port, "/favicon.ico").starts_with("HTTP/1.1 404"));
        assert!(!h.is_finished());
        // O fluxo legítimo ainda funciona depois dos intrusos.
        let ok = get(port, "/callback?code=good&state=abc");
        assert!(ok.contains("You can close this window and go back to ReEmu"));
        assert_eq!(h.join().unwrap(), Ok("good".to_owned()));
    }

    #[test]
    fn loopback_times_out_and_frees_port() {
        let lb = listen(Lang::PtBr).unwrap();
        let port = lb.port;
        let started = Instant::now();
        assert_eq!(
            lb.wait("abc", Duration::from_millis(200)),
            Err(LoopbackError::Timeout)
        );
        assert!(started.elapsed() >= Duration::from_millis(200));
        TcpListener::bind(("127.0.0.1", port)).expect("a porta tem que estar livre");
    }

    #[test]
    fn loopback_reports_access_denied() {
        let lb = listen(Lang::Es).unwrap();
        let port = lb.port;
        let h = wait_in_thread(lb, "abc", Duration::from_secs(5));
        let resp = get(port, "/callback?error=access_denied&state=abc");
        assert!(resp.contains("Conexión cancelada"));
        assert_eq!(h.join().unwrap(), Err(LoopbackError::Denied));
    }

    #[test]
    fn loopback_ignores_access_denied_with_wrong_state() {
        let lb = listen(Lang::PtBr).unwrap();
        let port = lb.port;
        let h = wait_in_thread(lb, "abc", Duration::from_millis(400));
        assert!(get(port, "/callback?error=access_denied&state=x").starts_with("HTTP/1.1 400"));
        assert_eq!(h.join().unwrap(), Err(LoopbackError::Timeout));
    }

    #[test]
    fn provider_error_gets_its_own_page_text() {
        let lb = listen(Lang::En).unwrap();
        let port = lb.port;
        let h = wait_in_thread(lb, "abc", Duration::from_secs(5));
        let resp = get(port, "/callback?error=server_error&state=abc");
        assert!(resp.contains("Could not connect"));
        assert!(!resp.contains("cancelled"));
        assert_eq!(
            h.join().unwrap(),
            Err(LoopbackError::Provider("server_error".into()))
        );
    }

    #[test]
    fn silent_connection_cannot_hold_past_the_deadline() {
        let lb = listen(Lang::PtBr).unwrap();
        let port = lb.port;
        let started = Instant::now();
        let h = wait_in_thread(lb, "abc", Duration::from_millis(300));
        // Abre e não manda nada (nem fecha) até o fim do teste.
        let _silent = TcpStream::connect(("127.0.0.1", port)).unwrap();
        assert_eq!(h.join().unwrap(), Err(LoopbackError::Timeout));
        assert!(started.elapsed() < Duration::from_millis(1500));
    }

    #[test]
    fn dribbling_connection_is_cut_by_the_overall_budget() {
        let lb = listen(Lang::PtBr).unwrap();
        let port = lb.port;
        let started = Instant::now();
        let h = wait_in_thread(lb, "abc", Duration::from_millis(600));
        let mut slow = TcpStream::connect(("127.0.0.1", port)).unwrap();
        // Um byte por 100 ms: o read nunca estoura o timeout por conta própria.
        for b in b"GET /callback?code=x&state=abc HT" {
            if slow.write_all(&[*b]).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(h.join().unwrap(), Err(LoopbackError::Timeout));
        assert!(started.elapsed() < Duration::from_millis(1500));
    }

    #[test]
    fn truncated_request_is_never_accepted() {
        let lb = listen(Lang::PtBr).unwrap();
        let port = lb.port;
        let h = wait_in_thread(lb, "abc", Duration::from_millis(500));
        // A linha de requisição completa, mas sem o fim do cabeçalho, e a
        // conexão fecha.
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET /callback?code=x&state=abc HTTP/1.1\r\nHost: a").unwrap();
        drop(s);
        assert_eq!(h.join().unwrap(), Err(LoopbackError::Timeout));
    }

    #[test]
    fn language_tags_map_with_pt_br_fallback() {
        assert_eq!(Lang::from_tag("en-US"), Lang::En);
        assert_eq!(Lang::from_tag("es"), Lang::Es);
        assert_eq!(Lang::from_tag("pt-BR"), Lang::PtBr);
        assert_eq!(Lang::from_tag("ja"), Lang::PtBr);
    }
}
