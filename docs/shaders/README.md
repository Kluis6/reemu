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

**Última medição (2026-09-19): 2547/2554 = 99,7%** (era 92,7% em 2026-09-06).

Por pacote: tudo 100%, menos `bezel/koko-aio` (173/179) — e os 6 que faltam
lá não são preset: são blocos de parâmetros em `refs/` (sem a chave
`shaders`), feitos pra serem referenciados por outro preset. Descontando
esses, a única falha real é `test/format.slangp`.

## O que ainda falha

- **`test/format.slangp`** — o 2º passe (`decode-format.slang`) lê um
  `usampler2D` (textura de inteiros) com `texelFetch` + `unpackUnorm4x8`.
  Não é caso de compilador: o executor (`gpu.rs`) só trabalha com alvo de
  render float, então suportar isso é feature de pipeline, não reescrita de
  GLSL. É um preset de TESTE da própria libretro.
- Os 6 `.slangp` de `bezel/koko-aio/**/refs/` não têm `shaders` (são
  fragmentos de parâmetro) — recusá-los está certo.

## Como o long-tail (~7%) foi fechado (2026-09-19)

Cada item virou um passo da reescrita em `crates/shader-slang`, com teste
unitário em `compile.rs`/`preprocess.rs`:

- **Macro de tipo escondendo o sampler** (`#define SMAATexture2D(tex)
  sampler2D tex`) e **macro de repasse** (`#define SMAATexturePass2D(tex)
  tex`) — expandidas nos usos antes da divisão de parâmetros (smaa).
- **Parâmetro sampler dentro de `#if`** no meio da assinatura: a reescrita
  preservava só o nome e apagava as diretivas, tornando obrigatório um
  parâmetro condicional (smaa).
- **Macro multilinha usando o parâmetro sampler da função onde é expandida**
  (`VERTICAL_SINC_RESAMPLE_LOOP_BODY`, crt-royale).
- **Apelido de sampler** (`#define input_texture Source`), inclusive em
  cadeia (`iChannelCurr`→`iChannel0`→`Source`, metacrt) e condicional
  (`MASK_RESIZEtexture`, crt-royale): vira um par de defines `_SLANG_T`/
  `_SLANG_S` no mesmo ramo `#if`.
- **Guard de `#include` por estágio** quando o `#pragma stage` está num
  arquivo INCLUÍDO, não no raiz — o `screen-helper.h` sumia do fragment
  (crt-yah: `get_orientation`/`normalized_sigmoid` indefinidas).
- **`modf`** → helper com `trunc` (o frontend SPIR-V do naga não registra o
  tipo-resultado especial: `MissingSpecialType`) — gameboy, crt-1tap,
  xbr-lv3.
- **Função não-void sem `return` final** → `return <zero>;` no fim (o naga
  rejeita com `ExpressionAlreadyInScope`) — ega, lottesRVM, crt-Cyclon,
  patchy-ntsc.
- **Varying inteiro sem `flat`** (ntsc-blastem) — só a saída do vertex e a
  entrada do fragment, nunca atributo ou alvo de render.
- **Variável local com o nome de um sampler global** (crt-geom-deluxe).

Os helpers antes listados como não-resolvidos (`tex2Dblur9fast`, `tsample`,
`blur`, `get_orientation`, `normalized_sigmoid`) eram sintoma dos dois
primeiros itens — sumiram junto.

`REEMU_SHADER_DUMP=<pasta>` grava o GLSL já reescrito do estágio que falhou
— o número de linha do erro do glslang/naga se refere a ESSE texto, não ao
`.slang` original.

- ~~`mipmap_input` / `mipmap` de LUT: parseado, executor não gera a cadeia de
  mips~~ **feito (2026-09-18)** — ver `docs/ai-context/04`.
