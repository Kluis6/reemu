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
