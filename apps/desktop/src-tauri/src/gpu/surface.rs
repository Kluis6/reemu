//! Saída na janela: surface, HDR, redimensionamento e apresentação.

use super::*;

impl FrameProcessor {
    /// Anexa uma surface nativa a partir de raw handles (a `wl_surface` de uma
    /// subsurface no Linux; a janela em Win/macOS). A chain passa a desenhar
    /// nela em vez de fazer readback. `false` = falhou (segue no canvas).
    ///
    /// # Safety
    /// `display`/`window` têm que continuar válidos enquanto a surface viver.
    pub unsafe fn attach_surface(
        &mut self,
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
        w: u32,
        h: u32,
    ) -> bool {
        let surface = match unsafe {
            self.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(display),
                    raw_window_handle: window,
                })
        } {
            Ok(s) => s,
            Err(e) => {
                log::warn!("create_surface: {e}");
                return false;
            }
        };
        let caps = surface.get_capabilities(&self.adapter);
        if caps.formats.is_empty() {
            log::warn!("adapter não desenha nessa surface");
            return false;
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        // `REEMU_PRESENT_MODE` (fifo, mailbox, immediate, fiforelaxed) força
        // um modo pra teste (tarefa A4 do TASKS). Só vale se a surface
        // suportar: o wgpu entra em pânico com modo não suportado
        // (docs.rs wgpu 30, `SurfaceConfiguration::present_mode`).
        let forced = std::env::var("REEMU_PRESENT_MODE").ok().and_then(|v| {
            match v.trim().to_ascii_lowercase().as_str() {
                "fifo" => Some(wgpu::PresentMode::Fifo),
                "fiforelaxed" | "fifo_relaxed" => Some(wgpu::PresentMode::FifoRelaxed),
                "mailbox" => Some(wgpu::PresentMode::Mailbox),
                "immediate" => Some(wgpu::PresentMode::Immediate),
                _ => None,
            }
        });
        if let Some(m) = forced.filter(|m| !caps.present_modes.contains(m)) {
            log::warn!(
                "REEMU_PRESENT_MODE={m:?} não suportado por esta surface ({:?}) — ignorado",
                caps.present_modes
            );
        }
        let present_mode = forced
            .filter(|m| caps.present_modes.contains(m))
            .into_iter()
            .chain([
                wgpu::PresentMode::Mailbox,
                wgpu::PresentMode::Immediate,
                wgpu::PresentMode::Fifo,
            ])
            .find(|m| caps.present_modes.contains(m))
            .unwrap_or(wgpu::PresentMode::Fifo);
        // `REEMU_FRAME_LATENCY` (1–3) pra teste; padrão 1. "Desired maximum
        // number of monitor refreshes between a get_current_texture call and
        // the texture being presented"; no Vulkan o swapchain fica com
        // latência + 1 imagens. 1 "minimizes latency but prevents CPU-GPU
        // parallelism" (docs.rs wgpu 30, `SurfaceConfiguration`): o render
        // do pump leva ~2,4 ms, o paralelismo não faz falta. Medido no
        // Windows em 2026-10-02 (RTX 3060, Mailbox): com 2, ~15,6 ms de
        // espera no `get_current_texture` por quadro e 1 quadro perdido a
        // cada ~5 s; com 1, ~0,03 ms e nenhum perdido.
        let frame_latency = std::env::var("REEMU_FRAME_LATENCY")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|n| (1..=3).contains(n))
            .unwrap_or(1);
        // Nunca configurar acima do teto de textura do device (senão o
        // swapchain não é criado e `get_current_texture` fica `Outdated`).
        let cap = self.device.limits().max_texture_dimension_2d;
        let (cw, ch) = (w.clamp(1, cap), h.clamp(1, cap));
        if (cw, ch) != (w, h) {
            log::warn!("surface nativa: {w}x{h} > teto {cap} — limitando a {cw}x{ch}");
        }
        // `Opaque` quando a superfície tiver: "the alpha channel ... is
        // ignored in the compositing process ... treated as if it has a
        // constant alpha of 1.0" (wgpu-types 30, `CompositeAlphaMode`). A
        // lista traz "at least one element, Opaque or Inherit"; com `Inherit`
        // (o 1º em algumas plataformas) quem decide é o compositor, e o alfa 0
        // de alguns cores (PPSSPP) deixava o jogo transparente — tela branca.
        // O blit também grava alfa 1 (`BLIT_WGSL`), pros casos sem `Opaque`.
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        log::info!(
            "surface nativa: alfa {alpha_mode:?} (modos: {:?})",
            caps.alpha_modes
        );
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: cw,
            height: ch,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: frame_latency,
        };
        surface.configure(&self.device, &config);
        // SAFETY: o chamador garante os handles vivos; transmute pro 'static.
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

        let blit_pipeline = blit_pipeline(&self.device, &self.comp.bgl, format);
        // HDR: a surface aceita FP16 em scRGB? (`format_capabilities`.)
        let hdr_capable = caps.format_capabilities.iter().any(|fc| {
            fc.format == wgpu::TextureFormat::Rgba16Float
                && fc
                    .color_spaces
                    .contains(wgpu::SurfaceColorSpaces::EXTENDED_SRGB_LINEAR)
        });
        let blit_rect = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blit rect"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        log::info!(
            "surface nativa: {cw}x{ch} {format:?} {present_mode:?}, latência {frame_latency} (modos suportados: {:?})",
            caps.present_modes
        );
        self.viewport = (cw, ch);
        self.surface = Some(SurfaceOut {
            surface,
            config,
            blit_pipeline,
            blit_rect,
            sdr_format: format,
            hdr_capable,
            hdr_active: false,
            display: Default::default(),
            hdr_used: (0.0, 0.0),
        });
        self.apply_hdr();
        true
    }

    /// Aplica a configuração de HDR à surface: scRGB (`Rgba16Float` +
    /// `ExtendedSrgbLinear`, a opção 1 da Microsoft, "the only option that
    /// works for all types of Advanced Color displays") quando pedida e
    /// suportada; senão o formato SDR de sempre. Relê o monitor
    /// (`display_hdr_info`: o branco SDR "moves with the brightness slider")
    /// e grava os parâmetros do passe final.
    pub fn set_hdr(&mut self, s: HdrSettings) {
        self.hdr = s;
        self.apply_hdr();
    }

    pub(super) fn apply_hdr(&mut self) {
        let settings = self.hdr;
        let Some(s) = self.surface.as_mut() else {
            return;
        };
        s.display = s.surface.display_hdr_info(&self.adapter);
        let want = settings.enabled && s.hdr_capable;
        let (format, color_space) = if want {
            (
                wgpu::TextureFormat::Rgba16Float,
                wgpu::SurfaceColorSpace::ExtendedSrgbLinear,
            )
        } else {
            (s.sdr_format, wgpu::SurfaceColorSpace::Auto)
        };
        if s.config.format != format || s.config.color_space != color_space {
            s.config.format = format;
            s.config.color_space = color_space;
            s.surface.configure(&self.device, &s.config);
            s.blit_pipeline = blit_pipeline_for(&self.device, &self.comp.bgl, format, want);
            log::info!(
                "surface nativa: {format:?} {color_space:?} (HDR {})",
                if want { "ligado" } else { "desligado" }
            );
        }
        s.hdr_active = want;
        let lum = s.display.luminance.unwrap_or_default();
        // Sem o valor do sistema: "On desktop HDR monitors, SDR reference
        // white levels are typically set to around 200 nits" (Microsoft).
        let paper = match settings.paper_white_nits {
            0 => lum.sdr_white_nits.unwrap_or(200.0),
            n => n as f32,
        }
        .max(1.0);
        // Pico automático: o brilho sustentado em tela cheia — "Prefer it
        // over max_nits for large bright regions" (wgpu, `DisplayLuminance`);
        // tela clara inteira é comum em jogo antigo.
        let peak = match settings.peak_nits {
            0 => lum.max_full_frame_nits.or(lum.max_nits).unwrap_or(1000.0),
            n => n as f32,
        };
        let k = if settings.highlights && peak > paper {
            1.0 - paper / peak
        } else {
            0.0
        };
        s.hdr_used = (paper, peak.max(paper));
        self.queue
            .write_buffer(&s.blit_rect, 16, f32s_bytes(&[paper / 80.0, k, 0.0, 0.0]));
    }

    /// A configuração de HDR pedida (pra repassar a um `FrameProcessor` novo).
    pub fn hdr_settings(&self) -> HdrSettings {
        self.hdr
    }

    /// Estado do HDR pra interface (`video_hdr_status`).
    pub fn hdr_status(&self) -> HdrStatus {
        let Some(s) = self.surface.as_ref() else {
            return HdrStatus::default();
        };
        let lum = s.display.luminance.unwrap_or_default();
        HdrStatus {
            surface: true,
            capable: s.hdr_capable,
            active: s.hdr_active,
            system_hdr: s.display.coarse.and_then(|c| c.high_dynamic_range),
            sdr_white_nits: lum.sdr_white_nits,
            peak_nits: lum.max_nits,
            full_frame_nits: lum.max_full_frame_nits,
            paper_white_used: s.hdr_used.0,
            peak_used: s.hdr_used.1,
        }
    }

    pub fn resize_surface(&mut self, w: u32, h: u32) {
        let cap = self.device.limits().max_texture_dimension_2d;
        let (w, h) = (w.clamp(1, cap), h.clamp(1, cap));
        self.viewport = (w, h);
        let mut resized = false;
        if let Some(s) = &mut self.surface {
            if (s.config.width, s.config.height) != (w, h) {
                s.config.width = w;
                s.config.height = h;
                s.surface.configure(&self.device, &s.config);
                self.surface_fail_streak = 0;
                resized = true;
            }
        }
        // A janela pode ter mudado de monitor: relê o HDR dele.
        if resized {
            self.apply_hdr();
        }
        // `scale_type = viewport` mudou de tamanho → realoca alvos.
        for p in &mut self.passes {
            p.bound = false;
        }
    }

    /// Modo canvas (sem surface nativa, ex.: Windows): tamanho em pixels
    /// FÍSICOS da área onde a webview desenha o jogo. É o "viewport" da spec
    /// slang ("the area requested by the frontend, represented by
    /// `FinalViewportSize`" — docs.libretro.com, slang-shaders). Sem ele, a
    /// passada `scale_type = viewport` caía em 3× a nativa (1920x1440 pro
    /// Dreamcast): a máscara de fósforo do CRT era desenhada nesse tamanho e
    /// reduzida no canvas → moiré em listras; e o readback de ~11 MB/quadro
    /// pelo IPC derrubava o desempenho. Com surface nativa o viewport vem
    /// dela (`resize_surface`) e isto não faz nada.
    pub fn set_canvas_viewport(&mut self, w: u32, h: u32) {
        if self.surface.is_some() || w == 0 || h == 0 {
            return;
        }
        let cap = self.device.limits().max_texture_dimension_2d;
        let v = (w.min(cap), h.min(cap));
        if v != self.viewport {
            self.viewport = v;
            for p in &mut self.passes {
                p.bound = false;
            }
        }
    }

    /// Viewport atual (pra preservar ao trocar de `FrameProcessor`).
    pub fn viewport(&self) -> (u32, u32) {
        self.viewport
    }

    /// Liga/desliga integer scaling ao vivo (`update_video_config`) — só
    /// precisa marcar a flag, o próximo `render_to_surface` já lê o valor
    /// novo (sem precisar recarregar o jogo).
    pub fn set_integer_scaling(&mut self, on: bool) {
        self.integer_scaling = on;
    }

    /// Pra carregar o valor atual num `FrameProcessor` NOVO que substitui
    /// este (§Beetle — troca de device Vulkan em runtime, ver `lib.rs`) —
    /// sem isto, o FP novo nasceria sempre com integer scaling desligado.
    pub fn integer_scaling(&self) -> bool {
        self.integer_scaling
    }

    /// Caminho da surface nativa: roda a chain e desenha o resultado (com
    /// letterbox) direto na surface, sem tocar a CPU. Sem frame novo é no-op —
    /// a `wl_surface` segura o último buffer apresentado (freeze no pause).
    pub fn render_to_surface(&mut self, frame: Option<&Frame>) {
        if self.surface.is_none() {
            return;
        }
        let Some(frame) = frame else { return };

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("surface"),
            });
        let Some((out_w, out_h, use_comp)) = self.run_chain(frame, &mut enc) else {
            return;
        };
        self.last_surface_out = Some((out_w, out_h, use_comp));

        // Proporção de exibição: a moldura impõe a sua; senão a AR declarada do
        // core (respeita PAR ≠ 1); só cai na proporção de pixels se não houver.
        // Com rotação de 90°/270° e sem moldura, a AR do core inverte — mais
        // simples usar as dimensões (já rotacionadas) da saída.
        let quarter = matches!(frame.metadata.rotation_degrees % 360, 90 | 270);
        let core_ar = self.effective_aspect(frame.metadata.aspect_ratio);
        let ar_src = self
            .decoration_aspect()
            .filter(|_| use_comp)
            .or(Some(core_ar).filter(|a| *a > 0.0 && !quarter))
            .unwrap_or(out_w as f32 / out_h.max(1) as f32);

        let s = self.surface.as_ref().unwrap();
        let (dw, dh) = (s.config.width.max(1), s.config.height.max(1));
        let ar_dst = dw as f32 / dh as f32;
        // Integer scaling (backlog): só se aplica ao pixel CRU do core, sem
        // moldura (`!use_comp`) — uma moldura/bezel é foto, não pixel art, e
        // já foi desenhada pro retângulo dela (`decoration_aspect()` acima).
        // `native_width/height` não giram sozinhos com `SET_ROTATION`; troca
        // W↔H no giro de 90/270° igual ao `ar_src` acima.
        let (hw, hh) = if self.aspect_mode == AspectMode::Fill && !use_comp {
            (1.0, 1.0)
        } else if self.integer_scaling && !use_comp {
            let nh = if quarter {
                frame.metadata.native_width
            } else {
                frame.metadata.native_height
            };
            integer_scale_rect(dw, dh, nh, ar_src)
        } else if ar_src > ar_dst {
            (1.0, ar_dst / ar_src)
        } else {
            (ar_src / ar_dst, 1.0)
        };
        self.queue
            .write_buffer(&s.blit_rect, 0, f32s_bytes(&[0.0, 0.0, hw, hh]));

        let presented = self.blit_last(enc, use_comp);
        self.surface_redraw_pending = !presented;
    }

    /// Período do monitor medido pelas esperas do `get_current_texture`
    /// (`VblankEstimator`). `None` até somar refreshes suficientes, ou se o
    /// acquire nunca bloqueia (VRR, janela escondida).
    pub fn display_period(&self) -> Option<std::time::Duration> {
        self.vblank.period()
    }

    /// `(get_current_texture, submit + present)` do último quadro apresentado,
    /// e zera. Pro diagnóstico `REEMU_PERF` do pump.
    pub fn take_present_timing(&mut self) -> Option<(std::time::Duration, std::time::Duration)> {
        self.last_present_timing.take()
    }

    /// Reapresenta o último resultado da chain se a apresentação anterior
    /// falhou (ver `blit_last`). Chamado pelo pump quando o core não mandou
    /// quadro novo.
    pub fn redraw_surface_if_pending(&mut self) {
        if !self.surface_redraw_pending || self.surface.is_none() {
            return;
        }
        let Some((_, _, use_comp)) = self.last_surface_out else {
            return;
        };
        let enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("surface redraw"),
            });
        self.surface_redraw_pending = !self.blit_last(enc, use_comp);
    }

    /// Desenha a saída atual da chain (o retângulo já está em `blit_rect`) na
    /// surface e apresenta. `false` se não conseguiu textura da surface.
    pub(super) fn blit_last(&mut self, mut enc: wgpu::CommandEncoder, use_comp: bool) -> bool {
        let s = self.surface.as_ref().unwrap();
        let src_view = if use_comp {
            &self.comp.target.as_ref().unwrap().1
        } else if let Some(v) = &self.rot_view {
            v
        } else {
            &self.passes.last().unwrap().target.as_ref().unwrap().1
        };
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit bg"),
            layout: &self.comp.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: s.blit_rect.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                },
            ],
        });

        let t_acquire = std::time::Instant::now();
        let frame_tex = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => {
                self.surface_fail_streak = 0;
                t
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                self.surface_fail_streak = 0;
                t
            }
            other => {
                self.surface_fail_streak += 1;
                if matches!(self.surface_fail_streak, 1 | 30 | 300) {
                    log::warn!(
                        "surface nativa: get_current_texture {:?} ({}x) — config {}x{}; reconfigurando",
                        std::mem::discriminant(&other),
                        self.surface_fail_streak,
                        s.config.width,
                        s.config.height,
                    );
                }
                // wgpu 30 (`CurrentSurfaceTexture`): Outdated → "Call
                // `Surface::configure()` and try again"; Timeout/Occluded →
                // "skip the current frame and try again later". Reconfigura e
                // tenta de novo JÁ; se ainda assim não vier textura, o quadro
                // fica pendente e o pump reapresenta sem esperar quadro novo
                // do core (cores de Atari mandam quadro repetido — NULL — com
                // a tela parada, e a tela ficava preta até a pausa).
                if matches!(
                    other,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost
                ) {
                    s.surface.configure(&self.device, &s.config);
                    match s.surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(t)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                        _ => return false,
                    }
                } else {
                    return false;
                }
            }
        };
        self.vblank.observe(t_acquire, std::time::Instant::now());
        let view = frame_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        // letterbox = preto ao redor da imagem.
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&s.blit_pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.set_vertex_buffer(0, self.quad.slice(..));
            rp.draw(0..4, 0..1);
        }
        let acquire = t_acquire.elapsed();
        let t_present = std::time::Instant::now();
        self.queue.submit([enc.finish()]);
        self.queue.present(frame_tex);
        self.last_present_timing = Some((acquire, t_present.elapsed()));
        self.surface_fail_streak = 0;
        if !self.surface_presented {
            self.surface_presented = true;
            let s = self.surface.as_ref().unwrap();
            log::info!(
                "surface nativa: 1º frame apresentado ({}x{})",
                s.config.width,
                s.config.height
            );
        }
        true
    }

    /// Apresenta um frame preto opaco na surface nativa. No Windows o pump
    /// chama isto antes de esconder a surface (sem jogo / carregando): a
    /// janela filha escondida guarda o último quadro e podia reaparecer com o
    /// jogo anterior; no protótipo `REEMU_WIN_OVERLAY` ele apareceria pela
    /// `PlayScreen` transparente. Mesmo `LoadOp::Clear` do
    /// letterbox do `blit_last` (wgpu, `RenderPassColorAttachment`).
    pub fn clear_surface(&mut self) {
        let Some(s) = self.surface.as_ref() else {
            return;
        };
        let frame_tex = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return,
        };
        let view = frame_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear surface"),
            });
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.queue.submit([enc.finish()]);
        self.queue.present(frame_tex);
    }

    /// Lê de volta o último frame que foi pra surface nativa (RGBA8 apertado) —
    /// pra usar de fundo do menu de pausa. Bloqueante; chamado 1× por pause,
    /// não no caminho de render.
    pub fn capture_surface_frame(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        let (w, h, use_comp) = self.last_surface_out?;
        let src = if use_comp {
            &self.comp.target.as_ref()?.0
        } else if let Some((t, _, _, _)) = self.rot_tgt.as_ref().filter(|_| self.rot_view.is_some())
        {
            t
        } else {
            &self.passes.last()?.target.as_ref()?.0
        };
        let padded = w * 4 + (ROW_ALIGN - (w * 4) % ROW_ALIGN) % ROW_ALIGN;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("capture"),
            });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: src,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let rgba = {
            let mapped = buf.slice(..).get_mapped_range().ok()?;
            unpad_rows(&mapped, w, h, padded)
        };
        buf.unmap();
        Some((w, h, rgba))
    }
}
