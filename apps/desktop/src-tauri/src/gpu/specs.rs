//! Preset → especificação de passes (`build_specs`) → recursos wgpu (`realize`).

use super::*;

/// `plain|crt|lcd` ou um caminho `.slangp` → preset pronto (nome, params,
/// metadados dos parâmetros, passes).
pub(super) fn build_specs(want: &str) -> Result<BuiltSpecs, String> {
    if let Some(c) = curated_by_wire(want) {
        let p = curated_slangp(c).ok_or_else(|| {
            format!(
                "'{}' precisa do pacote de shaders — baixe em Config › Vídeo",
                c.label
            )
        })?;
        let s = p.to_string_lossy();
        return build_specs(&s);
    }
    if let Some(b) = BUILTINS.iter().find(|b| b.name == want) {
        let passes = b
            .passes
            .iter()
            .map(|p| PassSpec {
                scale_x: Scale::Source(p.scale),
                scale_y: Scale::Source(p.scale),
                linear: p.linear,
                wrap: WrapMode::ClampToEdge,
                vs_wgsl: BUILTIN_VS.to_string(),
                fs_wgsl: format!("{BUILTIN_FS_PRELUDE}\n{}", p.fs),
                uniform: UniformMode::Fixed,
                textures: Vec::new(),
                fmt: FMT,
                frame_count_mod: 0,
                alias: None,
                feedback: false,
                mipmap_input: false,
            })
            .collect();
        return Ok(BuiltSpecs {
            name: b.name.to_string(),
            params: HashMap::new(),
            meta: Vec::new(),
            passes,
            luts: Vec::new(),
            history_depth: 1,
        });
    }

    let path = Path::new(want);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "slangp" => {}
        "glslp" | "cgp" => {
            return Err(format!(
                "preset '.{ext}' (GLSL/Cg) não é suportado — use um preset slang (.slangp)"
            ))
        }
        "slang" | "glsl" | "cg" => {
            return Err("escolha o arquivo de preset `.slangp`, não o `.slang`".into())
        }
        "" if path.is_dir() => return Err("escolha um arquivo `.slangp`, não uma pasta".into()),
        _ => {
            return Err(if ext.is_empty() {
                format!("'{}' não é um preset .slangp", path.display())
            } else {
                format!("extensão '.{ext}' não reconhecida (esperado .slangp)")
            })
        }
    }
    log::info!("carregando preset slang: {}", path.display());
    let preset = shader_slang::parse_slangp_file(path).map_err(|e| e.to_string())?;
    if preset.passes.is_empty() {
        return Err("preset sem passes".into());
    }

    let mut params: HashMap<String, f32> = HashMap::new();
    let mut meta: Vec<shader_slang::Parameter> = Vec::new();
    let mut specs = Vec::new();
    let mut history_depth = 1usize;
    // aliases só ficam conhecidos depois de ler todos os passes (um passe pode
    // referenciar o alias de outro que vem depois? não — mas o alias pode não
    // ser o `#pragma name`; usamos o `aliasN` do `.slangp`).
    let aliases: Vec<Option<String>> = preset.passes.iter().map(|p| p.alias.clone()).collect();
    // um passe é fonte de feedback se `feedback_pass{i}` OU se algum passe
    // amostra `PassFeedback<i>` / `<aliasI>Feedback`.
    let mut needs_feedback = vec![false; preset.passes.len()];
    for (i, pass) in preset.passes.iter().enumerate() {
        let src = shader_slang::preprocess_file(&pass.shader_path).map_err(|e| e.to_string())?;
        for p in &src.parameters {
            if !params.contains_key(&p.name) {
                params.insert(p.name.clone(), p.default);
                meta.push(p.clone());
            }
        }
        let compiled = shader_slang::compile(&src)
            .map_err(|e| format!("passe {i} ({}): {e}", pass.shader_path.display()))?;
        for t in &compiled.textures {
            match &t.semantic {
                TextureSemantic::OriginalHistory(n) => {
                    history_depth = history_depth.max(*n as usize + 1);
                }
                TextureSemantic::PassFeedback(n) => {
                    if let Some(f) = needs_feedback.get_mut(*n as usize) {
                        *f = true;
                    }
                }
                TextureSemantic::Named {
                    name,
                    feedback: true,
                } => {
                    if let Some(pi) = aliases.iter().position(|a| a.as_deref() == Some(name)) {
                        needs_feedback[pi] = true;
                    }
                }
                _ => {}
            }
        }
        if pass.feedback {
            needs_feedback[i] = true;
        }
        let fmt = if pass.float_framebuffer {
            wgpu::TextureFormat::Rgba16Float
        } else if pass.srgb_framebuffer {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            FMT
        };
        specs.push(PassSpec {
            scale_x: pass.scale_x,
            scale_y: pass.scale_y,
            linear: pass.filter_linear,
            wrap: pass.wrap_mode,
            vs_wgsl: compiled.vertex_wgsl,
            fs_wgsl: compiled.fragment_wgsl,
            uniform: UniformMode::Slang(compiled.uniforms),
            textures: compiled.textures,
            fmt,
            frame_count_mod: pass.frame_count_mod,
            alias: pass.alias.clone(),
            feedback: false, // preenchido abaixo
            mipmap_input: pass.mipmap_input,
        });
    }
    for (spec, need) in specs.iter_mut().zip(needs_feedback) {
        spec.feedback = need;
    }
    // valores do `.slangp` sobrescrevem os defaults dos `#pragma parameter`
    for (k, v) in &preset.parameters {
        params.insert(k.clone(), *v);
    }

    // texturas do usuário (LUT/máscara) — decodifica agora, sobe pra GPU depois.
    let mut luts = Vec::new();
    for tex in &preset.textures {
        match crate::decoration::decode_png(&tex.path) {
            Ok((rgba, w, h)) => luts.push(LutSpec {
                name: tex.name.clone(),
                rgba,
                w,
                h,
                linear: tex.linear,
                wrap: tex.wrap_mode,
                mipmap: tex.mipmap,
            }),
            Err(e) => log::warn!("LUT '{}' ({}): {e}", tex.name, tex.path.display()),
        }
    }

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("slangp")
        .to_string();
    Ok(BuiltSpecs {
        name,
        params,
        meta,
        passes: specs,
        luts,
        history_depth,
    })
}

