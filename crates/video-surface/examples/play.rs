//! Player standalone: abre uma janela wgpu, carrega um core libretro numa
//! `EmuSession` e desenha os frames. Espaço = pausa/resume.
//!
//!   cargo run -p video-surface --example play -- <core.so> <rom>
//!
//! Sem argumentos, usa o core-fake (`testcore`) — a janela mostra uma cor
//! que muda a cada frame (prova que o pipeline frame→textura→tela funciona).

use std::sync::Arc;
use video_surface::{create_device, Renderer};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

struct App {
    session: Arc<emu_session::EmuSession>,
    paused: bool,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("ReEmu — video-surface");
        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let (adapter, device, queue) =
            create_device(&instance, Some(&surface)).expect("nenhum adapter wgpu");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, format);
        self.gpu = Some(Gpu {
            surface,
            device,
            queue,
            config,
            renderer,
        });
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                gpu.config.width = size.width.max(1);
                gpu.config.height = size.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.config);
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Space),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                self.paused = !self.paused;
                self.session.set_paused(self.paused);
            }
            WindowEvent::RedrawRequested => {
                if let Some(frame) = self.session.take_latest_frame() {
                    gpu.renderer.upload(&gpu.device, &gpu.queue, &frame);
                }
                use wgpu::CurrentSurfaceTexture as Cst;
                match gpu.surface.get_current_texture() {
                    Cst::Success(t) | Cst::Suboptimal(t) => {
                        let view = t
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                        gpu.renderer.render(
                            &gpu.device,
                            &gpu.queue,
                            &view,
                            gpu.config.width,
                            gpu.config.height,
                        );
                        gpu.queue.present(t);
                    }
                    Cst::Outdated | Cst::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                    }
                    _ => {}
                }
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let core = args
        .next()
        .unwrap_or_else(|| core_loader_desktop::testcore_path().to_string());
    let rom = args.next().unwrap_or_else(|| {
        let p = std::env::temp_dir().join("reemu-play-dummy.bin");
        std::fs::write(&p, b"dummy").unwrap();
        p.to_string_lossy().into_owned()
    });

    let tmp = std::env::temp_dir();
    let session = Arc::new(emu_session::EmuSession::spawn(
        emu_session::SessionConfig::new(tmp.clone(), tmp.clone(), tmp),
    ));
    match session.load(&core, &rom) {
        Ok(av) => log::info!(
            "core carregado: {}x{} @ {} fps",
            av.geometry.base_width,
            av.geometry.base_height,
            av.timing.fps
        ),
        Err(e) => {
            eprintln!("falha ao carregar o core: {e}");
            std::process::exit(1);
        }
    }

    let event_loop = EventLoop::new().unwrap();
    event_loop
        .run_app(&mut App {
            session,
            paused: false,
            window: None,
            gpu: None,
        })
        .unwrap();
}
