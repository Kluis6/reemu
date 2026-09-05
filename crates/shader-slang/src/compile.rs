//! Compilação `.slang` (GLSL Vulkan) → WGSL.
//!
//! Pipeline: reescrita mínima de layout → **glslang** (GLSL Vulkan → SPIR-V,
//! compilador de referência Khronos) → **naga** (SPIR-V → validação → WGSL).
//! O frontend SPIR-V do `naga` é bem mais completo que o antigo glsl-in: aceita
//! `#define`-macro pesado, construtores `mat4`, ternário em const, etc.
//!
//! Ainda reescrevemos o fonte antes do glslang pra fixar os bindings do jeito
//! que o executor (`gpu.rs`) espera:
//!
//! 1. `layout(push_constant) uniform Push { }` → UBO em `binding = 0`.
//! 2. `uniform UBO { }` → `binding = 1`.
//! 3. `sampler2D` combinado → `texture2D` + `sampler` separados (`<n>_SLANG_T`
//!    / `<n>_SLANG_S`, bindings `2 + 2i` / `3 + 2i`), e **todo uso do
//!    identificador** vira `sampler2D(<n>_SLANG_T, <n>_SLANG_S)`. Trocar o
//!    identificador (não só as chamadas `texture(…)`) é o que mantém válido o
//!    shader que passa o sampler pra função própria. Cada sampler é
//!    classificado por nome ([`TextureSemantic`]) pro executor saber o que
//!    ligar nele.

use crate::preprocess::SlangSource;

/// O que uma textura amostrada pelo shader representa na cadeia (pela
/// convenção de nomes do RetroArch). O executor (`gpu.rs`) resolve isto pro
/// recurso wgpu de cada frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureSemantic {
    /// Saída do passe anterior (ou o frame do core, no 1º passe).
    Source,
    /// O frame do core, sempre (não a saída do passe anterior).
    Original,
    /// O frame do core de N frames atrás (N ≥ 1).
    OriginalHistory(u32),
    /// Saída do passe N **deste** frame (`PassOutputN`).
    PassOutput(u32),
    /// Saída do passe N do frame **anterior** (`PassFeedbackN`).
    PassFeedback(u32),
    /// Nome não reconhecido: um alias de passe (`<alias>` / `<alias>Feedback`)
    /// ou uma textura do usuário do `.slangp`. Quem desambigua é o `gpu.rs`
    /// (tem o preset). `feedback` = o nome terminava em `Feedback`.
    Named { name: String, feedback: bool },
}

/// Um sampler declarado no fragmento, já classificado e com os bindings wgpu
/// que o `rewrite` fixou.
#[derive(Debug, Clone)]
pub struct TextureBind {
    /// Nome do sampler no GLSL (ex.: `Source`, `PassFeedback0`, `LUT`).
    pub name: String,
    pub semantic: TextureSemantic,
    pub tex_binding: u32,
    pub samp_binding: u32,
}

