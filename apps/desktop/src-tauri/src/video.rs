//! Surface nativa de vídeo — o jogo (wgpu) numa `wl_subsurface` da janela GTK
//! (Linux) ou numa janela filha do HWND principal (Windows).
//!
//! ## Linux (Wayland) — padrão (`REEMU_NATIVE_VIDEO=0` volta pro `<canvas>`)
//!
//! O WebKitGTK 2.x nesse combo NVIDIA+Wayland **não entrega webview
//! transparente** (bug upstream, ver `docs/ai-context/03`). Então em vez de
//! "webview transparente sobre o vídeo", a subsurface fica **ACIMA** da webview
//! opaca (padrão do protocolo — nasce no topo):
//!
//! Jogando: a subsurface (jogo, `render_to_surface` no `gpu.rs`) cobre a webview
//! (menu fechado, nada se perde). Menu: o Rust captura 1 frame
//! (`FrameProcessor::capture_surface_frame`), esconde a subsurface
//! (`Subsurface::set_hidden` = attach de buffer nulo) e a webview opaca reaparece
//! com esse print de fundo (comando `pause_background`) + blur/escurece por CSS.
//! Coreografia = máquina de estado `commands::VideoMenu`, dirigida pelo
//! `toggle_and_emit`, no `reemu-video-pump`.
//!
//! Sem janela transparente = sem o bug NVIDIA. Zero cópia de CPU no caminho de
//! vídeo (a chain desenha direto na imagem do swapchain da subsurface).
//! Fallback automático pro `<canvas>` se não for Wayland ou o attach falhar.
//!
//! ## Windows — padrão (`REEMU_NATIVE_VIDEO=0` volta pro `<canvas>`)
//!
//! Mesma coreografia, com uma janela filha (`WS_CHILD`) do HWND da janela no
//! lugar da subsurface. A surface não pode ficar no HWND principal: o
//! WebView2 é uma janela filha dele e cobriria o jogo. A nossa é criada
//! depois e vai pro topo da ordem Z entre as irmãs (`HWND_TOP`), cobrindo o
//! WebView2 enquanto joga; no menu ela some (`SWP_HIDEWINDOW`) e o WebView2
//! aparece com o print de fundo. Decisões (documentação da Microsoft, Win32):
//!
//! - `WS_DISABLED`: "When a child window is disabled, the system passes the
//!   child's mouse input messages to the parent window" (Window Features ›
//!   Disabled Windows) — clicar no jogo não tira o foco do teclado do
//!   WebView2, que é quem recebe as teclas do jogo e o Esc do menu.
//! - `WS_CLIPSIBLINGS`: a irmã (WebView2) não desenha por cima da nossa área
//!   (Window Features › Child Windows).
//! - `SWP_ASYNCWINDOWPOS`: quem mexe na janela é o `reemu-video-pump`, não a
//!   thread que a criou (a principal, do event loop). Com a flag, "the system
//!   posts the request to the thread that owns the window. This prevents the
//!   calling thread from blocking" (SetWindowPos) — sem deadlock com a thread
//!   principal esperando um lock que o pump segura.
//! - Present fora da thread da janela: o risco de deadlock que o DXGI descreve
//!   (DXGI overview › Multithread considerations) é de swapchain em tela
//!   cheia exclusiva; a nossa é em janela (o "tela cheia" do app é janela
//!   sem borda).
//!
//! Mostrar vem DEPOIS do present (ao contrário do Wayland): a janela
//! escondida guarda o último quadro apresentado, e mostrá-la antes do quadro
//! novo piscaria o jogo anterior. Validação pendente em máquina Windows.
//!
//! ### Protótipo: WebView2 transparente por cima (`REEMU_WIN_OVERLAY`)
//!
//! Em vez de o jogo cobrir a interface e sumir no menu, o jogo fica ATRÁS e o
//! WebView2 transparente por cima: `DefaultBackgroundColor` com alfa 0 —
//! "In the case of a transparent DefaultBackgroundColor WebView will render
//! hosting app content as the background" (Microsoft, WebView2
//! `ICoreWebView2Controller2::put_DefaultBackgroundColor`; só alfa 0 ou 255).
//! A `PlayScreen` já é transparente no vídeo nativo; o menu de pausa e os
//! avisos viram camadas HTML sobre o jogo ao vivo, sem print nem esconder.
//! Sem jogo / carregando, a surface é limpa de preto (`clear_surface`) em vez
//! de escondida. Duas variantes pra testar qual o WebView2 em modo janela
//! deixa aparecer por baixo (a documentação não detalha):
//!
//! - `REEMU_WIN_OVERLAY=1`: surface no próprio HWND principal (o "hosting app
//!   content" mais literal);
//! - `REEMU_WIN_OVERLAY=child`: a janela filha, mas no FUNDO das irmãs
//!   (`HWND_BOTTOM`), abaixo do WebView2.
//!
//! ## macOS
//!
//! Surface direto no handle da janela. Não verificado.

