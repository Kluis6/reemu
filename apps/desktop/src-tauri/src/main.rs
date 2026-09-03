// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // A surface de vídeo no Linux usa uma child window X11 sob o XID do GTK
    // (ver src/video.rs) — precisa do backend X11/XWayland. Tem que ser
    // setado antes do GTK inicializar (dentro de `run`). Só respeitamos um
    // `GDK_BACKEND` explícito que não seja "wayland".
    #[cfg(target_os = "linux")]
    {
        // A child window X11 pro vídeo (ver src/video.rs) exige XWayland — e
        // nesse combo XWayland + WebKitGTK 2.52 + NVIDIA a webview simplesmente
        // não pinta na tela (o DOM renderiza, os pixels não chegam). Então o
        // padrão agora é **sem** a child window: deixa o GTK usar Wayland
        // nativo, que renderiza a UI de verdade. O vídeo do jogo passa a ser
        // desenhado dentro da webview (canvas). `REEMU_X11_VIDEO=1` volta ao
        // esquema antigo pra quem quiser testar.
        if std::env::var_os("REEMU_X11_VIDEO").is_some() {
            let cur = std::env::var("GDK_BACKEND").unwrap_or_default();
            if cur.is_empty() || cur == "wayland" {
                std::env::set_var("GDK_BACKEND", "x11");
            }
        }
        // O DMA-BUF renderer do WebKitGTK dá "internal error"/tela branca nesse
        // combo NVIDIA — sempre desligado (o SW renderer é o estável).
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Compositing acelerado: no padrão (canvas) fica DESLIGADO (mais
        // estável). No vídeo nativo precisa ficar LIGADO — a transparência da
        // página (o "buraco" pro subsurface) só funciona com compositing.
        if std::env::var_os("REEMU_NATIVE_VIDEO").is_none()
            && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none()
        {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    app_lib::run();
}
