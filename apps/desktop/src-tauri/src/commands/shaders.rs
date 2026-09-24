//! Shaders: presets curados, pacote slang, preset por ROM e parâmetros.

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedPresetInfo {
    /// Wire id — `curated:<id>` (o que `set_shader` espera).
    pub id: String,
    pub label: String,
    pub desc: String,
    /// `false` = o pacote `slang-shaders` ainda não foi baixado.
    pub available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderInfo {
    /// Preset ativo — nome do builtin, `curated:<id>` ou caminho `.slangp`.
    pub active: String,
    pub available: Vec<String>,
    /// Presets "de 1 clique" que apontam pro pacote `slang-shaders`.
    pub curated: Vec<CuratedPresetInfo>,
    /// `false` = sem adapter wgpu; a troca de preset não tem efeito.
    pub gpu: bool,
}

fn curated_infos() -> Vec<CuratedPresetInfo> {
    crate::gpu::CURATED
        .iter()
        .map(|c| CuratedPresetInfo {
            id: format!("curated:{}", c.id),
            label: c.label.to_string(),
            desc: c.desc.to_string(),
            available: crate::gpu::curated_slangp(c).is_some(),
        })
        .collect()
}

#[tauri::command]
pub fn get_shader_info(state: State<'_, AppState>) -> ShaderInfo {
    let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    match guard.as_ref() {
        Some(fp) => ShaderInfo {
            active: fp.preset_source().to_string(),
            available: crate::gpu::builtin_preset_names(),
            curated: curated_infos(),
            gpu: true,
        },
        None => ShaderInfo {
            active: "plain".into(),
            available: crate::gpu::builtin_preset_names(),
            curated: curated_infos(),
            gpu: false,
        },
    }
}

/// Um preset `.slangp` encontrado varrendo a pasta de shaders do usuário
/// (RetroArch / RetroBat). `category` é a subpasta de topo (`crt`, `handheld`,
/// …) ou `""` pros presets na raiz.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlangpEntry {
    pub path: String,
    pub name: String,
    pub category: String,
}

/// Teto de segurança — a árvore `slang-shaders` tem ~1200 `.slangp`.
const MAX_SLANGP_ENTRIES: usize = 6000;

/// Estado do pacote de shaders baixado (`libretro/slang-shaders`).
#[tauri::command]
pub fn shader_pack_status(state: State<'_, AppState>) -> crate::shader_pack::PackStatus {
    crate::shader_pack::status(&state.shaders_dir)
}

/// Baixa o pacote `libretro/slang-shaders` (~130 MB) e o extrai pra
/// `<dados>/shaders/slang-shaders`. Devolve o caminho instalado; a
/// `ShaderLibrary` aponta a raiz pra ele.
#[tauri::command]
pub async fn download_shader_pack(
    state: State<'_, AppState>,
    on_progress: tauri::ipc::Channel<crate::shader_pack::DownloadProgress>,
) -> Result<String, String> {
    let dir = state.shaders_dir.clone();
    let path = crate::shader_pack::download(&dir, on_progress).await?;
    Ok(path.to_string_lossy().into_owned())
}

/// Varre `root` recursivamente e lista todo `*.slangp`. Não compila nada — só
/// enumera pra UI de biblioteca (a compilação acontece no `set_shader`).
#[tauri::command]
pub fn list_slangp_dir(root: String) -> Result<Vec<SlangpEntry>, String> {
    let root = std::path::PathBuf::from(&root);
    if !root.is_dir() {
        return Err(format!("'{}' não é uma pasta", root.display()));
    }
    let mut out = Vec::new();
    walk_slangp(&root, &root, &mut out, 0);
    out.sort_by(|a, b| {
        (a.category.to_lowercase(), a.name.to_lowercase())
            .cmp(&(b.category.to_lowercase(), b.name.to_lowercase()))
    });
    Ok(out)
}

fn walk_slangp(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<SlangpEntry>,
    depth: usize,
) {
    if depth > 8 || out.len() >= MAX_SLANGP_ENTRIES {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            walk_slangp(root, &p, out, depth + 1);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("slangp"))
        {
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("preset")
                .to_string();
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let category = if rel.components().count() >= 2 {
                rel.components()
                    .next()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            out.push(SlangpEntry {
                path: p.to_string_lossy().into_owned(),
                name,
                category,
            });
        }
        if out.len() >= MAX_SLANGP_ENTRIES {
            return;
        }
    }
}

