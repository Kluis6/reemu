//! Processamento e leitura de volta do quadro, inclusive o preview parado.

use super::*;

impl FrameProcessor {
    /// Preview de shader (Configurações › Vídeo): roda a chain `runs` vezes
    /// sobre a mesma imagem parada, com a saída em `out_w`×`out_h`, e devolve
    /// o último resultado (RGBA8). Várias passadas porque o readback tem 1
    /// quadro de atraso e presets com histórico/feedback leem quadros
    /// anteriores. Só pra processador sem surface (o viewport vem daqui).
    pub fn render_still(
        &mut self,
        frame: &Frame,
        out_w: u32,
        out_h: u32,
        runs: usize,
    ) -> Option<(u32, u32, Vec<u8>)> {
        self.set_canvas_viewport(out_w, out_h);
        // Erro de validação no meio da chain (ver `set_preset`) vira "sem
        // imagem" em vez de pânico no handler de erro não capturado.
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut last = None;
        for _ in 0..runs.max(2) {
            if let Some((w, h, px)) = self.process(frame) {
                last = Some((w, h, px.to_vec()));
            }
        }
        if let Some(e) = pollster::block_on(scope.pop()) {
            log::warn!("preview do shader '{}' falhou: {e}", self.preset_name);
            return None;
        }
        last
    }

    /// Roda a chain e lê o resultado de volta pra CPU (RGBA8, sem cabeçalho).
    /// `None` se não há frame novo pronto ainda (readback com 1 quadro de
    /// atraso — ver o comentário de `rb`). O `&[u8]` é emprestado de
    /// `self.readback_scratch`. Usado pelo `render_still` e pelos testes; o
    /// canvas usa `process_packed`.
    pub fn process(&mut self, frame: &Frame) -> Option<(u32, u32, &[u8])> {
        let (slot, w, h, padded) = self.readback_frame(frame)?;
        {
            let mapped = self.rb.slots[slot]
                .as_ref()?
                .buf
                .slice(..)
                .get_mapped_range()
                .ok()?;
            unpad_rows_into(&mut self.readback_scratch, &mapped, w, h, padded);
        }
        self.release_readback(slot);
        Some((w, h, self.readback_scratch.as_slice()))
    }

    /// Igual ao `process`, mas já no formato do `poll_frame`: `[w u32 LE]
    /// [h u32 LE][RGBA…]` num `Vec` só, pronto pra virar a resposta IPC. O
    /// `process` + montar o cabeçalho copiava o frame inteiro de novo
    /// (8 MB por frame em 1080p); aqui o readback escreve direto no destino.
    #[cfg(test)]
    pub fn process_packed(&mut self, frame: &Frame) -> Option<Vec<u8>> {
        self.process_packed_inner(frame, false)
    }

    /// Formato do `poll_frame` no modo canvas: `[w][h][deco_gen u32]
    /// [retângulo do jogo 4×f32][proporção f32]` (32 bytes, LE) + RGBA só do
    /// jogo. Com
    /// `deco_gen == 0` não há moldura e o retângulo não vale. Liga
    /// `split_decoration` (a moldura vai pelo `decoration_image`).
    pub fn process_packed_split(&mut self, frame: &Frame) -> Option<Vec<u8>> {
        self.split_decoration = true;
        self.process_packed_inner(frame, true)
    }

    /// `(geração, imagem RGBA, largura, altura)` da moldura atual.
    pub fn decoration_image(&self) -> Option<(u32, &[u8], u32, u32)> {
        self.decoration
            .as_ref()
            .map(|d| (self.deco_gen, d.rgba.as_slice(), d.w, d.h))
    }

    pub(super) fn process_packed_inner(
        &mut self,
        frame: &Frame,
        split_header: bool,
    ) -> Option<Vec<u8>> {
        let (slot, w, h, padded) = self.readback_frame(frame)?;
        let out = {
            let mapped = self.rb.slots[slot]
                .as_ref()?
                .buf
                .slice(..)
                .get_mapped_range()
                .ok()?;
            let row = (w * 4) as usize;
            let mut out = Vec::with_capacity(36 + row * h as usize);
            out.extend_from_slice(&w.to_le_bytes());
            out.extend_from_slice(&h.to_le_bytes());
            if split_header {
                let gen = if self.decoration.is_some() && !self.deco_suppressed {
                    self.deco_gen
                } else {
                    0
                };
                out.extend_from_slice(&gen.to_le_bytes());
                for v in self.game_rect {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                out.extend_from_slice(
                    &self
                        .effective_aspect(frame.metadata.aspect_ratio)
                        .to_le_bytes(),
                );
                out.extend_from_slice(&self.deco_zoom.to_le_bytes());
            }
            for y in 0..h as usize {
                out.extend_from_slice(&mapped[y * padded as usize..][..row]);
            }
            out
        };
        self.release_readback(slot);
        Some(out)
    }

    /// Roda a chain, pede o readback deste frame e deixa MAPEADO o slot do
    /// frame anterior (pipeline de 1 quadro — ver `rb`). Devolve
    /// `(slot, w, h, bytes_por_linha_com_padding)`; o chamador copia do
    /// mapeamento e chama `release_readback(slot)`.
    pub(super) fn readback_frame(&mut self, frame: &Frame) -> Option<(usize, u32, u32, u32)> {
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("etapa04"),
            });
        let (out_w, out_h, use_comp) = self.run_chain(frame, &mut enc)?;

