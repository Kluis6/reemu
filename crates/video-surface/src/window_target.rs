//! `WindowTarget`: um `Renderer` + a `wgpu::Surface` de uma janela nativa
//! (criada a partir de raw handles — o shell Tauri passa os do
//! `WebviewWindow`). Renderiza os frames do core direto na janela.

use crate::renderer::{create_device, Renderer};
use domain::frame_source::Frame;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

pub struct WindowTarget {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

impl WindowTarget {
    /// # Safety
    /// `display` e `window` têm que continuar válidos enquanto o
    /// `WindowTarget` viver (a janela do Tauri vive o app inteiro, então ok).
    pub unsafe fn from_raw_handles(
        display: RawDisplayHandle,
        window: RawWindowHandle,
        width: u32,
        height: u32,
    ) -> Option<Self> {
        let instance = wgpu::Instance::default();
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(display),
                    raw_window_handle: window,
                })
                .ok()?
        };

        let (adapter, device, queue) = create_device(&instance, Some(&surface))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        // Mailbox/Immediate não bloqueiam em present — importante quando o
        // render divide a thread com o event loop do shell.
        let present_mode = [
            wgpu::PresentMode::Mailbox,
            wgpu::PresentMode::Immediate,
            wgpu::PresentMode::Fifo,
        ]
        .into_iter()
        .find(|m| caps.present_modes.contains(m))
        .unwrap_or(wgpu::PresentMode::Fifo);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: width.max(1),
            height: height.max(1),
            present_mode,
            // Pré-multiplicado quando disponível — deixa a webview compor por cima.
            alpha_mode: caps
                .alpha_modes
                .iter()
                .copied()
                .find(|m| *m == wgpu::CompositeAlphaMode::PreMultiplied)
                .unwrap_or(caps.alpha_modes[0]),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, format);
        Some(Self {
            surface,
            device,
            queue,
            config,
            renderer,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    /// Sobe `frame` (se houver) e desenha na janela. Sem frame novo, redesenha
    /// o último (não limpa) — o pause simplesmente para de passar frames.
    pub fn render(&mut self, frame: Option<&Frame>) {
        if let Some(f) = frame {
            self.renderer.upload(&self.device, &self.queue, f);
        }
        if !self.renderer.has_frame() {
            return; // nada pra desenhar ainda — deixa a janela transparente
        }

        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                let view = t
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                self.renderer.render(
                    &self.device,
                    &self.queue,
                    &view,
                    self.config.width,
                    self.config.height,
                );
                self.queue.present(t);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
            }
            _ => {}
        }
    }
}
