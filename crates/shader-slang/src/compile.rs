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
    let (frag, textures) = rewrite(&src.fragment_glsl, Stage::Fragment);
    let (vert, _) = rewrite(&src.vertex_glsl, Stage::Vertex);

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
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Vertex,
    Fragment,
}

fn rewrite(glsl: &str, stage: Stage) -> (String, Vec<TextureBind>) {
    // 0. builtins do GLSL que o backend WGSL do naga não tem: injeta um
    //    equivalente e troca as chamadas.
    let glsl = patch_missing_builtins(glsl);

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
    // 3-. função não-void sem `return` final → `return <zero>;` no fim (o
    //     frontend SPIR-V do naga rejeita com `ExpressionAlreadyInScope`).
    let out = ensure_trailing_returns(&out);
    // 3b. varying de struct/array → `location`s escalares (WGSL não aceita
    //     agregado atravessando estágio).
    let out = flatten_io_aggregates(&out);
    let out = flat_integer_varyings(&out, stage);
    // 3a. macro de TIPO que esconde o `sampler2D` do parâmetro (SMAA:
    //     `#define SMAATexture2D(tex) sampler2D tex`) — sem isto o passo
    //     seguinte não enxerga o parâmetro e o call-site vira construtora.
    let out = expand_sampler_type_macros(&out);
    let (out, mut fns) = split_sampler_params(&out);
    // nomes de parâmetro sampler de FUNÇÕES (antes de entrar as macros no mapa)
    let fn_sampler_params: std::collections::HashSet<String> =
        fns.values().flat_map(|e| e.names.iter().cloned()).collect();

    // 3c. macro que repassa sampler pra função (Mega Bezel:
    //     `#define COMPAT_TEXTURE(c,d) HSM_GetCroppedTexSample(c,d)`).
    let out = split_sampler_macros(&out, &mut fns);

    // 4. Cada uso de identificador de sampler na forma certa pro contexto
    //    (construtora no ponto de uso, ou par como argumento de função).
    let mut globals: Vec<String> = binds.iter().map(|b| b.name.clone()).collect();
    // 3d. apelido de sampler global (`#define input_texture Source`, crt-royale)
    //     → vira um par de defines `_SLANG_T`/`_SLANG_S` e passa a contar como
    //     sampler, senão o call-site de função do usuário recebe a construtora
    //     em vez do par.
    let (out, aliases) = split_sampler_aliases(&out, &globals, &fn_sampler_params);
    globals.extend(aliases);
    let out = rewrite_sampler_uses(&out, &globals, &fns, &fn_sampler_params);
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

// ---------------------------------------------------------------------------
// Achatamento de varying agregado (struct / array) → `location`s escalares.
//
// WGSL não aceita struct nem array como entrada/saída de estágio; o naga
// rejeita com `NotIOShareableType`. Vários shaders usam mesmo assim — o
// scanline-classic passa um `struct TimebaseConfig` do vertex pro fragment, o
// koko-aio um `float[N]`.
//
// Reescrever os USOS não serve: o array é indexado com variável de laço, e não
// há como quebrar isso em variáveis soltas. Então mantemos a variável original
// como global do estágio e achatamos só as `location`s, copiando nas pontas:
//   - vertex:   epílogo no fim do `main`     `tb_SLANG_V0 = tb.a; …`
//   - fragment: prólogo no início do `main`  `tb.a = tb_SLANG_V0; …`
// O corpo do shader fica intacto, indexação dinâmica inclusive.
//
// Nos shaders reais o agregado é sempre o varying de maior `location`, então
// expandir pra cima a partir da location dele não colide com os outros.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct StructMember {
    ty: String,
    name: String,
    /// tamanho como veio no fonte (número ou nome de `#define`)
    array: Option<String>,
}

type StructDefs = std::collections::HashMap<String, Vec<StructMember>>;

/// Tipos que o WGSL aceita direto numa `location`.
fn is_io_scalar(ty: &str) -> bool {
    matches!(
        ty,
        "float"
            | "vec2"
            | "vec3"
            | "vec4"
            | "int"
            | "ivec2"
            | "ivec3"
            | "ivec4"
            | "uint"
            | "uvec2"
            | "uvec3"
            | "uvec4"
    )
}

/// `"mat3"` → `(3, "vec3")`, `"mat3x2"` → `(3, "vec2")`. Matriz também não é
/// IO-shareable no WGSL; vira uma `location` por coluna — que é exatamente o
/// espaço que ela já ocupava (o shader pula as locations dela).
fn mat_shape(ty: &str) -> Option<(usize, String)> {
    let rest = ty.strip_prefix("mat")?;
    let (cols, rows) = match rest.split_once('x') {
        Some((c, r)) => (c.parse::<usize>().ok()?, r.parse::<usize>().ok()?),
        None => {
            let n = rest.parse::<usize>().ok()?;
            (n, n)
        }
    };
    (2..=4)
        .contains(&cols)
        .then(|| (cols, format!("vec{rows}")))
}

/// Varying inteiro sem `flat` ganha `flat`. O GLSL só exige `flat` na
/// ENTRADA do fragment, então vários shaders declaram o `out int` do vertex
/// sem — o naga exige nas duas pontas (`InvalidInterpolationForInteger`,
/// ntsc-blastem). Só a saída do vertex e a entrada do fragment são varyings:
/// atributo do vertex e alvo de render do fragment não aceitam `flat`.
fn flat_integer_varyings(src: &str, stage: Stage) -> String {
    let is_int = |ty: &str| {
        matches!(ty, "int" | "uint")
            || ((ty.starts_with("ivec") || ty.starts_with("uvec")) && ty.len() == 5)
    };
    let mut out = String::with_capacity(src.len() + 32);
    for line in src.split_inclusive('\n') {
        let fixed = parse_varying(line).and_then(|(_, is_out, quals, ty, _, _)| {
            let is_varying = is_out == (stage == Stage::Vertex);
            if !is_varying || !is_int(&ty) || quals.split_whitespace().any(|q| q == "flat") {
                return None;
            }
            let close = line.find(')')?;
            let kw = if is_out { "out" } else { "in" };
            let at = find_word(line, kw, close + 1)?;
            let mut l = line.to_string();
            l.insert_str(at, "flat ");
            Some(l)
        });
        out.push_str(fixed.as_deref().unwrap_or(line));
    }
    out
}

/// Inteiro só atravessa estágio como `flat`.
fn needs_flat(ty: &str) -> bool {
    ty.starts_with('i') || ty.starts_with('u')
}

