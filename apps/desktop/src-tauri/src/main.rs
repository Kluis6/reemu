// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // A surface de vídeo nativa é um `wl_subsurface` da janela GTK (Wayland
    // nativo — ver src/video.rs). Nada de child window X11: naquele combo
    // XWayland + WebKitGTK + NVIDIA a webview monta o DOM mas não pinta.
    #[cfg(target_os = "linux")]
    configure_webkit_gpu();

    app_lib::run();
}

/// Compositing acelerado do WebKitGTK.
///
/// Com o driver **NVIDIA proprietário** o DMA-BUF renderer + compositing
/// acelerado entram em loop de `internallyFailedLoadTimerFired` (o GPU process
/// cai) → tela branca. Por isso, nesse caso, forçamos o renderer de software.
///
/// Em AMD/Intel (Mesa) o caminho de GPU é estável e deixa as animações do
/// frontend MUITO mais leves (sem ele, todo `transform`/`opacity`/`filter` é
/// repintado na CPU). Então lá deixamos ligado.
///
/// `REEMU_WEBKIT_COMPOSITING=1|0` força (útil pra testar em NVIDIA com driver
/// novo, onde o bug pode já estar resolvido).
#[cfg(target_os = "linux")]
fn configure_webkit_gpu() {
    let accel = match std::env::var("REEMU_WEBKIT_COMPOSITING").ok().as_deref() {
        Some("1" | "on" | "true" | "yes") => Some(true),
        Some("0" | "off" | "false" | "no") => Some(false),
        _ => None,
    }
    .unwrap_or_else(|| !is_nvidia_proprietary());

    if accel {
        eprintln!("webkit: compositing acelerado LIGADO (GPU não-NVIDIA ou forçado)");
        return;
    }

    eprintln!("webkit: compositing acelerado DESLIGADO (NVIDIA proprietário — evita tela branca)");
    for (k, v) in [
        ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
    ] {
        if std::env::var_os(k).is_none() {
            std::env::set_var(k, v);
        }
    }
}

/// `true` se o driver NVIDIA proprietário está carregado (o `nouveau` livre
/// não cria estes caminhos e usa Mesa, sem o bug).
#[cfg(target_os = "linux")]
fn is_nvidia_proprietary() -> bool {
    std::path::Path::new("/proc/driver/nvidia/version").exists()
        || std::path::Path::new("/dev/nvidia0").exists()
        || std::env::var("__GLX_VENDOR_LIBRARY_NAME").is_ok_and(|v| v == "nvidia")
}
