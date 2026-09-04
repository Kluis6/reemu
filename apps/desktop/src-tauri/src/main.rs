// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // A surface de vídeo nativa é um `wl_subsurface` da janela GTK (Wayland
    // nativo — ver src/video.rs). Nada de child window X11: naquele combo
    // XWayland + WebKitGTK + NVIDIA a webview monta o DOM mas não pinta.
    #[cfg(target_os = "linux")]
    {
        // O DMA-BUF renderer do WebKitGTK dá "internal error"/tela branca nesse
        // combo NVIDIA — sempre desligado (o SW renderer é o estável).
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Compositing acelerado: SEMPRE desligado nesse combo NVIDIA. Com ele
        // ligado o WebProcess entra em loop de `internallyFailedLoadTimerFired`
        // (GPU process caindo) → tela branca. O vídeo nativo hoje usa subsurface
        // `place_above` + hide-no-menu (ver video.rs), então a página é OPACA e
        // não precisa de compositing pra "furar" nada.
        if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    app_lib::run();
}