#[cfg(target_os = "linux")]
use raw_window_handle::WaylandWindowHandle;
#[cfg(target_os = "windows")]
use raw_window_handle::Win32WindowHandle;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
#[cfg(target_os = "linux")]
use std::ptr::NonNull;
use tauri::{AppHandle, Manager, Runtime};

/// Handles pro wgpu anexar a surface. Não guardados no `VideoSurface` (não são
/// `Send`) — o chamador usa na hora, no `spawn`.
pub struct SurfaceHandles {
    pub display: RawDisplayHandle,
    pub window: RawWindowHandle,
}

pub struct VideoSurface {
    #[cfg(target_os = "linux")]
    _wl: wl::Subsurface,
    /// `None` = surface no próprio HWND principal (`REEMU_WIN_OVERLAY=1`).
    #[cfg(target_os = "windows")]
    _win: Option<win::ChildWindow>,
    /// WebView2 transparente por cima do jogo (`REEMU_WIN_OVERLAY`).
    #[cfg(target_os = "windows")]
    overlay: bool,
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    _priv: (),
}

/// Variante do protótipo `REEMU_WIN_OVERLAY` (ver o cabeçalho do módulo).
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WinOverlay {
    Off,
    Parent,
    Child,
}

#[cfg(target_os = "windows")]
fn win_overlay() -> WinOverlay {
    match std::env::var("REEMU_WIN_OVERLAY").as_deref() {
        Ok("1") | Ok("parent") => WinOverlay::Parent,
        Ok("child") => WinOverlay::Child,
        _ => WinOverlay::Off,
    }
}

// SAFETY: só a thread principal (event loop do Tauri) toca isso —
// `spawn`/`resize` rodam nela. O `Mutex` no `AppState` é só pra caber no
// managed state.
unsafe impl Send for VideoSurface {}

