#version 450

/*
    AMD FidelityFX Contrast Adaptive Sharpening (CAS) 1.20190610, sharpen-only
    path, ported to libretro slang for ReEmu.

    Ported from the official source:
    https://github.com/GPUOpen-Effects/FidelityFX-CAS/blob/master/ffx-cas/ffx_cas.h

    Copyright (c) 2017-2019 Advanced Micro Devices, Inc. All rights reserved.

    Permission is hereby granted, free of charge, to any person obtaining a copy
    of this software and associated documentation files (the "Software"), to
    deal in the Software without restriction, including without limitation the
    rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
    sell copies of the Software, and to permit persons to whom the Software is
    furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice shall be included in
    all copies or substantial portions of the Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.

    Port notes (what follows ffx_cas.h and why):
    - CasFilter() with noScaling = true: CAS scaling is limited to 4x area
      (CAS_AREA_LIMIT), far below emulator upscales, so this pass sharpens at
      the source size and the frontend scales afterwards.
    - Default build of the filter: CAS_BETTER_DIAGONALS, CAS_SLOW and
      CAS_GO_SLOWER undefined -> 5-tap "circle" (b, d, e, f, h), green-channel
      weights for all channels. Exact rcp/sqrt instead of the APrx* hardware
      approximations (same math, GLSL has no portable equivalent).
    - "This filter does not function well on sRGB or gamma 2.2 non-linear
      data": the input is UNORM sRGB, so the documented "UNORM with linear
      conversion approximation" is used - CasInput() squares the inputs
      (gamma 2.0) and the output goes through sqrt().
    - CasSetup(): peak = -1 / lerp(8, 5, saturate(sharpness)); "0 := default
      (lower ringing), 1 := maximum (highest ringing)".
    - Guard: an all-black neighbourhood gives max = 0; the filter divides by
      it, so the amount is forced to 0 there (no sharpening on black).
*/

#pragma parameter CAS_SHARPNESS "CAS Sharpness (0 = AMD default)" 0.0 0.0 1.0 0.05

layout(push_constant) uniform Push {
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
    float CAS_SHARPNESS;
}
params;

layout(std140, set = 0, binding = 0) uniform UBO {
    mat4 MVP;
}
global;

#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;

void main() {
    gl_Position = global.MVP * Position;
    vTexCoord = TexCoord;
}

#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 1) uniform sampler2D Source;

// CasLoad() + CasInput(): integer texel fetch, clamped to the image, then
// gamma 2.0 -> linear (square).
vec3 cas_load(ivec2 p, ivec2 size_max) {
    vec3 c = texelFetch(Source, clamp(p, ivec2(0), size_max), 0).rgb;
    return c * c;
}

void main() {
    ivec2 size_max = ivec2(params.SourceSize.xy) - ivec2(1);
    ivec2 sp = ivec2(vTexCoord * params.SourceSize.xy);

    //   b
    // d e f
    //   h
    vec3 b = cas_load(sp + ivec2(0, -1), size_max);
    vec3 d = cas_load(sp + ivec2(-1, 0), size_max);
    vec3 e = cas_load(sp, size_max);
    vec3 f = cas_load(sp + ivec2(1, 0), size_max);
    vec3 h = cas_load(sp + ivec2(0, 1), size_max);

    // Soft min and max (the 5-tap circle).
    vec3 mn = min(min(min(d, e), min(f, b)), h);
    vec3 mx = max(max(max(d, e), max(f, b)), h);

    // Smooth minimum distance to signal limit divided by smooth max, then
    // shaped with sqrt().
    vec3 amp = clamp(min(mn, vec3(1.0) - mx) / max(mx, vec3(1e-6)), 0.0, 1.0);
    amp = mix(vec3(0.0), sqrt(amp), greaterThan(mx, vec3(0.0)));

    // Filter shape (green coefficient for all channels):
    //  0 w 0
    //  w 1 w
    //  0 w 0
    float peak = -1.0 / mix(8.0, 5.0, clamp(params.CAS_SHARPNESS, 0.0, 1.0));
    float w = amp.g * peak;
    float rcp_weight = 1.0 / (1.0 + 4.0 * w);
    vec3 pix = clamp((b * w + d * w + f * w + h * w + e) * rcp_weight, 0.0, 1.0);

    // Linear -> gamma 2.0 before store.
    FragColor = vec4(sqrt(pix), 1.0);
}
