//! Parser do arquivo `.slangp` (formato de preset do RetroArch/librashader).
//!
//! Formato: linhas `chave = valor` (valor pode vir entre aspas), comentários
//! com `#` ou `//`. Chaves indexadas por passe: `shader0`, `filter_linear0`,
//! `scale_type0`, `alias0`, etc. `shaders = N` dá a contagem. `#reference
//! "outro.slangp"` herda de outro preset (o atual sobrescreve).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SlangError {
    #[error("io em {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`shaders` ausente ou inválido no preset")]
    MissingShaderCount,
    #[error("`shader{0}` ausente (esperado por `shaders = {1}`)")]
    MissingShader(usize, usize),
    #[error("#reference aninhado demais (ciclo?) em {0}")]
    ReferenceLoop(String),
}

/// Como o tamanho de um passe é derivado, por eixo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// Múltiplo do tamanho da entrada do passe.
    Source(f32),
    /// Múltiplo do viewport final.
    Viewport(f32),
    /// Pixels absolutos.
    Absolute(u32),
}

impl Default for Scale {
    fn default() -> Self {
        Scale::Source(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    ClampToEdge,
    ClampToBorder,
    Repeat,
    MirroredRepeat,
}

impl WrapMode {
    fn parse(s: &str) -> Self {
        match s.trim().trim_matches('"') {
            "clamp_to_border" => WrapMode::ClampToBorder,
            "repeat" => WrapMode::Repeat,
            "mirrored_repeat" => WrapMode::MirroredRepeat,
            _ => WrapMode::ClampToEdge,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Pass {
    /// Caminho absoluto do `.slang` (resolvido contra o dir do `.slangp`).
    pub shader_path: PathBuf,
    pub alias: Option<String>,
    pub filter_linear: bool,
    pub wrap_mode: WrapMode,
    pub scale_x: Scale,
    pub scale_y: Scale,
    pub float_framebuffer: bool,
    pub srgb_framebuffer: bool,
    pub mipmap_input: bool,
    /// `FrameCount % N` antes de ir pro shader (0 = sem módulo).
    pub frame_count_mod: u32,
}

#[derive(Debug, Clone)]
pub struct TextureRef {
    pub name: String,
    pub path: PathBuf,
    pub linear: bool,
    pub wrap_mode: WrapMode,
    pub mipmap: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Preset {
    pub passes: Vec<Pass>,
    /// Valores default dos parâmetros declarados no preset (os `#pragma
    /// parameter` dos `.slang` entram depois, no preprocessador).
    pub parameters: BTreeMap<String, f32>,
    pub textures: Vec<TextureRef>,
}

/// Lê e parseia um `.slangp` do disco (segue `#reference`).
pub fn parse_slangp_file(path: &Path) -> Result<Preset, SlangError> {
    parse_file_inner(path, 0)
}

/// Parseia o conteúdo de um `.slangp` já em memória. `base_dir` resolve os
/// caminhos relativos (`shaderN`, `textures`). `#reference` é ignorado aqui
/// (use [`parse_slangp_file`] pra seguir referências).
pub fn parse_slangp(text: &str, base_dir: &Path) -> Result<Preset, SlangError> {
    let kv = parse_kv(text);
    build_preset(&kv, base_dir)
}

/// Lê um arquivo de shader/preset como texto tolerando bytes não-UTF-8 — os
/// shaders do RetroArch às vezes têm comentário de autor em Latin-1. Só afeta
/// comentários (o código GLSL é ASCII); byte inválido vira U+FFFD.
pub(crate) fn read_shader_text(path: &Path) -> Result<String, SlangError> {
    let bytes = std::fs::read(path).map_err(|source| SlangError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn parse_file_inner(path: &Path, depth: u8) -> Result<Preset, SlangError> {
    if depth > 8 {
        return Err(SlangError::ReferenceLoop(path.display().to_string()));
    }
    let text = read_shader_text(path)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let kv = parse_kv(&text);

    // `#reference` → herda; as chaves deste arquivo sobrescrevem.
    if let Some(reference) = kv.get("#reference") {
        let ref_path = resolve(base_dir, reference.trim_matches('"'));
        let mut base = parse_file_inner(&ref_path, depth + 1)?;
        let over = build_preset_inner(&kv, base_dir, false)?;
        if !over.passes.is_empty() {
            base.passes = over.passes;
        }
        base.parameters.extend(over.parameters);
        if !over.textures.is_empty() {
            base.textures = over.textures;
        }
        return Ok(base);
    }

    build_preset(&kv, base_dir)
}

/// `chave -> valor` (última ocorrência vence). `#reference` é guardado com a
/// chave literal `#reference`.
fn parse_kv(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#reference") {
            map.insert("#reference".to_string(), rest.trim().to_string());
            continue;
        }
        if line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        // tira comentário no fim da linha (fora de aspas)
        let v = strip_trailing_comment(v.trim());
        map.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
    }
    map
}

fn strip_trailing_comment(v: &str) -> &str {
    let mut in_quotes = false;
    let bytes = v.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_quotes = !in_quotes,
            b'#' if !in_quotes => return &v[..i],
            b'/' if !in_quotes && i + 1 < bytes.len() && bytes[i + 1] == b'/' => return &v[..i],
            _ => {}
        }
        i += 1;
    }
    v
}

fn resolve(base: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    }
}

fn build_preset(kv: &BTreeMap<String, String>, base_dir: &Path) -> Result<Preset, SlangError> {
    build_preset_inner(kv, base_dir, true)
}

fn build_preset_inner(
    kv: &BTreeMap<String, String>,
    base_dir: &Path,
    strict: bool,
) -> Result<Preset, SlangError> {
    let mut preset = Preset::default();

    let count: usize = match kv.get("shaders").and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None if strict => return Err(SlangError::MissingShaderCount),
        None => 0,
    };

    for i in 0..count {
        let shader = kv
            .get(&format!("shader{i}"))
            .ok_or(SlangError::MissingShader(i, count))?;
        let (sx, sy) = scale_for(kv, i);
        preset.passes.push(Pass {
            shader_path: resolve(base_dir, shader),
            alias: kv
                .get(&format!("alias{i}"))
                .filter(|s| !s.is_empty())
                .cloned(),
            filter_linear: flag(kv, &format!("filter_linear{i}"), false),
            wrap_mode: kv
                .get(&format!("wrap_mode{i}"))
                .map(|s| WrapMode::parse(s))
                .unwrap_or_default(),
            scale_x: sx,
            scale_y: sy,
            float_framebuffer: flag(kv, &format!("float_framebuffer{i}"), false),
            srgb_framebuffer: flag(kv, &format!("srgb_framebuffer{i}"), false),
            mipmap_input: flag(kv, &format!("mipmap_input{i}"), false),
            frame_count_mod: kv
                .get(&format!("frame_count_mod{i}"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        });
    }

    // texturas do usuário
    if let Some(list) = kv.get("textures") {
        for name in list.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(path) = kv.get(name) {
                preset.textures.push(TextureRef {
                    name: name.to_string(),
                    path: resolve(base_dir, path),
                    linear: flag(kv, &format!("{name}_linear"), false),
                    wrap_mode: kv
                        .get(&format!("{name}_wrap_mode"))
                        .map(|s| WrapMode::parse(s))
                        .unwrap_or_default(),
                    mipmap: flag(kv, &format!("{name}_mipmap"), false),
                });
            }
        }
    }

    // Parâmetros: qualquer `KEY = <float>` que não seja chave estrutural nem
    // valor de textura. O `parameters = "..."` só declara nomes; os valores
    // podem aparecer soltos (e num `#reference` filho, sem a lista).
    let tex_names: Vec<&str> = kv
        .get("textures")
        .map(|l| l.split(';').map(str::trim).collect())
        .unwrap_or_default();
    for (k, v) in kv {
        if is_structural_key(k) || tex_names.iter().any(|t| k.starts_with(t)) {
            continue;
        }
        if let Ok(f) = v.trim().parse::<f32>() {
            preset.parameters.insert(k.clone(), f);
        }
    }

    Ok(preset)
}

/// Chaves que controlam a estrutura do preset (não são parâmetros de shader).
fn is_structural_key(k: &str) -> bool {
    const EXACT: &[&str] = &["shaders", "parameters", "textures", "#reference"];
    const PREFIXES: &[&str] = &[
        "shader",
        "filter_linear",
        "scale_type",
        "scale",
        "alias",
        "wrap_mode",
        "float_framebuffer",
        "srgb_framebuffer",
        "mipmap_input",
        "frame_count_mod",
    ];
    EXACT.contains(&k) || PREFIXES.iter().any(|p| k.starts_with(p))
}

fn flag(kv: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    kv.get(key)
        .map(|s| matches!(s.trim(), "true" | "1" | "\"true\""))
        .unwrap_or(default)
}

/// `scale_type{i}` (ou `_x`/`_y`) + `scale{i}` (ou `_x`/`_y`). Sem nada →
/// `source 1.0`.
fn scale_for(kv: &BTreeMap<String, String>, i: usize) -> (Scale, Scale) {
    let axis = |suffix: &str| -> Scale {
        let ty = kv
            .get(&format!("scale_type_{suffix}{i}"))
            .or_else(|| kv.get(&format!("scale_type{i}")))
            .map(|s| s.trim().to_string());
        let val = kv
            .get(&format!("scale_{suffix}{i}"))
            .or_else(|| kv.get(&format!("scale{i}")))
            .and_then(|s| s.trim().parse::<f32>().ok());
        match ty.as_deref() {
            Some("viewport") => Scale::Viewport(val.unwrap_or(1.0)),
            Some("absolute") => Scale::Absolute(val.unwrap_or(0.0) as u32),
            Some("source") => Scale::Source(val.unwrap_or(1.0)),
            _ => match val {
                Some(v) => Scale::Source(v),
                None => Scale::Source(1.0),
            },
        }
    };
    (axis("x"), axis("y"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_pass_defaults() {
        let p = parse_slangp("shaders = 1\nshader0 = crt.slang\n", Path::new("/x")).unwrap();
        assert_eq!(p.passes.len(), 1);
        assert_eq!(p.passes[0].shader_path, Path::new("/x/crt.slang"));
        assert!(!p.passes[0].filter_linear);
        assert_eq!(p.passes[0].scale_x, Scale::Source(1.0));
        assert_eq!(p.passes[0].wrap_mode, WrapMode::ClampToEdge);
    }

    #[test]
    fn parses_multipass_with_options_and_comments() {
        let src = r#"
# comentário
shaders = 2

shader0 = "a/first.slang"
filter_linear0 = true
scale_type0 = source
scale0 = 2.0        // hqx-ish
alias0 = FirstPass

shader1 = second.slang
scale_type_x1 = viewport
scale_x1 = 1.0
scale_type_y1 = viewport
scale_y1 = 1.0
wrap_mode1 = clamp_to_border
float_framebuffer1 = true

parameters = "GAMMA;BRIGHT"
GAMMA = 2.4
BRIGHT = 1.0
"#;
        let p = parse_slangp(src, Path::new("/base")).unwrap();
        assert_eq!(p.passes.len(), 2);
        assert_eq!(p.passes[0].shader_path, Path::new("/base/a/first.slang"));
        assert!(p.passes[0].filter_linear);
        assert_eq!(p.passes[0].scale_x, Scale::Source(2.0));
        assert_eq!(p.passes[0].alias.as_deref(), Some("FirstPass"));
        assert_eq!(p.passes[1].scale_x, Scale::Viewport(1.0));
        assert_eq!(p.passes[1].scale_y, Scale::Viewport(1.0));
        assert_eq!(p.passes[1].wrap_mode, WrapMode::ClampToBorder);
        assert!(p.passes[1].float_framebuffer);
        assert_eq!(p.parameters.get("GAMMA"), Some(&2.4));
        assert_eq!(p.parameters.get("BRIGHT"), Some(&1.0));
    }

    #[test]
    fn read_shader_text_tolerates_latin1_bytes() {
        // shaders do RetroArch às vezes têm o nome do autor em Latin-1.
        let dir = std::env::temp_dir().join("reemu_slang_utf8");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("latin1.slang");
        let mut bytes = b"// por Jos\xE9 Nu\xF1ez\n#version 450\n".to_vec();
        bytes.extend_from_slice(b"void main() {}\n");
        std::fs::write(&f, &bytes).unwrap();

        let text = read_shader_text(&f).unwrap();
        assert!(text.contains("#version 450"));
        assert!(text.contains("void main"));
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn missing_shader_count_errors() {
        assert!(matches!(
            parse_slangp("shader0 = x.slang\n", Path::new("/")),
            Err(SlangError::MissingShaderCount)
        ));
    }

    #[test]
    fn missing_indexed_shader_errors() {
        assert!(matches!(
            parse_slangp("shaders = 2\nshader0 = a.slang\n", Path::new("/")),
            Err(SlangError::MissingShader(1, 2))
        ));
    }

    #[test]
    fn follows_reference_and_overrides() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("base.slangp"),
            "shaders = 1\nshader0 = base.slang\nparameters = \"G\"\nG = 1.0\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("child.slangp"),
            "#reference \"base.slangp\"\nG = 2.0\n",
        )
        .unwrap();
        let p = parse_slangp_file(&dir.path().join("child.slangp")).unwrap();
        assert_eq!(p.passes.len(), 1); // herdado
        assert_eq!(p.parameters.get("G"), Some(&2.0)); // sobrescrito
    }

    #[test]
    fn parses_user_textures() {
        let src = "shaders = 1\nshader0 = a.slang\ntextures = \"BG\"\nBG = img/bg.png\nBG_linear = true\nBG_wrap_mode = repeat\n";
        let p = parse_slangp(src, Path::new("/p")).unwrap();
        assert_eq!(p.textures.len(), 1);
        assert_eq!(p.textures[0].name, "BG");
        assert_eq!(p.textures[0].path, Path::new("/p/img/bg.png"));
        assert!(p.textures[0].linear);
        assert_eq!(p.textures[0].wrap_mode, WrapMode::Repeat);
    }
}