impl VideoSurface {
    /// `None` se não dá pra montar (não é Wayland, faltou global…) → o shell
    /// segue no `<canvas>`. Devolve os handles pro `attach_surface` do wgpu.
    pub fn spawn<R: Runtime>(app: &AppHandle<R>) -> Option<(Self, SurfaceHandles)> {
        let main = app.get_webview_window("main")?;
        let display = main.display_handle().ok()?.as_raw();
        let window = main.window_handle().ok()?.as_raw();

        #[cfg(target_os = "linux")]
        let (this, handles) = {
            let (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) = (display, window)
            else {
                log::warn!("REEMU_NATIVE_VIDEO precisa de Wayland (rode sem GDK_BACKEND=x11)");
                return None;
            };
            let size = main.inner_size().ok()?;
            // HiDPI: `inner_size` é físico, mas `set_position`/região da
            // subsurface e o `wl_surface` do pai são em coords LÓGICAS (buffer /
            // buffer_scale). Sem casar isso, num monitor 4K com escala 2 a
            // subsurface fica 2× maior que o pai e o compositor não mostra nada
            // (tela preta). O swapchain do wgpu continua em pixels físicos.
            let scale = main.scale_factor().unwrap_or(1.0).round().max(1.0) as i32;
            let sub = wl::Subsurface::create(
                d.display.as_ptr(),
                w.surface.as_ptr(),
                size.width,
                size.height,
                scale,
            )?;
            let wh = WaylandWindowHandle::new(NonNull::new(sub.wl_surface_ptr())?);
            log::info!(
                "surface de vídeo: wl_subsurface ({}x{} físico, buffer_scale {})",
                size.width,
                size.height,
                scale
            );
            (
                Self { _wl: sub },
                SurfaceHandles {
                    display,
                    window: RawWindowHandle::Wayland(wh),
                },
            )
        };

        #[cfg(target_os = "windows")]
        let (this, handles) = {
            let RawWindowHandle::Win32(parent) = window else {
                return None;
            };
            let mode = win_overlay();
            if mode != WinOverlay::Off {
                // Só o WebView2 (não a janela): alfa 0 deixa ver o conteúdo do
                // app por baixo. As telas com fundo opaco no CSS continuam
                // opacas; só a `PlayScreen` é transparente.
                let webview: &tauri::Webview<R> = main.as_ref();
                if let Err(e) =
                    webview.set_background_color(Some(tauri::webview::Color(0, 0, 0, 0)))
                {
                    log::warn!("REEMU_WIN_OVERLAY: WebView2 transparente falhou ({e})");
                    return None;
                }
            }
            let size = main.inner_size().ok()?;
            if mode == WinOverlay::Parent {
                log::info!(
                    "surface de vídeo: HWND principal atrás do WebView2 transparente ({}x{} físico) — protótipo REEMU_WIN_OVERLAY",
                    size.width,
                    size.height
                );
                (
                    Self {
                        _win: None,
                        overlay: true,
                    },
                    SurfaceHandles { display, window },
                )
            } else {
                let below = mode == WinOverlay::Child;
                let child =
                    win::ChildWindow::create(parent.hwnd.get(), size.width, size.height, below)?;
                if below {
                    // Fica sempre visível no fundo; quem some é a cor do
                    // WebView2 por cima.
                    child.show();
                }
                let mut wh = Win32WindowHandle::new(std::num::NonZeroIsize::new(child.hwnd())?);
                wh.hinstance = std::num::NonZeroIsize::new(child.hinstance());
                log::info!(
                    "surface de vídeo: janela filha {} do WebView2 ({}x{} físico){}",
                    if below { "abaixo" } else { "acima" },
                    size.width,
                    size.height,
                    if below {
                        " — protótipo REEMU_WIN_OVERLAY=child"
                    } else {
                        ""
                    }
                );
                (
                    Self {
                        _win: Some(child),
                        overlay: below,
                    },
                    SurfaceHandles {
                        display,
                        window: RawWindowHandle::Win32(wh),
                    },
                )
            }
        };

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let (this, handles) = (Self { _priv: () }, SurfaceHandles { display, window });

        Some((this, handles))
    }

    /// Reposiciona e redimensiona a subsurface. `(x, y)` é relativo à
    /// `wl_surface` do GTK — `(0, 0)` em fullscreen; a espessura da decoração
    /// (CSD) quando em janela.
    pub fn reconfigure(&self, x: i32, y: i32, width: u32, height: u32) {
        #[cfg(target_os = "linux")]
        self._wl.reconfigure(x, y, width, height);
        #[cfg(target_os = "windows")]
        if let Some(w) = &self._win {
            w.reconfigure(x, y, width, height);
        }
        let _ = (x, y, width, height);
    }

    /// WebView2 transparente por cima do jogo (protótipo `REEMU_WIN_OVERLAY`):
    /// a interface é desenhada sobre o jogo, então nada de esconder/mostrar —
    /// o pump limpa a surface de preto quando não há jogo.
    pub fn overlay(&self) -> bool {
        #[cfg(target_os = "windows")]
        return self.overlay;
        #[cfg(not(target_os = "windows"))]
        false
    }

    /// Esconde a subsurface do jogo (menu aberto, load em andamento, ou
    /// sessão ociosa). Chamar `show()` antes do próximo present real —
    /// mostrar não é mais implícito (ver `wl::Subsurface::set_hidden`).
    pub fn set_hidden(&self, hidden: bool) {
        #[cfg(target_os = "linux")]
        self._wl.set_hidden(hidden);
        #[cfg(target_os = "windows")]
        if let (Some(w), false) = (&self._win, self.overlay) {
            w.set_hidden(hidden);
        }
        let _ = hidden;
    }

    /// Desfaz `set_hidden(true)` — recoloca a subsurface na frente do
    /// parent. Chamar antes de anexar o próximo frame.
    pub fn show(&self) {
        #[cfg(target_os = "linux")]
        self._wl.show();
    }

