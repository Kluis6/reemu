//! Preprocessador do fonte `.slang` (GLSL Vulkan + convenções do RetroArch).
//!
//! - `#include "rel/path"` — recursivo, relativo ao arquivo que inclui.
//! - `#pragma stage vertex` / `#pragma stage fragment` — o que vem antes do
//!   primeiro `#pragma stage` vai pros DOIS estágios; depois, só pro estágio
//!   nomeado.
//! - `#pragma name X` — nome do passe (aliases/feedback).
//! - `#pragma parameter NAME "Rótulo" default min max [step]` — tunável.
//! - `#version` é normalizado pra uma linha só no topo de cada estágio.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::slangp::SlangError;

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub label: String,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

#[derive(Debug, Clone)]
pub struct SlangSource {
    /// `#pragma name` (se houver).
    pub name: Option<String>,
    pub vertex_glsl: String,
    pub fragment_glsl: String,
    pub parameters: Vec<Parameter>,
}

/// Lê e preprocessa um `.slang` do disco.
pub fn preprocess_file(path: &Path) -> Result<SlangSource, SlangError> {
    let mut seen = HashSet::new();
    let flat = flatten_includes(path, &mut seen, 0)?;
    Ok(split(&flat))
}

/// Preprocessa um fonte já em memória (sem seguir `#include`).
pub fn preprocess_str(src: &str) -> SlangSource {
    split(src)
}

fn flatten_includes(
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    depth: u8,
) -> Result<String, SlangError> {
    if depth > 32 {
        return Err(SlangError::ReferenceLoop(path.display().to_string()));
    }
    let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !seen.insert(canon) {
        return Ok(String::new()); // include guard implícito
    }
    let text = std::fs::read_to_string(path).map_err(|source| SlangError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("#include") {
            let inc = rest
                .trim()
                .trim_matches('"')
                .trim_matches('<')
                .trim_matches('>');
            let inc_path = dir.join(inc);
            out.push_str(&flatten_includes(&inc_path, seen, depth + 1)?);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}

fn split(src: &str) -> SlangSource {
    let mut version = "#version 450".to_string();
    let mut name = None;
    let mut params = Vec::new();
    let mut common = String::new();
    let mut vert = String::new();
    let mut frag = String::new();
    // 0 = comum, 1 = vertex, 2 = fragment
    let mut stage = 0u8;

    for line in src.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("#version") {
            version = format!("#version{v}");
            continue;
        }
        if let Some(rest) = t.strip_prefix("#pragma") {
            let rest = rest.trim();
            if let Some(s) = rest.strip_prefix("stage") {
                stage = match s.trim() {
                    "vertex" => 1,
                    "fragment" => 2,
                    _ => stage,
                };
                continue;
            }
            if let Some(n) = rest.strip_prefix("name") {
                name = Some(n.trim().to_string());
                continue;
            }
            if let Some(p) = rest.strip_prefix("parameter") {
                if let Some(param) = parse_parameter(p.trim()) {
                    params.push(param);
                }
                continue;
            }
            // outros #pragma: ignora
            continue;
        }
        match stage {
            1 => {
                vert.push_str(line);
                vert.push('\n');
            }
            2 => {
                frag.push_str(line);
                frag.push('\n');
            }
            _ => {
                common.push_str(line);
                common.push('\n');
            }
        }
    }

    let assemble = |body: &str| format!("{version}\n{common}\n{body}");
    SlangSource {
        name,
        vertex_glsl: assemble(&vert),
        fragment_glsl: assemble(&frag),
        parameters: params,
    }
}

/// `NAME "Rótulo com espaços" default min max [step]`
fn parse_parameter(s: &str) -> Option<Parameter> {
    let name_end = s.find(char::is_whitespace)?;
    let name = s[..name_end].to_string();
    let rest = s[name_end..].trim_start();
    let (label, nums) = if let Some(stripped) = rest.strip_prefix('"') {
        let close = stripped.find('"')?;
        (stripped[..close].to_string(), stripped[close + 1..].trim())
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        (rest[..end].to_string(), rest[end..].trim())
    };
    let mut it = nums
        .split_whitespace()
        .filter_map(|n| n.parse::<f32>().ok());
    let default = it.next()?;
    let min = it.next().unwrap_or(default);
    let max = it.next().unwrap_or(default);
    let step = it.next().unwrap_or(0.0);
    Some(Parameter {
        name,
        label,
        default,
        min,
        max,
        step,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_stages_with_shared_prelude() {
        let src = r#"
#version 450
layout(location = 0) out vec2 vUV;
#pragma stage vertex
void main() { vUV = vec2(0.0); }
#pragma stage fragment
layout(location = 0) out vec4 c;
void main() { c = vec4(vUV, 0.0, 1.0); }
"#;
        let s = preprocess_str(src);
        assert!(s.vertex_glsl.contains("vUV"));
        assert!(s.vertex_glsl.contains("void main() { vUV"));
        assert!(!s.vertex_glsl.contains("vec4 c"));
        assert!(s.fragment_glsl.contains("vUV")); // prelúdio compartilhado
        assert!(s.fragment_glsl.contains("vec4 c"));
        assert!(s.vertex_glsl.starts_with("#version 450"));
    }

    #[test]
    fn extracts_pragma_name_and_parameters() {
        let src = r#"
#pragma name CRT_PASS
#pragma parameter SCANLINE "Scanline Weight" 0.3 0.0 1.0 0.05
#pragma parameter BRIGHTNESS "Brightness" 1.0 0.5 2.0
#pragma stage fragment
void main() {}
"#;
        let s = preprocess_str(src);
        assert_eq!(s.name.as_deref(), Some("CRT_PASS"));
        assert_eq!(s.parameters.len(), 2);
        assert_eq!(s.parameters[0].name, "SCANLINE");
        assert_eq!(s.parameters[0].label, "Scanline Weight");
        assert_eq!(s.parameters[0].default, 0.3);
        assert_eq!(s.parameters[0].step, 0.05);
        assert_eq!(s.parameters[1].max, 2.0);
        assert_eq!(s.parameters[1].step, 0.0);
    }

    #[test]
    fn flattens_includes_with_guard() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("common.inc"),
            "float helper() { return 1.0; }\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("main.slang"),
            "#version 450\n#include \"common.inc\"\n#include \"common.inc\"\n#pragma stage fragment\nvoid main() {}\n",
        )
        .unwrap();
        let s = preprocess_file(&dir.path().join("main.slang")).unwrap();
        // incluído uma vez só (guard)
        assert_eq!(s.fragment_glsl.matches("float helper()").count(), 1);
    }
}
