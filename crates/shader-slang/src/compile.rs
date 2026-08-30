//! Compilação `.slang` (GLSL Vulkan) → WGSL, via `naga`.
//!
//! O frontend GLSL do `naga` não aceita duas coisas que todo shader slang do
//! RetroArch usa, então reescrevemos o fonte antes:
//!
//! 1. `layout(push_constant) uniform ... { }` → um UBO normal.
//! 2. `sampler2D` combinado → `texture2D` + `sampler` separados, com os
//!    call-sites (`texture(...)`, `textureLod`, `textureSize`, `texelFetch`…)
//!    reescritos pra `sampler2D(tex, samp)`.
//!
//! Isso cobre shaders simples (scanline/CRT de arquivo único que só usam
//! `Source`). Multi-sampler pesado (Mega Bezel: `PassFeedback`, history,
//! LUTs) ainda não — retorna `Unsupported` com o motivo.

use crate::preprocess::SlangSource;

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("GLSL ({stage}): {msg}")]
    Glsl { stage: &'static str, msg: String },
    #[error("validação naga ({stage}): {msg}")]
    Validate { stage: &'static str, msg: String },
    #[error("geração WGSL ({stage}): {msg}")]
    Wgsl { stage: &'static str, msg: String },
    #[error("recurso de shader ainda não suportado: {0}")]
    Unsupported(String),
}

/// Tipo de um campo do bloco uniforme (pra saber o que escrever nele).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformFieldKind {
    Mat4,
    Vec4,
    Vec3,
    Vec2,
    F32,
    U32,
    I32,
    Other,
}

#[derive(Debug, Clone)]
pub struct UniformField {
    pub name: String,
    pub offset: u32,
    pub kind: UniformFieldKind,
}

/// Layout std140 do (único) bloco uniforme do shader — o executor usa pra
/// montar o buffer preenchendo semânticas (`MVP`, `SourceSize`, `FrameCount`…)
/// e parâmetros por nome.
#[derive(Debug, Clone, Default)]
pub struct UniformLayout {
    pub size: u32,
    pub fields: Vec<UniformField>,
}

#[derive(Debug, Clone)]
pub struct CompiledSlang {
    /// WGSL com `@vertex fn main(...)`.
    pub vertex_wgsl: String,
    /// WGSL com `@fragment fn main(...)`.
    pub fragment_wgsl: String,
    /// Nomes dos samplers combinados que viraram `texture2D` + `sampler`
    /// (ordem de declaração) — o executor usa pra montar os bind groups.
    pub samplers: Vec<String>,
    /// `(binding, layout)` de cada bloco uniforme. Binding 0 = `Push`,
    /// binding 3 = `UBO` (ver `rewrite`).
    pub uniforms: Vec<(u32, UniformLayout)>,
}

pub fn compile(src: &SlangSource) -> Result<CompiledSlang, CompileError> {
    let (frag, samplers) = rewrite(&src.fragment_glsl);
    let (vert, _) = rewrite(&src.vertex_glsl);

    for feat in ["Feedback", "OriginalHistory", "PassFeedback", "PassOutput"] {
        if frag.contains(feat) || vert.contains(feat) {
            return Err(CompileError::Unsupported(format!("semântica `{feat}`")));
        }
    }
    if samplers.iter().any(|s| s != "Source") {
        return Err(CompileError::Unsupported(
            "só `Source` como sampler por ora".into(),
        ));
    }

    let (vertex_wgsl, _) = compile_stage(&vert, naga::ShaderStage::Vertex, "vertex")?;
    let (fragment_wgsl, frag_module) =
        compile_stage(&frag, naga::ShaderStage::Fragment, "fragment")?;
    let uniforms = reflect_all(&frag_module)?;
    if uniforms.iter().any(|(b, _)| *b != 0 && *b != 3) {
        return Err(CompileError::Unsupported(
            "bloco uniforme em binding inesperado".into(),
        ));
    }

    Ok(CompiledSlang {
        vertex_wgsl,
        fragment_wgsl,
        samplers,
        uniforms,
    })
}

/// Reflete TODOS os blocos uniformes do módulo → `(binding, layout)`.
/// (RetroArch: `Push` em binding 0, `UBO` em binding 3 — ver `rewrite`.)
fn reflect_all(module: &naga::Module) -> Result<Vec<(u32, UniformLayout)>, CompileError> {
    let mut out = Vec::new();
    for (_, g) in module
        .global_variables
        .iter()
        .filter(|(_, g)| g.space == naga::AddressSpace::Uniform)
    {
        let binding = g.binding.as_ref().map(|b| b.binding).unwrap_or(0);
        out.push((binding, reflect_struct(module, g.ty)?));
    }
    Ok(out)
}

fn reflect_struct(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
) -> Result<UniformLayout, CompileError> {
    let naga::TypeInner::Struct { members, span } = &module.types[ty].inner else {
        return Err(CompileError::Unsupported(
            "bloco uniforme não-struct".into(),
        ));
    };
    let mut fields = Vec::new();
    for m in members {
        let Some(name) = &m.name else { continue };
        let kind = match &module.types[m.ty].inner {
            naga::TypeInner::Matrix {
                columns: naga::VectorSize::Quad,
                rows: naga::VectorSize::Quad,
                ..
            } => UniformFieldKind::Mat4,
            naga::TypeInner::Vector { size, .. } => match size {
                naga::VectorSize::Quad => UniformFieldKind::Vec4,
                naga::VectorSize::Tri => UniformFieldKind::Vec3,
                naga::VectorSize::Bi => UniformFieldKind::Vec2,
            },
            naga::TypeInner::Scalar(s) => match s.kind {
                naga::ScalarKind::Float => UniformFieldKind::F32,
                naga::ScalarKind::Uint => UniformFieldKind::U32,
                naga::ScalarKind::Sint => UniformFieldKind::I32,
                _ => UniformFieldKind::Other,
            },
            _ => UniformFieldKind::Other,
        };
        fields.push(UniformField {
            name: name.clone(),
            offset: m.offset,
            kind,
        });
    }
    Ok(UniformLayout {
        size: (*span).max(16),
        fields,
    })
}

/// Reescreve os blocos uniformes e os samplers combinados. Devolve o GLSL
/// novo + os nomes de sampler encontrados. Bindings finais:
///   0 = `Push`/params · 1 = texture `Source` · 2 = sampler · 3 = `UBO`/global.
fn rewrite(glsl: &str) -> (String, Vec<String>) {
    // 1. blocos uniformes: força o binding pelo nome do bloco.
    let mut s = String::with_capacity(glsl.len() + 128);
    for line in glsl.lines() {
        let l = if line.contains("push_constant") || declares_block(line, "Push") {
            set_layout(line, "layout(std140, set = 0, binding = 0)")
        } else if declares_block(line, "UBO") {
            set_layout(line, "layout(std140, set = 0, binding = 3)")
        } else {
            line.to_string()
        };
        s.push_str(&l);
        s.push('\n');
    }

    // 2. samplers combinados → texture2D + sampler com bindings determinísticos
    //    (sampler i → texture em 1+2i, sampler em 2+2i).
    let mut samplers = Vec::new();
    let mut out = String::with_capacity(s.len() + 256);
    for line in s.lines() {
        if let Some(name) = decl_combined_sampler(line) {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let i = samplers.len() as u32;
            out.push_str(&format!(
                "{indent}layout(set = 0, binding = {}) uniform texture2D {name};\n\
                 {indent}layout(set = 0, binding = {}) uniform sampler {name}_SLANG_S;\n",
                1 + 2 * i,
                2 + 2 * i,
            ));
            samplers.push(name);
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    // 3. call-sites: `fn(NAME, ...)` → `fn(sampler2D(NAME, NAME_SLANG_S), ...)`.
    for name in &samplers {
        for func in [
            "texture",
            "textureLod",
            "textureProj",
            "textureGrad",
            "textureGather",
            "textureSize",
            "texelFetch",
            "textureOffset",
        ] {
            let from = format!("{func}({name},");
            let to = format!("{func}(sampler2D({name}, {name}_SLANG_S),");
            out = out.replace(&from, &to);
            // variante sem espaço depois da vírgula já coberta; com espaço antes:
            let from_sp = format!("{func}( {name},");
            out = out.replace(&from_sp, &to);
        }
    }
    (out, samplers)
}

/// `true` se a linha declara `uniform <name>` (`<name>` como palavra inteira,
/// seguida de espaço, `{` ou fim de linha).
fn declares_block(line: &str, name: &str) -> bool {
    let needle = format!("uniform {name}");
    let Some(pos) = line.find(&needle) else {
        return false;
    };
    match line[pos + needle.len()..].chars().next() {
        None => true,
        Some(c) => c.is_whitespace() || c == '{',
    }
}

/// Troca o `layout(...)` do começo da linha (ignorando espaços) por `new`.
fn set_layout(line: &str, new: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let Some(rest) = trimmed.strip_prefix("layout") else {
        return line.to_string();
    };
    // pula até fechar o primeiro `)`
    match rest.find(')') {
        Some(i) => format!("{indent}{new}{}", &rest[i + 1..]),
        None => line.to_string(),
    }
}

/// `... uniform sampler2D NAME;` → `Some("NAME")`.
fn decl_combined_sampler(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.contains("uniform") || !t.contains("sampler2D") {
        return None;
    }
    let after = t.split("sampler2D").nth(1)?.trim();
    let name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn compile_stage(
    glsl: &str,
    stage: naga::ShaderStage,
    label: &'static str,
) -> Result<(String, naga::Module), CompileError> {
    let mut frontend = naga::front::glsl::Frontend::default();
    let module = frontend
        .parse(
            &naga::front::glsl::Options {
                stage,
                defines: Default::default(),
            },
            glsl,
        )
        .map_err(|e| CompileError::Glsl {
            stage: label,
            msg: format!("{e:?}"),
        })?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| CompileError::Validate {
        stage: label,
        msg: format!("{e:?}"),
    })?;
    let wgsl =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .map_err(|e| CompileError::Wgsl {
                stage: label,
                msg: format!("{e:?}"),
            })?;
    Ok((wgsl, module))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocess::preprocess_str;

    const CRT_SLANG: &str = r#"
#version 450
layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OutputSize;
    uint FrameCount;
    float SCANLINE;
} params;
#pragma parameter SCANLINE "Scanline" 0.3 0.0 1.0 0.05

#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;
void main() {
    gl_Position = Position;
    vTexCoord = TexCoord;
}

#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
void main() {
    vec3 c = texture(Source, vTexCoord).rgb;
    float line = fract(vTexCoord.y * params.SourceSize.y);
    float s = 1.0 - params.SCANLINE * pow(sin(line * 3.14159), 2.0);
    FragColor = vec4(c * s, 1.0);
}
"#;

    #[test]
    fn compiles_single_pass_crt_to_wgsl() {
        let src = preprocess_str(CRT_SLANG);
        assert_eq!(src.parameters.len(), 1);
        let out = compile(&src).expect("deve compilar");
        assert_eq!(out.samplers, vec!["Source".to_string()]);
        assert!(out.fragment_wgsl.contains("@fragment"));
        assert!(out.fragment_wgsl.contains("fn main"));
        assert!(out.vertex_wgsl.contains("@vertex"));
        // o sampler combinado foi separado
        assert!(out.fragment_wgsl.contains("texture_2d"));
        // reflection do bloco uniforme (Push → binding 0)
        assert_eq!(out.uniforms.len(), 1);
        let (b, layout) = &out.uniforms[0];
        assert_eq!(*b, 0);
        assert!(layout.size >= 16);
        let f = |n: &str| layout.fields.iter().find(|f| f.name == n);
        assert_eq!(
            f("SourceSize").map(|f| f.kind),
            Some(UniformFieldKind::Vec4)
        );
        assert_eq!(f("FrameCount").map(|f| f.kind), Some(UniformFieldKind::U32));
        assert_eq!(f("SCANLINE").map(|f| f.kind), Some(UniformFieldKind::F32));
    }

    #[test]
    fn compiles_retroarch_style_ubo_plus_push() {
        // convenção RetroArch: UBO (MVP + sizes) + Push (params)
        let s = r#"
#version 450
layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
    vec4 SourceSize;
    vec4 OutputSize;
} global;
layout(push_constant) uniform Push {
    float SCANLINE_BASE;
} params;
#pragma parameter SCANLINE_BASE "Scanline" 0.5 0.0 1.0 0.05
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;
void main() { gl_Position = global.MVP * Position; vTexCoord = TexCoord; }
#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
void main() {
    vec3 c = texture(Source, vTexCoord).rgb;
    float s = 1.0 - params.SCANLINE_BASE * abs(sin(vTexCoord.y * global.SourceSize.y * 3.14159));
    FragColor = vec4(c * s, 1.0);
}
"#;
        let out = compile(&preprocess_str(s)).expect("UBO+Push deve compilar");
        let bindings: Vec<u32> = out.uniforms.iter().map(|(b, _)| *b).collect();
        assert!(bindings.contains(&0)); // Push
        assert!(bindings.contains(&3)); // UBO
        let ubo = out.uniforms.iter().find(|(b, _)| *b == 3).unwrap();
        assert!(ubo.1.fields.iter().any(|f| f.name == "MVP"));
    }

    #[test]
    fn rejects_feedback_semantics() {
        let src = preprocess_str(
            "#version 450\n#pragma stage fragment\nlayout(set=0,binding=2) uniform sampler2D OriginalHistory1;\nlayout(location=0) out vec4 c;\nvoid main(){ c = texture(OriginalHistory1, vec2(0.0)); }\n",
        );
        assert!(matches!(compile(&src), Err(CompileError::Unsupported(_))));
    }
}
