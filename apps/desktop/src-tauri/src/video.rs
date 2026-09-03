//! Surface nativa de vídeo — o jogo (wgpu) atrás da webview transparente.
//!
//! ## Linux (Wayland)
//!
//! O GTK é dono da submissão de buffer da `wl_surface` da janela — anexar um
//! swapchain nela dá `Gdk-Message: Error 71`. E o backend X11 (child window)
//! faz a webview não pintar nesse combo (WebKitGTK + NVIDIA + XWayland). O
//! caminho que sobra é o jeito Wayland-nativo: uma **`wl_subsurface`** da janela
//! GTK, `place_below`, com `wl_surface` própria que o wgpu dirige. O compositor
//! empilha a subsurface embaixo; a webview (fundo transparente) compõe por cima.
//!
//! Só liga com `REEMU_NATIVE_VIDEO=1` (o padrão segue no `<canvas>`).
//!
//! ## Windows / macOS
//!
//! Surface direto no handle da janela (webview transparente compõe por cima).
//! Não verificado.

use raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle, WaylandWindowHandle,
};
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
    #[cfg(not(target_os = "linux"))]
    _priv: (),
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
            let sub = wl::Subsurface::create(
                d.display.as_ptr(),
                w.surface.as_ptr(),
                size.width,
                size.height,
            )?;
            let wh = WaylandWindowHandle::new(NonNull::new(sub.wl_surface_ptr())?);
            log::info!(
                "surface de vídeo: wl_subsurface ({}x{})",
                size.width,
                size.height
            );
            (
                Self { _wl: sub },
                SurfaceHandles {
                    display,
                    window: RawWindowHandle::Wayland(wh),
                },
            )
        };

        #[cfg(not(target_os = "linux"))]
        let (this, handles) = (Self { _priv: () }, SurfaceHandles { display, window });

        Some((this, handles))
    }

    pub fn resize(&self, width: u32, height: u32) {
        #[cfg(target_os = "linux")]
        self._wl.resize(width, height);
        let _ = (width, height);
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
        ) -> Option<Self> {
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
            subsurface.place_below(&parent);
            // desync: a subsurface apresenta no ritmo do jogo, não no do GTK.
            subsurface.set_desync();

            // região opaca cobrindo tudo — o compositor não mistura alpha atrás.
            let region = compositor.create_region(&qh, ());
            region.add(0, 0, w.max(1) as i32, h.max(1) as i32);
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
            })
        }

        pub fn wl_surface_ptr(&self) -> *mut c_void {
            self.video.id().as_ptr().cast()
        }

        pub fn resize(&self, w: u32, h: u32) {
            let qh: QueueHandle<Globals> = self.conn.new_event_queue().handle();
            let region = self.compositor.create_region(&qh, ());
            region.add(0, 0, w.max(1) as i32, h.max(1) as i32);
            self.video.set_opaque_region(Some(&region));
            self.video.commit();
            self.parent.commit();
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
