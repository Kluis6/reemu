# 04 — Shader Chain + Decoração (Mega Bezel / Bezels)

## Objetivo desta etapa

Implementar `ShaderChainResolver` e `DecorationResolver` (portas já
definidas em `domain`), com suporte ao formato **slang** e ao importador
de compatibilidade de pacotes estilo RetroBat/The Bezel Project.

## Decisões relevantes

- Formato suportado: **slang** apenas (compilar pra SPIR-V via
  `shaderc`/`naga`). ReShade FX é backlog — não implemente.
- Resolução em cascata: rom → sistema → default (mesma lógica de
  `01-domain-db.md`, reaproveite a função de resolução genérica se ela já
  existir).
- **Exclusão mútua Mega Bezel vs decoração externa por padrão**: antes de
  resolver `DecorationAssignment`, verifique se o `ShaderPreset` ativo tem
  `includes_bezel = true` — se sim, pule a resolução de decoração
  inteiramente. Isso deve ser configurável (toggle avançado pra permitir
  empilhar), mas o padrão é sempre pular.
- O importador de pacotes (`DecorationPackImporter`) precisa reconhecer a
  convenção de pastas do RetroBat: arquivos organizados por
  `<pack>/games/<sistema>/<rom>.png` (específico de rom),
  `<pack>/<sistema>/<sistema>.png` (específico de sistema), e
  `<pack>/default.png` (fallback) — populando `decoration_assignments`
  automaticamente a partir disso, não em runtime a cada resolução.

## Download automático de assets

- **Slang shaders** (`apps/desktop/src-tauri/src/shader_pack.rs`): baixa o
  `shaders_slang.zip` do buildbot da libretro — a mesma fonte do "Online
  Updater → Update Slang Shaders" do RetroArch. GitHub (`codeload`) é
  fallback. A extração detecta sozinha (`zip_common_root`) se o zip embrulha
  tudo num dir raiz.
- **Bezels por sistema** (`apps/desktop/src-tauri/src/bezel_pack.rs`): baixa
  um repo `bezelproject-<X>` do **The Bezel Project** (família overlay do
  RetroArch) sob demanda, extrai só `retroarch/overlay/**` remapeado pra
  `<dados>/decorations/bezelproject/<system_id>/` e re-importa a árvore
  inteira via `decoration::import_pack` (modelo "um pack ativo" — todos os
  sistemas convivem sob a mesma raiz). Catálogo `system_id → repo` no
  módulo; `bezel_catalog` / `download_bezel_pack` são os comandos. UI em
  `components/BezelLibrary.tsx` (Config › Vídeo), só sistemas presentes na
  biblioteca. Casamento bezel↔ROM continua por nome de arquivo
  (No-Intro/Redump).
- **Mega Bezel** (HyperspaceMadness): ainda manual (`.slangp` externo). O
  pack completo + GRAPHICS packages vêm de releases próprios, fora do
  `libretro/slang-shaders` — automação é backlog.

## Presets curados ("de 1 clique")

`gpu.rs::CURATED` — lista fixa de `{id, label, desc, relpath}` apontando pra
`.slangp` **confirmados** em `docs/shaders/working-presets.txt` (xBR, ScaleFX,
Super-xBR, NTSC adaptativo, crt-guest-advanced). Wire id = `curated:<id>`;
`build_specs` resolve via `SHADER_ROOT` (setado no startup pra
`<dados>/shaders/slang-shaders`) e erra com dica se o pacote não foi baixado.
`get_shader_info` devolve `curated[]` com `available` por preset; a UI
(`SettingsVideo`, `RomDetail`) mostra como opções fixas ao lado dos builtins.
`get_shader_info.active` agora é `preset_source()` (não mais `preset_name()`),
então o realce bate pra builtin, `curated:<id>` e caminho `.slangp`.

## Pendências de qualidade de imagem

- **Mipmaps** (`gpu.rs` TODOs em `PassSpec`/`LutSpec`): `mipmap_input` e
  `mipmap` de LUT são parseados mas o executor não gera a cadeia — passes de
  bloom/glow/halation (Mega Bezel, crt-royale) amostram só o nível 0 e ficam
  "chapados". É a maior lacuna visual hoje.
- ~7% dos presets não compilam (crt-royale mask-resize, alguns GB, smaa,
  xbr multipass) — padrões de GLSL que a reescrita do `compile.rs` não cobre.

## Pipeline de composição (ordem fixa)

```
1. FrameSource        -> textura crua do core
2. ShaderChain          -> N passes em sequência (slang)
3. DecorationResolver     -> compõe dentro do bezel (se não pulado)
4. OverlayCompositor        -> menu Fluent por cima (sempre)
```

Cada estágio consome só a textura do anterior — não acople o código de um
estágio ao conhecimento de como o anterior foi produzido.

## Isolamento do compilador slang

Trate o parser/compilador de `.slangp` como um módulo isolado
(`shader-chain-slang-adapter` ou equivalente) — não espalhe lógica de
parsing de shader de terceiros pelo resto do código. Isso é
especificamente porque é código compilando conteúdo não confiável vindo
de fora do projeto.

## Depende de

`03-tauri-desktop-shell.md` (precisa da surface nativa e do
`FrameSource` já funcionando).

## Critério de pronto

- Um preset Mega Bezel conhecido carrega e renderiza corretamente sobre
  um core software-only
- Importar um pacote de decoração no formato RetroBat popula
  `decoration_assignments` automaticamente, sem edição manual do banco
- Ativar um preset com `includes_bezel = true` visivelmente não desenha
  decoração duplicada