        // --- readback com pipeline ---
        self.rb.ensure(&self.device, out_w, out_h);
        let write_i = self.rb.write;
        let read_i = write_i ^ 1;

        // 1. codifica a cópia GPU→buffer no slot de escrita e submete.
        {
            let ws = self.rb.slots[write_i].as_ref()?;
            let src_tex = if use_comp {
                &self.comp.target.as_ref()?.0
            } else if let Some((t, _, _, _)) =
                self.rot_tgt.as_ref().filter(|_| self.rot_view.is_some())
            {
                t
            } else {
                &self.passes.last()?.target.as_ref()?.0
            };
            enc.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: src_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &ws.buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(ws.padded),
                        rows_per_image: Some(out_h),
                    },
                },
                wgpu::Extent3d {
                    width: out_w,
                    height: out_h,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.queue.submit([enc.finish()]);

        // 2. pede o map do slot recém-escrito (o callback marca `ready`).
        {
            let ws = self.rb.slots[write_i].as_mut()?;
            ws.ready.store(false, Ordering::Relaxed);
            let ready = ws.ready.clone();
            ws.buf.slice(..).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    ready.store(true, Ordering::Relaxed);
                }
            });
            ws.inflight = true;
        }

        // 3. drena callbacks pendentes sem bloquear.
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.rb.write = read_i;

        // 4. lê o slot do frame anterior. Se ainda não mapeou (GPU atrasada),
        //    aí sim espera — raro, e melhor que perder o frame.
        let rs = self.rb.slots[read_i].as_mut()?;
        if !rs.inflight {
            return None; // 1º frame / logo após resize
        }
        if !rs.ready.load(Ordering::Relaxed) {
            self.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        }
        Some((read_i, rs.w, rs.h, rs.padded))
    }

    pub(super) fn release_readback(&mut self, slot: usize) {
        if let Some(rs) = self.rb.slots[slot].as_mut() {
            rs.buf.unmap();
            rs.inflight = false;
        }
    }

    pub(super) fn ensure_comp_target(&mut self, w: u32, h: u32) {
        if matches!(&self.comp.target, Some((_, _, tw, th)) if *tw == w && *th == h) {
            return;
        }
        let (t, v) = new_tex(
            &self.device,
            w,
            h,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                // `render_to_surface` samplia esta textura no blit pra surface.
                | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        self.comp.target = Some((t, v, w, h));
    }

    /// Alvo pra o passe de rotação (`SET_ROTATION`). `false` se `w`/`h` for 0.
    pub(super) fn ensure_rot_target(&mut self, w: u32, h: u32) -> bool {
        if w == 0 || h == 0 {
            return false;
        }
        if !matches!(&self.rot_tgt, Some((_, _, tw, th)) if *tw == w && *th == h) {
            let (t, v) = new_tex(
                &self.device,
                w,
                h,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            );
            self.rot_tgt = Some((t, v, w, h));
        }
        true
    }

    /// Garante `history_depth` slots do frame do core, todos `w`×`h`. `[0]` é o
    /// frame atual (`Original`); `[n]`, n frames atrás (`OriginalHistoryN`).
    pub(super) fn ensure_history(&mut self, w: u32, h: u32) {
        let ok = self.history.len() == self.history_depth
            && self
                .history
                .iter()
                .all(|(_, _, tw, th)| *tw == w && *th == h);
        if ok {
            return;
        }
        self.history = (0..self.history_depth)
            .map(|_| {
                let (t, v) = new_tex(
                    &self.device,
                    w,
                    h,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                (t, v, w, h)
            })
            .collect();
        for p in &mut self.passes {
            p.bound = false;
        }
    }

    /// Rotaciona o ring de history e grava o frame novo em `[0]`.
    pub(super) fn push_history(&mut self, rgba: &[u8], w: u32, h: u32) {
        if self.history.is_empty() {
            return;
        }
        self.history.rotate_right(1);
        let (tex, _, _, _) = &self.history[0];
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        for p in &mut self.passes {
            p.bound = false;
        }
    }
}
