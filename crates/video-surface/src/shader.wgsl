// Desenha a textura do frame do core como um quad centralizado, com
// letterbox (barras pretas) pra respeitar o aspect ratio.

struct Uniforms {
    // Fração do viewport ocupada pela imagem (<= 1 em cada eixo).
    scale: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var frame_tex: texture_2d<f32>;
@group(0) @binding(2) var frame_samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) idx: u32) -> VsOut {
    // Triângulo que cobre a tela toda.
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = verts[idx];
    var out: VsOut;
    out.clip = vec4<f32>(p, 0.0, 1.0);
    out.ndc = p;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // ndc in [-1,1]. Divide pela fração ocupada -> [-1,1] nas bordas da imagem.
    let img = in.ndc / u.scale;
    var uv = img * 0.5 + vec2<f32>(0.5, 0.5);
    uv.y = 1.0 - uv.y;

    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); // letterbox
    }
    return textureSample(frame_tex, frame_samp, uv);
}