/// `"blurCoordinates[5]"` → `("blurCoordinates", Some("5"))`.
fn split_array_suffix(s: &str) -> (String, Option<String>) {
    let s = s.trim();
    match s.find('[') {
        None => (s.to_string(), None),
        Some(o) => {
            let name = s[..o].trim().to_string();
            let inner = s[o + 1..]
                .split(']')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            (name, Some(inner))
        }
    }
}

fn matching_brace(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in b.iter().enumerate().skip(open) {
        match c {
            b'{' => depth += 1,
            b'}' => {
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

/// `struct NOME { tipo campo; … };` → membros, na ordem de declaração.
fn parse_struct_defs(code: &str) -> StructDefs {
    let mut out = StructDefs::default();
    let b = code.as_bytes();
    let mut i = 0usize;
    while let Some(p) = find_word(code, "struct", i) {
        let name: String = code[p + 6..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let Some(open) = code[p..].find('{').map(|o| p + o) else {
            break;
        };
        let Some(close) = matching_brace(b, open) else {
            break;
        };
        if !name.is_empty() {
            let members = code[open + 1..close]
                .split(';')
                .filter_map(|decl| {
                    let mut it = decl.split_whitespace();
                    let ty = it.next()?;
                    let rest = it.next()?;
                    let (nm, arr) = split_array_suffix(rest);
                    Some(StructMember {
                        ty: ty.to_string(),
                        name: nm,
                        array: arr,
                    })
                })
                .collect();
            out.insert(name, members);
        }
        i = close + 1;
    }
    out
}

/// `#define NOME 12` → mapa, pra resolver tamanho de array simbólico.
fn parse_int_defines(code: &str) -> std::collections::HashMap<String, usize> {
    let mut out = std::collections::HashMap::new();
    for line in code.lines() {
        let Some(rest) = line.trim().strip_prefix("#define") else {
            continue;
        };
        let mut it = rest.split_whitespace();
        let (Some(name), Some(val)) = (it.next(), it.next()) else {
            continue;
        };
        if it.next().is_some() || name.contains('(') {
            continue;
        }
        if let Ok(v) = val.parse::<usize>() {
            out.insert(name.to_string(), v);
        }
    }
    out
}

fn array_len(spec: &str, defines: &std::collections::HashMap<String, usize>) -> Option<usize> {
    spec.parse::<usize>()
        .ok()
        .or_else(|| defines.get(spec).copied())
        .filter(|n| *n > 0 && *n <= 64)
}

/// Expande `ty name[array]` nas folhas `(tipo, sufixo de acesso)`.
/// `false` = tem algo que não sabemos achatar (mat*, tipo desconhecido) —
/// nesse caso o chamador deixa a declaração como está.
fn flatten_leaves(
    ty: &str,
    access: &str,
    array: Option<&str>,
    structs: &StructDefs,
    defines: &std::collections::HashMap<String, usize>,
    depth: u32,
    out: &mut Vec<(String, String)>,
) -> bool {
    if depth > 4 || out.len() > 48 {
        return false;
    }
    if let Some(spec) = array {
        let Some(n) = array_len(spec, defines) else {
            return false;
        };
        return (0..n).all(|i| {
            flatten_leaves(
                ty,
                &format!("{access}[{i}]"),
                None,
                structs,
                defines,
                depth + 1,
                out,
            )
        });
    }
    if let Some(members) = structs.get(ty) {
        return members.iter().all(|m| {
            flatten_leaves(
                &m.ty,
                &format!("{access}.{}", m.name),
                m.array.as_deref(),
                structs,
                defines,
                depth + 1,
                out,
            )
        });
    }
    if let Some((cols, col_ty)) = mat_shape(ty) {
        for c in 0..cols {
            out.push((col_ty.clone(), format!("{access}[{c}]")));
        }
        return true;
    }
    if is_io_scalar(ty) {
        out.push((ty.to_string(), access.to_string()));
        return true;
    }
    false
}

/// Uma declaração `layout(location = N) [quals] in|out TIPO nome[arr];`.
struct VaryingDecl {
    line_start: usize,
    line_end: usize,
    location: u32,
    is_out: bool,
    quals: String,
    ty: String,
    name: String,
    array: Option<String>,
}

/// Parseia a linha se ela declarar um varying com `layout(location = N)`.
fn parse_varying(line: &str) -> Option<(u32, bool, String, String, String, Option<String>)> {
    let t = line.trim();
    let loc_at = t.find("location")?;
    if !t.starts_with("layout") {
        return None;
    }
    let after_eq = t[loc_at..].split('=').nth(1)?;
    let location: u32 = after_eq
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    let close = t.find(')')?;
    let rest = t[close + 1..].trim().trim_end_matches(';').trim();
    let mut quals = Vec::new();
    let mut words = rest.split_whitespace().peekable();
    let mut is_out = None;
    while let Some(w) = words.peek() {
        match *w {
            "flat" | "noperspective" | "smooth" | "centroid" | "sample" | "highp" | "mediump"
            | "lowp" => {
                quals.push(words.next()?.to_string());
            }
            "in" => {
                words.next();
                is_out = Some(false);
                break;
            }
            "out" => {
                words.next();
                is_out = Some(true);
                break;
            }
            _ => return None,
        }
    }
    let is_out = is_out?;
    let ty = words.next()?.to_string();
    let (name, array) = split_array_suffix(&words.collect::<Vec<_>>().join(" "));
    (!name.is_empty()).then_some((location, is_out, quals.join(" "), ty, name, array))
}

/// Ver o comentário do topo do módulo.
fn flatten_io_aggregates(glsl: &str) -> String {
    let code = blank_comments(glsl);
    let structs = parse_struct_defs(&code);
    let defines = parse_int_defines(&code);

    // acha os varyings que precisam achatar
    let mut decls: Vec<VaryingDecl> = Vec::new();
    let mut off = 0usize;
    for line in code.split_inclusive('\n') {
        let start = off;
        off += line.len();
        let Some((location, is_out, quals, ty, name, array)) = parse_varying(line) else {
            continue;
        };
        // já é aceitável pro WGSL? então não mexe
        if array.is_none() && is_io_scalar(&ty) {
            continue;
        }
        decls.push(VaryingDecl {
            line_start: start,
            line_end: off,
            location,
            is_out,
            quals,
            ty,
            name,
            array,
        });
    }
    if decls.is_empty() {
        return glsl.to_string();
    }

    // monta a substituição de cada declaração + as cópias do prólogo/epílogo
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut copies: Vec<String> = Vec::new();
    let mut any_out = false;
    for d in &decls {
        let mut leaves = Vec::new();
        if !flatten_leaves(
            &d.ty,
            &d.name,
            d.array.as_deref(),
            &structs,
            &defines,
            0,
            &mut leaves,
        ) {
            continue; // não sabemos achatar — deixa como está
        }
        let mut repl = String::new();
        for (i, (lty, access)) in leaves.iter().enumerate() {
            let mut q = d.quals.clone();
            if needs_flat(lty) && !q.contains("flat") {
                q = format!("flat {q}");
            }
            let q = if q.trim().is_empty() {
                String::new()
            } else {
                format!("{} ", q.trim())
            };
            let dir = if d.is_out { "out" } else { "in" };
            let var = format!("{}_SLANG_V{i}", d.name);
            repl.push_str(&format!(
                "layout(location = {}) {q}{dir} {lty} {var};\n",
                d.location + i as u32
            ));
            copies.push(if d.is_out {
                format!("    {var} = {access};")
            } else {
                format!("    {access} = {var};")
            });
        }
        // a variável original vira global do estágio (o corpo não muda)
        let arr = d
            .array
            .as_deref()
            .map(|a| format!("[{a}]"))
            .unwrap_or_default();
        repl.push_str(&format!("{} {}{arr};\n", d.ty, d.name));
        any_out |= d.is_out;
        edits.push((d.line_start, d.line_end, repl));
    }
    if edits.is_empty() {
        return glsl.to_string();
    }

    // aplica as substituições de declaração (de trás pra frente, offsets estáveis)
    let mut out = glsl.to_string();
    for (s, e, repl) in edits.iter().rev() {
        out.replace_range(*s..*e, repl);
    }

    // injeta as cópias no `main`: epílogo se é saída (vertex), prólogo se é
    // entrada (fragment).
    let body = copies.join("\n");
    let scan = blank_comments(&out);
    let Some(mp) = find_word(&scan, "main", 0) else {
        return out;
    };
    let Some(open) = scan[mp..].find('{').map(|o| mp + o) else {
        return out;
    };
    if any_out {
        let Some(close) = matching_brace(scan.as_bytes(), open) else {
            return out;
        };
        out.insert_str(close, &format!("\n{body}\n"));
    } else {
        out.insert_str(open + 1, &format!("\n{body}\n"));
    }
    out
}

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
                    new_params.push(split_sampler_decl(raw, &pname));
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

/// `(nome, corpo)` de um `#define NOME(p) <corpo>` de 1 parâmetro, com o
/// corpo já trocado pra `$` no lugar do parâmetro (ex: `sampler2D $`, `$`).
fn one_param_macro(line: &str) -> Option<(String, String)> {
    let rest = line.trim_start().strip_prefix("#define")?.trim_start();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    let after = rest[name.len()..].strip_prefix('(')?;
    let close = after.find(')')?;
    let param = after[..close].trim();
    if name.is_empty() || param.is_empty() || param.contains(',') {
        return None;
    }
    let body: Vec<&str> = after[close + 1..]
        .split_whitespace()
        .map(|w| if w == param { "$" } else { w })
        .collect();
    Some((name, body.join(" ")))
}

/// Expande, nos USOS, as macros de 1 parâmetro que escondem um sampler da
/// reescrita (SMAA):
/// - de tipo, `#define SMAATexture2D(tex) sampler2D tex` → `sampler2D x`, pro
///   parâmetro ser visto como sampler. Basta UMA definição ser essa: as
///   outras variantes são HLSL (`Texture2D tex`) em ramos `#if` inativos.
/// - de repasse, `#define SMAATexturePass2D(tex) tex` → `x`, pro call-site
///   ser visto como argumento de função. Só se TODAS as definições do nome
///   forem repasse (aí é exatamente o que o pré-processador faria).
///   As linhas `#define` em si ficam intactas.
fn expand_sampler_type_macros(src: &str) -> String {
    let mut bodies: std::collections::HashMap<String, Vec<String>> = Default::default();
    for (name, body) in src.lines().filter_map(one_param_macro) {
        bodies.entry(name).or_default().push(body);
    }
    let mut repl: Vec<(String, &str)> = Vec::new();
    for (name, defs) in &bodies {
        if defs.iter().any(|b| b == "sampler2D $") {
            repl.push((name.clone(), "sampler2D "));
        } else if defs.iter().all(|b| b == "$") {
            repl.push((name.clone(), ""));
        }
    }
    if repl.is_empty() {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    for line in src.split_inclusive('\n') {
        if line.trim_start().starts_with('#') {
            out.push_str(line);
            continue;
        }
        let mut l = line.to_string();
        for (name, prefix) in &repl {
            let mut from = 0;
            while let Some(pos) = find_word(&l, name, from) {
                let open = pos + name.len();
                let b = l.as_bytes();
                let paren = (open..l.len()).find(|&i| !b[i].is_ascii_whitespace());
                let Some(open) = paren.filter(|&i| b[i] == b'(') else {
                    from = open;
                    continue;
                };
                let Some(close) = matching_paren(b, open) else {
                    break;
                };
                let text = format!("{prefix}{}", l[open + 1..close].trim());
                l.replace_range(pos..=close, &text);
                from = pos + text.len();
            }
        }
        out.push_str(&l);
    }
    out
}

/// `#define APELIDO <sampler global>` → `#define APELIDO_SLANG_T X_SLANG_T` +
/// `#define APELIDO_SLANG_S X_SLANG_S`, no MESMO lugar (mesmo ramo `#if`), e o
/// APELIDO entra na lista de samplers. Assim a reescrita dos usos gera o par ou
/// a construtora a partir do apelido, e o pré-processador resolve o ramo — o
/// crt-royale tem `MASK_RESIZEtexture` apontando pra `Source` num ramo e pra
/// `MASK_RESIZE` no outro. Nome que já é sampler (koko-aio:
/// `#define FPS_ESTIMATE_PASS <sampler>`) fica como está. `#undef` acompanha.
///
/// Só vira apelido o nome cujas definições TODAS apontam pra sampler global
/// (senão o ramo não-sampler referenciaria `NOME_SLANG_T` inexistente), e que
/// não é nome de parâmetro sampler de função (o `#define NOME_SLANG_T` gerado
/// substituiria o parâmetro já renomeado dentro da função).
fn split_sampler_aliases(
    src: &str,
    globals: &[String],
    fn_sampler_params: &std::collections::HashSet<String>,
) -> (String, Vec<String>) {
    // (nome, corpo) de cada `#define NOME <uma palavra ou nada>` sem parâmetros;
    // corpo `None` = definição que não é uma palavra só (nunca é apelido).
    let mut defs: Vec<(String, Option<String>)> = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.trim_start().strip_prefix("#define") else {
            continue;
        };
        let mut words = rest.split_whitespace();
        let Some(name) = words.next() else {
            continue;
        };
        if !name.bytes().all(is_word_byte) {
            continue; // macro com parâmetros
        }
        let body = match (words.next(), words.next()) {
            (Some(b), None) => Some(b.to_string()),
            _ => None,
        };
        defs.push((name.to_string(), body));
    }
    // Ponto fixo: apelido de apelido também vale (metacrt:
    // `iChannelCurr` → `iChannel0` → `Source`).
    let mut known: Vec<String> = globals.to_vec();
    let mut aliases: Vec<String> = Vec::new();
    loop {
        let mut grew = false;
        for (name, _) in &defs {
            if known.contains(name) || fn_sampler_params.contains(name) {
                continue;
            }
            let all_point_to_known = defs
                .iter()
                .filter(|(n, _)| n == name)
                .all(|(_, b)| b.as_ref().is_some_and(|b| known.contains(b)));
            if all_point_to_known {
                known.push(name.clone());
                aliases.push(name.clone());
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    if aliases.is_empty() {
        return (src.to_string(), aliases);
    }
    let is_alias = |w: &str| aliases.iter().any(|a| a == w);
    let mut out = String::with_capacity(src.len() + 256);
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        let indent = &line[..line.len() - t.len()];
        let words = t
            .strip_prefix("#define")
            .map(|r| r.split_whitespace().collect::<Vec<_>>())
            .unwrap_or_default();
        if words.len() == 2 && is_alias(words[0]) {
            let (name, body) = (words[0], words[1]);
            out.push_str(&format!(
                "{indent}#define {name}_SLANG_T {body}_SLANG_T\n\
                 {indent}#define {name}_SLANG_S {body}_SLANG_S\n"
            ));
            continue;
        }
        if let Some(name) = t.strip_prefix("#undef").map(str::trim) {
            if is_alias(name) {
                out.push_str(&format!(
                    "{indent}#undef {name}_SLANG_T\n{indent}#undef {name}_SLANG_S\n"
                ));
                continue;
            }
        }
        out.push_str(line);
    }
    (out, aliases)
}

/// Última palavra (identificador) no fim de `s`, ignorando espaço.
fn last_word(s: &str) -> &str {
    let t = s.trim_end();
    let start = t
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map_or(0, |i| i + 1);
    &t[start..]
}

/// Tipo básico GLSL (ou apelido `floatN` do `compat_macros`) — o que vem
/// antes do nome numa declaração de variável.
fn is_type_word(w: &str) -> bool {
    zero_value(w).is_some()
}

/// Valor zero de um tipo básico GLSL (ou dos apelidos `floatN` do
/// `compat_macros`); `None` pra struct/void/desconhecido.
fn zero_value(ty: &str) -> Option<String> {
    Some(match ty {
        "float" | "half" => "0.0".into(),
        "int" => "0".into(),
        "uint" => "0u".into(),
        "bool" => "false".into(),
        _ => {
            let last = ty.chars().last()?;
            let n_ok = matches!(last, '2' | '3' | '4');
            let base = &ty[..ty.len() - 1];
            match base {
                "vec" | "float" | "half" if n_ok => format!("{ty}(0.0)"),
                "ivec" | "int" if n_ok => format!("{ty}(0)"),
                "uvec" | "uint" if n_ok => format!("{ty}(0u)"),
                "bvec" | "bool" if n_ok => format!("{ty}(false)"),
                "mat" if n_ok => format!("{ty}(0.0)"),
                _ if ty.starts_with("mat") && ty.len() == 6 && ty.as_bytes()[4] == b'x' => {
                    format!("{ty}(0.0)")
                }
                _ => return None,
            }
        }
    })
}

/// Em toda função não-void de tipo básico cuja última instrução não é
/// `return`, põe `return <zero>;` antes do `}` final. Cobre o padrão
/// "todos os `return` dentro de `if/else if`" (ega, lottesRVM, crt-Cyclon,
/// patchy-ntsc): o glslang gera um fim de função inalcançável que o frontend
/// SPIR-V do naga rejeita (`ExpressionAlreadyInScope`). Chegar ao fim sem
/// `return` é indefinido em GLSL, então o zero não muda nada que funcionava.
/// Roda com os comentários já apagados (`blank_comments`).
fn ensure_trailing_returns(src: &str) -> String {
    const QUALIFIERS: &[&str] = &["inline", "highp", "mediump", "lowp", "precise", "const"];
    let b = src.as_bytes();
    let mut inserts: Vec<(usize, String)> = Vec::new();
    let mut depth = 0i32;
    let mut stmt_start = 0usize; // início do trecho atual no nível de arquivo
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'#' if depth == 0 => {
                // diretiva: pula a linha (e as continuações `\`)
                while i < b.len() && b[i] != b'\n' {
                    if b[i] == b'\\' && b.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    i += 1;
                }
                stmt_start = i;
            }
            b';' if depth == 0 => stmt_start = i + 1,
            b'{' => {
                if depth == 0 {
                    let header = src[stmt_start..i].trim();
                    let close = matching_brace(b, i);
                    if let (true, Some(close)) = (header.ends_with(')'), close) {
                        let before_paren = header.split('(').next().unwrap_or("");
                        let ty = before_paren
                            .split_whitespace()
                            .find(|w| !QUALIFIERS.contains(w))
                            .unwrap_or("");
                        let is_fn = before_paren.split_whitespace().count() >= 2;
                        if let (true, Some(zero)) = (is_fn, zero_value(ty)) {
                            if !ends_with_return(&src[i + 1..close]) {
                                inserts.push((close, format!(" return {zero}; ")));
                            }
                        }
                    }
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    stmt_start = i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let mut out = src.to_string();
    for (at, text) in inserts.into_iter().rev() {
        out.insert_str(at, &text);
    }
    out
}

/// A última instrução do corpo (ignorando diretivas `#`) é um `return`?
fn ends_with_return(body: &str) -> bool {
    let code: String = body
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let t = code.trim_end();
    if !t.ends_with(';') {
        return false; // termina em `}` de bloco (if/else, laço…)
    }
    let t = &t[..t.len() - 1];
    let last = t.rfind([';', '{', '}']).map_or(t, |p| &t[p + 1..]);
    last.trim_start().starts_with("return")
}

/// Troca SÓ o trecho `[qualificadores] sampler2D nome` de um parâmetro pelo
/// par texture+sampler, mantendo o resto do texto. O resto importa: o SMAA
/// põe parâmetro dentro de `#if SMAA_PREDICATION … #endif` no meio da lista,
/// e as linhas `#if`/`#endif` caem no texto de um parâmetro vizinho —
/// reconstruir o parâmetro do zero apagava as diretivas e o parâmetro
/// condicional virava obrigatório.
fn split_sampler_decl(raw: &str, pname: &str) -> String {
    let Some(pos) = find_word(raw, "sampler2D", 0) else {
        return raw.to_string();
    };
    // recua sobre qualificadores (`in`, `const`…) colados antes do tipo
    let mut start = pos;
    loop {
        let before = raw[..start].trim_end();
        let word_start = before
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map_or(0, |i| i + 1);
        if matches!(
            &before[word_start..],
            "in" | "const" | "highp" | "mediump" | "lowp"
        ) {
            start = word_start;
        } else {
            break;
        }
    }
    let Some(name_at) = find_word(raw, pname, pos + "sampler2D".len()) else {
        return raw.to_string();
    };
    let end = name_at + pname.len();
    format!(
        "{}texture2D {pname}_SLANG_T, sampler {pname}_SLANG_S{}",
        &raw[..start],
        &raw[end..]
    )
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

/// Macro função-like que repassa um sampler pra uma função que espera o par.
///
/// O Mega Bezel faz `#define COMPAT_TEXTURE(c,d) HSM_GetCroppedTexSample(c,d)`.
/// Depois que a assinatura de `HSM_GetCroppedTexSample` virou
/// `(texture2D, sampler, vec2)`, a macro passa a estar errada: ela repassa `c`
/// como UM argumento onde agora vão DOIS. Então a macro precisa do mesmo
/// tratamento que a função — parâmetro vira par, corpo repassa o par — e entra
/// no mapa `fns` pro call-site dela também expandir.
///
/// Roda em ponto fixo (macro pode chamar macro), no máximo 4 rodadas.
fn split_sampler_macros(src: &str, fns: &mut SamplerFns) -> String {
    let mut out = src.to_string();
    for _ in 0..4 {
        let mut changed = false;
        let mut next = String::with_capacity(out.len() + 128);
        for line in out.split_inclusive('\n') {
            match rewrite_macro_line(line, fns) {
                Some(new_line) => {
                    next.push_str(&new_line);
                    changed = true;
                }
                None => next.push_str(line),
            }
        }
        out = next;
        if !changed {
            break;
        }
    }
    out
}

/// `Some(linha nova)` se esta linha é um `#define` que repassa sampler.
fn rewrite_macro_line(line: &str, fns: &mut SamplerFns) -> Option<String> {
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let rest = trimmed.strip_prefix("#define")?;
    let head = rest.trim_start();
    let name: String = head
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() || fns.contains_key(&name) {
        return None; // já tratada numa rodada anterior
    }
    let after_name = &head[name.len()..];
    if !after_name.starts_with('(') {
        return None; // macro sem parâmetro
    }
    let close = matching_paren(after_name.as_bytes(), 0)?;
    let params: Vec<String> = split_args(&after_name[1..close])
        .into_iter()
        .map(|(a, z)| after_name[1..close][a..z].trim().to_string())
        .collect();
    let body = &after_name[close + 1..];

    // quais parâmetros da macro caem em posição de sampler no corpo?
    let mut sampler_params: Vec<usize> = Vec::new();
    let bb = body.as_bytes();
    let mut i = 0usize;
    while i < body.len() {
        let Some(open) = body[i..].find('(').map(|o| i + o) else {
            break;
        };
        let Some(cl) = matching_paren(bb, open) else {
            break;
        };
        if let Some((callee, _)) = ident_before(body, open) {
            if let Some(entry) = fns.get(&callee) {
                let inner = &body[open + 1..cl];
                for (idx, (a, z)) in split_args(inner).into_iter().enumerate() {
                    if !entry.positions.contains(&idx) {
                        continue;
                    }
                    let arg = inner[a..z].trim();
                    if let Some(pi) = params.iter().position(|p| p == arg) {
                        if !sampler_params.contains(&pi) {
                            sampler_params.push(pi);
                        }
                    }
                }
            }
        }
        i = open + 1;
    }
    if sampler_params.is_empty() {
        return None;
    }
    sampler_params.sort_unstable();

    // parâmetro sampler vira par, no cabeçalho e no corpo
    let new_params: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if sampler_params.contains(&i) {
                format!("{p}_SLANG_T, {p}_SLANG_S")
            } else {
                p.clone()
            }
        })
        .collect();
    let mut new_body = body.to_string();
    for i in &sampler_params {
        let p = &params[*i];
        new_body = replace_macro_arg(&new_body, p);
    }
    fns.insert(
        name.clone(),
        SamplerFn {
            positions: sampler_params,
            names: Vec::new(), // parâmetro de macro não é variável de escopo
        },
    );
    Some(format!(
        "{}#define {name}({}){new_body}",
        &line[..indent_len],
        new_params.join(", ")
    ))
}

/// Troca o parâmetro `p` da macro pelo par, como palavra inteira.
fn replace_macro_arg(body: &str, p: &str) -> String {
    let mut out = String::with_capacity(body.len() + 32);
    let mut i = 0usize;
    while let Some(off) = find_word(&body[i..], p, 0) {
        let at = i + off;
        out.push_str(&body[i..at]);
        out.push_str(p);
        out.push_str("_SLANG_T, ");
        out.push_str(p);
        out.push_str("_SLANG_S");
        i = at + p.len();
    }
    out.push_str(&body[i..]);
    out
}

/// Passo (c): reescreve cada uso de identificador de sampler na forma certa
/// pro contexto. `globals` valem no arquivo todo; os parâmetros de uma função
/// só valem dentro do corpo dela (daí o rastreio de `{}` e da função corrente).
///
/// Corpo de `#define` (inclusive as linhas continuadas com `\`) é caso à
/// parte: o crt-royale tem `#define VERTICAL_SINC_RESAMPLE_LOOP_BODY …tex…`
/// fora de qualquer função, onde `tex` é o parâmetro `sampler2D` da função em
/// que a macro é EXPANDIDA. Ali valem também `fn_sampler_params` (parâmetros
/// sampler de qualquer função), menos os parâmetros da própria macro.
fn rewrite_sampler_uses(
    src: &str,
    globals: &[String],
    fns: &SamplerFns,
    fn_sampler_params: &std::collections::HashSet<String>,
) -> String {
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
    let mut at_line_start = true;
    // `Some(params da macro)` enquanto dentro do corpo de um `#define`
    let mut define_params: Option<Vec<String>> = None;
    // nomes de sampler que uma variável LOCAL sombreia na função corrente
    // (crt-geom-deluxe: `vec3 delta[2][4]` com um sampler global `delta`)
    let mut shadowed: Vec<String> = Vec::new();

    while i < src.len() {
        let c = b[i];
        if c == b'\n' && define_params.is_some() {
            // termina a macro se a linha não acabou em `\` (continuação)
            let line_end = src[..i].trim_end_matches([' ', '\t', '\r']);
            if !line_end.ends_with('\\') {
                define_params = None;
            }
        }
        // linha de preprocessador. `#define NOME[(args)] corpo`: o NOME e a
        // lista de args são copiados crus (renomear o NOME do macro quebra a
        // diretiva — o koko-aio tem `#define FPS_ESTIMATE_PASS <sampler>` onde
        // o NOME é IGUAL a um sampler global). O CORPO segue pra reescrita
        // normal (o Mega Bezel tem `#define PassFeedback <sampler>` e usa o
        // alias no código). Outras diretivas (`#if`, `#pragma`…) vão cruas.
        if at_line_start && c == b'#' {
            let eol = src[i..].find('\n').map(|k| i + k + 1).unwrap_or(src.len());
            let line = &src[i..eol];
            let trimmed = line.trim_start_matches(|c: char| c == '#' || c.is_whitespace());
            if let Some(after) = trimmed.strip_prefix("define") {
                // copia `#define NOME` (+ `(args)` colado no nome, se houver)
                let name_start = i + (line.len() - after.len());
                let mut k = name_start;
                while k < eol && (b[k] as char).is_whitespace() {
                    k += 1;
                }
                while k < eol && is_word_byte(b[k]) {
                    k += 1;
                }
                let mut macro_params = Vec::new();
                if k < eol && b[k] == b'(' {
                    // lista de parâmetros do macro
                    let args_start = k + 1;
                    let mut depth = 0i32;
                    while k < eol {
                        match b[k] {
                            b'(' => depth += 1,
                            b')' => {
                                depth -= 1;
                                k += 1;
                                if depth == 0 {
                                    break;
                                }
                                continue;
                            }
                            _ => {}
                        }
                        k += 1;
                    }
                    macro_params = src[args_start..k.saturating_sub(1)]
                        .split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect();
                }
                define_params = Some(macro_params);
                out.push_str(&src[i..k]);
                i = k;
                at_line_start = false;
                continue;
            }
            out.push_str(line);
            i = eol;
            at_line_start = true;
            continue;
        }
        at_line_start = c == b'\n' || (at_line_start && (c as char).is_whitespace());
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

            let in_macro_body = cur_fn.is_none()
                && define_params
                    .as_ref()
                    .is_some_and(|ps| !ps.iter().any(|p| p == word));
            // declaração de variável com o nome de um sampler? (palavra
            // anterior é um tipo) → daqui até o fim da função é a local.
            if cur_fn.is_some()
                && !is_call
                && is_type_word(last_word(&out))
                && !shadowed.iter().any(|w| w == word)
            {
                shadowed.push(word.to_string());
            }
            let is_sampler = !is_call
                && !shadowed.iter().any(|w| w == word)
                && (globals.iter().any(|g| g == word)
                    || cur_fn
                        .as_ref()
                        .and_then(|f| fns.get(f))
                        .is_some_and(|e| e.names.iter().any(|n| n == word))
                    || (in_macro_body && fn_sampler_params.contains(word)));

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
                    shadowed.clear();
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

/// Troca cada CHAMADA `name(` (palavra inteira) por `new(`.
fn rename_calls(src: &str, name: &str, new: &str) -> String {
    let mut out = String::with_capacity(src.len() + 64);
    let mut from = 0;
    while let Some(pos) = find_word(src, name, from) {
        let after = src[pos + name.len()..].trim_start();
        out.push_str(&src[from..pos]);
        out.push_str(if after.starts_with('(') { new } else { name });
        from = pos + name.len();
    }
    out.push_str(&src[from..]);
    out
}

/// Builtins que o `naga` não leva até o WGSL: injeta um equivalente e troca
/// as chamadas (só quando o fonte usa — a maioria dos shaders não).
/// - `isinf`/`isnan`: o backend WGSL não tem (`UnsupportedRelationalFunction`;
///   o WGSL removeu esses builtins).
/// - `modf(x, out i)`: o glslang emite `OpExtInst Modf`, que o frontend SPIR-V
///   do naga vira `MathFunction::Modf` sem registrar o tipo-resultado
///   especial → `MissingSpecialType` na validação (gameboy, crt-1tap, xbr-lv3).
///   `trunc` + subtração dá o mesmo resultado (parte inteira truncada pra
///   zero, fração com o sinal de `x`).
fn patch_missing_builtins(glsl: &str) -> String {
    let has_inf = glsl.contains("isinf(");
    let has_nan = glsl.contains("isnan(");
    let has_modf = find_word(glsl, "modf", 0).is_some();
    if !has_inf && !has_nan && !has_modf {
        return glsl.to_string();
    }
    let mut out = glsl
        .replace("isinf(", "reemu_isinf(")
        .replace("isnan(", "reemu_isnan(");
    let mut inject = String::from(
        "\n        bool reemu_isinf(float x) { return x != 0.0 && x == x * 2.0; }\n        \
         bool reemu_isnan(float x) { return x != x; }\n",
    );
    if has_modf {
        out = rename_calls(&out, "modf", "reemu_modf");
        for t in ["float", "vec2", "vec3", "vec4"] {
            inject.push_str(&format!(
                "        {t} reemu_modf({t} x, out {t} i) {{ i = trunc(x); return x - i; }}\n"
            ));
        }
    }
    // depois do `#version`, senão vira erro de "código antes da diretiva".
    if let Some(pos) = out.find("#version") {
        let eol = out[pos..]
            .find('\n')
            .map(|i| pos + i + 1)
            .unwrap_or(out.len());
        out.insert_str(eol, &inject);
    } else {
        out.insert_str(0, &inject);
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

/// `REEMU_SHADER_DUMP=<pasta>`: quando um estágio falha, grava ali o GLSL
/// JÁ reescrito (`<estágio>-<n>.glsl`) — o número de linha do erro do
/// glslang/naga se refere a este texto, não ao `.slang` original.
fn dump_failed_stage(glsl: &str, label: &str) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    if let Some(dir) = std::env::var_os("REEMU_SHADER_DUMP") {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let _ = std::fs::write(dir.join(format!("{label}-{n}.glsl")), glsl);
    }
}

fn compile_stage(
    glsl: &str,
    stage: glslang::ShaderStage,
    label: &'static str,
) -> Result<(String, naga::Module), CompileError> {
    compile_stage_inner(glsl, stage, label).inspect_err(|_| dump_failed_stage(glsl, label))
}

fn compile_stage_inner(
    glsl: &str,
    stage: glslang::ShaderStage,
    label: &'static str,
) -> Result<(String, naga::Module), CompileError> {
    let spirv = glsl_to_spirv(glsl, stage, label)?;

    // `adjust_coordinate_space: false` — NÃO negar o Y de `gl_Position`.
    // O executor (`gpu.rs`) manda o MVP como ortho `[0,1]→[-1,1]` sem flip e o
    // quad com uv top-left = (0,0), casando com a convenção Y-up do WGSL. Isto
    // reproduz o que o antigo frontend `naga::front::glsl` fazia; com `true`
    // (default do naga) os shaders slang saíam de cabeça pra baixo.
    let module = naga::front::spv::Frontend::new(
        spirv.iter().copied(),
        &naga::front::spv::Options {
            adjust_coordinate_space: false,
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

    /// SMAA declara o parâmetro por macro de tipo, com uma variante HLSL
    /// num `#if` inativo — só os usos viram `sampler2D x`, os `#define` não.
    #[test]
    fn sampler_type_macro_parameter_compiles() {
        let s = r#"
#version 450
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vUV;
void main() { gl_Position = Position; vUV = TexCoord; }
#pragma stage fragment
#if defined(SMAA_HLSL_4)
#define SMAATexture2D(tex) Texture2D tex
#else
#define SMAATexture2D(tex) sampler2D tex
#endif
layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
#define SMAATexturePass2D(tex) tex
vec2 edges(vec2 uv, SMAATexture2D(colorTex)) { return texture(colorTex, uv).rg; }
vec2 outer(vec2 uv, SMAATexture2D(t)) { return edges(uv, SMAATexturePass2D(t)); }
void main() { FragColor = vec4(outer(vUV, SMAATexturePass2D(Source)), 0.0, 1.0); }
"#;
        let out = compile(&preprocess_str(s)).expect("parâmetro via macro de tipo");
        assert_eq!(out.textures.len(), 1);
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    /// Parâmetro de sampler dentro de `#if` no meio da assinatura (SMAA):
    /// as diretivas precisam sobreviver à reescrita da assinatura.
    #[test]
    fn conditional_sampler_parameter_keeps_its_if_block() {
        let s = r#"
#version 450
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vUV;
void main() { gl_Position = Position; vUV = TexCoord; }
#pragma stage fragment
#define PRED 0
layout(location = 0) in vec2 vUV;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
vec4 edges(vec2 uv,
           sampler2D colorTex
           #if PRED
           , sampler2D predTex
           #endif
           ) { return texture(colorTex, uv); }
void main() { FragColor = edges(vUV, Source); }
"#;
        let out = compile(&preprocess_str(s)).expect("parâmetro condicional");
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    /// crt-royale: macro de várias linhas, fora de função, usando o
    /// parâmetro sampler da função onde ela é expandida.
    #[test]
    fn multiline_macro_using_function_sampler_param_compiles() {
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
#define LOOP_BODY \
    acc += texture(tex, uv + vec2(float(i) * 0.01, 0.0)).rgb
#define SCALE(tex) (tex * 0.5)
vec3 resample(sampler2D tex, vec2 uv) {
    vec3 acc = vec3(0.0);
    for (int i = 0; i < 4; i++) { LOOP_BODY; }
    return SCALE(acc);
}
void main() { FragColor = vec4(resample(Source, vUV), 1.0); }
"#;
        let out = compile(&preprocess_str(s)).expect("macro com sampler de função");
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    /// crt-royale: `#define input_texture Source` passado pra função do
    /// usuário que espera sampler.
    #[test]
    fn sampler_alias_macro_passed_to_user_function_compiles() {
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
layout(set = 0, binding = 3) uniform sampler2D MASK;
#define input_texture Source
#define USE_SOURCE 1
#if USE_SOURCE
    #define mask_tex Source
#else
    #define mask_tex MASK
#endif
vec4 lin(sampler2D tex, vec2 uv) { return texture(tex, uv); }
void main() {
    FragColor = lin(input_texture, vUV) + texture(input_texture, vUV) + lin(mask_tex, vUV);
}
"#;
        let out = compile(&preprocess_str(s)).expect("apelido de sampler");
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    /// `modf` virava `MissingSpecialType` na validação do naga.
    #[test]
    fn modf_compiles_via_trunc_helper() {
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
void main() {
    vec2 whole;
    const vec2 part = modf(vUV * 8.0, whole);
    float w1;
    float p1 = modf(vUV.x, w1);
    FragColor = vec4(part, whole.x + p1 + w1, 1.0);
}
"#;
        let out = compile(&preprocess_str(s)).expect("modf deve compilar");
        assert!(out.fragment_wgsl.contains("trunc"));
    }

    #[test]
    fn alias_rules_skip_mixed_definitions_and_param_names() {
        let globals = vec!["Source".to_string()];
        let params: std::collections::HashSet<String> = ["tex".to_string()].into();
        let src = "#define a Source\n\
                   #if X\n#define mixed Source\n#else\n#define mixed 1.0\n#endif\n\
                   #define tex Source\n\
                   #define chained a\n";
        let (out, aliases) = split_sampler_aliases(src, &globals, &params);
        assert_eq!(aliases, vec!["a".to_string(), "chained".to_string()]);
        assert!(out.contains("#define chained_SLANG_T a_SLANG_T"), "{out}");
        assert!(out.contains("#define a_SLANG_T Source_SLANG_T"));
        assert!(out.contains("#define mixed Source"), "misto fica intacto");
        assert!(
            out.contains("#define tex Source"),
            "nome de parâmetro fica intacto"
        );
    }

    /// Função não-void sem `return` final (todos os `return` dentro de
    /// `if/else`): o naga rejeitava com `ExpressionAlreadyInScope` (ega,
    /// lottesRVM, crt-Cyclon, patchy-ntsc).
    #[test]
    fn function_without_trailing_return_compiles() {
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
vec3 slot(vec2 pos) {
    float h = fract(pos.x);
    float odd;
    if (fract(pos.y) < 0.5) odd = 0.0; else odd = 1.0;
    if (odd == 0.0)
        {if (h < 0.5) return vec3(0.5); else return vec3(1.5);}
    else if (odd == 1.0)
        {if (h < 0.5) return vec3(1.5); else return vec3(0.5);}
}
float pick(float x) {
    if (x < 0.5) return 1.0;
    else if (x < 0.8) return 2.0;
}
void main() { FragColor = vec4(slot(vUV * 100.0) * pick(vUV.x), 1.0); }
"#;
        let out = compile(&preprocess_str(s)).expect("sem return final");
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    #[test]
    fn trailing_return_only_added_where_missing() {
        let src = "float a(float x) { return x; }\n\
                   vec3 b(float x) { if (x > 0.0) return vec3(1.0); }\n\
                   void c() { }\n\
                   S d() { if (true) return S(1); }\n\
                   inline float4 e(float x) {\n#if X\n return float4(x);\n#endif\n}\n";
        let out = ensure_trailing_returns(src);
        assert!(out.contains("float a(float x) { return x; }"), "{out}");
        assert!(
            out.contains("return vec3(1.0);  return vec3(0.0); }"),
            "{out}"
        );
        assert!(out.contains("void c() { }"), "{out}");
        assert!(
            out.contains("S d() { if (true) return S(1); }"),
            "struct fica: {out}"
        );
        assert!(
            !out.contains("float4(0.0)"),
            "return dentro de #if conta: {out}"
        );
    }

    #[test]
    fn integer_varyings_get_flat() {
        let vert = "layout(location = 0) in int attr;\n\
                    layout(location = 2) out int curfield;\n\
                    layout(location = 4) out uvec2 u;\n\
                    layout(location = 5) out float w;\n";
        let out = flat_integer_varyings(vert, Stage::Vertex);
        assert!(
            out.contains("layout(location = 0) in int attr;"),
            "atributo: {out}"
        );
        assert!(
            out.contains("layout(location = 2) flat out int curfield;"),
            "{out}"
        );
        assert!(
            out.contains("layout(location = 4) flat out uvec2 u;"),
            "{out}"
        );
        assert!(out.contains("layout(location = 5) out float w;"), "{out}");
        let frag = "layout(location = 3) in int scanlines;\n\
                    layout(location = 0) out uvec4 FragColor;\n";
        let out = flat_integer_varyings(frag, Stage::Fragment);
        assert!(
            out.contains("layout(location = 3) flat in int scanlines;"),
            "{out}"
        );
        assert!(
            out.contains("layout(location = 0) out uvec4 FragColor;"),
            "alvo: {out}"
        );
    }

    /// Variável local com o mesmo nome de um sampler global (crt-geom-deluxe).
    #[test]
    fn local_variable_shadowing_a_sampler_compiles() {
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
layout(set = 0, binding = 2) uniform sampler2D delta;
vec3 pick(int i) {
    vec3 delta[2] = { vec3(1.0), vec3(0.5) };
    return delta[i];
}
void main() { FragColor = vec4(pick(1) * texture(delta, vUV).rgb, 1.0); }
"#;
        let out = compile(&preprocess_str(s)).expect("local sombreando sampler");
        assert_eq!(out.textures.len(), 1);
    }

    #[test]
    fn split_sampler_decl_keeps_surrounding_text() {
        assert_eq!(
            split_sampler_decl("\n  in sampler2D t\n  #if X\n", "t"),
            "\n  texture2D t_SLANG_T, sampler t_SLANG_S\n  #if X\n"
        );
    }

    #[test]
    fn sampler_type_macro_only_matches_the_exact_shape() {
        let kind = |l: &str| one_param_macro(l).map(|(_, b)| b);
        assert_eq!(
            one_param_macro("#define SMAATexture2D(tex) sampler2D tex"),
            Some(("SMAATexture2D".into(), "sampler2D $".into()))
        );
        assert_eq!(
            kind("#define T(tex) Texture2D tex"),
            Some("Texture2D $".into())
        );
        assert_eq!(kind("#define T(a, b) sampler2D a"), None);
        assert_eq!(
            kind("#define T(tex) sampler2D other"),
            Some("sampler2D other".into())
        );
        assert_eq!(
            one_param_macro("#define SMAATexturePass2D(tex) tex"),
            Some(("SMAATexturePass2D".into(), "$".into()))
        );
    }

    #[test]
    fn splits_signature_and_picks_the_right_use_form() {
        let glsl = "layout(set=0,binding=2) uniform sampler2D Source;\n\
                    vec4 tap(sampler2D t, vec2 uv) { return texture(t, uv); }\n\
                    void main() { FragColor = tap(Source, vUV) + texture(Source, vUV); }\n";
        let (out, binds) = rewrite(glsl, Stage::Fragment);
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
        let (out, _) = rewrite(glsl, Stage::Fragment);
        assert!(out.contains("params.SourceSize.x"), "{out}");
        assert!(!out.contains("SourceSize_SLANG"), "{out}");
    }

    /// WGSL não aceita struct atravessando estágio (`NotIOShareableType`).
    /// O scanline-classic faz isso; achatamos as `location`s e copiamos nas
    /// pontas do `main`, deixando o corpo intacto.
    #[test]
    fn struct_varying_is_flattened() {
        let s = r#"
#version 450
struct TB { float freq; vec2 phase; };
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vUV;
layout(location = 1) out TB tb;
void main() { gl_Position = Position; vUV = TexCoord; tb.freq = 1.0; tb.phase = vec2(0.5); }
#pragma stage fragment
layout(location = 0) in vec2 vUV;
layout(location = 1) in TB tb;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
void main() { FragColor = texture(Source, vUV) * tb.freq * tb.phase.x; }
"#;
        let out = compile(&preprocess_str(s)).expect("struct varying deve compilar");
        assert!(out.vertex_wgsl.contains("@vertex"));
        assert!(out.fragment_wgsl.contains("@fragment"));
    }

    /// Mesmo caso, com array (koko-aio). O array continua sendo uma variável
    /// de verdade — indexação dinâmica no corpo segue funcionando.
    #[test]
    fn array_varying_is_flattened_and_stays_indexable() {
        let s = r#"
#version 450
#define N 4
#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vUV;
layout(location = 1) flat out float w[N];
void main() {
    gl_Position = Position; vUV = TexCoord;
    for (int i = 0; i < N; i++) w[i] = float(i);
}
#pragma stage fragment
layout(location = 0) in vec2 vUV;
layout(location = 1) flat in float w[N];
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;
void main() {
    float acc = 0.0;
    for (int i = 0; i < N; i++) acc += w[i];
    FragColor = texture(Source, vUV) * acc;
}
"#;
        let out = compile(&preprocess_str(s)).expect("array varying deve compilar");
        assert!(out.vertex_wgsl.contains("@vertex"));
        assert!(out.fragment_wgsl.contains("@fragment"));
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
