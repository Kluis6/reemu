// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // A surface de vídeo no Linux usa uma child window X11 sob o XID do GTK
    // (ver src/video.rs) — precisa do backend X11/XWayland. Tem que ser
    // setado antes do GTK inicializar (dentro de `run`). Só respeitamos um
    // `GDK_BACKEND` explícito que não seja "wayland".
    #[cfg(target_os = "linux")]
    {
        let cur = std::env::var("GDK_BACKEND").unwrap_or_default();
        if cur.is_empty() || cur == "wayland" {
            std::env::set_var("GDK_BACKEND", "x11");
        }
        // webkitgtk + XWayland/NVIDIA: o renderer DMA-BUF costuma dar
        // "WebKit encountered an internal error" — o composited SW renderer
        // é estável.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    app_lib::run();
}