    /// Par do `show()` pra depois do present: no Windows é aqui que a janela
    /// filha aparece, já com o quadro novo (ver o cabeçalho do módulo).
    pub fn show_after_present(&self) {
        #[cfg(target_os = "windows")]
        if let (Some(w), false) = (&self._win, self.overlay) {
            w.show();
        }
    }
}

#[cfg(target_os = "windows")]
mod win {
    //! Janela filha (`WS_CHILD`) do HWND principal pra surface do wgpu.

    use windows_sys::Win32::Foundation::{GetLastError, ERROR_CLASS_ALREADY_EXISTS, HWND};
    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, PostMessageW, RegisterClassExW, SetWindowPos, HWND_BOTTOM,
        HWND_TOP, SWP_ASYNCWINDOWPOS, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SWP_SHOWWINDOW, WM_CLOSE, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
        WS_DISABLED,
    };

    pub struct ChildWindow {
        // HWND/HINSTANCE como inteiro: o ponteiro cru não é `Send`.
        hwnd: isize,
        hinstance: isize,
        /// No fundo das irmãs (abaixo do WebView2) em vez do topo.
        below: bool,
    }

    // Cada chamada daqui é um `SetWindowPos` assíncrono (seguro de outra
    // thread) ou um `PostMessageW`.
    unsafe impl Send for ChildWindow {}

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    impl ChildWindow {
        /// Cria escondida, do tamanho da área de cliente do pai. Chamar na
        /// thread principal: a janela pertence à thread que a cria, e é o
        /// laço de mensagens dela (o event loop do Tauri) que a atende.
        pub fn create(parent: isize, w: u32, h: u32, below: bool) -> Option<Self> {
            let class = wide("ReEmuVideo");
            // SAFETY: chamadas Win32 com argumentos válidos; `class` vive até
            // o fim da função (o sistema copia o nome no registro).
            unsafe {
                let hinstance = GetModuleHandleW(std::ptr::null());
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    // Sem tratamento próprio: pintura, tamanho e fechamento
                    // ficam com o padrão. O fundo preto cobre o intervalo até
                    // o 1º present.
                    lpfnWndProc: Some(DefWindowProcW),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance,
                    hIcon: std::ptr::null_mut(),
                    hCursor: std::ptr::null_mut(),
                    hbrBackground: GetStockObject(BLACK_BRUSH),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class.as_ptr(),
                    hIconSm: std::ptr::null_mut(),
                };
                if RegisterClassExW(&wc) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                    log::warn!("RegisterClassExW falhou: {}", GetLastError());
                    return None;
                }
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    std::ptr::null(),
                    WS_CHILD | WS_CLIPSIBLINGS | WS_DISABLED,
                    0,
                    0,
                    w.max(1) as i32,
                    h.max(1) as i32,
                    parent as HWND,
                    std::ptr::null_mut(),
                    hinstance,
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    log::warn!("CreateWindowExW falhou: {}", GetLastError());
                    return None;
                }
                Some(Self {
                    hwnd: hwnd as isize,
                    hinstance: hinstance as isize,
                    below,
                })
            }
        }

        pub fn hwnd(&self) -> isize {
            self.hwnd
        }

        pub fn hinstance(&self) -> isize {
            self.hinstance
        }

        fn set_pos(&self, x: i32, y: i32, w: i32, h: i32, flags: u32) {
            // SAFETY: HWND nosso, vivo até o `Drop`.
            unsafe {
                SetWindowPos(
                    self.hwnd as HWND,
                    if self.below { HWND_BOTTOM } else { HWND_TOP },
                    x,
                    y,
                    w,
                    h,
                    flags | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                );
            }
        }

        /// `(x, y)` em coordenadas de cliente do pai (sempre `(0, 0)` aqui,
        /// ver `csd_offset` no lib.rs), tamanho em pixels físicos. Sem
        /// `SWP_NOZORDER`: reafirma a posição em relação ao WebView2 (acima,
        /// ou abaixo no protótipo) a cada mudança.
        pub fn reconfigure(&self, x: i32, y: i32, w: u32, h: u32) {
            self.set_pos(x, y, w.max(1) as i32, h.max(1) as i32, 0);
        }

        pub fn set_hidden(&self, hidden: bool) {
            if hidden {
                self.set_pos(
                    0,
                    0,
                    0,
                    0,
                    SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                );
            }
        }

        /// Mostra e reafirma a posição entre as irmãs (no topo, acima do
        /// WebView2; ou no fundo, abaixo dele).
        pub fn show(&self) {
            self.set_pos(0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE);
        }
    }

    impl Drop for ChildWindow {
        fn drop(&mut self) {
            // `DestroyWindow` só vale na thread dona da janela; o `WM_CLOSE`
            // chega lá e o `DefWindowProcW` destrói.
            // SAFETY: HWND nosso.
            unsafe {
                PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod wl {
    //! `wl_subsurface` da janela GTK via `wayland-client` com display foreign.

    use std::os::raw::c_void;
    use wayland_client::backend::{Backend, ObjectId};
    use wayland_client::protocol::{
        wl_compositor::WlCompositor, wl_region::WlRegion, wl_registry,
        wl_subcompositor::WlSubcompositor, wl_subsurface::WlSubsurface, wl_surface::WlSurface,
    };
    use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};

    pub struct Subsurface {
        // ordem de drop: subsurface → surfaces → conn.
        subsurface: WlSubsurface,
        video: WlSurface,
        parent: WlSurface,
        compositor: WlCompositor,
        conn: Connection,
        /// `wl_surface.set_buffer_scale` do vídeo. Posição/região vêm em pixels
        /// físicos e são divididas por isto pra virar coords lógicas.
        buffer_scale: i32,
    }

    // O `Connection` foreign compartilha o fd do libwayland com o GTK; só
    // tocamos nele na thread principal (event loop). `Send` pro managed state.
    unsafe impl Send for Subsurface {}

    struct Globals {
        compositor: Option<WlCompositor>,
        subcompositor: Option<WlSubcompositor>,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for Globals {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name, interface, ..
            } = event
            {
                match interface.as_str() {
                    "wl_compositor" => {
                        state.compositor =
                            Some(registry.bind::<WlCompositor, _, _>(name, 4, qh, ()));
                    }
                    "wl_subcompositor" => {
                        state.subcompositor =
                            Some(registry.bind::<WlSubcompositor, _, _>(name, 1, qh, ()));
                    }
                    _ => {}
                }
            }
        }
    }

    delegate_noop!(Globals: WlCompositor);
    delegate_noop!(Globals: WlSubcompositor);
    delegate_noop!(Globals: WlSubsurface);
    delegate_noop!(Globals: WlRegion);
    delegate_noop!(Globals: ignore WlSurface);

    impl Subsurface {
        pub fn create(
            display_ptr: *mut c_void,
            parent_surface_ptr: *mut c_void,
            w: u32,
            h: u32,
            buffer_scale: i32,
        ) -> Option<Self> {
            let buffer_scale = buffer_scale.max(1);
            // SAFETY: `display_ptr` é o `wl_display` vivo do GTK (do RawDisplayHandle).
            let backend = unsafe { Backend::from_foreign_display(display_ptr.cast()) };
            let conn = Connection::from_backend(backend);
            let mut queue = conn.new_event_queue::<Globals>();
            let qh = queue.handle();
            let _registry = conn.display().get_registry(&qh, ());
            let mut g = Globals {
                compositor: None,
                subcompositor: None,
            };
            queue.roundtrip(&mut g).ok()?;
            let compositor = g.compositor?;
            let subcompositor = g.subcompositor?;

            // SAFETY: `parent_surface_ptr` é a `wl_surface` viva da janela GTK.
            let parent = unsafe {
                let id =
                    ObjectId::from_ptr(WlSurface::interface(), parent_surface_ptr.cast()).ok()?;
                WlSurface::from_id(&conn, id).ok()?
            };

            let video = compositor.create_surface(&qh, ());
            let subsurface = subcompositor.get_subsurface(&video, &parent, &qh, ());
            subsurface.set_position(0, 0);
            // Subsurface nasce ACIMA do parent (padrão do protocolo) — o jogo
            // cobre a webview opaca enquanto joga; some quando o menu abre e a
            // webview reaparece com o print do jogo de fundo. Sem janela
            // transparente = sem o bug NVIDIA+WebKitGTK.
            // desync: apresenta no ritmo do jogo, não no do GTK.
            subsurface.set_desync();
            // Buffer do wgpu é físico; casa a escala pro compositor tratar a
            // subsurface no mesmo espaço lógico do pai.
            video.set_buffer_scale(buffer_scale);

            // região opaca cobrindo tudo (é o fundo do jogo, sem alpha) — em
            // coords lógicas.
            let (lw, lh) = (
                (w.max(1) as i32 / buffer_scale).max(1),
                (h.max(1) as i32 / buffer_scale).max(1),
            );
            let region = compositor.create_region(&qh, ());
            region.add(0, 0, lw, lh);
            video.set_opaque_region(Some(&region));
            video.commit();
            parent.commit();
            let _ = conn.flush();

            Some(Self {
                subsurface,
                video,
                parent,
                compositor,
                conn,
                buffer_scale,
            })
        }

        pub fn wl_surface_ptr(&self) -> *mut c_void {
            self.video.id().as_ptr().cast()
        }

        /// Esconde a subsurface do jogo: destaca o buffer E manda ela pra
        /// TRÁS do parent (`place_below`) — não só `attach(None)`. Detach de
        /// buffer sozinho depende do compositor recompor a região que a
        /// subsurface cobria (ela nasce ACIMA do parent); em alguns
        /// compositor/driver isso não acontecia de forma confiável e o
        /// último frame do jogo ficava "grudado" na tela mesmo com o buffer
        /// já destacado (relatado 2026-09-12, sempre, não só numa troca
        /// rápida). Mudança de ordem de empilhamento é um mecanismo do
        /// protocolo muito mais básico/testado que "compositor percebe
        /// buffer nulo": com o parent (opaco, sem essa região transparente
        /// por design) na FRENTE, ele aparece garantido, incondicional a
        /// como o compositor trata detach de buffer.
        ///
        /// `show()` (chamado no próximo present real) desfaz com
        /// `place_above`.
        ///
        /// **NÃO chamar `commit()`/`damage_buffer()` no PARENT** — essa
        /// `wl_surface` é gerenciada pelo GTK/GDK (é a janela real); um
        /// commit nosso, por fora do ciclo de desenho dele, colide com o
        /// estado pendente que o GDK mantém pra ela. Tentado 2026-09-12 —
        /// causou `Gdk-Message: Error 22 (Argumento inválido) dispatching to
        /// Wayland display` na hora, revertido. Só mexer nos objetos que
        /// criamos (`self.subsurface`/`self.video`), nunca em `self.parent`.
        pub fn set_hidden(&self, hidden: bool) {
            if hidden {
                self.subsurface.place_below(&self.parent);
                self.video.attach(None, 0, 0);
                self.video.commit();
                let _ = self.conn.flush();
                log::info!(
                    "Subsurface::set_hidden(true) — buffer destacado + movida pra trás do parent"
                );
            }
        }

        /// Desfaz o `set_hidden(true)`: recoloca a subsurface na FRENTE do
        /// parent. Chamar antes do próximo `attach` de um frame novo (senão
        /// o jogo volta a renderizar, mas atrás da webview — invisível).
        pub fn show(&self) {
            self.subsurface.place_above(&self.parent);
            self.video.commit();
            let _ = self.conn.flush();
        }

        /// `x, y, w, h` chegam em pixels FÍSICOS (área de conteúdo da janela);
        /// posição e região vão em coords lógicas (÷ `buffer_scale`).
        pub fn reconfigure(&self, x: i32, y: i32, w: u32, h: u32) {
            let sc = self.buffer_scale.max(1);
            let qh: QueueHandle<Globals> = self.conn.new_event_queue().handle();
            let region = self.compositor.create_region(&qh, ());
            region.add(
                0,
                0,
                (w.max(1) as i32 / sc).max(1),
                (h.max(1) as i32 / sc).max(1),
            );
            self.video.set_opaque_region(Some(&region));
            self.video.set_buffer_scale(sc);
            self.subsurface.set_position(x / sc, y / sc);
            self.video.commit();
            self.parent.commit(); // set_position só aplica no commit do parent
            let _ = self.conn.flush();
        }
    }

    impl Drop for Subsurface {
        fn drop(&mut self) {
            self.subsurface.destroy();
            self.video.destroy();
            let _ = self.conn.flush();
        }
    }
}
