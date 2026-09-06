# Compatibilidade de shaders `.slangp`

`working-presets.txt` — os `.slangp` do `libretro/slang-shaders` que passam
pelo compilador do ReEmu (`build_specs` → glslang → SPIR-V → naga → WGSL).
Um por linha, caminho relativo à raiz do repo `slang-shaders`.

**Gerado por** `gpu.rs::tests::field_validate_real_presets` (`#[ignore]`):

```
REEMU_SHADER_DIR=~/.local/share/com.reemu.desktop/shaders/slang-shaders \
REEMU_SHADER_OK_LIST=docs/shaders/working-presets.txt \
cargo test -p reemu-desktop --lib field_validate_real_presets -- --ignored --nocapture
```

**Última medição (2026-09-06): 2368/2554 = 92,7%.**

Por pacote: scanline-classic 100%, Mega_Bezel 97%, koko-aio 97%, hdr/blurs/
dithering/pal/interpolation/downsample/deinterlacing/scanlines 100%.
Mais fracos: `presets/` 71%, `crt/` 71% (família crt-royale), anti-aliasing 67%.

## Long-tail que ainda falha (~7%)

- **crt-royale** (`crt-royale-mask-resize-vertical` / `-first-pass-linearize`):
  `#define tex <sampler>` usado no vertex, sampler só declarado no fragment.
- **gameboy / authentic_gbc / xbr multipass**: `validação naga` —
  função/expressão que o backend WGSL não digere.
- **smaa**: `sampler constructor must appear at point of use` num caso que a
  reescrita ainda não cobre.
- helpers não-resolvidos (`tex2Dblur9fast`, `tsample`, `blur`,
  `get_orientation`, `normalized_sigmoid`) — macro/forward-ref.
- `mipmap_input` / `mipmap` de LUT: parseado, executor não gera a cadeia de
  mips (nenhum preset *falha* por isso — degrada silenciosamente).
