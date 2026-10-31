//! Presets embutidos (`plain`, `crt`, `lcd`) e os de 1 clique do pacote de shaders.

pub(super) const BUILTIN_VS: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn main(@location(0) position: vec4<f32>, @location(1) texcoord: vec2<f32>) -> VOut {
    return VOut(vec4<f32>(position.xy * 2.0 - vec2<f32>(1.0, 1.0), 0.0, 1.0), texcoord);
}
"#;

pub(super) const BUILTIN_FS_PRELUDE: &str = r#"
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
struct Uniforms {
    source_size: vec4<f32>,
    output_size: vec4<f32>,
    orig_size: vec4<f32>,
    frame: vec4<f32>,
};
@group(0) @binding(0) var<uniform> U: Uniforms;
@group(0) @binding(2) var Source: texture_2d<f32>;
@group(0) @binding(3) var Samp: sampler;
"#;

pub(super) struct BuiltinPass {
    pub(super) scale: f32,
    pub(super) linear: bool,
    pub(super) fs: &'static str,
}
pub(super) struct Builtin {
    pub(super) name: &'static str,
    pub(super) passes: &'static [BuiltinPass],
}

pub(super) const PLAIN: Builtin = Builtin {
    name: "plain",
    passes: &[BuiltinPass {
        scale: 1.0,
        linear: false,
        fs: "@fragment fn main(v: VOut) -> @location(0) vec4<f32> { return textureSample(Source, Samp, v.uv); }",
    }],
};
pub(super) const CRT: Builtin = Builtin {
    name: "crt",
    passes: &[
        BuiltinPass {
            scale: 1.0,
            linear: false,
            fs: r#"
@fragment fn main(v: VOut) -> @location(0) vec4<f32> {
    let dx = U.source_size.z;
    var c = textureSample(Source, Samp, v.uv).rgb * 0.5;
    c += textureSample(Source, Samp, v.uv + vec2<f32>(dx, 0.0)).rgb * 0.25;
    c += textureSample(Source, Samp, v.uv - vec2<f32>(dx, 0.0)).rgb * 0.25;
    return vec4<f32>(c, 1.0);
}"#,
        },
        BuiltinPass {
            scale: 3.0,
            linear: true,
            fs: r#"
@fragment fn main(v: VOut) -> @location(0) vec4<f32> {
    var col = textureSample(Source, Samp, v.uv).rgb;
    col = pow(col, vec3<f32>(1.15));
    let line = fract(v.uv.y * U.source_size.y);
    col *= 1.0 - 0.30 * pow(sin(line * 3.14159265), 2.0);
    let m = i32(v.uv.x * U.output_size.x) % 3;
    var mask = vec3<f32>(0.94, 0.94, 0.94);
    if (m == 0) { mask.r = 1.06; } else if (m == 1) { mask.g = 1.06; } else { mask.b = 1.06; }
    col *= mask;
    let d = v.uv - vec2<f32>(0.5, 0.5);
    col *= 1.0 - dot(d, d) * 0.35;
    col *= 1.25;
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}"#,
        },
    ],
};
pub(super) const LCD: Builtin = Builtin {
    name: "lcd",
    passes: &[BuiltinPass {
        scale: 2.0,
        linear: false,
        fs: r#"
@fragment fn main(v: VOut) -> @location(0) vec4<f32> {
    var col = textureSample(Source, Samp, v.uv).rgb;
    let g = fract(v.uv * U.source_size.xy);
    let grid = smoothstep(0.0, 0.12, g.x) * smoothstep(0.0, 0.12, g.y);
    col *= mix(0.88, 1.0, grid);
    col = pow(col, vec3<f32>(0.95));
    return vec4<f32>(col * 1.05, 1.0);
}"#,
    }],
};
pub(super) const BUILTINS: &[&Builtin] = &[&PLAIN, &CRT, &LCD];

pub fn builtin_preset_names() -> Vec<String> {
    BUILTINS.iter().map(|p| p.name.to_string()).collect()
}