/// Registra os presets embutidos (`plain`/`crt`/`lcd`) em `shader_presets` no
/// startup — `shader_chain_assignments.preset_id` tem FK pra essa tabela.
pub async fn seed_builtin_shader_presets(pool: &db::Db) {
    let sc = db::ShaderChainRepo::new(pool.clone());
    for name in crate::gpu::builtin_preset_names() {
        let p = domain::shader_chain::ShaderPreset {
            id: name.clone(),
            name: name.clone(),
            source_path: name.clone(),
            format: domain::shader_chain::ShaderFormat::Slang,
            is_builtin: true,
            includes_bezel: false,
        };
        if let Err(e) = sc.upsert_preset(&p).await {
            log::warn!("seed do preset '{name}': {e}");
        }
    }
}

/// `builtin:<nome>` ou o próprio caminho `.slangp` (o que `set_preset` aceita).
fn preset_id_of(name: &str) -> (String, String, bool) {
    if let Some(c) = crate::gpu::curated_by_wire(name) {
        return (name.to_string(), c.label.to_string(), true);
    }
    if name.ends_with(".slangp") {
        let stem = std::path::Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("preset")
            .to_string();
        (name.to_string(), stem, false)
    } else {
        (name.to_string(), name.to_string(), true)
    }
}

/// Aplica `name` no processador GPU agora e, se `scope` for dado, persiste no
/// escopo: `"default"` (todos os jogos), `"system"` (uma plataforma —
/// `system_id` ou derivado de `rom_id`) ou `"rom"` (um jogo). `name` vazio =
/// limpar a atribuição desse escopo (volta pra cascata).
#[tauri::command]
pub async fn set_shader(
    state: State<'_, AppState>,
    name: String,
    scope: Option<String>,
    system_id: Option<String>,
    rom_id: Option<String>,
) -> Result<(), String> {
    let clearing = name.is_empty();
    if !clearing {
        let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        let Some(fp) = guard.as_mut() else {
            return Err("sem GPU — shader indisponível".into());
        };
        fp.set_preset(&name).inspect_err(|e| log::warn!("{e}"))?;
    }

    let Some(scope) = scope else { return Ok(()) };
    let pool = pool(&state)?;
    let (sc_scope, sys, rid) =
        shader_scope_args(&pool, &scope, system_id.as_deref(), rom_id.as_deref()).await?;
    let sc = db::ShaderChainRepo::new(pool);
    if clearing {
        return sc
            .clear_assignment(sc_scope, sys.as_deref(), rid.as_deref())
            .await
            .map_err(|e| e.to_string());
    }
    let (id, pname, is_builtin) = preset_id_of(&name);
    // heurística: presets com "bezel" no nome já desenham a moldura → o
    // DecorationResolver é pulado (exclusão mútua).
    let includes_bezel = name.to_lowercase().contains("bezel");
    sc.upsert_preset(&domain::shader_chain::ShaderPreset {
        id: id.clone(),
        name: pname,
        source_path: name.clone(),
        format: domain::shader_chain::ShaderFormat::Slang,
        is_builtin,
        includes_bezel,
    })
    .await
    .map_err(|e| e.to_string())?;
    sc.set_assignment(sc_scope, sys.as_deref(), rid.as_deref(), &id)
        .await
        .map_err(|e| e.to_string())
}

/// Preset de shader resolvido pra um jogo (rom → sistema → default) + o que
/// está atribuído EXATAMENTE em cada escopo (pra UI de escopo). `resolvedScope`
/// = de onde veio o `sourcePath` ("rom" | "system" | "default" | "none").
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomShader {
    pub source_path: Option<String>,
    /// mantido por compat: `resolvedScope == "rom"`.
    pub from_rom: bool,
    pub resolved_scope: String,
    pub system_id: String,
    /// atribuição exata em cada escopo (`None` = herda).
    pub at_rom: Option<String>,
    pub at_system: Option<String>,
    pub at_default: Option<String>,
}