/// Preset já com os recursos wgpu (passes, LUTs) criados — o que `new` e
/// `set_preset` precisam.
pub(super) struct Realized {
    pub(super) preset_name: String,
    pub(super) params: HashMap<String, f32>,
    pub(super) param_meta: Vec<shader_slang::Parameter>,
    pub(super) passes: Vec<Pass>,
    pub(super) luts: HashMap<String, (wgpu::Texture, wgpu::TextureView, wgpu::Sampler, u32, u32)>,
    pub(super) pass_alias: HashMap<String, usize>,
    pub(super) history_depth: usize,
}

pub(super) fn realize(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    built: BuiltSpecs,
) -> Option<Realized> {
    let BuiltSpecs {
        name,
        params,
        meta,
        passes: specs,
        luts: lut_specs,
        history_depth,
    } = built;

    let mut pass_alias = HashMap::new();
    for (i, s) in specs.iter().enumerate() {
        if let Some(a) = &s.alias {
            pass_alias.insert(a.clone(), i);
        }
    }
    let passes = specs
        .into_iter()
        .map(|s| build_pass(device, s))
        .collect::<Option<Vec<_>>>()?;

    let mut luts = HashMap::new();
    for l in lut_specs {
        let levels = if l.mipmap {
            mip_level_count(l.w, l.h)
        } else {
            1
        };
        let (tex, view) = new_tex_fmt_mips(
            device,
            l.w,
            l.h,
            FMT,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            levels,
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &l.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(l.w * 4),
                rows_per_image: Some(l.h),
            },
            wgpu::Extent3d {
                width: l.w,
                height: l.h,
                depth_or_array_layers: 1,
            },
        );
        // Resto da cadeia (níveis 1..levels): box downsample sucessivo na
        // CPU, um `write_texture` por nível — só roda no load do preset.
        let mut prev = (l.rgba, l.w, l.h);
        for lvl in 1..levels {
            let (down, dw, dh) = downsample_rgba8(&prev.0, prev.1, prev.2);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: lvl,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &down,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(dw * 4),
                    rows_per_image: Some(dh),
                },
                wgpu::Extent3d {
                    width: dw,
                    height: dh,
                    depth_or_array_layers: 1,
                },
            );
            prev = (down, dw, dh);
        }
        let filter = if l.linear {
            wgpu::FilterMode::Linear
        } else {
            wgpu::FilterMode::Nearest
        };
        let sampler = device.create_sampler(&sampler_desc(filter, l.wrap));
        luts.insert(l.name, (tex, view, sampler, l.w, l.h));
    }

    Some(Realized {
        preset_name: name,
        params,
        param_meta: meta,
        passes,
        luts,
        pass_alias,
        history_depth,
    })
}

pub(super) fn wrap_address(w: WrapMode) -> wgpu::AddressMode {
    match w {
        WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrapMode::ClampToBorder => wgpu::AddressMode::ClampToBorder,
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
        WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
    }
}

pub(super) fn sampler_desc(
    filter: wgpu::FilterMode,
    wrap: WrapMode,
) -> wgpu::SamplerDescriptor<'static> {
    let a = wrap_address(wrap);
    wgpu::SamplerDescriptor {
        label: Some("etapa04 sampler"),
        address_mode_u: a,
        address_mode_v: a,
        address_mode_w: a,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: if filter == wgpu::FilterMode::Linear {
            wgpu::MipmapFilterMode::Linear
        } else {
            wgpu::MipmapFilterMode::Nearest
        },
        border_color: matches!(wrap, WrapMode::ClampToBorder)
            .then_some(wgpu::SamplerBorderColor::TransparentBlack),
        ..Default::default()
    }
}
