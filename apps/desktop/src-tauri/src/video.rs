//! Surface nativa de vídeo — o jogo (wgpu) atrás da webview transparente.
//!
//! ## Linux
//!
//! O GTK (que o wry usa em toda janela) é dono da submissão de buffer da
//! `wl_surface` da janela — anexar um swapchain Vulkan nela dá
//! `Gdk-Message: Error 71` e crash. Solução: sob X11/XWayland, criamos uma
//! **child window X11** (`XCreateSimpleWindow`) filha do XID do GTK,
//! abaixada (`XLowerWindow`) para ficar atrás do conteúdo da webview, e a
//! `wgpu::Surface` vai nela. O GTK não gerencia o conteúdo dessa child.
//! (`main.rs` força `GDK_BACKEND=x11` no Linux por isso.)
//!
//! ## Windows / macOS
//!
//! A webview transparente compõe sobre a camada nativa da mesma janela —
//! a surface vai direto no handle da janela principal. **Não verificado**
//! (sem máquina).
//!
//! Render na thread principal, a cada `RunEvent::MainEventsCleared`.

use emu_session::EmuSession;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use tauri::{AppHandle, Manager, Runtime};
use video_surface::WindowTarget;

pub struct VideoSurface {
    target: WindowTarget,
    #[cfg(target_os = "linux")]
    _x11: Option<x11::X11Child>,
}

impl VideoSurface {
    pub fn spawn<R: Runtime>(app: &AppHandle<R>) -> Option<Self> {
        let main = app.get_webview_window("main")?;
        let size = main.inner_size().ok()?;
        let raw_display = main.display_handle().ok()?.as_raw();
        let raw_window = main.window_handle().ok()?.as_raw();

        #[cfg(target_os = "linux")]
        match (raw_display, raw_window) {
            (RawDisplayHandle::Xlib(disp), RawWindowHandle::Xlib(win)) => {
                let child = x11::X11Child::create(disp, win.window, size.width, size.height)?;
                let (cdisp, cwin) = child.raw_handles();
                // SAFETY: a child window vive dentro do `VideoSurface` (Drop a destrói).
                let target = unsafe {
                    WindowTarget::from_raw_handles(cdisp, cwin, size.width, size.height)?
                };
                log::info!(
                    "surface de vídeo: child window X11 {:#x} ({}x{})",
                    child.xid(),
                    size.width,
                    size.height
                );
                Some(Self {
                    target,
                    _x11: Some(child),
                })
            }
            (RawDisplayHandle::Wayland(_), _) => {
                log::warn!(
                    "vídeo no app precisa de X11 — rode com GDK_BACKEND=x11 \
                     (ou use `cargo run -p video-surface --example play`)"
                );
                None
            }
            _ => None,
        }

        #[cfg(not(target_os = "linux"))]
        {
            // SAFETY: a janela do Tauri vive o app inteiro.
            let target = unsafe {
                WindowTarget::from_raw_handles(raw_display, raw_window, size.width, size.height)?
            };
            log::info!(
                "surface de vídeo anexada à janela ({}x{})",
                size.width,
                size.height
            );
            Some(Self { target })
        }
    }

    pub fn render(&mut self, session: &EmuSession) {
        let frame = session.take_latest_frame();
        self.target.render(frame.as_ref());
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        #[cfg(target_os = "linux")]
        if let Some(c) = &self._x11 {
            c.resize(width, height);
        }
        self.target.resize(width, height);
    }
}

#[cfg(target_os = "linux")]
mod x11 {
    use raw_window_handle::{
        RawDisplayHandle, RawWindowHandle, XlibDisplayHandle, XlibWindowHandle,
    };
    use std::ffi::c_ulong;
    use std::ptr::NonNull;
    use x11_dl::xlib::{Display, Xlib};

    pub struct X11Child {
        xlib: Xlib,
        display: *mut Display,
        screen: i32,
        window: c_ulong,
        visual_id: u32,
    }

    // O `*mut Display` é compartilhado com o GTK; só tocamos nele na thread
    // principal (onde o event loop roda). `Send` é pra caber no managed state.
    unsafe impl Send for X11Child {}

    impl X11Child {
        pub fn create(
            disp: XlibDisplayHandle,
            parent: c_ulong,
            width: u32,
            height: u32,
        ) -> Option<Self> {
            let xlib = Xlib::open().ok()?;
            let display = disp.display?.as_ptr() as *mut Display;
            let window = unsafe {
                (xlib.XCreateSimpleWindow)(
                    display,
                    parent,
                    0,
                    0,
                    width.max(1),
                    height.max(1),
                    0,
                    0,
                    0, // fundo preto
                )
            };
            if window == 0 {
                return None;
            }
            let visual_id = unsafe {
                let vis = (xlib.XDefaultVisual)(display, disp.screen);
                (xlib.XVisualIDFromVisual)(vis) as u32
            };
            unsafe {
                (xlib.XLowerWindow)(display, window); // atrás do conteúdo da webview
                (xlib.XMapWindow)(display, window);
                (xlib.XFlush)(display);
            }
            Some(Self {
                xlib,
                display,
                screen: disp.screen,
                window,
                visual_id,
            })
        }

        pub fn xid(&self) -> c_ulong {
            self.window
        }

        pub fn resize(&self, width: u32, height: u32) {
            unsafe {
                (self.xlib.XResizeWindow)(self.display, self.window, width.max(1), height.max(1));
                (self.xlib.XFlush)(self.display);
            }
        }

        pub fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
            let mut d = XlibDisplayHandle::new(NonNull::new(self.display as *mut _), self.screen);
            let mut w = XlibWindowHandle::new(self.window);
            w.visual_id = self.visual_id as c_ulong;
            let _ = &mut d;
            let _ = &mut w;
            (RawDisplayHandle::Xlib(d), RawWindowHandle::Xlib(w))
        }
    }

    impl Drop for X11Child {
        fn drop(&mut self) {
            unsafe {
                (self.xlib.XDestroyWindow)(self.display, self.window);
                (self.xlib.XFlush)(self.display);
            }
        }
    }
}