#[tauri::command]
pub async fn get_rom_shader(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<RomShader, String> {
    use domain::shader_chain::ShaderChainResolver;
    let pool = pool(&state)?;
    let system = db::RomsRepo::new(pool.clone())
        .get(&rom_id)
        .await
        .map_err(|e| e.to_string())?
        .map(|r| r.system_id)
        .unwrap_or_default();
    let sc = db::ShaderChainRepo::new(pool);
    let presets = sc.list_presets().await.map_err(|e| e.to_string())?;
    let src_of = |id: &str| {
        presets
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.source_path.clone())
    };

    // Atribuição EXATA em cada escopo: `resolve` com alvos progressivamente
    // mais amplos, aceitando só quando o escopo que venceu é o esperado.
    let resolved = sc
        .resolve(&system, Some(&rom_id))
        .await
        .map_err(|e| e.to_string())?;
    let at_rom = resolved
        .as_ref()
        .filter(|a| a.scope == AssignmentScope::Rom)
        .and_then(|a| src_of(&a.preset_id));
    let at_system = match sc.resolve(&system, None).await {
        Ok(Some(a)) if a.scope == AssignmentScope::System => src_of(&a.preset_id),
        _ => None,
    };
    let at_default = match sc.resolve("", None).await {
        Ok(Some(a)) if a.scope == AssignmentScope::Default => src_of(&a.preset_id),
        _ => None,
    };

    let (source_path, resolved_scope) = match &resolved {
        Some(a) => (
            src_of(&a.preset_id),
            match a.scope {
                AssignmentScope::Rom => "rom",
                AssignmentScope::System => "system",
                AssignmentScope::Default => "default",
            }
            .to_string(),
        ),
        None => (None, "none".to_string()),
    };
    Ok(RomShader {
        from_rom: resolved_scope == "rom",
        source_path,
        resolved_scope,
        system_id: system,
        at_rom,
        at_system,
        at_default,
    })
}

/// Um parâmetro ajustável do shader ativo (`#pragma parameter NAME "label"
/// default min max step`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderParamDto {
    pub name: String,
    pub label: String,
    pub value: f32,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// Parâmetros do shader carregado agora no processador GPU. Vazio pros builtins
/// (`plain`/`crt`/`lcd`) e quando não há GPU.
#[tauri::command]
pub fn get_shader_params(state: State<'_, AppState>) -> Vec<ShaderParamDto> {
    let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    let Some(fp) = guard.as_ref() else {
        return Vec::new();
    };
    fp.shader_param_meta()
        .iter()
        .map(|m| ShaderParamDto {
            value: fp.shader_param_value(&m.name).unwrap_or(m.default),
            name: m.name.clone(),
            label: m.label.clone(),
            default: m.default,
            min: m.min,
            max: m.max,
            step: m.step,
        })
        .collect()
}

/// Ajusta um parâmetro do shader agora e, se `scope` for dado, persiste
/// (`"default"` ou `"rom"` + `rom_id`). O escopo precisa já ter um shader
/// atribuído (a UI garante isso antes de mostrar os controles).
#[tauri::command]
pub async fn set_shader_param(
    state: State<'_, AppState>,
    name: String,
    value: f32,
    scope: Option<String>,
    system_id: Option<String>,
    rom_id: Option<String>,
) -> Result<(), String> {
    {
        let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(fp) = guard.as_mut() {
            fp.set_shader_param(&name, value);
        }
    }
    let Some(scope) = scope else { return Ok(()) };
    let pool = pool(&state)?;
    let (sc_scope, sys, rid) =
        shader_scope_args(&pool, &scope, system_id.as_deref(), rom_id.as_deref()).await?;
    db::ShaderChainRepo::new(pool)
        .set_parameter_override(
            sc_scope,
            sys.as_deref(),
            rid.as_deref(),
            &name,
            &value.to_string(),
        )
        .await
        .map_err(|e| e.to_string())
}

/// Volta os parâmetros do shader pros defaults do preset: limpa os overrides
/// persistidos do escopo e recarrega o preset no processador GPU.
#[tauri::command]
pub async fn reset_shader_params(
    state: State<'_, AppState>,
    scope: Option<String>,
    system_id: Option<String>,
    rom_id: Option<String>,
) -> Result<(), String> {
    if let Some(scope) = &scope {
        let pool = pool(&state)?;
        let (sc_scope, sys, rid) =
            shader_scope_args(&pool, scope, system_id.as_deref(), rom_id.as_deref()).await?;
        db::ShaderChainRepo::new(pool)
            .clear_parameter_overrides(sc_scope, sys.as_deref(), rid.as_deref())
            .await
            .map_err(|e| e.to_string())?;
    }
    apply_resolved_shader_ex(&state, rom_id.as_deref(), true).await;
    Ok(())
}