/// Preset "de 1 clique" que aponta pra um `.slangp` do pacote `slang-shaders`.
/// A UI mostra estes como opções fixas; ficam indisponíveis se o pacote não
/// foi baixado. Wire id = `curated:<id>` — resolvido por [`build_specs`].
pub struct Curated {
    pub id: &'static str,
    pub label: &'static str,
    pub desc: &'static str,
    /// Caminho relativo à raiz do `slang-shaders` (ou, com `bundled`, à
    /// pasta dos shaders próprios — `bundled_shaders`).
    pub relpath: &'static str,
    /// Shader próprio do ReEmu, embutido no app: não depende do pacote.
    pub bundled: bool,
}

/// Só entram aqui presets confirmados no `docs/shaders/working-presets.txt`.
pub const CURATED: &[Curated] = &[
    Curated {
        id: "xbr",
        label: "Suavizar pixel art (xBR)",
        desc: "Deixa o 2D liso sem borrar — ideal pra 8/16-bit.",
        relpath: "edge-smoothing/xbr/other presets/xbr-lv2-standalone.slangp",
        bundled: false,
    },
    Curated {
        id: "scalefx",
        label: "Suavizar pixel art (ScaleFX)",
        desc: "Alternativa ao xBR; segura melhor os detalhes finos.",
        relpath: "edge-smoothing/scalefx/scalefx.slangp",
        bundled: false,
    },
    Curated {
        id: "super-xbr",
        label: "Suavizar 2D e 3D (Super-xBR)",
        desc: "Também serve pra consoles 3D (PS1/N64).",
        relpath: "edge-smoothing/xbr/super-xbr.slangp",
        bundled: false,
    },
    Curated {
        id: "ntsc",
        label: "Cor de TV antiga (NTSC)",
        desc: "Sangramento de vídeo composto — a cara certa de NES/Mega Drive.",
        relpath: "ntsc/ntsc-adaptive.slangp",
        bundled: false,
    },
    Curated {
        id: "crt-guest",
        label: "CRT avançado (Guest)",
        desc: "Tubo completo: máscara de fósforo, brilho e geometria.",
        relpath: "crt/crt-guest-advanced.slangp",
        bundled: false,
    },
    // Ampliação/nitidez espaciais (sem histórico de quadros — servem pra
    // emulação). FSR 1 = EASU (amplia pra resolução da tela) + RCAS
    // (nitidez), nessa ordem, conforme a doc da AMD
    // (gpuopen.com/fidelityfx-superresolution). Os três rodam na GPU no teste
    // `field_render_upscalers`.
    Curated {
        id: "fsr",
        label: "Ampliar com nitidez (AMD FSR)",
        desc: "Amplia e realça as bordas — bom pro 3D de PS1, N64 e PSP.",
        relpath: "edge-smoothing/fsr/fsr.slangp",
        bundled: false,
    },
    Curated {
        id: "nis",
        label: "Ampliar com nitidez (NVIDIA NIS)",
        desc: "Alternativa ao FSR, com nitidez um pouco mais forte.",
        relpath: "edge-smoothing/nis/nis.slangp",
        bundled: false,
    },
    Curated {
        id: "rcas",
        label: "Só nitidez (RCAS)",
        desc: "Realça as bordas sem ampliar — deixa o 3D menos borrado.",
        relpath: "sharpen/rca_sharpen.slangp",
        bundled: false,
    },
    // AMD FidelityFX CAS (MIT), portado da fonte oficial
    // (GPUOpen-Effects/FidelityFX-CAS, `ffx_cas.h`) — o pacote da libretro não
    // tem. Só nitidez: a ampliação do CAS vai até 4x de área
    // (`CAS_AREA_LIMIT`), abaixo do que a emulação pede.
    Curated {
        id: "cas",
        label: "Nitidez adaptativa (AMD CAS)",
        desc: "Realça detalhes conforme o contraste local, sem ampliar.",
        relpath: "cas/cas.slangp",
        bundled: true,
    },
];

pub fn curated_by_wire(wire: &str) -> Option<&'static Curated> {
    let id = wire.strip_prefix("curated:")?;
    CURATED.iter().find(|c| c.id == id)
}
