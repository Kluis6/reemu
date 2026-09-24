//! Execução da cadeia de shader: passes, alvos, mips e samplers.

use super::*;

impl FrameProcessor {
    /// Roda a cadeia inteira (entrada + N passes + composição da moldura)
    /// gravando em `enc`. Devolve `(out_w, out_h, com_moldura)` — a textura
    /// final ainda não foi consumida (readback ou blit fica pro chamador).
    pub(super) fn run_chain(
        &mut self,
        frame: &Frame,
        enc: &mut wgpu::CommandEncoder,
    ) -> Option<(u32, u32, bool)> {
        let nw = frame.metadata.native_width;
        let nh = frame.metadata.native_height;
        if nw == 0 || nh == 0 {
            log::warn!("run_chain: frame com dimensão zero (nw={nw} nh={nh}) — pulando");
            return None;
        }
        // Entrada da chain: buffer cru (vai pro ring de history) ou textura
        // dma_buf já na GPU (interop; history fica como o frame atual).
        match &frame.origin {
            FrameOrigin::SoftwareRawBuffer {
                data,
                pitch,
                format,
            } => {
                // Buffer reusado (sai do `self` só pra o `push_history`
                // poder pegar `&mut self` junto).
                let mut rgba = std::mem::take(&mut self.rgba_scratch);
                to_rgba8_into(&mut rgba, data, nw, nh, *pitch, *format);
                self.ensure_history(nw, nh);
                self.push_history(&rgba, nw, nh);
                self.rgba_scratch = rgba;
                self.interop_view = None;
            }
            FrameOrigin::HardwareTexture(handle) => {
                if !self.bind_interop_input(handle.as_ref(), nw, nh, enc) {
                    log::warn!(
                        "run_chain: bind_interop_input falhou pra {nw}x{nh} — pulando frame"
                    );
                    return None;
                }
                // a entrada troca de slot a cada frame → rebuild de todo bg
                for p in &mut self.passes {
                    p.bound = false;
                }
            }
            FrameOrigin::HardwareVulkanImage(handle) => {
                if !self.bind_vulkan_input(handle.as_ref()) {
                    log::warn!("run_chain: bind_vulkan_input falhou — pulando frame");
                    return None;
                }
                for p in &mut self.passes {
                    p.bound = false;
                }
            }
        }

        // dimensões de cada alvo (`viewport` usa o tamanho real da saída).
        let vp = self.viewport;
        let mut sizes = Vec::with_capacity(self.passes.len());
        let (mut cw, mut ch) = (nw, nh);
        for p in &self.passes {
            cw = axis_size(p.scale_x, cw, nw, vp.0);
            ch = axis_size(p.scale_y, ch, nh, vp.1);
            sizes.push((cw, ch));
        }
        let Some(&(fw, fh)) = sizes.last() else {
            log::warn!("run_chain: nenhum passe de shader configurado — pulando frame");
            return None;
        };
        if fw * fh > MAX_OUT_PIXELS {
            log::warn!(
                "run_chain: saída final {fw}x{fh} ({} px) excede MAX_OUT_PIXELS ({MAX_OUT_PIXELS}) — tela ficaria preta, pulando frame. Verifique os passes do shader ativo (viewport={}x{}, entrada={nw}x{nh}).",
                fw as u64 * fh as u64,
                vp.0,
                vp.1
            );
            return None;
        }
        let final_vp = if vp.0 > 0 { vp } else { (fw, fh) };

        self.frame_count = self.frame_count.wrapping_add(1);
        let fc = self.frame_count;

        for (idx, (pw, ph)) in sizes.iter().copied().enumerate() {
            self.ensure_target(idx, pw, ph);
            let (in_w, in_h) = if idx == 0 { (nw, nh) } else { sizes[idx - 1] };
            let fc_pass = {
                let m = self.passes[idx].frame_count_mod as u64;
                if m > 0 {
                    fc % m
                } else {
                    fc
                }
            };
            // nome-base de cada textura do passe → tamanho (pro `<Nome>Size`).
            let tex_sizes = self.tex_sizes_for(idx, (in_w, in_h), (nw, nh), &sizes);
            match &self.passes[idx].uniform {
                UniformMode::Fixed => {
                    let mut b = vec![0u8; 64];
                    b[0..16].copy_from_slice(f32s_bytes(&size_vec(in_w, in_h)));
                    b[16..32].copy_from_slice(f32s_bytes(&size_vec(pw, ph)));
                    b[32..48].copy_from_slice(f32s_bytes(&size_vec(nw, nh)));
                    b[48..64].copy_from_slice(f32s_bytes(&[fc_pass as f32, 1.0, 0.0, 0.0]));
                    self.queue.write_buffer(&self.passes[idx].ubuf[0], 0, &b);
                }
                UniformMode::Slang(blocks) => {
                    for (binding, layout) in blocks {
                        let b = fill_slang(
                            layout,
                            &self.params,
                            (in_w, in_h),
                            (pw, ph),
                            (nw, nh),
                            final_vp,
                            fc_pass,
                            &tex_sizes,
                        );
                        let slot = if *binding == 0 { 0 } else { 1 };
                        self.queue.write_buffer(&self.passes[idx].ubuf[slot], 0, &b);
                    }
                }
            }
        }

        // Samplers de cada (passe, textura) — precisa de `&mut self` (cache), então
        // resolvemos antes da seção de bind groups (que só empresta `&self`).
        let samplers: Vec<Vec<wgpu::Sampler>> = (0..self.passes.len())
            .map(|idx| {
                let (linear, wrap) = (self.passes[idx].linear, self.passes[idx].wrap);
                (0..self.passes[idx].textures.len())
                    .map(|t| {
                        let sem = self.passes[idx].textures[t].semantic.clone();
                        if let TextureSemantic::Named {
                            name,
                            feedback: false,
                        } = &sem
                        {
                            if let Some((_, _, s, _, _)) = self.luts.get(name) {
                                return s.clone();
                            }
                        }
                        self.sampler_for(linear, wrap)
                    })
                    .collect()
            })
            .collect();

        #[allow(clippy::needless_range_loop)] // idx indexa passes (mut) + samplers
        for idx in 0..self.passes.len() {
            if self.passes[idx].bind_group.is_some() && self.passes[idx].bound {
                continue;
            }
            // Resolve as views (clona — `TextureView` é Arc barato) antes de
            // pegar `&mut self` pra gravar o bind group.
            let Some(binds): Option<Vec<(u32, u32, wgpu::TextureView)>> = self.passes[idx]
                .textures
                .iter()
                .map(|b| {
                    self.resolve_tex_view(&b.semantic, idx)
                        .map(|v| (b.tex_binding, b.samp_binding, v.clone()))
                })
                .collect()
            else {
                log::warn!(
                    "run_chain: passe {idx} não conseguiu resolver uma textura de bind group — pulando frame"
                );
                return None;
            };

            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.passes[idx].ubuf[0].as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.passes[idx].ubuf[1].as_entire_binding(),
                },
            ];
            for (t, (tb, sb, view)) in binds.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: *tb,
                    resource: wgpu::BindingResource::TextureView(view),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: *sb,
                    resource: wgpu::BindingResource::Sampler(&samplers[idx][t]),
                });
            }
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("etapa04 bg"),
                layout: &self.passes[idx].bgl,
                entries: &entries,
            });
            drop(entries);
            drop(binds);
            self.passes[idx].bind_group = Some(bg);
            self.passes[idx].bound = true;
        }

        for idx in 0..self.passes.len() {
            let (tex, view, _, _) = self.passes[idx].target.as_ref()?;
            let levels = self.passes[idx].target_mip_levels;
            // Com mip chain, o attachment de render só pode ser 1 nível — a
            // `view` "cheia" (todos os níveis) fica só pra sampling depois
            // (`resolve_tex_view`). Sem mip (caso comum), usa a mesma de sempre.
            let level0_view;
            let attach_view: &wgpu::TextureView = if levels > 1 {
                level0_view = tex.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    ..Default::default()
                });
                &level0_view
            } else {
                view
            };
            let bg = self.passes[idx].bind_group.as_ref()?;
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("etapa04 pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: attach_view,
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
            rp.set_pipeline(&self.passes[idx].pipeline);
            rp.set_bind_group(0, bg, &[]);
            rp.set_vertex_buffer(0, self.quad.slice(..));
            rp.draw(0..4, 0..1);
            drop(rp);
            if levels > 1 {
                // gera o resto da cadeia ANTES do próximo passe (que sampleia
                // este alvo) rodar — mesmo `enc`, ordem de submissão garante.
                self.generate_mips(enc, tex, levels);
            }
        }

        // Feedback: guarda a saída dos passes marcados pro próximo frame (o
        // sampler `*Feedback` lê esta cópia). Copiado depois de todos os passes
        // pra um passe poder ler o feedback dele mesmo.
        for idx in 0..self.passes.len() {
            if !self.passes[idx].feedback {
                continue;
            }
            let (Some((src, _, sw, sh)), Some((dst, _, _, _))) = (
                self.passes[idx].target.as_ref(),
                self.passes[idx].feedback_target.as_ref(),
            ) else {
                continue;
            };
            enc.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: src,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: dst,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: *sw,
                    height: *sh,
                    depth_or_array_layers: 1,
                },
            );
            self.passes[idx].bound = false; // o feedback_target mudou
        }

        // Rotação de tela (`SET_ROTATION`) — redesenha a saída da chain
        // rotacionada num alvo próprio; a moldura e o blit final leem daí, então
        // a moldura fica em pé. Faz ANTES da composição de propósito.
        self.rot_view = None;
        let (mut fw, mut fh) = (fw, fh);
        let deg = frame.metadata.rotation_degrees % 360;
        let quarter = deg == 90 || deg == 270;
        if deg != 0 {
            let (rw, rh) = if deg == 90 || deg == 270 {
                (fh, fw)
            } else {
                (fw, fh)
            };
            // giro do UV = -giro da imagem. `SET_ROTATION` do libretro é
            // anti-horário (turns×90° CCW): 90 → (0,1); 180 → (-1,0); 270 → (0,-1).
            let (cos, sin) = match deg {
                90 => (0.0f32, 1.0f32),
                180 => (-1.0, 0.0),
                270 => (0.0, -1.0),
                _ => (1.0, 0.0),
            };
            if self.ensure_rot_target(rw, rh) {
                let src = &self.passes.last()?.target.as_ref()?.1;
                let rot_view = self.rot_tgt.as_ref()?.1.clone();
                let ubuf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rot u"),
                    size: 16,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue
                    .write_buffer(&ubuf, 0, f32s_bytes(&[cos, sin, 0.0, 0.0]));
                let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("rot bg"),
                    layout: &self.comp.bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: ubuf.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(src),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                        },
                    ],
                });
                {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("rot pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &rot_view,
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
                    rp.set_pipeline(&self.rot_pipeline);
                    rp.set_bind_group(0, &bg, &[]);
                    rp.set_vertex_buffer(0, self.quad.slice(..));
                    rp.draw(0..4, 0..1);
                }
                self.rot_view = Some(rot_view);
                fw = rw;
                fh = rh;
            }
        }

        // Composição da moldura (etapa 04 fatia 4), se houver uma.
        let (out_w, out_h, use_comp) = if let Some((dw, dh, vp)) =
            self.decoration.as_ref().map(|d| (d.w, d.h, d.vp))
        {
            let dar0 = if frame.metadata.aspect_ratio > 0.0 {
                frame.metadata.aspect_ratio
            } else {
                nw as f32 / nh.max(1) as f32
            };
            // com rotação de 90°/270° a AR de exibição inverte.
            let dar = if quarter && dar0 > 0.0 {
                1.0 / dar0
            } else {
                dar0
            };
            let (cx, cy, hw, hh) = match vp {
                // Janela do jogo conhecida (do `.cfg` ou detectada pela
                // transparência da arte): por padrão o jogo PREENCHE a janela
                // (sem letterbox — a moldura foi desenhada pra esse
                // retângulo). Com integer scaling ligado, a ALTURA fica no
                // múltiplo inteiro MAIS PRÓXIMO da altura do canvas inteiro
                // da moldura (`dh`, não `v.h`/vidro) — `round`, não `floor`
                // puro: quando o fator de baixo deixaria uma barra preta
                // pequena, tudo bem; mas perto do meio do caminho entre dois
                // fatores (ex: `dh/nativa` = 4.5), sempre arredondar pra CIMA
                // significa saltar um fator inteiro inteiro maior que o
                // necessário só pra não ter barra nenhuma — isso cortava
                // linhas inteiras de HUD/texto perto da borda (visto com
                // Batman Beyond/PS1: fator 4→5 cortava "Developed by" no
                // topo). `round` escolhe sempre o menor erro em pixels entre
                // faixa preta (fator de baixo) e corte de jogo (fator de
                // cima). O excesso que ainda passar de `dh` é cortado pelo
                // clipping normal da GPU (`hw`/`hh` SEM `.clamp` — um NDC >1
                // já sai da viewport sozinho).
                // A LARGURA vem de `altura × dar` (proporção já corrigida de
                // PAR/pixel não-quadrado acima), NÃO de `native_width ×
                // fator` — cores com pixel não-quadrado (PS1, N64, Mega
                // Drive…) têm `native_width/native_height` cru diferente da
                // proporção real de exibição; multiplicar os dois pelo mesmo
                // fator inteiro deixava a imagem mais larga que o vidro,
                // cortando texto/HUD nas bordas esquerda/direita (visto no
                // teste com Batman Beyond/PS1). Isso é genérico — usa o
                // `aspect_ratio` que TODO core já reporta, sem tabela por
                // sistema. A moldura em si nunca muda de tamanho.
                Some(v) if v.w > 0.0 && v.h > 0.0 && self.integer_scaling => {
                    let gnh = if quarter { nw } else { nh };
                    let factor = ((dh as f32 / gnh.max(1) as f32).round() as u32).max(1);
                    let gh = (gnh * factor) as f32;
                    let gw = gh * dar;
                    let (cx0, cy0) = (v.x + v.w / 2.0, v.y + v.h / 2.0);
                    (
                        cx0 / dw as f32 * 2.0 - 1.0,
                        1.0 - cy0 / dh as f32 * 2.0,
                        gw / dw as f32,
                        gh / dh as f32,
                    )
                }
                Some(v) if v.w > 0.0 && v.h > 0.0 => (
                    (v.x + v.w / 2.0) / dw as f32 * 2.0 - 1.0,
                    1.0 - (v.y + v.h / 2.0) / dh as f32 * 2.0,
                    (v.w / dw as f32).clamp(0.0, 1.0),
                    (v.h / dh as f32).clamp(0.0, 1.0),
                ),
                _ => {
                    let vw = (dh as f32 * dar).min(dw as f32);
                    (0.0, 0.0, vw / dw as f32, 1.0)
                }
            };
            self.queue
                .write_buffer(&self.comp.rect_game, 0, f32s_bytes(&[cx, cy, hw, hh]));
            self.queue
                .write_buffer(&self.comp.rect_bezel, 0, f32s_bytes(&[0.0, 0.0, 1.0, 1.0]));
            self.ensure_comp_target(dw, dh);

            let game_view = match &self.rot_view {
                Some(v) => v,
                None => &self.passes.last()?.target.as_ref()?.1,
            };
            let deco_view = &self.decoration.as_ref()?.view;
            let comp_view = &self.comp.target.as_ref()?.1;
            let mk_bg = |rect: &wgpu::Buffer, tex: &wgpu::TextureView| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("comp bg"),
                    layout: &self.comp.bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: rect.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(tex),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                        },
                    ],
                })
            };
            let game_bg = mk_bg(&self.comp.rect_game, game_view);
            let bezel_bg = mk_bg(&self.comp.rect_bezel, deco_view);
            {
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("comp pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: comp_view,
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
                rp.set_vertex_buffer(0, self.quad.slice(..));
                rp.set_pipeline(&self.comp.game_pipeline);
                rp.set_bind_group(0, &game_bg, &[]);
                rp.draw(0..4, 0..1);
                rp.set_pipeline(&self.comp.bezel_pipeline);
                rp.set_bind_group(0, &bezel_bg, &[]);
                rp.draw(0..4, 0..1);
            }
            (dw, dh, true)
        } else {
            (fw, fh, false)
        };
        Some((out_w, out_h, use_comp))
    }

    pub(super) fn ensure_target(&mut self, idx: usize, w: u32, h: u32) {
        let fmt = self.passes[idx].fmt;
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        // `mipmap_input<idx+1>` do `.slangp` — o passe SEGUINTE quer amostrar
        // a saída deste com mip (bloom/glow via mip como blur barato, ex.
        // crt-royale/Mega Bezel). Mesma convenção do RetroArch: quem pede a
        // cadeia é o passe que LÊ, não o que escreve.
        let levels = if self.passes.get(idx + 1).is_some_and(|p| p.mipmap_input) {
            mip_level_count(w, h)
        } else {
            1
        };
        let stale = !matches!(&self.passes[idx].target, Some((_, _, tw, th)) if *tw == w && *th == h)
            || self.passes[idx].target_mip_levels != levels;
        if stale {
            let (t, v) = new_tex_fmt_mips(&self.device, w, h, fmt, usage, levels);
            self.passes[idx].target = Some((t, v, w, h));
            self.passes[idx].target_mip_levels = levels;
            // qualquer passe pode amostrar este (PassOutput/Source) → rebind todos
            for p in &mut self.passes {
                p.bound = false;
            }
        }
        // alvo de feedback (cópia do frame anterior) — mesmo tamanho/formato.
        if self.passes[idx].feedback {
            let fb_stale = !matches!(
                &self.passes[idx].feedback_target,
                Some((_, _, tw, th)) if *tw == w && *th == h
            );
            if fb_stale {
                let (t, v) = new_tex_fmt(&self.device, w, h, fmt, usage);
                self.passes[idx].feedback_target = Some((t, v, w, h));
                for p in &mut self.passes {
                    p.bound = false;
                }
            }
        }
    }

    /// Gera os níveis `1..levels` da cadeia de mip de `tex` (nível 0 já
    /// desenhado pelo passe) — um blit linear por nível, cada um lendo o
    /// nível anterior e escrevendo no seguinte. wgpu não tem um "generate
    /// mipmaps" embutido (ao contrário de `vkCmdBlitImage`, que o RetroArch
    /// usa pra isso em `vulkan_framebuffer_generate_mips`) — aqui é a mesma
    /// técnica via render pass: reusa o pipeline/shader do blit de
    /// composição (`COMP_WGSL`, um quad cheio via `mip_rect = [0,0,1,1]`)
    /// com `sampler_linear` pra suavizar cada redução (igual ao
    /// `VK_FILTER_LINEAR` da referência).
    pub(super) fn generate_mips(
        &self,
        enc: &mut wgpu::CommandEncoder,
        tex: &wgpu::Texture,
        levels: u32,
    ) {
        for i in 1..levels {
            let src_view = tex.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: i - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let dst_view = tex.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: i,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mip gen bg"),
                layout: &self.comp.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.mip_rect.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler_linear),
                    },
                ],
            });
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mip gen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst_view,
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
            rp.set_pipeline(&self.mip_pipeline);
            rp.set_bind_group(0, &bg, &[]);
            rp.set_vertex_buffer(0, self.quad.slice(..));
            rp.draw(0..4, 0..1);
        }
    }

    /// Entrada da chain neste frame: interop (dma_buf) ou o topo do ring de
    /// history (frame do core).
    pub(super) fn chain_input(&self) -> Option<&wgpu::TextureView> {
        self.interop_view
            .as_ref()
            .or_else(|| self.history.first().map(|(_, v, _, _)| v))
    }

    /// Resolve uma [`TextureSemantic`] pra `TextureView` deste frame.
    pub(super) fn resolve_tex_view(
        &self,
        sem: &TextureSemantic,
        pass_idx: usize,
    ) -> Option<&wgpu::TextureView> {
        let out_of = |i: usize| {
            self.passes
                .get(i)
                .and_then(|p| p.target.as_ref())
                .map(|(_, v, _, _)| v)
        };
        let fb_of = |i: usize| {
            self.passes.get(i).and_then(|p| {
                p.feedback_target
                    .as_ref()
                    .or(p.target.as_ref())
                    .map(|(_, v, _, _)| v)
            })
        };
        match sem {
            TextureSemantic::Source => {
                if pass_idx == 0 {
                    self.chain_input()
                } else {
                    out_of(pass_idx - 1)
                }
            }
            TextureSemantic::Original => self.chain_input(),
            TextureSemantic::OriginalHistory(n) => {
                if self.interop_view.is_some() {
                    self.interop_view.as_ref()
                } else {
                    let last = self.history.len().saturating_sub(1);
                    self.history
                        .get((*n as usize).min(last))
                        .map(|(_, v, _, _)| v)
                }
            }
            TextureSemantic::PassOutput(n) => out_of(*n as usize),
            TextureSemantic::PassFeedback(n) => fb_of(*n as usize),
            TextureSemantic::Named { name, feedback } => {
                if let Some(&pi) = self.pass_alias.get(name) {
                    if *feedback {
                        fb_of(pi)
                    } else {
                        out_of(pi)
                    }
                } else {
                    self.luts.get(name).map(|(_, v, _, _, _)| v)
                }
            }
        }
    }

    /// Nome-base de cada textura do passe → tamanho (pros uniformes `<Nome>Size`).
    pub(super) fn tex_sizes_for(
        &self,
        pass_idx: usize,
        input: (u32, u32),
        native: (u32, u32),
        sizes: &[(u32, u32)],
    ) -> HashMap<String, (u32, u32)> {
        let mut m = HashMap::new();
        m.insert("Source".to_string(), input);
        m.insert("Original".to_string(), native);
        for b in &self.passes[pass_idx].textures {
            let (base, sz) = match &b.semantic {
                TextureSemantic::Source => continue,
                TextureSemantic::Original => continue,
                TextureSemantic::OriginalHistory(k) => (format!("OriginalHistory{k}"), native),
                TextureSemantic::PassOutput(k) => (
                    format!("PassOutput{k}"),
                    sizes.get(*k as usize).copied().unwrap_or(native),
                ),
                TextureSemantic::PassFeedback(k) => (
                    format!("PassFeedback{k}"),
                    sizes.get(*k as usize).copied().unwrap_or(native),
                ),
                TextureSemantic::Named { name, .. } => {
                    let sz = if let Some(&pi) = self.pass_alias.get(name) {
                        sizes.get(pi).copied().unwrap_or(native)
                    } else if let Some((_, _, _, w, h)) = self.luts.get(name) {
                        (*w, *h)
                    } else {
                        native
                    };
                    (name.clone(), sz)
                }
            };
            m.insert(base, sz);
        }
        m
    }

    /// Sampler `(filtro, wrap)` do cache (cria na 1ª vez). Clona — samplers wgpu
    /// são handles baratos.
    pub(super) fn sampler_for(&mut self, linear: bool, wrap: WrapMode) -> wgpu::Sampler {
        self.sampler_cache
            .entry((linear, wrap))
            .or_insert_with(|| {
                let f = if linear {
                    wgpu::FilterMode::Linear
                } else {
                    wgpu::FilterMode::Nearest
                };
                self.device.create_sampler(&sampler_desc(f, wrap))
            })
            .clone()
    }
}