impl TextureSemantic {
    fn classify(name: &str) -> TextureSemantic {
        let num = |s: &str| s.parse::<u32>().ok();
        if name == "Source" {
            TextureSemantic::Source
        } else if name == "Original" {
            TextureSemantic::Original
        } else if let Some(n) = name.strip_prefix("OriginalHistory").and_then(num) {
            TextureSemantic::OriginalHistory(n.max(1))
        } else if let Some(n) = name.strip_prefix("PassOutput").and_then(num) {
            TextureSemantic::PassOutput(n)
        } else if let Some(n) = name.strip_prefix("PassFeedback").and_then(num) {
            TextureSemantic::PassFeedback(n)
        } else if let Some(base) = name.strip_suffix("Feedback") {
            TextureSemantic::Named {
                name: base.to_string(),
                feedback: true,
            }
        } else {
            TextureSemantic::Named {
                name: name.to_string(),
                feedback: false,
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("glslang ({stage}): {msg}")]
    Glslang { stage: &'static str, msg: String },
    #[error("SPIR-V→naga ({stage}): {msg}")]
    SpirV { stage: &'static str, msg: String },
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
    /// Cada sampler combinado que virou `texture2D` + `sampler`, na ordem de
    /// declaração (= a ordem dos bindings) — o executor liga o recurso certo
    /// em cada um pela [`TextureSemantic`].
    pub textures: Vec<TextureBind>,
    /// `(binding, layout)` de cada bloco uniforme. Binding 0 = `Push`,
    /// binding 1 = `UBO` (ver `rewrite`).
    pub uniforms: Vec<(u32, UniformLayout)>,
}

pub fn compile(src: &SlangSource) -> Result<CompiledSlang, CompileError> {
    let (frag, textures) = rewrite(&src.fragment_glsl);
    let (vert, _) = rewrite(&src.vertex_glsl);

    let (vertex_wgsl, vert_module) = compile_stage(&vert, glslang::ShaderStage::Vertex, "vertex")?;
    let (fragment_wgsl, frag_module) =
        compile_stage(&frag, glslang::ShaderStage::Fragment, "fragment")?;
    // Reflete os DOIS estágios e une por binding — o `UBO` (MVP/sizes) costuma
    // aparecer só no vertex, o `Push` (params) só no fragment.
    let mut uniforms = reflect_all(&frag_module)?;
    for (b, layout) in reflect_all(&vert_module)? {
        match uniforms.iter_mut().find(|(vb, _)| *vb == b) {
            Some((_, existing)) if layout.size > existing.size => *existing = layout,
            Some(_) => {}
            None => uniforms.push((b, layout)),
        }
    }
    uniforms.sort_by_key(|(b, _)| *b);
    if uniforms.iter().any(|(b, _)| *b != 0 && *b != 1) {
        return Err(CompileError::Unsupported(
            "bloco uniforme em binding inesperado".into(),
        ));
    }

    Ok(CompiledSlang {
        vertex_wgsl,
        fragment_wgsl,
        textures,
        uniforms,
    })
}

/// Reflete TODOS os blocos uniformes do módulo → `(binding, layout)`.
/// (`Push` em binding 0, `UBO` em binding 1 — ver `rewrite`.)
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
/// novo + os samplers classificados. Bindings finais:
///   0 = `Push` · 1 = `UBO` · 2+2i / 3+2i = textura/sampler i.
fn rewrite(glsl: &str) -> (String, Vec<TextureBind>) {
    // 1. blocos uniformes: força o binding pelo nome do bloco.
    let mut s = String::with_capacity(glsl.len() + 128);
    for line in glsl.lines() {
        let l = if line.contains("push_constant") || declares_block(line, "Push") {
            set_layout(line, "layout(std140, set = 0, binding = 0)")
        } else if declares_block(line, "UBO") {
            set_layout(line, "layout(std140, set = 0, binding = 1)")
        } else {
            line.to_string()
        };
        s.push_str(&l);
        s.push('\n');
    }

    // 2. samplers combinados → texture2D + sampler separados, com bindings
    //    determinísticos (sampler i → textura em 2+2i, sampler em 3+2i).
    //    O nome original NÃO fica com a textura: vira `<name>_SLANG_T`, e todo
    //    uso do identificador é trocado pela expressão construtora no passo 3.
    let mut binds: Vec<TextureBind> = Vec::new();
    let mut out = String::with_capacity(s.len() + 256);
    for line in s.lines() {
        if let Some(name) = decl_combined_sampler(line) {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let i = binds.len() as u32;
            let (tb, sb) = (2 + 2 * i, 3 + 2 * i);
            out.push_str(&format!(
                "{indent}layout(set = 0, binding = {tb}) uniform texture2D {name}_SLANG_T;\n\
                 {indent}layout(set = 0, binding = {sb}) uniform sampler {name}_SLANG_S;\n",
            ));
            binds.push(TextureBind {
                semantic: TextureSemantic::classify(&name),
                name,
                tex_binding: tb,
                samp_binding: sb,
            });
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    // 3. Assinaturas de função que recebem `sampler2D` → par texture+sampler.
    //    A partir daqui trabalhamos sem comentários (ver `blank_comments`).
    let out = blank_comments(&out);
    let (out, fns) = split_sampler_params(&out);

    // 4. Cada uso de identificador de sampler na forma certa pro contexto
    //    (construtora no ponto de uso, ou par como argumento de função).
    let globals: Vec<String> = binds.iter().map(|b| b.name.clone()).collect();
    let out = rewrite_sampler_uses(&out, &globals, &fns);
    (out, binds)
}

// ---------------------------------------------------------------------------
// Separação de sampler combinado (GLSL → GLSL), pré-glslang.
//
// O naga spv-in não consome `OpTypeSampledImage` (ver `probe::
// naga_accepts_combined_sampler`), então todo `sampler2D` precisa virar um par
// `texture2D` + `sampler`. Isso tem três lados:
//
//   a) o global   `uniform sampler2D S;`   → `texture2D S_SLANG_T` + `sampler S_SLANG_S`
//   b) a função   `f(sampler2D t, …)`      → `f(texture2D t_SLANG_T, sampler t_SLANG_S, …)`
//   c) cada uso do identificador, em uma de DUAS formas:
//        - argumento de função do usuário que espera sampler → `X_SLANG_T, X_SLANG_S`
//        - qualquer outro ponto (built-in `texture(…)` etc.)  → `sampler2D(X_SLANG_T, X_SLANG_S)`
//
// A distinção em (c) é obrigatória: GLSL só aceita a construtora `sampler2D(…)`
// no ponto de uso — passá-la como argumento dá
// "sampler constructor must appear at point of use".
// ---------------------------------------------------------------------------

/// Troca todo comentário (`//…` e `/*…*/`) por espaços, preservando o
/// comprimento em bytes e as quebras de linha. Os scanners de sampler abaixo
/// decidem estrutura (parênteses, chaves, a palavra `sampler2D`) contando
/// bytes, e o Mega Bezel tem MUITA assinatura e chave dentro de comentário —
/// sem isto o `{}` desbalanceia e a função corrente é identificada errado.
/// Comentário não faz falta pro glslang, e manter o offset mantém as linhas
/// dos erros dele coerentes.
fn blank_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let end = src[i + 2..]
                .find("*/")
                .map(|o| i + 2 + o + 2)
                .unwrap_or(b.len());
            while i < end {
                out.push(if b[i] == b'\n' { b'\n' } else { b' ' });
                i += 1;
            }
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    // só trocamos bytes ASCII por espaço; o resto ficou intacto
    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

/// Índices dos parâmetros `sampler2D` de cada função do usuário (na numeração
/// original de argumentos) + os nomes desses parâmetros.
#[derive(Debug, Default, Clone)]
struct SamplerFn {
    /// posições (0-based) que eram `sampler2D` na assinatura original
    positions: Vec<usize>,
    /// nome de cada um desses parâmetros
    names: Vec<String>,
}

type SamplerFns = std::collections::HashMap<String, SamplerFn>;

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Acha `needle` como palavra inteira a partir de `from`.
fn find_word(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let b = hay.as_bytes();
    let mut i = from;
    while i <= hay.len() {
        let off = hay.get(i..)?.find(needle)?;
        let p = i + off;
        let e = p + needle.len();
        let before_ok = p == 0 || !is_word_byte(b[p - 1]);
        let after_ok = e >= b.len() || !is_word_byte(b[e]);
        if before_ok && after_ok {
            return Some(p);
        }
        i = e;
    }
    None
}

/// Índice do `)` que fecha o `(` em `open`.
fn matching_paren(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Identificador imediatamente antes do `(` em `open` → `(nome, início)`.
fn ident_before(hay: &str, open: usize) -> Option<(String, usize)> {
    let b = hay.as_bytes();
    let mut e = open;
    while e > 0 && (b[e - 1] as char).is_whitespace() {
        e -= 1;
    }
    let mut s = e;
    while s > 0 && is_word_byte(b[s - 1]) {
        s -= 1;
    }
    (s < e).then(|| (hay[s..e].to_string(), s))
}

/// Fatia a lista de argumentos por vírgula de nível 0 → ranges dentro de `s`.
fn split_args(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let (mut out, mut depth, mut start) = (Vec::new(), 0i32, 0usize);
    for (i, c) in b.iter().enumerate() {
        match c {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
    }
    if !s[start..].trim().is_empty() || !out.is_empty() {
        out.push((start, s.len()));
    }
    out
}

/// Passo (b): reescreve as listas de parâmetro que recebem `sampler2D`.
/// Devolve o fonte novo + o mapa de funções (pro passo (c) saber onde um
/// argumento vira dois).
fn split_sampler_params(src: &str) -> (String, SamplerFns) {
    let mut fns: SamplerFns = SamplerFns::default();
    let mut out = String::with_capacity(src.len() + 256);
    let mut cur = 0usize;

    // Cada `(` … `)` que contenha o token `sampler2D` é lista de parâmetros:
    // nesta altura ainda não geramos nenhuma construtora `sampler2D(`.
    let b = src.as_bytes();
    let mut i = 0usize;
    while i < src.len() {
        let Some(open) = src[i..].find('(').map(|o| i + o) else {
            break;
        };
        let Some(close) = matching_paren(b, open) else {
            break;
        };
        let inner = &src[open + 1..close];
        if find_word(inner, "sampler2D", 0).is_none() {
            i = open + 1;
            continue;
        }
        let Some((fname, _)) = ident_before(src, open) else {
            i = open + 1;
            continue;
        };

        let mut entry = SamplerFn::default();
        let mut new_params: Vec<String> = Vec::new();
        for (idx, (a, z)) in split_args(inner).into_iter().enumerate() {
            let raw = &inner[a..z];
            match sampler_param_name(raw) {
                Some(pname) => {
                    new_params.push(format!(
                        " texture2D {pname}_SLANG_T, sampler {pname}_SLANG_S"
                    ));
                    entry.positions.push(idx);
                    entry.names.push(pname);
                }
                None => new_params.push(raw.to_string()),
            }
        }
        out.push_str(&src[cur..=open]);
        out.push_str(&new_params.join(","));
        out.push(')');
        cur = close + 1;
        i = close + 1;
        if !entry.positions.is_empty() {
            fns.insert(fname, entry);
        }
    }
    out.push_str(&src[cur..]);
    (out, fns)
}

/// `"in sampler2D tex"` → `Some("tex")`; qualquer outra coisa → `None`.
fn sampler_param_name(param: &str) -> Option<String> {
    let t = param.trim();
    let pos = find_word(t, "sampler2D", 0)?;
    // só qualificadores podem vir antes (`in`, `const`…)
    if t[..pos]
        .split_whitespace()
        .any(|w| !matches!(w, "in" | "const" | "highp" | "mediump" | "lowp"))
    {
        return None;
    }
    let name: String = t[pos + "sampler2D".len()..]
        .trim()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Passo (c): reescreve cada uso de identificador de sampler na forma certa
/// pro contexto. `globals` valem no arquivo todo; os parâmetros de uma função
/// só valem dentro do corpo dela (daí o rastreio de `{}` e da função corrente).
fn rewrite_sampler_uses(src: &str, globals: &[String], fns: &SamplerFns) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 512);
    // pilha de chamadas: (callee, índice do argumento corrente)
    let mut calls: Vec<(Option<String>, usize)> = Vec::new();
    let mut brace_depth = 0i32;
    let mut cur_fn: Option<String> = None;
    // (nome, índice do `)`) da última chamada fechada no nível de arquivo —
    // se o `{` seguinte só tiver espaço no meio, é a definição dessa função.
    let mut last_top_close: Option<(String, usize)> = None;
    let mut i = 0usize;

    while i < src.len() {
        let c = b[i];
        // identificador?
        if is_word_byte(c) && !c.is_ascii_digit() {
            let s = i;
            while i < src.len() && is_word_byte(b[i]) {
                i += 1;
            }
            let word = &src[s..i];
            // é chamada? (próximo não-branco é `(`)
            let mut j = i;
            while j < src.len() && (b[j] as char).is_whitespace() {
                j += 1;
            }
            let is_call = j < src.len() && b[j] == b'(';

            let is_sampler = !is_call
                && (globals.iter().any(|g| g == word)
                    || cur_fn
                        .as_ref()
                        .and_then(|f| fns.get(f))
                        .is_some_and(|e| e.names.iter().any(|n| n == word)));

            if is_sampler {
                // forma par só quando é argumento de função do usuário numa
                // posição que era `sampler2D`; senão, construtora no ponto de uso.
                let pair = calls
                    .last()
                    .and_then(|(callee, idx)| {
                        callee.as_ref().and_then(|c| fns.get(c)).map(|e| (e, *idx))
                    })
                    .is_some_and(|(e, idx)| e.positions.contains(&idx));
                if pair {
                    out.push_str(word);
                    out.push_str("_SLANG_T, ");
                    out.push_str(word);
                    out.push_str("_SLANG_S");
                } else {
                    out.push_str("sampler2D(");
                    out.push_str(word);
                    out.push_str("_SLANG_T, ");
                    out.push_str(word);
                    out.push_str("_SLANG_S)");
                }
            } else {
                out.push_str(word);
            }
            if is_call {
                out.push_str(&src[i..j]);
                out.push('(');
                calls.push((Some(word.to_string()), 0));
                i = j + 1;
            }
            continue;
        }
        match c {
            b'(' => {
                calls.push((None, 0));
                out.push('(');
                i += 1;
            }
            b')' => {
                let popped = calls.pop();
                if brace_depth == 0 && calls.is_empty() {
                    if let Some((Some(name), _)) = popped {
                        last_top_close = Some((name, i));
                    }
                }
                out.push(')');
                i += 1;
            }
            b',' => {
                if let Some((_, idx)) = calls.last_mut() {
                    *idx += 1;
                }
                out.push(',');
                i += 1;
            }
            b'{' => {
                if brace_depth == 0 {
                    cur_fn = last_top_close.as_ref().and_then(|(name, close)| {
                        src[close + 1..i].trim().is_empty().then(|| name.clone())
                    });
                }
                brace_depth += 1;
                out.push('{');
                i += 1;
            }
            b'}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    cur_fn = None;
                }
                out.push('}');
                i += 1;
            }
            _ => {
                // avança por CHAR (o fonte pode ter comentário em UTF-8)
                let ch = src[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    out
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
    stage: glslang::ShaderStage,
    label: &'static str,
) -> Result<(String, naga::Module), CompileError> {
    let spirv = glsl_to_spirv(glsl, stage, label)?;

    // SPIR-V do RetroArch é clip-space Vulkan; `adjust_coordinate_space` volta
    // pra convenção do naga/wgsl (flip Y do `BuiltIn::Position`).
    let module = naga::front::spv::Frontend::new(
        spirv.iter().copied(),
        &naga::front::spv::Options {
            adjust_coordinate_space: true,
            strict_capabilities: false,
            block_ctx_dump_prefix: None,
        },
    )
    .parse()
    .map_err(|e| CompileError::SpirV {
        stage: label,
        msg: e.to_string(),
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

/// GLSL Vulkan → SPIR-V (`Vec<u32>`) pelo glslang. `#include` já foi achatado
/// pelo preprocessador, então não passamos includer.
fn glsl_to_spirv(
    glsl: &str,
    stage: glslang::ShaderStage,
    label: &'static str,
) -> Result<Vec<u32>, CompileError> {
    let err = |msg: String| CompileError::Glslang { stage: label, msg };
    let compiler = glslang::Compiler::acquire()
        .ok_or_else(|| err("glslang_initialize_process falhou".into()))?;
    let source = glslang::ShaderSource::from(glsl.to_string());
    // Default = alvo Vulkan 1.0 / SPIR-V 1.0 — a mesma convenção do RetroArch.
    let options = glslang::CompilerOptions::default();
    let input = glslang::ShaderInput::new(
        &source,
        stage,
        &options,
        None::<&[(&str, Option<&str>)]>,
        None,
    )
    .map_err(|e| err(e.to_string()))?;
    let shader = glslang::Shader::new(compiler, input).map_err(|e| err(e.to_string()))?;
    shader.compile().map_err(|e| err(e.to_string()))
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
        assert_eq!(out.textures.len(), 1);
        assert_eq!(out.textures[0].name, "Source");
        assert_eq!(out.textures[0].semantic, TextureSemantic::Source);
        assert_eq!(out.textures[0].tex_binding, 2);
        assert_eq!(out.textures[0].samp_binding, 3);
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
        assert!(bindings.contains(&1)); // UBO
        let ubo = out.uniforms.iter().find(|(b, _)| *b == 1).unwrap();
        assert!(ubo.1.fields.iter().any(|f| f.name == "MVP"));
    }

    #[test]
    fn classifies_multi_texture_semantics() {
        let s = r#"
#version 450
layout(push_constant) uniform Push { vec4 SourceSize; } params;
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;
void main() { gl_Position = Position; vTexCoord = TexCoord; }
#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
layout(set = 0, binding = 3) uniform sampler2D Original;
layout(set = 0, binding = 4) uniform sampler2D OriginalHistory2;
layout(set = 0, binding = 5) uniform sampler2D PassOutput0;
layout(set = 0, binding = 6) uniform sampler2D CrtPassFeedback;
layout(set = 0, binding = 7) uniform sampler2D SnesMask;
void main() {
    FragColor = texture(Source, vTexCoord)
        + texture(Original, vTexCoord) + texture(OriginalHistory2, vTexCoord)
        + texture(PassOutput0, vTexCoord) + texture(CrtPassFeedback, vTexCoord)
        + texture(SnesMask, vTexCoord);
}
"#;
        let out = compile(&preprocess_str(s)).expect("deve compilar");
        let sem: Vec<_> = out.textures.iter().map(|t| t.semantic.clone()).collect();
        assert_eq!(sem[0], TextureSemantic::Source);
        assert_eq!(sem[1], TextureSemantic::Original);
        assert_eq!(sem[2], TextureSemantic::OriginalHistory(2));
        assert_eq!(sem[3], TextureSemantic::PassOutput(0));
        assert_eq!(
            sem[4],
            TextureSemantic::Named {
                name: "CrtPass".into(),
                feedback: true
            }
        );
        assert_eq!(
            sem[5],
            TextureSemantic::Named {
                name: "SnesMask".into(),
                feedback: false
            }
        );
        // bindings determinísticos: textura 2+2i, sampler 3+2i.
        assert_eq!(out.textures[3].tex_binding, 8);
        assert_eq!(out.textures[3].samp_binding, 9);
    }

    // GLSL que o antigo frontend glsl-in do naga rejeitava: `#define`-macro
    // funcional, construtor `mat4`, laço `for` com `textureLod`, ternário.
    // glslang (compilador de referência) engole tudo.
    #[test]
    fn compiles_macro_heavy_glsl_that_naga_glsl_in_rejected() {
        let s = r#"
#version 450
#define PI 3.14159265
#define SAT(x) clamp((x), 0.0, 1.0)
#define TAPS 4
#define TAP_UV(uv, o) ((uv) + vec2((o) * params.SourceSize.z, 0.0))
layout(push_constant) uniform Push {
    vec4 SourceSize;
    float STRENGTH;
} params;
#pragma parameter STRENGTH "Strength" 1.0 0.0 2.0 0.01
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;
void main() { gl_Position = Position; vTexCoord = TexCoord; }
#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
mat4 rot(float a) {
    float c = cos(a), s = sin(a);
    return mat4(c, -s, 0.0, 0.0,
                s,  c, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0);
}
void main() {
    vec3 acc = vec3(0.0);
    for (int i = 0; i < TAPS; i++) {
        float o = float(i) - 1.5;
        acc += textureLod(Source, TAP_UV(vTexCoord, o), 0.0).rgb;
    }
    acc /= float(TAPS);
    float k = params.STRENGTH > 1.0 ? SAT(params.STRENGTH - 1.0) : 0.0;
    vec3 rotated = (rot(k * PI) * vec4(acc, 1.0)).rgb;
    FragColor = vec4(mix(acc, rotated, k), 1.0);
}
"#;
        let out = compile(&preprocess_str(s)).expect("glslang deve compilar");
        assert!(out.fragment_wgsl.contains("@fragment"));
        assert!(out.fragment_wgsl.contains("fn main"));
        let (_, layout) = &out.uniforms[0];
        assert!(layout.fields.iter().any(|f| f.name == "STRENGTH"));
    }

    /// O padrão que travava ~1265 presets: o shader passa o sampler pra uma
    /// função própria. A construtora `sampler2D(…)` NÃO pode ser argumento
    /// ("sampler constructor must appear at point of use"), então a assinatura
    /// tem que virar um par e o call-site expandir em dois argumentos.
    #[test]
    fn sampler_passed_to_user_function_compiles() {
        let s = r#"
#version 450
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vUV;
void main() { gl_Position = Position; vUV = TexCoord; }
#pragma stage fragment
layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
vec4 tap(sampler2D t, vec2 uv) { return texture(t, uv); }
vec3 blur(sampler2D t, vec2 uv, float d) {
    return (tap(t, uv).rgb + tap(t, uv + vec2(d, 0.0)).rgb) * 0.5;
}
void main() { FragColor = vec4(blur(Source, vUV, 0.01), 1.0); }
"#;
        let out = compile(&preprocess_str(s)).expect("sampler em função do usuário");
        assert_eq!(out.textures.len(), 1);
        assert_eq!(out.textures[0].semantic, TextureSemantic::Source);
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    #[test]
    fn splits_signature_and_picks_the_right_use_form() {
        let glsl = "layout(set=0,binding=2) uniform sampler2D Source;\n\
                    vec4 tap(sampler2D t, vec2 uv) { return texture(t, uv); }\n\
                    void main() { FragColor = tap(Source, vUV) + texture(Source, vUV); }\n";
        let (out, binds) = rewrite(glsl);
        assert_eq!(binds.len(), 1);
        // assinatura virou par
        assert!(
            out.contains("texture2D t_SLANG_T, sampler t_SLANG_S"),
            "{out}"
        );
        // dentro do corpo, o parâmetro é usado na forma construtora
        assert!(
            out.contains("texture(sampler2D(t_SLANG_T, t_SLANG_S), uv)"),
            "{out}"
        );
        // no call-site da função do usuário, vira DOIS argumentos (sem construtora)
        assert!(
            out.contains("tap(Source_SLANG_T, Source_SLANG_S, vUV)"),
            "{out}"
        );
        // no built-in, construtora
        assert!(
            out.contains("texture(sampler2D(Source_SLANG_T, Source_SLANG_S), vUV)"),
            "{out}"
        );
    }

    /// `SourceSize`/`Source_SLANG_T` não podem ser atingidos pela troca de
    /// `Source` — a substituição é por palavra inteira.
    #[test]
    fn does_not_touch_longer_identifiers() {
        let glsl = "layout(set=0,binding=2) uniform sampler2D Source;\n\
                    void main() { float a = params.SourceSize.x; vec4 c = texture(Source, uv); }\n";
        let (out, _) = rewrite(glsl);
        assert!(out.contains("params.SourceSize.x"), "{out}");
        assert!(!out.contains("SourceSize_SLANG"), "{out}");
    }

    #[test]
    fn history_sampler_now_compiles_and_is_classified() {
        let s = r#"
#version 450
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vUV;
void main() { gl_Position = Position; vUV = TexCoord; }
#pragma stage fragment
layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 c;
layout(set = 0, binding = 2) uniform sampler2D OriginalHistory1;
void main() { c = texture(OriginalHistory1, vUV); }
"#;
        let out = compile(&preprocess_str(s)).expect("history agora compila (fase 2)");
        assert_eq!(
            out.textures[0].semantic,
            TextureSemantic::OriginalHistory(1)
        );
    }
}

#[cfg(test)]
mod probe {
    /// Experimento: o naga spv-in aceita sampler COMBINADO (`uniform sampler2D`)
    /// vindo do glslang? Se aceitasse, todo o `rewrite` de sampler seria
    /// desnecessário. Ignorado — é sonda de investigação, não regressão.
    #[test]
    #[ignore]
    fn naga_accepts_combined_sampler() {
        let glsl = r#"#version 450
layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
vec4 helper(sampler2D t, vec2 uv) { return texture(t, uv); }
void main() { FragColor = helper(Source, vUV); }
"#;
        let src = glslang::ShaderSource::from(glsl);
        let input = glslang::ShaderInput::new(
            &src,
            glslang::ShaderStage::Fragment,
            &glslang::CompilerOptions {
                source_language: glslang::SourceLanguage::GLSL,
                target: glslang::Target::Vulkan {
                    version: glslang::VulkanVersion::Vulkan1_0,
                    spirv_version: glslang::SpirvVersion::SPIRV1_0,
                },
                ..Default::default()
            },
            None,
            None,
        )
        .expect("glslang aceita sampler combinado + helper");
        let spv = glslang::Compiler::acquire()
            .unwrap()
            .create_shader(input)
            .expect("shader")
            .compile()
            .expect("spirv");
        eprintln!("glslang OK: {} words", spv.len());

        match naga::front::spv::Frontend::new(
            spv.iter().copied(),
            &naga::front::spv::Options {
                adjust_coordinate_space: true,
                strict_capabilities: false,
                block_ctx_dump_prefix: None,
            },
        )
        .parse()
        {
            Ok(m) => {
                eprintln!("naga spv-in OK. globals:");
                for (_, g) in m.global_variables.iter() {
                    eprintln!(
                        "   {:?} space={:?} ty={:?}",
                        g.name, g.space, m.types[g.ty].inner
                    );
                }
                let info = naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&m);
                eprintln!(
                    "validação: {:?}",
                    info.map(|_| "OK").map_err(|e| e.to_string())
                );
            }
            Err(e) => eprintln!("naga spv-in FALHOU: {e:?}"),
        }
    }
}
