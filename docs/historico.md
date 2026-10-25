# Histórico do ReEmu

Tudo o que o `TASKS.md` acumulou até 2026-09-24: a tabela de etapas com o
status detalhado, as notas de cada decisão, o backlog com os itens riscados
(feitos) e as notas de progresso por sessão. Foi movido **sem edição** pra
cá quando o `TASKS.md` virou só a lista do que está em aberto. O `TASKS.md`
aponta pra cá quando um item precisa do contexto completo.

Registros novos entram na seção **Notas de progresso**, mais recente
primeiro.

---

## Fundação (feito neste scaffold)

- [x] `done` — Estrutura de workspace (Cargo + pnpm)
- [x] `done` — Traits do `domain` para todas as portas definidas no design
- [x] `done` — Migration SQL inicial com todos os schemas decididos
- [x] `done` — `.gitignore` e `TASKS.md`

## Setup local (ver STEP_BY_STEP.md)

- [x] `done` — `git init` + primeiro commit (remote: github.com/Kluis6/reemu)
- [x] `done` — Toolchain instalada (Rust 1.97, pnpm 11, cargo-tauri 2.11)
- [x] `done` — `cargo check --workspace` passa sem erro (inclui o crate Tauri)
- [x] `done` — `apps/desktop`: `cargo tauri init` executado (pacote `reemu-desktop`,
      id `com.reemu.desktop`, linka `domain` + `db`, no workspace)
- [x] `done` — `packages/app-desktop`: Vite + React 19 + Fluent + Zustand +
      TanStack Query instalados; `vite.config.ts` afinado p/ Tauri (porta 1420)
- [x] `done` — `cargo tauri dev` abre a janela sem erro (2026-08-27)
- [ ] `todo` — `apps/mobile` / `packages/app-mobile` / `packages/ui` /
      `packages/shared` — parte da **Etapa 11 (Android)**, adiada pelo usuário;
      nenhum dos 4 existe ainda (nem a pasta). Os pacotes compartilhados só
      nascem quando o mobile for o 2º consumidor (ver nota de 2026-09-19 no
      backlog › Infra).

## Etapas de implementação (docs/ai-context/01 a 12)

| # | Etapa | Status | Depende de |
|---|---|---|---|
| 01 | Domain + DB — repositórios sqlx | `done` | Setup local |
| 02 | Core Loader Desktop — caminho GL | `done` (software + HW render GL: contexto EGL offscreen + FBO + readback; validado c/ Super Mario 64 2026-09-02. Interop dma_buf zero-cópia `REEMU_GL_INTEROP=1` **validado em hw c/ N64** 2026-09-04, imagem + flip Y ok) | 01 |
| 03 | Tauri Desktop Shell — surface nativa | `done` (**surface nativa `wl_subsurface` `place_above` é o PADRÃO** desde `c29508f`; `REEMU_NATIVE_VIDEO=0` volta pro `<canvas>`. SNES/N64 + bezel + menu de pausa com print borrado; SET_ROTATION 2026-08-31 — ver doc 03) | 02 |
| 04 | Shader Chain + Decoração | `done` (shader + decoração + params na UI — 2026-08-30) | 03 |
| 05 | Input, Hotkeys, UI de Binding | `done` (validado c/ DualSense 2026-08-29; foco de menu 2026-08-30; `RETRO_DEVICE_ANALOG` — analógico do N64 — 2026-09-04) | 03 |
| 06 | Áudio — Dynamic Rate Control | `done` (DRC + sink + "aplicar ao vivo"; `SET_SYSTEM_AV_INFO` runtime + estimador de taxa real 2026-09-04 — N64 sem picote; instrumentação `REEMU_AUDIO_DEBUG=1`) | 03 |
| 07 | Frontend React — Fluent/Zustand/Toast | `done` (modo Xbox completo: Início/Biblioteca/RomDetail/PlayScreen, Griffel, busca, nav por controle) | 03 |
| 08 | Save States e Save RAM | `done` (thumbnail por slot + painel na UI + "jogar daqui" — 2026-08-30) | 02 |
| 09 | Scraping de Metadata | `done` (ScreenScraper por CRC + revisão manual; multi-provider/IGDB no backlog) | 01 |
| 10 | Catálogo e Download de Cores | `done` (68 cores do buildbot: software + GL usáveis; badge "OpenGL"; Vulkan-only fora até 12) | 01 |
| 11 | Port Android | `todo` (desbloqueado — 01–10 `done`; usuário adiou 2026-08-30) | 03–10 completas no desktop |
| 12 | Vulkan HW Render Fase 2 | `done` pra Beetle PSX HW (`mednafen_psx_hw`, validado em hw do usuário 2026-09-11/12 — device criado pelo frontend, wgpu adota, `VkBlit` do scanout A1R5G5B5, save state e stdout do core resolvidos). **Pendente**: flycast/mupen como 2º/3º alvo + Fase C (tirar CPU-wait do blit/submit, validar sob carga) — ver doc 12 | ver doc 12 |

**Desktop (01–10) fechado.**

**Caminho crítico #1 — GL HW render** (passo 4 da etapa 02): fechado. Readback
validado c/ Super Mario 64 (2026-09-02); **interop dma_buf zero-cópia
(`REEMU_GL_INTEROP=1`) validado em hardware NVIDIA c/ N64 (2026-09-04)** —
imagem correta, flip Y ok, bezel ok.

**Caminho crítico #2 — surface nativa de vídeo**: **FECHADO — é o padrão**
(`c29508f`, 2026-09-05). `wl_subsurface` `place_above` da webview **opaca** — o
jogo (wgpu, zero-cópia) cobre a webview jogando; ao abrir o menu o Rust captura
1 frame, esconde a subsurface e a webview reaparece com o print borrado (menu
de pausa estilo RetroArch). Máquina de estado `VideoMenu` no `reemu-video-pump`.
`REEMU_NATIVE_VIDEO=0` / `REEMU_GL_INTEROP=0` voltam pro `<canvas>` / readback.
Subsurface posicionada pelo `Resized` (offset de CSD via inner/outer position;
0,0 em fullscreen). `mod x11` e `x11-dl` removidos. Validado com SNES/N64,
bezel, troca de ROM e menu de pausa.

Bugs de jogo do N64 resolvidos 2026-09-04 (`63e99bc`..`850c6cc`): tela branca
(NUL no PlayScreen.tsx), tela preta ao trocar ROM (pump esconde a subsurface
no idle), analógico (`RETRO_DEVICE_ANALOG`), áudio picotado (`SET_SYSTEM_AV_INFO`
+ estimador de taxa). **Crash de reload do N64 resolvido** (`3eb1a9f`): core
libretro isolado num processo filho descartável (`reemu-core-host` +
`crates/core-ipc`) — ver `n64-reload-crash` na memória.

**Polimento de UI (2026-09-05, `1fb2bb9`/`2b2d784`)**: casca responsiva pra
telas grandes (grid `100vh`/`minmax(0,1fr)`, tiles fluidos), sistema de temas
de cor (Verde Xbox/Roxo/Âmbar via `createDarkTheme` + tokens custom `--reemu*`,
`useThemeStore`). Tela Configurações › Aparência e modo claro já existem
(salvos no `localStorage`). FALTA: tema de alto contraste e persistir o tema
no lado Rust.

**Compilador slang (opção A) — FEITO** (2026-09-05, `98e8668` fase 1 +
`33765a3` fase 2):

- **Fase 1**: frontend GLSL trocado de `naga::front::glsl` (engasgava com
  `#define`-macro, construtor `mat4`, ternário em const) pra **glslang
  vendorizado → SPIR-V → `naga::front::spv` → WGSL**. CI += `g++`.
- **Fase 2**: executor multi-entrada em `gpu.rs` — `TextureSemantic`
  (`Source`/`Original`/`OriginalHistoryN`/`PassOutputN`/`PassFeedbackN`/alias/LUT), BGL
  por passe, ring de history, feedback (cópia pro próximo frame),
  `PassOutput`/aliases, LUTs do `.slangp` (`decode_png`), `float/srgb_
  framebuffer`, `frame_count_mod`, `wrap_mode`, `scale_type=viewport`.
  Interop (HW render): history = frame atual (degradação aceita).
- **Pendente**: `mipmap_input` / `mipmap` de LUT (parsing existe; falta gerar
  a cadeia de mips — wgpu não faz automático). TODOs em `PassSpec`/`LutSpec`.
- Ver memória `reemu-slang-shader-pipeline`.

**Validação de campo — FEITA** (2026-09-05, `53771e8`). `libretro/slang-shaders`
clonado em `~/.local/share/com.reemu.desktop/shaders/slang-shaders`.
Harness `gpu.rs::tests::field_validate_real_presets` (`#[ignore]`, fora do CI):
`cargo test -p reemu-desktop --lib field_validate_real_presets -- --ignored --nocapture`
(`REEMU_SHADER_DIR`, `REEMU_SHADER_LIMIT`, `REEMU_SHADER_FULL_ERR=1`,
`REEMU_SHADER_DUMP=<pasta>` pro GLSL reescrito do estágio que falhou).

**Resultado final (2026-09-19): 2547/2554 = 99,7%.** Lista dos que compilam:
`docs/shaders/working-presets.txt`. Trajetória: 28,3% (fase 2) → 40,6%
(blocker #1) → 60,8% → 92,7% (2026-09-06) → **99,7%**. Sobra 1 falha real
(`test/format.slangp`, textura de inteiros — feature de pipeline, não de
compilador) + 6 `.slangp` de `koko-aio/**/refs/` que nem são preset (sem a
chave `shaders`).

| 100% | scanline-classic · hdr · interpolation · pal · blurs · dithering · deinterlacing · scanlines · motionblur · stereoscopic-3d · denoisers · sharpen · subframe-bfi · downsample · gpu · cel · deblur · film |
|---|---|
| 90–98% | Mega_Bezel 97 · koko-aio 97 · border 95 · misc 92 · pixel-art… 91 · edge-smoothing 90 · uborder 90 |
| 70–88% | ntsc 88 · vhs 86 · handheld 78 · nes_raw_palette 80 · presets 71 · crt 71 |
| baixo | anti-aliasing 67 |

**Todos os blockers grandes fechados:**

1. ~~`sampler2D` como parâmetro de função~~ **FEITO** (`13f404f`) — reescrita
   cobre assinatura/corpo/call-site + `blank_comments`.
2. ~~varying de struct/array (`NotIOShareableType`)~~ **FEITO** (`b14740f`) —
   `flatten_io_aggregates` achata em `location`s escalares (+ `mat_shape`
   pra `mat3`/`mat4` varying).
3. ~~scanline-classic (602)~~ **FEITO** (`dbf5b84`) — `isinf`/`isnan` viram
   helpers escalares (`patch_missing_builtins`; o backend WGSL do naga não
   tem `UnsupportedRelationalFunction`).
4. ~~koko-aio (179)~~ **FEITO** (`0fbfaba` + `dbf5b84`) — guard de `#include`
   por estágio + reescrita de sampler não toca no NOME de `#define`.
5. Macro que repassa sampler (`#define COMPAT_TEXTURE(c,d) HSM_...(c,d)`) —
   `split_sampler_macros`, foi o que levou Mega_Bezel de 23% a 97%.
6. Orientação — shaders slang saíam de cabeça pra baixo
   (`adjust_coordinate_space: false`, `b14740f`).

**Long-tail ~7% que ainda falha** (documentado em `docs/shaders/README.md`):
família crt-royale (`#define tex <sampler>` no vertex, sampler só no
fragment), gameboy/authentic_gbc/xbr multipass (`validação naga`), smaa
(constructor num caso não coberto), helpers macro/forward-ref não resolvidos.
`mipmap` NÃO é causa de falha de nenhum preset — segue de baixa prioridade
(degrada silenciosamente).

**Downloader de shaders — FEITO** (`3d29717`): `shader_pack.rs` baixa
`libretro/slang-shaders` (~130 MB, streaming) pra `<dados>/shaders/`; botão
"Baixar pacote de shaders" na `ShaderLibrary` com toast de progresso.

**TRILHA DO COMPILADOR SLANG: FECHADA.**

**Decisão pendente**: (B) etapa 12 Vulkan HW, (C) etapa 11 Android,
(D) auditoria de perf do caminho do core. Ver backlog abaixo.

## Como atualizar

Ao concluir uma etapa:
1. Marque a linha correspondente como `done` nesta tabela
2. Se algo ficou pra trás/foi simplificado, anote em uma linha de nota
   logo abaixo da tabela (ex: "Etapa 04: Mega Bezel funcional, mas sem
   suporte a preset com sub-diretórios aninhados — ver issue X")
3. Se uma etapa não pode prosseguir por dependência externa (ex: aguarda
   decisão sua), marque `blocked` com o motivo

## Backlog (fora da ordem principal — não iniciar antes de fechar 06/08/09)

Renderização / filtros (independente da etapa 12):
- ~~**Integer scaling**~~ **feito (2026-09-17), com 2 correções no mesmo
  dia** — `domain::video` + `db::VideoConfigRepo` (linha única, mesmo
  padrão de `audio_config`) + comandos `get_video_config`/
  `update_video_config` + toggle em Config › Vídeo (`SettingsVideo.tsx`).
  Cálculo em `gpu.rs`: sem moldura, fator `floor(min(dw/nw, dh/nh))` (sem
  corte, pode sobrar letterbox); com moldura/bezel, fator
  `round(altura_da_moldura / altura_nativa)` e deixa a GPU cortar o excesso
  quando passar (sem `.clamp` no NDC).
  - **Correção 1** (testado com Batman Beyond/PS1): a largura usava
    `native_width × fator` (pixel quadrado); cores com PAR ≠ 1 (PS1, N64,
    Mega Drive…) ficavam mais largos que o vidro da moldura, cortando texto/
    HUD nas bordas esquerda/direita. Fix: largura = `altura × aspect_ratio`
    (já corrigido de PAR), genérico pra qualquer core — não precisa de
    tabela por plataforma.
  - **Correção 2**: o fator sempre arredondava pra CIMA (`ceil`) pra nunca
    sobrar barra preta; quando a conta caía perto do meio do caminho entre
    dois fatores (ex: 4.5), isso saltava um fator inteiro MAIOR que o
    necessário, cortando linhas inteiras de HUD perto da borda ("Developed
    by" sumia no boot do Batman Beyond). Trocado pra `round` (fator mais
    próximo, menor erro em pixels entre barra preta e corte).
  - **Ainda não é o resultado ideal** (confirmado com o usuário, ficou como
    está por ora): em molduras onde a proporção cai bem no meio do caminho
    entre dois fatores, a barra preta que sobra pode ficar maior do que o
    usuário gostaria (visto no mesmo teste — bordas pretas grossas em vez de
    quase preencher a moldura). Não redimensiona a janela do app nem a
    moldura (photo bezel), só o retângulo do jogo dentro dela — feature de
    resize de janela foi tentada e revertida
  a pedido do usuário (queria o jogo se ajustando à janela, não o inverso).
- ~~**Seleção de preset por pasta**~~ **feito (2026-09-02)** — comando
  `list_slangp_dir` (varre recursivo, agrupa por subpasta, teto 6000) +
  `<ShaderLibrary>` em Config › Vídeo e RomDetail: aponta pra `shaders_slang`
  (raiz no `localStorage`, o shader ativo continua persistido no DB via
  `set_shader`), filtro de texto, grupos colapsáveis. O "Carregar .slangp
  avulso…" continua pra arquivos fora da pasta. Não tocou `gpu.rs`.
- ~~**Compilador slang via glslang→SPIR-V**~~ **feito (2026-09-05, `98e8668`)**
  — substitui o `naga` glsl-in. Ver o bloco "Compilador slang (opção A)" acima.
- ~~**Feedback / OriginalHistory / LUT no `gpu.rs`**~~ **feito (2026-09-05,
  `33765a3`)** — ring de history + cópia de feedback + LUTs do `.slangp`.
  Falta só `mipmap_input`/`mipmap` de LUT (gerar a cadeia de mips).
- ~~**Validar contra presets reais**~~ **FEITO (2026-09-06)** — 2368/2554 =
  92,7%. `docs/shaders/working-presets.txt`. Ver o bloco "Validação de campo"
  acima.
- ~~**Downloader de pacote de shaders (opção B)**~~ **FEITO (`3d29717`)** —
  `shader_pack.rs`; botão na `ShaderLibrary`.
- ~~**Mipmaps**~~ **feito (2026-09-18)** — `mipmap_input` (passe) e `mipmap`
  (LUT) do `.slangp` agora geram a cadeia de verdade (antes só eram
  parseados, o executor amostrava sempre o nível 0 — passes de bloom/glow/
  halation do Mega Bezel/crt-royale ficavam "chapados"). Passe: em
  `ensure_target` (`gpu.rs`), o alvo do passe N aloca `mip_level_count` só
  quando o passe N+1 tem `mipmap_input` (mesma convenção do RetroArch —
  conferida no `shader_vulkan.c` deles, não de memória); `generate_mips`
  preenche os níveis 1..N via blit linear nível-a-nível a cada frame (wgpu
  não tem "generate mipmaps" embutido, ao contrário do `vkCmdBlitImage`),
  reusando o pipeline de blit da composição. LUT: cadeia gerada uma vez, na
  CPU (box downsample 2×2), no load do preset — não por frame. Validado com
  teste real de GPU (`gpu::tests::mipmap_input_generates_full_mip_chain_
  for_next_pass`): xadrez 8×8 no passe 0 + `textureLod` no nível mais alto
  no passe 1 — confirmado que FALHA sem o fix (xadrez cru vazando, LOD
  grudado no nível 0) e passa com ele (saída uniforme ~127, a média
  correta). `crates/shader-slang` não mudou (já parseava os dois campos).
- ~~**Long-tail de shaders (~7%)**~~ **feito (2026-09-19)** — 92,7% → 99,7%
  (+179 presets), zero regressão (diff contra o `working-presets.txt`
  anterior). 9 correções em `crates/shader-slang`, cada uma com teste
  unitário: macro de tipo/repasse escondendo o sampler (smaa), parâmetro
  sampler dentro de `#if` no meio da assinatura (smaa), macro multilinha
  usando o parâmetro sampler da função (crt-royale), apelido de sampler em
  cadeia e condicional (metacrt/crt-royale), guard de `#include` por estágio
  quando o `#pragma stage` está num arquivo incluído (crt-yah), `modf` via
  `trunc` (`MissingSpecialType` do naga), `return` final em função não-void
  (`ExpressionAlreadyInScope`), `flat` em varying inteiro (ntsc-blastem),
  variável local sombreando sampler global (crt-geom-deluxe). Famílias que
  estavam em 67–78% foram a 100%: anti-aliasing, crt, handheld,
  edge-smoothing, presets, ntsc, misc, border, pixel-art-scaling, vhs,
  reshade, nes_raw_palette. Ver `docs/shaders/README.md`.
- **FSR 1.0 / RCAS / CAS como preset de upscaling** — destravado pelo
  compilador; espacial (1 frame), diferente de DLSS/FSR2/XeSS temporais que
  **não servem** pra emulação (sem motion vectors/depth/jitter — o core só
  entrega o framebuffer pronto). Avaliado 2026-09-05.
- **HDR / tonemapping** — depois da validação de campo.

Cores com GPU:
- ~~**N64: app fecha ao carregar a 2ª ROM na mesma sessão**~~ **resolvido
  (2026-09-04)** — o core libretro passou a rodar num **processo filho
  descartável** (`reemu-core-host`, novo bin `crates/core-host-desktop`);
  `emu-session` mata e sobe um processo novo a cada `load` (nunca reusa,
  mesmo pro mesmo core), então a re-entrância do parallel_n64 deixa de
  importar (memória sempre parte limpa). IPC em `crates/core-ipc` (socket
  Unix `SOCK_SEQPACKET` + `SCM_RIGHTS` pro dma_buf/memfd do anel de frame —
  `gpu.rs` não mudou nada, o `GpuTextureHandle` de interop continua igual).
  API pública de `EmuSession` ficou a mesma; `emu-session/tests/session.rs`
  tem um teste novo (`each_load_spawns_a_fresh_process`) provando que o PID
  muda a cada troca. Ver memória `n64-reload-crash` (histórico da
  investigação) e o cabeçalho de `emu-session/src/session.rs`.
- ~~**GL HW render (etapa 02 passo 4)**~~ **feito (2026-09-02)** — `gl_context.rs`
  + `dmabuf.rs`: contexto EGL offscreen + FBO + os 4 callbacks. Frame por
  readback (default) ou interop dma_buf (`REEMU_GL_INTEROP=1`). Mario 64 roda.
  Falta: validar o interop em hw, trocar `glFinish` por semáforo cross-API,
  tirar o gate; `.7z` no scan; GLES-only sem core pra testar.
- **Etapa 12 (Vulkan HW)** — `blocked`, ver doc 12. Só depois do GL estável +
  lista de cores-alvo definida. NÃO temporal upscaling (DLSS/FSR2/XeSS não
  servem — sem motion vectors na emulação).

Metadata (etapa 09 fechada no MVP):
- Multi-provider (IGDB / TheGamesDB) + cascata; rate-limit por provider;
  match por MD5 além de CRC; badge de "N pendências" no rail.

Scan de ROMs / identificação de sistema (`library-scan`, 2026-09-04):
- ~~Sistemas de disco (PS1/PS2/Saturn/Dreamcast/PSP/Sega CD/PC-FX/3DO) todos
  caindo no balde genérico `"disc"`~~ **resolvido** — `system_from_folder_name`
  (movido de `decoration.rs` pra `systems.rs`, agora a tabela canônica única)
  desambigua pela pasta ancestral (`<roms>/psx/*.iso` → `system_id "psx"`)
  quando a extensão é ambígua (`AMBIGUOUS_DISC_EXTS`); sem pasta reconhecida,
  continua caindo em `"disc"` (fallback preservado).
- ~~Arcade (MAME/FBNeo) invisível no scan e travava no load~~ **resolvido** —
  `.zip` numa pasta reconhecida como arcade (`arcade`/`mame`/`fbneo`/`fba`/
  `neogeo`/`cps1-3`) vira `system_id "arcade"`, hash do **arquivo inteiro**
  (não tem "a ROM" dentro, só chip dumps avulsos); `core-loader-desktop::
  loader::open_core` não trava mais quando `extract_rom` não acha nada pra
  extrair — cai pro caminho do `.zip` original (o que `need_fullpath=true`
  do fbneo/mame espera).
- ~~Cores do catálogo pra sistemas que o scan não reconhecia~~ **feito
  (2026-09-19)** — o app baixava core pra Nintendo DS, SG-1000,
  SuperGrafx, Atari 5200/8-bit/Jaguar, Pokémon Mini, Supervision, MSX,
  Vectrex, Odyssey², C64, Amiga, ZX Spectrum e Amstrad CPC, mas as ROMs
  desses sistemas ficavam invisíveis. Os 15 entraram em `systems.rs`
  (extensão exclusiva + nomes de pasta + pasta de capa), com extensões
  tiradas do `.info` oficial de cada core (`libretro-core-info`) e pastas
  de capa conferidas nos repos do `libretro-thumbnails`. Extensões
  genéricas desses cores (`.bin`/`.rom`/`.dsk`/`.tap`/`.cas`) só contam
  dentro da pasta do sistema (`folder_only_exts` + ramo novo em
  `scan.rs`) — fora dela continuam ignoradas. `.sgx` saiu de `pcengine`
  pra `supergrafx`. `ROM_EXTS` do `core-loader-desktop::archive` estava
  dessincronizado (faltavam `a78`/`vb`/`col`/`int`: esses sistemas
  zipados eram catalogados mas não extraídos no load) — sincronizado.
  **Não feito**: id do ScreenScraper dos 15 (a lista oficial exige
  credencial de dev, que o app não tem — ficam só com capa do libretro,
  como PS1/Saturn); BIOS desses sistemas (Amiga Kickstart, Atari 5200,
  MSX, firmware do DS) em `domain::bios`; DOS e ScummVM (jogos são
  pastas/`.zip` sem extensão própria — precisam de outro tratamento).
- Novos sistemas cobertos (extensão ou pasta): `vb`, `atari7800`, `coleco`,
  `intellivision`, `ps2`, `pcenginecd`, `pcfx`. Boxart (`thumbnails.libretro.
  com`) coberto pros de disco+os cartuchos novos (conferido contra o org
  `libretro-thumbnails` no GitHub); `arcade` fica sem (MAME/FBNeo são sets
  separados lá, sem 1 pasta única).
- ~~Suporte a `.7z` no scan~~ **feito (2026-09-17)** — `sevenz-rust2`
  (fork mantido do `sevenz-rust`, puro Rust — sem libarchive/7z nativo,
  compila limpo no CI Windows+Linux). `library-scan::archive` generalizado
  (`is_supported_archive`/`peek_archive`/`read_archive_entry`, dispatch por
  extensão) e `core-loader-desktop::archive` idem (`is_archive`/
  `extract_rom`), mesmo tratamento de set de arcade sem entrada de cartucho
  reconhecível (`NotFound` → cai pro caminho do arquivo original). Testado
  de ponta a ponta com fixtures `.7z` reais (`ArchiveWriter`, feature
  `compress` só em dev-dependencies) — não só compila: scan cataloga
  hash/sistema certo, `extract_rom` extrai bytes certos, fallback de arcade
  funciona. `default-features = false` no build de produção (sem aes256/
  bzip2/ppmd — não usados por sets de ROM comuns).
- ~~UI pra corrigir sistema errado na mão~~ **já existia, nota corrigida
  (2026-09-17)** — a anotação anterior estava desatualizada. `set_metadata`
  (`domain::library::RomRepository`) já suporta trocar `system_id`; comando
  Tauri `set_rom_metadata` já registrado; `RomDetail.tsx` já tem o botão
  "Editar nome e plataforma" (ícone de lápis na hero) abrindo um diálogo
  com `<Select>` de todas as plataformas (`knownPlatforms()` em
  `lib/platform.ts`), hint explícito ("Corrige ROMs que o scan não
  identificou, ficam em 'Disco'"). Nada a fazer aqui.

BIOS / arquivos de sistema (2026-09-04 — feature nova, não existia nada antes):
- `domain::bios` — tabela pura (`system_id` → arquivo(s) esperado(s),
  subpasta, MD5, obrigatório?), conferida contra `docs.libretro.com/library/
  <core>/` (Beetle PSX, Kronos, Flycast, FBNeo), não de memória. Cobre
  `psx`/`saturn`/`dreamcast`/`arcade`/`segacd`/`pcenginecd`/`pcfx`.
- `apps/desktop/src-tauri/src/bios.rs` — `check_all` (presença + MD5 contra
  `<dados>/system`), `import_bios_file` (copia + renomeia pro nome
  canônico), `remove_bios_file`. **Nunca baixa nada** — BIOS é copyright da
  fabricante.
- Comandos `list_bios_status`/`import_bios_file`/`remove_bios_file`; aba
  nova **Configurações › BIOS** (`SettingsBios.tsx`, mesmo estilo de
  `SettingsCores`); `RomDetail` avisa (toast, não bloqueia) se o sistema da
  ROM tem um arquivo `required: true` faltando antes de navegar pro jogo.
- ~~Cobertura só dos 4 sistemas mais comuns~~ **Sega CD, PC Engine CD e
  PC-FX adicionados (2026-09-19)**, conferidos em `docs.libretro.com`
  (Genesis Plus GX, PicoDrive, Beetle PCE Fast, Beetle PC-FX) e nas strings
  dos `.so` instalados. `BiosFile::md5` virou lista de MD5s aceitos: o
  `bios_CD_U.bin` tem dois documentados (GPGX e PicoDrive citam revisões
  diferentes), e um MD5 só marcaria uma BIOS boa como errada. Obrigatórios:
  `syscard3.pce` (PCE CD) e `pcfx.rom`. As 3 BIOS de Sega CD ficaram
  opcionais (cada uma só vale pros jogos da sua região, e ninguém tem as
  3) — por isso o `RomDetail` não avisa de Sega CD; avisar certo exigiria
  saber a região da ROM.
- PSP **não** entra em `domain::bios`: o PPSSPP não usa BIOS, e sim a pasta
  `assets` do projeto PPSSPP (GPL) em `<system>/PPSSPP/` (fontes, telas de
  memory card, configurações por jogo). Como é GPL, dá pra baixar como os
  shaders (`shader_pack.rs`) — **todo**, se os jogos de PSP mostrarem
  problema sem ela.

Áudio (etapa 06):
- ~~Validação de sessão longa~~ — N64 ~1min sem underrun (2026-09-04). Restam 2
  hitches isolados/sessão (~30-55ms) que o buffer de 250ms quase absorve.
- Resampler linear → `rubato` se a qualidade não bastar.
- `SET_SYSTEM_AV_INFO` runtime propaga só timing; a geometry nova (aspect) ainda
  não — ver "Correção de vídeo" abaixo.

Correção de vídeo (cores software):
- ~~`RETRO_ENVIRONMENT_SET_ROTATION`~~ — **feito 2026-08-31**: `ffi_state.rs`
  captura o valor, `FrameMetadata.rotation_degrees`, `domain::rotate_rgba`
  aplica na CPU no `poll_frame` (ambos os caminhos), o `<canvas>` acompanha a
  AR (declarada quando a orientação bate, dos pixels quando veio rotacionado).
  Falta: validar com um jogo vertical real (FBNeo). Direção assumida =
  anti-horário (libretro.h); flipar se sair espelhado.
- **`SET_GEOMETRY` / `SET_SYSTEM_AV_INFO` em runtime** — `aspect_ratio` vem do
  `av_info` do load; se o core muda a proporção no meio do jogo não pega
  (resolução em si já pega, é per-frame). Baixa prioridade pros cores atuais.

Infra:
- ~~`packages/ui`, `packages/shared` — ainda sem `package.json`~~
  **adiado pra Etapa 11 (2026-09-19, decisão do usuário)** — não existia
  nem a pasta. Criar agora seria reorganizar código sem um 2º consumidor.
  Removidos do `package.json` raiz os scripts `dev:mobile`/`build:ui`, que
  apontavam pra pacotes inexistentes (`pnpm --filter` sem match não roda
  nada). Pro Android, esta máquina tem só um SDK parcial (platform 37,
  build-tools 36): faltam NDK, JDK 17, `cmdline-tools` e os targets Rust
  `*-linux-android*`.
- `apps/mobile` / `packages/app-mobile` (etapa 11 Android) — só depois do
  desktop ponta a ponta (decisão do usuário 2026-08-30: deixar pra depois).
- Windows/macOS — **bug de build Windows corrigido (2026-09-16, `019cb06`)**:
  os 8 crates internos (`domain`/`db`/`emu-session`/`video-surface`/
  `audio-desktop`/`library-scan`/`input-desktop`/`shader-slang`) estavam
  presos dentro de `[target.'cfg(target_os = "linux")'.dependencies]` em
  `apps/desktop/src-tauri/Cargo.toml` — no Windows nenhum linkava (221 erros
  "unresolved crate"). Movidos pra `[dependencies]`; só `wayland-client`
  continua linux-only.
- ~~`core-loader-desktop/build.rs` quebrava `cargo tauri dev` inteiro no
  Windows~~ **feito (2026-09-18)** — 1º teste real numa máquina Windows do
  usuário: `LINK : fatal error LNK1561: pontos de entrada devem ser
  definidos` ao compilar `fixtures/testcore.c` (o core-fake em C usado nos
  testes deste crate, mas o `build.rs` roda pra QUALQUER build, não só
  teste). Causa: o `build.rs` usava flags estilo GCC (`-shared`, `-o`)
  incondicionalmente por `target_os`; no toolchain padrão do rustup pra
  Windows (`*-pc-windows-msvc`) o compilador é `cl.exe`/`link.exe`, que não
  reconhece essas flags — cai no padrão de EXE comum sem `/DLL`, e como
  `testcore.c` não tem `main()`, o linker recusa por falta de entry point.
  Fix: detecta o compilador de verdade via `cc::Tool::is_like_msvc()` (não
  só o SO-alvo — cobre também `*-pc-windows-gnu`/MinGW, que continua
  precisando das flags GCC) e usa `/LD` + `/Fe:<saída>` no MSVC. Validado no
  Linux (não regrediu, mesmo branch de antes) — o usuário confirmou no
  Windows: o LNK1561 sumiu e o build avançou até o próximo erro (abaixo).
- ~~`core-ipc`/`core-loader-desktop` não compilavam no Windows~~ **feito
  (2026-09-19)** — 2º teste real no Windows: 16 erros `E0432`/`E0433`, todos
  por uso de `std::os::fd`/`rustix::{net,fs,mm,io,stdio}` sem nenhum
  `#[cfg(unix)]`. Não era só o build: o `core-ipc` (socketpair `SEQPACKET` +
  `SCM_RIGHTS` + `memfd`) é o canal de TODO carregamento de jogo, então foi
  portado de verdade, mantendo o processo filho isolado por jogo (evita o
  bug de reload do N64):
  - `core-ipc/src/transport_win.rs`: dois pipes nomeados unidirecionais em
    modo byte + framing (`u32` LE + bincode). Dois e não um duplex porque,
    num handle síncrono, o `ReadFile` bloqueado da thread leitora trava o
    `WriteFile` de outra thread no MESMO handle (deadlock). Os handles vão
    pro filho por herança (`SetHandleInformation` + `bInheritHandles=TRUE`,
    que o `std::process::Command` já usa por padrão — conferido na fonte do
    `std`), serializados no `--fd` como `<rd>:<wr>:<nome>` (`ChannelArg`).
  - `core-ipc/src/shm_ring_win.rs`: anel de frame em memória compartilhada
    NOMEADA (`CreateFileMappingW`, nome derivado do nome do canal) no lugar
    do memfd via `SCM_RIGHTS`, que não existe em pipe nomeado.
  - interop dma_buf/GBM (`dmabuf.rs`, parte de `gl_context.rs`, braço
    `HardwareTexture` do core-host) só em `#[cfg(unix)]`; no Windows o GL
    sempre cai no readback. `silence_core_stdout`/`with_core_stdout_silenced`
    ganharam versão Windows (`SetStdHandle` + `NUL`). `rustix` virou
    dependência só de Unix nos 4 crates.
  Validado: `cargo check --target x86_64-pc-windows-gnu` limpo nos 4 crates
  (com um stub falso de `x86_64-w64-mingw32-gcc` só pro `build.rs` do
  testcore passar — sem link de verdade); Linux clippy `-D warnings` limpo e
  testes passando (o transporte Unix não mudou). **Não validado**: nada disso
  rodou num Windows de verdade ainda (os testes de `transport_win.rs` só
  compilam aqui) — próximo passo é o usuário rodar `cargo tauri dev` +
  `cargo test -p core-ipc` no Windows. **Atualização (2026-09-19)**: o
  usuário rodou `cargo test -p core-ipc` no Windows — os 5 testes do
  transporte passaram (incluindo o de deadlock recv/send).
- ~~`cargo tauri dev` exigia um build RELEASE do core-host~~ **feito
  (2026-09-19)** — 3º erro no Windows: `resource path
  ..\..\..\target\release\reemu-core-host.exe doesn't exist`. O `build.rs`
  do Tauri valida `bundle.resources` até no dev, e os
  `tauri.<os>.conf.json` são mesclados automaticamente. Latente no Linux
  também (só funcionava porque já havia um release antigo em `target/`).
  Fix: renomeados pra `tauri.bundle.{linux,windows}.json` (não
  auto-mesclados), passados via `--config` só no `cargo tauri build` do
  `release.yml`. Novo `scripts/dev.ps1` (equivalente do `dev.sh`: builda o
  core-host debug antes do `cargo tauri dev`).
- `emu-session/tests/session.rs::pause_freezes_emulation_then_resume` é
  intermitente sob carga (visto 2×, rodando a suíte de vários crates em
  paralelo; passa isolado): um `FrameReady` que já estava no canal chega
  depois do round-trip do `SetPaused(true)` e avança o `frame_seq`. Corrida
  pré-existente do teste/protocolo, não da porta Windows.
- Windows — **ainda falta**: ninguém terminou de compilar/rodar de fato
  numa máquina Windows real ponta a ponta (o teste acima já pegou 1 bug
  real) — os paths do buildbot de cores, o `video.rs` (`#[cfg(not(linux))]`,
  ver risco conhecido da superfície nativa) e o bundle continuam não
  verificados na prática.
- ~~`cargo tauri dev` direto não recompilava o `reemu-core-host`~~ **feito
  (2026-09-24)** — o core-host (`crates/core-host-desktop`) NÃO é dependência
  de `apps/desktop/src-tauri/Cargo.toml`, e só o `scripts/dev.sh` o
  compilava; rodando `cargo tauri dev` direto o binário ficava
  desatualizado/ausente (incidentes 2026-09-11 e 2026-09-16). Agora o
  `beforeDevCommand` do `tauri.conf.json` faz `cargo build -p
  core-host-desktop && pnpm --filter app-desktop dev`, então qualquer forma
  de subir o app recompila (validado: `touch` no core-host → `cargo tauri
  dev` recompilou antes do Vite). O `release.yml` continua buildando o
  core-host release explicitamente (o `beforeBuildCommand` não mudou).
- **`cargo fmt --check` falhava em 20 arquivos** (2026-09-24) — código
  commitado sem formatar quebrava o 1º passo do job `rust` do CI. Corrigido
  com `cargo fmt --all`. `STEP_BY_STEP.md` reescrito (era do scaffold): deps
  de sistema iguais às do CI (incl. `libudev-dev`/`libasound2-dev`), fluxo
  dev atual, checagem pré-commit e build de release.
- Frontend — responsividade revisada de ponta a ponta (2026-09-16, commits
  `36d469a`..`208a3bd`): elementos de tamanho fixo do Fluent trocados por
  `clamp()` (topbar, avatar, cards, setas do carrossel), padding lateral das
  páginas passou a derivar da largura real da rail via CSS var (alinhamento
  consistente em qualquer resolução, não só acima de 1600px), cards da
  grade/prateleira agora esticam dinamicamente até a borda (`auto-fit` +
  largura calculada em JS). Hero do RomDetail redesenhado no modelo "página
  de produto de loja" (ícone+título+ações no topo, banner de fundo colado no
  topo/sangrando a largura toda, tabs sobrepondo a base do hero).
- **`core_host_path()` — 2 bugs de empacotamento corrigidos (2026-09-17)**,
  achados ao montar o pipeline de release abaixo (não empacotados ainda, sem
  release publicada até agora — só rodava via `scripts/dev.sh`/`cargo tauri
  dev`, que sempre caem no branch "irmão do executável"):
  1. Windows nunca procurava `reemu-core-host.exe` (faltava a extensão).
  2. Linux `.deb`/AppImage: o `resource_dir` do Tauri (onde `bundle.resources`
     copia o binário) NÃO é `/usr/lib/<nome-do-executável>` como a doc do
     `PathResolver` sugere genericamente — é `/usr/lib/<productName>`
     (`ReEmu`, de `tauri.conf.json`). Confirmado empacotando um `.deb`/
     `.AppImage` reais localmente e inspecionando o conteúdo (`dpkg-deb -c`,
     `--appimage-extract`) antes de escrever o fix — a suposição inicial
     (nome do binário) teria saído quebrada em produção.
- **CI/CD de release (2026-09-17)** — pipeline nos moldes do Flycast: tag
  `v*` → `.github/workflows/release.yml` builda Linux (`.deb`+`.AppImage`) e
  Windows (`.msi`+`.exe` NSIS) em matrix, builda `reemu-core-host` ANTES do
  `cargo tauri build` (não é dependência do crate principal, ver nota acima)
  e sobe os artefatos como Release **draft** no GitHub (`softprops/
  action-gh-release@v2` — revisar e publicar manual). `tauri.linux.conf.json`
  / `tauri.windows.conf.json` (hoje `tauri.bundle.<os>.json`, ver entrada de 2026-09-19; eram auto-mergeados pelo Tauri por nome de
  arquivo) declaram `bundle.resources` apontando pro `reemu-core-host[.exe]`
  de `target/release/`. Só dispara em tag (não em todo push/merge — decisão
  do usuário). Validado com uma build de release real local (não só CI):
  `cargo build --release -p core-host-desktop` + `cargo tauri build`
  produziram `.deb`/`.rpm`/`.AppImage` com o `reemu-core-host` no lugar
  certo (`dpkg-deb -c` confirmou o path, ver bug 2 acima). **Falta**: badge +
  seção "Downloads" no README apontando pra Releases; nunca rodou de fato no
  GitHub Actions (sem runner Windows pra testar aqui).
- Desempenho do caminho do core (auditado 2026-09-02 — os itens fáceis já
  feitos: `rotate_rgba` sem cópia no no-op, flush da `.srm` off-thread, spin
  com menos leitura de relógio). Ainda no radar, por ordem de impacto:
  * ~~**Cópias de frame por frame**~~ **2 das ~4-5 cópias eliminadas
    (2026-09-18)**: (1) `gpu.rs::process()` — o readback (`unpad_rows`)
    alocava um `Vec` novo por frame só pra `pack_frame` copiar de novo em
    cima; agora `process()` escreve num buffer persistente do
    `FrameProcessor` (`readback_scratch`, reusado — só redimensiona se o
    tamanho mudar) e devolve `&[u8]` emprestado em vez de `Vec<u8>` dono, um
    `alloc+copy` a menos por frame no caminho GPU (o dominante — roda
    sempre que há GPU disponível, que é quase sempre). (2) JS
    (`PlayScreen.tsx`) — trocado `buf.slice(8, 8+need)` (copia o
    `ArrayBuffer` inteiro) por `new Uint8ClampedArray(buf, 8, need)` (view
    sobre o mesmo buffer, sem cópia; `Uint8ClampedArray` não tem restrição
    de alinhamento, o offset de 8 é seguro). **Ainda no radar**: FFI aloca o
    buffer nativo do core, `to_rgba8` aloca o RGBA, `pack_frame` ainda aloca
    (prepend do header de 8 bytes) — esses continuam um `alloc+copy` cada.
  * `latest_frame: Mutex<Option<Frame>>` → `triple_buffer`/`ArcSwap` (lock-free).
  * `push_samples` aloca `Vec<[f32;2]>` por frame — resample direto do `&[i16]`.
  * `drain_audio` = `mem::take` → `Vec` novo por frame (pequeno); reservar.
  * Spin de pacing ainda queima ~0,5ms de CPU/frame (aceitável em multi-core;
    ruim em laptop/bateria) — considerar `spin_sleep` ou janela menor.
  * Rebase do `next_deadline` só a >4 frames atrás → core lento roda flat-out
    (0 sleep, 100% CPU) por até 66ms antes de desistir; `* 2` seria menos pico.
  * Sem prioridade de thread no `emu-core-loop` — sob carga o scheduler pode
    não dar time-slice suficiente (RetroArch às vezes usa nice/SCHED_FIFO).
- Canvas WebGL (`texImage2D`) em vez de `putImageData` (CPU).
- ~~Dois controles idênticos colidem na porta (mesmo GUID SDL)~~ **feito
  (2026-09-18)** — `GamepadPoller` (`input-desktop::gamepad`) indexava todo
  o estado por conexão (`ports`/`down`/`stick`/`rstick`/`hat`) pelo `uuid`
  (GUID do SDL_GameControllerDB), que é por MODELO — duas unidades
  idênticas (mesmo GUID) colidiam na mesma entrada do mapa: só uma porta
  era atribuída pras duas, e os botões de uma se misturavam no estado da
  outra. Trocado pra `gilrs::GamepadId` (identidade da CONEXÃO física,
  distinta mesmo entre unidades idênticas — confirmado `Copy+Eq+Hash` na
  doc do gilrs 0.11). GUID continua sendo usado só onde é inerentemente por
  modelo: atribuição fixa de porta salva pelo usuário e remapeamento de
  botão (`device_port_assignment`/`controller_mappings`) — duas unidades
  idênticas ainda competem pelo mesmo *override salvo*, mas isso é limite
  do GUID do SDL não ter número de série (RetroArch tem a mesma limitação).
  **Sem teste automatizado**: `GamepadId` não tem construtor público (só
  nasce de hardware real conectado via `Gilrs`), não dá pra forjar duas
  conexões num teste unitário — validado por leitura do código + os 18
  testes existentes do crate continuam passando (sem regressão); validação
  com hardware real (dois controles idênticos) fica pra quem tiver o par.
  (~~eixo analógico → RetroPad~~ feito 2026-09-04: `RETRO_DEVICE_ANALOG`.)
- `GET_INPUT_BITMASKS` não anunciado — cores caem no query por id (ok, mas
  perde a otimização).
- ~~`docs/ai-context/01,02,05,06,07,08,09.md` com "Estado atual"
  desatualizado~~ **feito (2026-09-19)** — seções de 01 e 05–09 reescritas
  conferindo contra o código (02 já estava em dia). Os "Falta" que
  sobraram estão no fim de cada seção.
- `SaveStateMetadata.play_time_at_save` sempre `None` (sem tracking de tempo
  de jogo).

## Notas de progresso

- **2026-09-25 — `GET_INPUT_BITMASKS` anunciado**: pelo `libretro.h`, o
  frontend que responde `true` a `GET_INPUT_BITMASKS` (51 | EXPERIMENTAL)
  aceita `RETRO_DEVICE_ID_JOYPAD_MASK` (256) no `input_state` e devolve
  todos os botões num `int16_t`, bit N = botão de id N. O estado do RetroPad
  já era guardado assim (B=0 … R3=15), então a resposta é a máscara da
  porta. Core falso agora pergunta e lê a máscara como um core real; teste
  com A, Start e R3 (bit 15, `int16_t` negativo).

- **2026-09-25 — FSR, NIS e RCAS nos presets recomendados**: o pacote
  `slang-shaders` já trazia FSR 1 (`edge-smoothing/fsr`), NIS
  (`edge-smoothing/nis`) e RCAS (`sharpen/rca_sharpen`); CAS não vem nele.
  Conferido na doc da AMD (gpuopen.com/fidelityfx-superresolution): FSR 1 é
  espacial (sem histórico — serve pra emulação), EASU amplia pra resolução
  da tela e o RCAS dá nitidez depois, em espaço gama; o `fsr.slangp` segue
  isso. Os presets compilam (validação de campo) e rodam na GPU num teste
  novo (`field_render_upscalers`, tabuleiro 64×48 → 192×144, saída com
  contraste). Entraram na lista curada: "Ampliar com nitidez (AMD FSR)",
  "(NVIDIA NIS)" e "Só nitidez (RCAS)". Sem superfície nativa (modo canvas)
  o passe `viewport` amplia pra 3× o nativo.

- **2026-09-25 — proporção de tela muda em runtime (`SET_GEOMETRY`)**: o
  loader só logava o `SET_GEOMETRY` e, do `SET_SYSTEM_AV_INFO`, usava só o
  timing — a proporção de cada quadro ficava a do carregamento. Pelo
  `libretro.h` oficial, `SET_GEOMETRY` é o caminho indicado pra mudar a
  proporção sem reiniciar o vídeo (e ignora `max_width`/`max_height`). Agora
  os dois guardam `geometry_update` e o `DesktopCore` aplica depois de cada
  `retro_run`; a proporção segue nos metadados do quadro (superfície nativa)
  e no cabeçalho do `poll_frame` (modo canvas, agora 32 bytes). Core falso
  com ROM "GEOM" pede `SET_GEOMETRY` no 3º quadro; teste confere a mudança.

- **2026-09-25 — sem alocação por quadro no caminho software**: o
  `emu-session` lia cada quadro do anel de memória compartilhada num `Vec`
  novo (`reconstruct_frame`), liberado logo depois de apresentado. Agora a
  sessão tem um pool de até 2 buffers: `FrameRing::read_slot_into` copia pra
  um buffer reaproveitado, quem apresenta devolve o quadro com
  `EmuSession::recycle_frame` (`poll_frame` no modo canvas e o laço da
  superfície nativa), e um quadro substituído antes de ser apresentado
  também devolve o seu. Teste de integração com o core-fake confere que o
  buffer devolvido volta a ser usado.

- **2026-09-25 — OpenGL por hardware no Windows (WGL)**: o contexto GL dos
  cores vinha só do EGL, que o Windows não tem, então Beetle PSX HW, flycast
  e mupen64plus/parallel em GL não subiam lá. `gl_context.rs` agora separa a
  plataforma (`PlatCtx`): EGL no Linux (inalterado) e WGL no Windows, sem
  dependência nova (`windows-sys`) — janela oculta 1×1 com pixel format
  RGBA8/depth24/stencil8, contexto temporário pra obter
  `wglCreateContextAttribsARB` e o contexto final na versão e perfil do
  core (core, compat ou ES via `WGL_EXT_create_context_es2_profile`);
  `wglGetProcAddress` com fallback pro `opengl32.dll` (GL 1.1). O frame sai
  pelo readback (`glReadPixels`), como já era o caminho sem interop.
  Constantes e regras conferidas nas especificações da Khronos
  (`WGL_ARB_create_context`, `WGL_EXT_create_context_es2_profile`).
  Validado: clippy nas duas plataformas, testes de GPU real do EGL no
  Linux (readback e ring dma_buf). Falta rodar no Windows.

- **2026-09-25 — revisão de UI (heurísticas de Nielsen + guia Xbox/TV da
  Microsoft + Fluent 2)**. Telas capturadas em 1920×1080 com backend
  simulado; o que foi achado e corrigido:
  * H9 (recuperar de erros): erro de execução mostrava a página técnica do
    React Router em inglês. `RouteError` como `errorElement` — mensagem em
    português, "Tentar de novo"/"Voltar ao início", rail de pé, erro no log.
  * H6 (reconhecer > lembrar): jogo sem capa mostrava só as iniciais ("CA",
    "CA"…). Agora nome + plataforma no card.
  * H1/H10 (status e ajuda): "Jogar" desabilitado sem motivo visível (o aviso
    ficava escondido no pé da página). Aviso ao lado com botão "Abrir Cores".
  * TV (Microsoft: sem tooltip no controle, rótulos sempre visíveis): ações
    do detalhe do jogo com texto; confirmação de remoção no próprio botão
    ("Confirmar remoção"), em vermelho e separado.
  * H8/H4: texto do Fluent é `inline` mesmo com `as="p"` — título e
    descrição colavam ("Tema de corMuda…"); `block` onde faltava. Grade da
    plataforma com `auto-fill` (3 jogos viravam cards de ~560 px). "Limpar
    filtro" só com filtro ativo (antes: quadrado cinza vazio). Aba
    "Metadata" → "Metadados". Estado vazio de Controles explica o que fazer.
  * H7 (flexibilidade) / TV: "Tamanho da interface" em Aparência (100, 125,
    150%) com o zoom nativo do webview (`setZoom`, permissão
    `core:webview:allow-set-webview-zoom`) — a Microsoft pede ≥15 epx de
    texto a ~3 m e o Xbox renderiza a 200% em 1080p.

- **2026-09-25 — primeiro teste longo no Windows (RTX 3060)**: canvas WebGL
  confirmado (`canvas de vídeo: webgl`); download de ~40 cores, BIOS,
  PPSSPP, shaders e bezels ok; Mega Drive e GBA rodando com moldura.
  Problemas e correções:
  * `flycast` foi pela rota Vulkan in-process e derrubou o app
    (`STATUS_ACCESS_VIOLATION`). A escolha automática dessa rota agora só
    vale no Linux (validado); fora dele, só com `REEMU_HW=vulkan`.
  * Cores com render OpenGL por hardware não sobem no Windows: o contexto
    vem do EGL, que lá não existe. Mensagem de erro agora diz isso e sugere
    um core de software; WGL virou tarefa.
  * Uma ISO de PS3 ia para o PS1 (a checagem por "PLAYSTATION" pegava). O
    farejador reconhece as marcas do PS3 antes e deixa sem sistema.
  * Desempenho baixo com moldura, mesmo em core leve: no modo canvas a GPU
    compunha jogo + moldura no tamanho da moldura (1920×1080) e cada quadro
    voltava pra CPU e passava pelo IPC do WebView2 (~8 MB × 60/s). Agora
    `split_decoration`: o quadro sai só com o jogo (Mega Drive: ~290 KB),
    o cabeçalho do `poll_frame` (28 bytes) leva a geração da moldura e o
    retângulo do jogo, e a moldura vai uma vez por `decoration_image`; o
    `PlayScreen` empilha as duas camadas. A superfície nativa continua
    compondo na GPU. Teste de GPU novo + captura headless conferindo a
    posição (261,20 1401×1041 numa moldura 1920×1080).
  * Dois controles iguais (PS5) reconhecidos sem conflito — validado.
  * Prateleira "dançando" ao focar: `shelfFillWidth` arredondava a largura
    do card pra cima (`ceil`) e a fila passava da prateleira alguns px —
    virava rolável e o `scrollIntoView({ inline: 'center' })` do foco a
    deslocava. Agora largura fracionária truncada (nunca passa) e
    prateleira `fill` com `overflow-x: clip`. Medido em escala 125%: o 1º
    card fica parado em todos os 21 focos.
  * Texto serrilhado: `RouteTransition` e `riseIn`/`fadeIn` usavam
    `fill-mode: both`, deixando a tela presa numa camada composta (WebView2
    troca ClearType por antialias em cinza). Agora `backwards`.
  * Ícones borrados no rail: tamanho em `clamp(...vw)` fracionário + zoom
    por `scale`. Tamanho arredondado em múltiplos de 4 px (`round()`,
    pixel inteiro em 125/150/175%) e zoom pelo tamanho do ícone.
  * Separador do rail agora entre o sino e o desligar; legenda do destaque
    da tela inicial empilhada (saía "ContinuarJogo 2Super Nintendo").

- **2026-09-24 — página de downloads no GitHub Pages**
  (`https://kluis6.github.io/reemu/`): `site/` estático (HTML/CSS/JS puro),
  publicado pelo `pages.yml` quando `site/` muda. Lê as Releases da API do
  GitHub no navegador (cache de 10 min na sessão; 60 consultas/h por IP),
  então publicar uma Release já aparece sem novo deploy. Botão principal pelo
  sistema do visitante (AppImage/.deb no Linux, setup.exe/.msi no Windows),
  notas da versão mais recente abertas, antigas recolhidas, `.sig` e
  `latest.json` ocultos. Texto da API só via `textContent`. Visual da logo
  oficial (preto, neon verde → azul, linhas de CRT). Validado no Chrome
  headless (desktop, 390 px, sem versões, API bloqueada) com API simulada.

- **2026-09-24 — atualização automática, sino de notificações**:
  `tauri-plugin-updater` com comandos próprios (`updates.rs`:
  `update_check`/`update_install`, progresso pelo evento `update-progress`).
  Sem chave pública configurada, a verificação fica desligada, então o app
  não quebra antes de a chave existir. `requireSignedVersion` ligado (o CLI
  2.11 grava a versão na assinatura, o que bloqueia downgrade via manifesto
  adulterado).
  * Frontend: `useUpdateCheck` (8 s depois de abrir e a cada 6 h, só com o
    shell montado, nunca por cima do jogo) → notificação no sino
    (`NotificationBell`, acima do Encerrar, contador de não lidas) + toast
    com "Ver" → `UpdateDialog` (notas da Release, "Atualizar agora" com
    barra de download, "Fechar"). Depois do reinício, aviso "atualizado
    para X" com as novidades. O toast ganhou `action`; com botão, usa o
    layout multilinha (em uma linha o botão saía cortado).
  * CI (`release.yml`): assina se houver o secret
    `TAURI_SIGNING_PRIVATE_KEY` (`tauri.updater.json` →
    `createUpdaterArtifacts`), sobe os `.sig`, preenche o draft com
    `scripts/release-notes.sh` e, ao publicar a Release, o job
    `updater-manifest` gera o `latest.json` (`scripts/updater-manifest.mjs`)
    com as notas finais. Chaves `{os}-{arch}-{instalador}` + `{os}-{arch}`,
    conferidas contra o `get_urls` do plugin 2.12.
  * Validado: tsc, lint, vitest (+2 do parser das notas), clippy, check
    cruzado do Windows, scripts rodados localmente (notas desde a rc2;
    manifesto com `.sig` falsos) e capturas no Chrome headless com o
    backend simulado (toast, sino, lista, modal, progresso). Falta testar
    com Releases reais (depende da chave).

- **2026-09-24 — canvas de vídeo com WebGL**: o caminho `<canvas>` (padrão
  no Windows) desenhava com `putImageData`, que converte e copia o frame na
  CPU a cada quadro. Agora `lib/frameRenderer.ts` sobe o frame como textura
  WebGL (`texImage2D` na troca de tamanho, `texSubImage2D` no resto) e a GPU
  desenha um quad *nearest*; sem WebGL (ex.: WebKitGTK sem compositing) cai
  no `putImageData` de antes. O `PlayScreen` só redesenha quando chega frame
  novo e registra no log do Rust qual renderizador pegou
  (`canvas de vídeo: webgl|2d`). Trata perda/restauração de contexto.
  Validado no Chrome headless (SwiftShader) com frame assimétrico: WebGL e
  2D dão a mesma imagem, sem inverter nem espelhar. Falta ver no WebView2.
- **2026-09-24 (fim de noite) — desempenho: cópias e pacing**:
  * Conversor RGBA reusa buffer (`to_rgba8_into`/`to_rgba8_slice`): no
    caminho principal (core de software + GPU) era um `Vec` novo por
    quadro. Medido em release: 256×224 64→46 µs, 640×480 269→225 µs por
    quadro (7–28%; pouco em absoluto frente aos 16,67 ms).
  * Bug que já existia: com buffer de core mais curto que o declarado,
    `src.get(y*pitch..)` devolvia fatia vazia no limite exato e o índice
    entrava em pânico. Agora exige a linha inteira e zera o resto.
  * Canvas sem GPU converte direto no `Vec` da resposta IPC (sem RGBA
    intermediário nem a cópia do `pack_frame`) quando não há rotação.
  * `reemu-core-host`: o buffer do frame volta pro core depois de copiado
    pro anel (`DesktopCore::recycle_frame_buffer`) — sem alocação por
    quadro no callback de vídeo.
  * Pacer: recuperação de atraso limitada a 2 quadros (era 4, ~66 ms
    rodando sem dormir — "acelerava" e dava pico de CPU). Teste falha com
    o limite antigo.
  * Prioridade do processo do core: `ABOVE_NORMAL` no Windows; `nice -5`
    no Unix quando há privilégio (senão segue normal, log em debug).

- **2026-09-24 (madrugada) — metadata, DOS/ScummVM, PPSSPP, plano Rust**:
  * **ScreenScraper em todos os sistemas do scan** (eram 15): ids tirados da
    tabela do ES-DE (GPL, `ScreenScraper.cpp`) — os 15 que já existiam
    batem com os de lá. Consulta também por **MD5** (`md5=`), e `rommd5`
    igual conta como hash exato. Teste lê o `systems.rs` e exige id nos
    dois provedores pra todo sistema.
  * **TheGamesDB de reserva** quando o ScreenScraper não acha: busca por
    nome (tags `(USA)`/`[!]` removidas) + plataforma (ids do
    `GamesDBJSONScraper.cpp` do ES-DE), capa por `/Games/Images`. Sempre
    pra revisão. Chave no chaveiro (conta `thegamesdb`; `credentials.rs`
    generalizado pra N segredos, mesma migração/fallback). Não testado com
    chave real. IGDB **não** feito (sem fonte pública dos ids de plataforma).
  * Migration 0011 reenfileira os "não encontrado" (`external_id` vazio) —
    sugestões rejeitadas pelo usuário não voltam (conferido com SQL).
  * Selo de pendências de metadata no ícone de Configurações do rail.
  * **DOS e ScummVM no scan**: `.scummvm`/`.dosz` exclusivos; `.zip/.exe/
    .com/.bat/.conf` só SOLTOS na pasta `dos/` (`folder_only_flat` — os
    `.exe` de dentro da pasta de um jogo não viram entradas). +3 variantes
    do DOSBox no catálogo (127). Teste de scan com a árvore realista.
  * **Loader respeita `block_extract` / extensão aceita pelo core**: não
    extrai o `.zip` se o core quer o arquivo inteiro (regra do RetroArch).
    Conferido no DOSBox Pure real (`block_extract = true`). Antes um `.bin`
    dentro do zip de um jogo de DOS seria extraído e passado no lugar.
  * **Assets do PPSSPP**: `system_files.rs` baixa `assets/system/PPSSPP.zip`
    do buildbot (GPL, 193 arquivos), extrai em pasta temporária e troca
    atomicamente; teste com o pacote real. Linha própria na tela de BIOS.
  * Tela de BIOS usava um mapa de nomes próprio sem os sistemas novos
    (Amiga/Atari/MSX/DS apareciam como id cru) — agora `platformLabel`.
  * Aba Cores em grade de 2 colunas (3 a partir de 1600 px), com a área de
    Configurações liberada até 1400 px só nessa aba; toast com mais padding.
    Conferido por captura (Chrome headless com backend simulado).
  * `docs/ai-context/14-emuladores-nativos-rust.md`: levantamento de
    licenças (API do GitHub + texto) e plano de recriação em Rust a partir
    de fontes permissivas (ares ISC como base), bloqueado até a
    compatibilidade total com libretro.

- **2026-09-24 (noite) — desempenho, tempo de jogo, tema, BIOS, teste
  intermitente** (medições com `REEMU_PERF=1`, core falso de teste):
  * `reemu-video-pump` dormia 15 ms fixos + render, fora de fase com os
    16,67 ms do core: perdia **11 de 91 frames (12%)** com 3 ms de render.
    Agora acorda por `EmuSession::wait_for_frame` (`Condvar` no próprio
    `latest_frame`; espera fora do portão da VkQueue) — 0 perdidos. Teste
    `presenter_waking_on_frame_does_not_drop_frames` falha com o sleep
    antigo. O `Mutex` do `latest_frame` ficou: o frame é MOVIDO, o lock
    dura nanossegundos — trocar por `ArcSwap` não compraria nada.
  * Pacing: `core_loader_desktop::Pacer` (um só pro core-host e pro
    Vulkan in-process) com margem de spin adaptativa ao atraso real do
    `sleep` (150 µs–2 ms): spin **33 → 6,3 ms/s** de CPU, mesma precisão
    (16,67 ms, p99 igual, 0 atrasados).
  * Áudio: buffer de conversão reusado no `push_samples`; `drain_audio`
    do core devolve um `Vec` já com a capacidade do anterior (antes ~9
    realocações por frame).
  * Canvas: `FrameProcessor::process_packed` faz o readback direto no
    `Vec` da resposta IPC (cabeçalho + RGBA) — sem a 2ª cópia do frame
    inteiro (8 MB/frame em 1080p). Teste de GPU compara com `process`
    (largura 50 → exercita o padding).
  * Tempo de jogo: `roms.play_time_secs` (migration 0010) + `play_clock.rs`
    (amostra 1×/s, só conta `Running`, grava a cada 10 s e quando para /
    troca de ROM). `play_time_at_save` dos save states preenchido; página
    do jogo e menu de pausa mostram.
  * Tema: preset "Alto contraste" (base `teamsHighContrastTheme`; teste
    confere AAA 7:1 nos pares de texto) e escolha salva em
    `<dados>/appearance/theme.json` — o `localStorage` é por origem e
    perdia o tema entre dev/produção/Windows. Migra o valor antigo.
  * BIOS: Amiga, Atari 5200, Atari 8-bit, MSX e DS em `domain::bios`,
    conferidos nos docs E nos `.info` (20 MD5, comparados por script).
    Nenhum obrigatório: cada sistema tem um core no catálogo que roda sem.
  * Teste intermitente `pause_freezes_emulation_then_resume`: corrida real
    — `SetPaused` era "dispara e esquece" e um `FrameReady` do frame em
    andamento chegava depois do retorno. Agora o filho confirma
    (`ToParent::PausedAck`) e o pai espera. Sob 16 núcleos ocupados: antigo
    **6/40 falhas**, novo **0/40**.

- **2026-09-24 (noite) — Windows: dev não subia**:
  * `beforeDevCommand` compilava o core-host ANTES do Vite (mudança da manhã)
    e o `cargo tauri dev` só espera o Vite 180 s (tauri-cli 2.11.5,
    `dev.rs`, 90 × 2 s) → num clone novo no Windows estourava com `Could
    not connect to http://127.0.0.1:1420`. Agora `scripts/dev-before.mjs`
    (via `pnpm -w run tauri:before-dev`) sobe o Vite na hora e compila o
    core-host em paralelo; o `cargo run` do app espera a trava do `target/`,
    então a ordem continua garantida. Validado no Linux (ordem + nada sobra
    rodando ao encerrar).
  * Surface nativa fora do Linux ligava o wgpu no HWND da janela, atrás da
    janela filha do WebView2 (jogo invisível; `set_hidden`/`show` são no-op
    fora do Wayland). Padrão agora é `<canvas>` fora do Linux
    (`REEMU_NATIVE_VIDEO=1` força).
  * `scripts/check-windows.sh`: `cargo check` cruzado pra Windows com
    compilador C falso nos build scripts; job novo no CI.
  * STEP_BY_STEP › Windows: pré-requisitos exatos (MSVC, toolchain `-msvc`)
    e tabela de erros comuns.

- **2026-09-24 (noite) — catálogo de cores 69 → 124**: `scripts/gen_core_catalog.py`
  gera as entradas a partir do `libretro-core-info` oficial, cruzando com a
  listagem do buildbot e com o `library-scan`. Entra o core que: é da
  categoria `Emulator`; existe no buildbot de Linux E Windows; tem um
  sistema (campo `database` do `.info`) que o scan reconhece pela tabela de
  pastas (`system_from_folder_name`) ou é arcade; e roda em software ou
  OpenGL desktop. Fora: Vulkan/Direct3D/GLES-only e `hw_render = true` sem
  API declarada (só o Beetle PSX HW foi validado em Vulkan, doc 12);
  `play_libretro` (PS2 em GL sem declarar) e `mesen2_libretro` (nome
  gigante, sistemas já cobertos) por exclusão manual. Os 110 zips (55 × 2
  SOs) conferidos por HTTP `Range` (têm o `<id>.so`/`.dll` que o
  `download()` procura); amostra de 12 cores baixada e carregada via
  `retro_api_version`/`retro_get_system_info` sem erro. Aba Catálogo ganhou
  filtro por nome/sistema.

- **2026-09-24 (tarde) — shaders do upstream, capas no Windows, release**:
  * Upstream `libretro/slang-shaders@afb1416` tinha derrubado a validação de
    campo pra 92,9%. Duas correções no `shader-slang`, cada uma com teste
    unitário que falha sem ela:
    - koko-aio 1.9.101 (176 presets): `#define FPS_ESTIMATE_PASS
      avglum_passFeedback` + `uniform sampler2D FPS_ESTIMATE_PASS` atrás de
      `#if FPS_ESTIMATE_PASS != avglum_passFeedback`. O
      `split_sampler_aliases` pulava nomes já declarados como global; agora
      o `#define` prevalece (pro preprocessador a decl declara o ALVO), o
      `#if` fica com identificadores não definidos (`0 != 0`), como no
      RetroArch.
    - vectorscale novo (4 presets): `findLSB`/`findMSB` de `uint` devolvem
      `int`, o naga tipa como `uint` → `InvalidStoreTypes`. Helpers
      `reemu_findLSB`/`reemu_findMSB` em `patch_missing_builtins`.
    - Resultado: 2651/2658 = 99,7%, zero regressão contra a lista anterior
      (todos os 2547 continuam), `working-presets.txt` atualizado (+104).
  * Capas no Windows: `boxart` era `cover://localhost/<id>`, mas o WebView2
    só intercepta `http://cover.localhost/` pra subrecurso (conferido no
    `custom_protocol_workaround` do wry 0.55.1). `covers::cover_url` escolhe
    por plataforma. Não testado em Windows real.
  * **IPC com `wmem_max` padrão**: com o CI verde no rustfmt (vermelho desde
    pelo menos 2026-09-18, então os testes não rodavam lá), apareceu
    `roundtrips_a_2mb_message_inline` falhando. No padrão do Linux
    (`net.core.wmem_max` = 212992) o `SO_SNDBUF` fica em ~416KB e qualquer
    mensagem inline maior dava `EMSGSIZE` — save state de SNES (~800KB)
    inclusive. Esta máquina tem `wmem_max` = 4MB, por isso nunca apareceu
    aqui. `send` agora cai pro memfd no `EMSGSIZE`; teste novo força o
    buffer pequeno. Suíte inteira rodada com o buffer simulado do CI: ok.

- **2026-09-24 — revisão geral + melhorias fora do backlog**:
  * `cargo fmt --check` falhava em 20 arquivos (CI vermelho no 1º passo) →
    `cargo fmt --all`; hook `.githooks/pre-commit` (rustfmt + oxlint, ativar
    com `git config core.hooksPath .githooks`).
  * `beforeDevCommand` agora compila o `reemu-core-host` — `cargo tauri dev`
    direto funciona (antes só via `scripts/dev.sh`).
  * `STEP_BY_STEP.md` reescrito (era do scaffold): deps de sistema iguais às
    do CI (`libudev-dev`/`libasound2-dev` incluídos), fluxo dev atual.
  * **CSP** definida em `tauri.conf.json` (era `null`); violações vão pro log
    via `js_log` (`main.tsx`). Validada num build de produção (`tauri://`):
    `cover://`, `https:`, `blob:` e `data:` passam, origem não listada é
    bloqueada; `dangerousDisableAssetCspModification: ["style-src"]` porque
    o Griffel injeta `<style>` em runtime (nonce desligaria o
    `'unsafe-inline'`).
  * **Senha do ScreenScraper no chaveiro do SO** (`credentials.rs`, crate
    `keyring` 3) em vez de texto puro no SQLite; senha antiga migra na
    primeira leitura; sem chaveiro, cai no banco como antes (com `warn!`).
  * **Vitest no frontend** (17 testes: `shelf`, `platform`, `initials`,
    `useThemeStore`) + passo `test` no CI. Um dos testes confere que todo
    `system_id` de `library-scan/src/systems.rs` tem rótulo em
    `lib/platform.ts`.
  * `commands.rs` (2719 linhas) → `commands/` (10 módulos por área);
    `gpu.rs` (5062) → `gpu/` (`mod`, `chain`, `input`, `specs`, `pipelines`,
    `textures`, `vk_blit`, `tests`). Só mudou onde o código mora, a lógica
    é a mesma: a comparação linha a linha contra o original mostrou só
    cabeçalhos, visibilidade `pub(super)` e reflow do rustfmt.
    Validação de campo depois da divisão: 2470/2658 = 92,9% contra o
    upstream atual, mas a queda vem do upstream (koko-aio 1.9.101 e reescrita
    do vectorscale). O mesmo código passa 173/179 no koko-aio de antes do
    sync e 2/2 no vectorscale antigo, os números do baseline. Ver TASKS.md.

- **2026-08-30 — Etapa 04 fatias 3c/4/5 + modo Xbox + fixes**:
  * **Shader por jogo/sistema no DB** (fatia 3c): `ShaderChainStore`
    (upsert/list/set/clear assignment) em `ShaderChainRepo`; builtins semeados
    no startup; `set_shader(name, scope, rom_id)`; `load_game` resolve em
    cascata rom→sistema→default. `RomDetail` tem `<Select>` de shader do jogo.
  * **Decoração / bezels** (fatias 4+5) — **VALIDADO pelo usuário 2026-08-30**:
    `scan_decoration_pack` (Bezel Project/RetroBat, deep-scan `WalkDir`),
    `DecorationStore` em `DecorationRepo`, `import_pack` casa stem→rom_id
    (contra TODAS as linhas de ROM que batem — a lib pode ter duplicata),
    composição no `gpu.rs` (jogo no viewport do `.cfg` ou centralizado 4:3 +
    bezel alpha-blend), exclusão mútua com shader que já traz moldura.
    Comandos `import_decoration_pack` / `clear_decorations` em Config › Vídeo.
    - **Bug 1**: biblioteca duplicada (2 drives, mesmo rótulo "Novo volume") →
      bezel casava a linha errada. Fix: `match_roms` grava pra todas.
    - **Bug 2 (o que travava)**: bezels do Bezel Project são **PNG paletado +
      tRNS**; `decode_png` só tratava RGB/RGBA → erro silencioso. Fix:
      `Transformations::EXPAND | STRIP_16` + normalização → RGBA8. +1 teste.
  * **Remover ROMs**: comando `remove_rom` (1) + `remove_rom_system` (snes/nes/
    …) + `remove_rom_source` (pasta de origem) + `clear_library`. UI: chip
    "Gerenciar biblioteca" na tela Meus jogos + "Remover sistema" no cabeçalho
    de cada seção.
  * **Modo Xbox (etapa 07)**: `/` virou **Início** (`Home`: hero + faixas
    "Continuar jogando" / "Adicionados recentemente"); a biblioteca completa é
    `/library` ("Meus jogos", grade vertical por sistema). Busca global (Y / `/`
    / campo centralizado), menu de contexto no cartão (☰ / clique-direito),
    dicas de botão cientes de contexto, `last_played_at` / `added_at` no
    `RomDto`. **`styles/xbox.css` migrado 100% pra Griffel** (`styles/xbox.ts`,
    `makeStyles` + `tokens`); doc novo `docs/design/fluent2.md`
    (<https://fluent2.microsoft.design/>).
  * **Foco do controle nos menus**: o anel só respondia a `:focus-visible`, que
    o WebKitGTK não marca no `.focus()` vindo de evento Tauri → foco movia
    invisível. Trocado por `:focus`; `focusNav` pula o campo de busca.
  * **Fatia 6 — parâmetros de shader na UI (2026-08-30)**: `FrameProcessor`
    guarda `param_meta` (dos `#pragma parameter`); `set_shader_param(name,val)`
    clampa e entra no uniform buffer sem rebuild. `ShaderChainStore` ganhou
    `set_parameter_override`/`clear_parameter_overrides` (tabela
    `shader_parameter_overrides` já existia; upsert por `assignment_id::key`).
    Comandos `get_shader_params` / `set_shader_param(scope?)` /
    `reset_shader_params(scope?)`. `apply_resolved_shader_ex` aplica os
    overrides do assignment ao carregar o jogo. Front: `<ShaderParams>`
    (sliders Fluent + "Restaurar padrões", debounce 200ms) em `SettingsVideo`
    (scope default) e `RomDetail` (scope rom; ganhou "Carregar .slangp…" por
    jogo). +1 teste (`shader_parameter_overrides_roundtrip`). **Etapa 04 →
    `done`.**
  - VERIFICADO: `cargo test --workspace --all-features` (todos ok) + `clippy -D
    warnings` limpos; `tsc -b` / `oxlint` / `vite build` limpos.
- **2026-08-27 — Setup local**: bootstrap completo. `packages/app-desktop`
  (Vite+React19+Fluent+Zustand+TanStack Query) e `apps/desktop/src-tauri`
  (`reemu-desktop`, linka `domain`+`db`). `cargo tauri dev` abre a janela.
- **2026-08-27 — Etapa 01 (`done`)**:
  - Migration `0001` corrigida: `UNIQUE(scope,system_id,rom_id)` (não previne
    duplicata no SQLite) → CHECK de forma + índices únicos parciais por escopo;
    índices em todas as FKs; `ON DELETE CASCADE`. `foreign_keys` ligado por
    conexão no `pool.rs`.
  * `domain`: novo `error::RepoError`; traits de DB viraram `async` (`#[async_trait]`,
    decisão do usuário); novos ports/models `library::{Rom,RomRepository}`,
    `audio::AudioConfigRepository`, `core_loader::{InstalledCore,InstalledCoreRepository}`,
    `core_options::CoreOptionsStore::replace_schema`.
  * `crates/db`: `pool.rs`, `cascade.rs` (resolução rom→system→default genérica,
    1 função pros 2 casos), `convert.rs` (mapa enum↔CHECK do banco). Repos:
    `ShaderChainRepo`, `DecorationRepo`, `CoreOptionsRepo`, `AudioConfigRepo`,
    `InstalledCoresRepo`, `RomsRepo`, `SaveStateRepo`. **18 testes** de
    integração (SQLite in-memory) — cascata (3 escopos + none), FK/CHECK,
    upsert, cascade delete.
  * `save_state`: `SaveStateMetadata`/`SaveRamMetadata` ganharam `id`; port
    dividido — `SaveStateManager` (alto nível, core-loader, etapa 08) vs
    `SaveStateRepository` (só metadata, implementado agora).
  * **Fica pra depois (não bloqueia)**: métodos de *escrita* de assignment
    (criar/editar preset por rom/sistema) entram junto da UI (etapa 04/07);
    wiring dos repos no shell Tauri (managed state) é etapa 03.
- **2026-08-27 — Infra**: `rust-toolchain.toml` (1.97), `rustfmt.toml`,
  `.editorconfig`, `LICENSE` (MIT), `.github/workflows/ci.yml` (fmt/clippy
  `-D warnings`/test + oxlint/build do frontend). `cargo fmt --all` aplicado no
  workspace. Nada commitado ainda (a pedido).
- **2026-08-27 — Etapa 02 (`done` no caminho software)**: crate
  `crates/core-loader-desktop`. HW render GL (passo 4) fica como próximo item
  fora do MVP-software (Vulkan já era backlog/etapa 12); o `Renderer` tem o
  encaixe `FrameOrigin::HardwareTexture` pronto.
  - FFI libretro em `src/sys.rs` (valores/layout conferidos contra `libretro.h`
    do RetroArch — structs `retro_system_av_info`/`retro_hw_render_callback`,
    enums, `RETRO_ENVIRONMENT_*`).
  * `RawCore` (libloading + símbolos `retro_*`), `ffi_state` (estado global +
    callbacks `extern "C"` — libretro é **um core por processo**, sem userdata
    nos callbacks), `DesktopCore` (= `FrameSource`, cada `next_frame` roda um
    `retro_run`), `DesktopCoreLoader` (`CoreLoader` + `load_core` concreto).
  * **Caminho software-only completo**: dlopen → `retro_run` → `video_refresh`
    (buffer cru, repack sem padding) → `Frame::SoftwareRawBuffer`. `SET_HW_RENDER`
    é detectado, os requisitos persistidos (`InstalledCoreRepository`) e no cache,
    e o load recusado com `CoreLoadError::HwRenderUnsupported`.
  - Ponto de extensão de save state pronto (`request_save_state` /
    `poll_save_state` / `serialize_state`) — implementação real é etapa 08.
  - Teste: **core-fake em C** (`fixtures/testcore.c`, compilado pelo build.rs
    p/ .so) — 8 testes de integração (load/run/frames, save state, save RAM,
    core options, rejeição de HW core, um-por-processo, not-found).
  * **2026-08-28** — descoberta de cores + core options + save RAM:
    - `discover.rs` — `discover_cores(dir)` varre `*_libretro.<suf>`, espia
      `retro_get_system_info`/`retro_api_version` (sem `retro_init`).
    - `coreopts.rs` — parse de `SET_VARIABLES` (v0) / `SET_CORE_OPTIONS` (v1) /
      `SET_CORE_OPTIONS_V2` + variantes `_INTL`; `GET_VARIABLE`/`GET_VARIABLE_UPDATE`
      agora funcionam (`GET_CORE_OPTIONS_VERSION` → 2). API livre de thread:
      `core_options()` / `core_option_values()` / `set_core_option()` /
      `set_pending_core_option_values()` (valores do DB aplicados no load).
    - `DesktopCore::{save_ram, restore_save_ram}` — `retro_get_memory_data`/
      `_size(RETRO_MEMORY_SAVE_RAM)`.
  * **Próximo (fora do MVP-software)**: passo 4 — contexto GL real +
    callbacks (`get_current_framebuffer`, `get_proc_address`, `context_reset`),
    validado com um core GL real. Input (`input_state`) é stub aqui (etapa 05);
    áudio agora sai de verdade via `drain_audio` → `emu-session` → `CpalAudioSink`
    (etapa 06). `domain`: `frame_source` ganhou `SoftwarePixelFormat`;
    `core_loader` ganhou `SystemAvInfo` e o trait `LoadedCore` (substituiu o
    marker `LoadedCoreHandle`).
- **2026-08-27 — Etapa 06 (`in-progress`)**: `crates/audio-desktop`.
  * `rate_control.rs` — DRC como **função pura** (fração de buffer → fator de
    ajuste do resample, limitado a ±delta). 6 testes.
  * `sink.rs` — `CpalAudioSink` impl `domain::audio::AudioSink`: cpal 0.18,
    ring buffer, **resample linear de razão variável** (estado entre chamadas),
    fallback pro dispositivo padrão se o `output_device_id` salvo não existir
    (device por `DeviceId` persistente, não índice). 2 testes de resampler.
  * `domain::audio::AudioSink` perdeu `Send + Sync` (a `cpal::Stream` é `!Send`);
    `emu-session` recebe uma **factory `Send`** e constrói o sink na thread do
    core. `FocusController`/pause → `sink.pause()`/`resume()`.
  - App: lê o `AudioConfig` persistido no startup e passa pra factory.
    **Verificado**: o stream cpal abre neste sistema (sem erro).
  * **2026-08-30 — pacing + build otimizado** (travadas no GBA): o `core_loop`
    (session.rs) passou a pacear por **acumulador** (`next_deadline`) + **spin**
    no último ~1.2ms em vez de `thread::sleep` puro (que passa do ponto no
    Linux e causa microstutter, pior em cores com fps ≠ 60 como GBA 59.73).
    `poll_frame` era chamado ~250×/s → o fetch loop da `PlayScreen` agora
    espera ~11ms após pegar um frame. `Cargo.toml` raiz ganhou
    `[profile.dev] opt-level=2` + deps em `-O3` (sem isso o pipeline não
    sustenta 60fps em `cargo tauri dev`) + `[profile.release]` lto/1-cgu.
  * **2026-08-30 — aplicar ao vivo (etapa 06 `done`)**: `Command::ReloadAudio(
    AudioSinkFactory, reply)` + `EmuSession::reload_audio()` — recria o
    `AudioSink` na thread do core (dropa o stream cpal antigo antes). O comando
    `update_audio_config` persiste E chama `reload_audio` com a config nova
    (spawn_blocking). Muda device/sample rate sem recarregar o jogo. Toast
    "Áudio salvo e aplicado".
  - Falta (backlog): validar sessão longa sem glitch (core real + ouvir —
    usuário); resampler linear → `rubato` se a qualidade não bastar.
- **2026-08-27 — Etapa 05 (`in-progress`)**: input.
  * `core-loader-desktop`: `RetroPadState` global (atômico, por porta) fiado
    no callback `retro_input_state_t` (RetroPad digital). 2 testes.
  * `crates/input-desktop`: `sdl_db` — parser do SDL_GameControllerDB
    (swap Nintendo↔Xbox, `bN`/`hN.M`/`aN`), testado com string real de Xbox;
    `ComboHotkeyResolver` impl `HotkeyResolver` (combinação hold+press, combo
    vence tecla única); `KeyboardMap` + `web_code_to_retropad` (`KeyboardEvent.
    code` → RetroPad). ~11 testes.
  - App: comando `input_key` (Escape/F1 = hotkey de menu; senão teclado →
    RetroPad só em `GameFocused`); hook `useKeyboardInput` encaminha
    keydown/keyup. `FocusController` limpa o pad ao entrar no menu.
  * **2026-08-28** — `gilrs`: `input-desktop::GamepadPoller` (poll de gamepad
    físico numa thread de `emu-session`, `enable_gamepad`), `Button` normalizado
    → RetroPad (convenção libretro), 1ª controle = porta 0; botão `Mode` →
    toggle de menu (via `take_menu_request` no loop de eventos). +4 testes.
  * **2026-08-28** — UI de captura de binding: `input_desktop::capture` (flag
    global; enquanto ligada, teclado/gamepad vão pro frontend por
    `emit("raw-input-captured")` em vez de irem pro jogo). Comandos
    `start_binding_capture` / `cancel_binding_capture` / `save_binding`
    (`target` = `system_hotkey` | `controller_mapping`, ambos com combinação
    hold+press) / `list_system_hotkeys` / `clear_system_hotkey` /
    `list_controller_mappings`. `db::SystemHotkeysRepo` +
    `db::ControllerMappingsRepo` (trigger/layout serializados em JSON). +3
    testes. Frontend: `useBindingCaptureStore` (Zustand transitório, janela de
    ~300ms), `<BindingCapture>` (diálogo único), seção "Atalhos de sistema" em
    Settings.
  * **2026-08-28** — `HotkeyResolver` ligado ao DB em runtime:
    `input_desktop::held` (conjunto segurado global, teclado + gamepad),
    `AppState.hotkeys: Mutex<ComboHotkeyResolver>` semeado do `system_hotkeys`
    no startup (default `ToggleMenuOverlay` = `F1`; `Esc` fica hardcoded como
    rede de segurança). `commands::poll_hotkeys` roda a cada frame no loop de
    eventos ANTES do roteamento pro jogo (prioridade), dispara 1×/aperto:
    `ToggleMenuOverlay` alterna o foco, `QuickSave`/`QuickLoad` emitem
    `hotkey-action` (frontend só avisa por toast — falta contexto de ROM).
    `save_binding`/`clear_system_hotkey` recompõem o resolver. `FocusController`
    limpa o `held` na transição. +2 testes.
  * **2026-08-28** — mapeamento de controle do DB em runtime:
    `input_desktop::mappings` (override global lido pela thread de gamepad;
    `set`/`resolve`, combinação suportada). `GamepadPoller` agora recompõe o
    RetroPad por porta a cada evento a partir dos índices físicos segurados
    (`down` por gamepad + diff contra `applied`), usando o override do `guid`
    se houver, senão o mapa fixo do `gilrs`. `PollOutcome.gamepads`
    (`(guid, nome)` conectados) → `EmuSession::connected_gamepads()`. Comandos
    `list_gamepads` / `clear_controller_mapping`; `save_binding` e o startup
    republicam o override. Frontend `<ControllerMappings>` (seção "Controles"
    em Settings): junta gamepad conectado + mapa salvo, grade de 16 botões
    RetroPad com rebind/`+`, "Limpar mapa". +1 teste.
  * **2026-08-28** — fechamento da etapa 05:
    - stick esquerdo → d-pad (`GamepadPoller` trata `AxisChanged`, limiar 0.5,
      alimenta a mesma recomposição de RetroPad). +1 teste.
    - `QuickSave`/`QuickLoad` reais: `AppState.current_rom` (setado por
      `load_game(rom_id)`), `QUICK_SLOT = 0`, `poll_hotkeys` dispara uma task
      async que grava/restaura via `save_state.rs` e devolve toast por
      `hotkey-action {action, ok, message}`.
    - `device_port_assignment`: `domain::input::DevicePortRepository` +
      `db::DevicePortsRepo` (cria linha vazia em `controller_mappings` pro FK) +
      `input_desktop::mappings::{set_ports,port_for}` (poller consulta antes da
      ordem de conexão). Comandos `set_device_port`/`clear_device_port`/
      `list_device_ports`; `<Select>` de porta por controle na UI. +1 teste.
    - rótulos amigáveis do botão físico na UI (`describeRawInput`: "A (baixo)",
      "D-pad ↑", "Guia"…).
    - `<IdleScreen>` — tela cheia opaca quando `session_state == Idle` (a
      webview transparente só faz sentido com jogo rodando).
  * **2026-08-29 — validado em hardware (DualSense/PS5)**: jogo reconhece o
    controle (RetroPad OK), áudio OK. **Bug**: os menus não respondiam ao
    gamepad — o `useGamepadNav` dependia da Gamepad API do navegador, que o
    WebKitGTK 2.52 nesse setup não expõe (nunca dispara `gamepadconnected`).
    - Correção: navegação de menu passou a ser resolvida pelo **gilrs no
      backend**. `input_desktop::gamepad`: novo `NavPulse` (Up/Down/Left/Right/
      Confirm/Back) com edge-detection + auto-repeat (delay 380ms, repeat
      150ms); `PollOutcome.nav`. `emu-session`: `Shared.nav` +
      `EmuSession::take_nav_pulses()`. Shell: emite `menu-nav` no loop de
      eventos, **só quando `session.state() != Running`** (Idle no launcher,
      Paused no menu de pausa — durante o jogo o d-pad vai só pro RetroPad).
    - Frontend: `lib/focusNav.ts` (`moveFocus` extraído do hook), `onMenuNav`
      em `tauri.ts`, `useGamepadNav` escuta `menu-nav` em vez de pollar a
      Gamepad API, `PlayScreen` trata `menu-nav` no menu de pausa (confirm =
      clica, back = continua, setas = move foco; foca o 1º item ao abrir).
    - **2ª rodada**: a 1ª versão não funcionou — o event loop do Tauri fica em
      `Wait` quando a webview está ociosa, então o `MainEventsCleared` (onde a
      ponte de input rodava) não tiquetaqueia no launcher. Movido pra uma
      **thread dedicada `spawn_input_bridge`** (~60Hz, independente do loop);
      d-pad-como-eixo (DualSense) agora tratado (`Axis::DPadX/DPadY` → `hat`).
    - Cartões da Library "piscando": `QueryClient` com `refetchOnWindowFocus:
      false` + `staleTime: 5min` (o WebKitGTK dispara foco espúrio).
    - **3ª rodada**: menu de pausa ainda não navegava — `moveFocus` filtrava
      por `offsetParent` que o WebKitGTK zera dentro de `position: fixed`.
      Trocado por `getBoundingClientRect()`. Gate `state != Running` removido
      da emissão do `menu-nav`. **Validado com DualSense**: launcher, seleção
      1-a-1 e menu de pausa todos navegando pelo controle.
  * **2026-08-29 — 2 correções menores**:
    - `load_game` agora registra o core em `installed_cores` (via `get`+
      `register`) antes do `replace_schema` — a FK de `core_options_schema`
      falhava porque a descoberta por disco não persistia nada (WARN
      "salvando schema de core options: FOREIGN KEY constraint failed").
    - Splashscreen: `PlayScreen` no estado `loading` agora mostra capa (ou
      iniciais) + título + sistema + spinner, no lugar do spinner solto.
      `RomDetail` passa `boxart`/`system` no state da navegação; `initials`
      extraído pra `lib/initials.ts` (compartilhado com `GameCard`).
  * **2026-08-29 — Etapa 04 fatia 1 (caminho GPU)**: usuário escolheu o
    pipeline slang completo do doc. Como a surface nativa está desligada nesse
    ambiente, o wgpu roda **headless** (sem surface → sem conflito com o GTK).
    `src-tauri/src/gpu/` — `FrameProcessor` (contexto wgpu + `video_surface::
    Renderer` + alvo offscreen + readback). `poll_frame` passa o frame por ele
    (blit 1:1, passthrough) e cai no CPU (`to_rgba8`) em qualquer falha.
    `AppState.gpu: Mutex<Option<FrameProcessor>>`, init no setup do `lib.rs`.
    Dep `wgpu = "30"` no shell. Verificado: adapter Vulkan (RTX 3060) sobe
    headless sem crash. `REEMU_NO_GPU=1` desliga (volta pro caminho CPU).
    - **1ª tentativa distorceu** (usar `video_surface::Renderer` aqui aplicava
      `letterbox_scale` — pra SNES, squish vertical + tarjas). Reescrito como
      **blit 1:1 próprio** em `gpu.rs` (fullscreen-triangle + `textureSample`,
      sampler nearest, sem uniforms/escala) — é a primitiva de cada passe da
      cadeia multi-passe. A proporção continua sendo do `<canvas>`/CSS.
    - **Fatias 2+3 (2026-08-29) — motor multi-passe + parser `.slangp`**:
      - `crates/shader-slang` (NOVO, membro do workspace) — parser isolado do
        `.slangp` (`parse_slangp`/`parse_slangp_file`, segue `#reference`,
        scale_type x/y, wrap, alias, parâmetros, texturas do usuário). 6 testes.
      - `gpu.rs` reescrito como **cadeia multi-passe**: N passes fullscreen-
        triangle, cada um amostrando a saída do anterior, com escala/filtro por
        passe e uniforms semânticos (`source_size`/`output_size`/`orig_size`/
        `frame`). Presets embutidos em WGSL: `plain` (1p), `crt` (2p:
        sangramento H → scanline+máscara+vinheta 2x), `lcd` (1p, grade 2x).
        `REEMU_SHADER=` escolhe; `set_preset()` troca em runtime.
      - Comandos `get_shader_info` / `set_shader`; aba **Configurações › Vídeo**
        (`SettingsVideo.tsx`) com os 3 presets.
      - Verificado: `plain` e `crt` (2 passes) compilam WGSL e sobem sem erro.
      - **Fatia 2b (2026-08-29) — compilador `.slang`** em `crates/shader-slang`:
        `preprocess.rs` (`#include` c/ guard, split de estágios, `#pragma
        name`/`parameter`) + `compile.rs` (rewrite push_constant→UBO e
        `sampler2D`→texture+sampler + call-sites → `naga` glsl-in/wgsl-out;
        `Feedback`/`OriginalHistory` → `Unsupported`). **11 testes**. Falta o
        wiring no `gpu.rs` (reflection do bloco uniforme) — fatia 3b.
      - **Tela cheia**: `tauri.conf.json` `fullscreen: true`; comandos
        `is_fullscreen`/`set_fullscreen`; `useFullscreen`/`useFullscreenSync`
        (F11 global), botão na topbar + no menu de pausa.
      - Tuning CRT/LCD depois dos prints do usuário (CRT 3x, máscara mais leve).
      - **Fatia 3b (2026-08-29) — `.slangp` roda na cadeia**: `gpu.rs` reescrito
        com quad em vertex buffer (Position vec4 + TexCoord, triangle-strip);
        builtins e slang no mesmo motor. `UniformMode::{Fixed, Slang(layout)}` —
        no modo slang o buffer é montado por reflection (`compile.rs::reflect`
        via IR do naga: offsets/tipos dos campos) preenchendo `MVP` (ortho
        [0,1]→[-1,1]), `SourceSize`/`OriginalSize`/`OutputSize`/
        `FinalViewportSize`, `FrameCount`/`FrameDirection`, e parâmetros por
        nome (defaults do `#pragma parameter` + override do `.slangp`).
        `REEMU_SHADER=/caminho/x.slangp` ou botão "Carregar .slangp…" em
        Configurações › Vídeo (`pickSlangp`). **Verificado**: preset slang CRT
        de teste (`~/.local/share/com.reemu.desktop/shaders/test-crt.slangp`)
        compila, valida e sobe sem erro; builtins seguem OK.
      - **UBO + Push (2026-08-29)**: shaders slang do RetroArch têm 2 blocos
        uniformes. `reflect_all` → `Vec<(binding, layout)>`; `rewrite` força
        `Push`→binding 0, `UBO`→binding 3 (`declares_block` casa a palavra
        inteira; BGL do `gpu.rs` ganhou binding 3, `Pass.ubuf: [Buffer; 2]`).
        **Validado**: `scanline.slangp` real do RetroBat compila e sobe sem
        fallback (1 passe, 2 parâmetros).
      - Nota: `/` estava 100% cheio → limpei `target/debug/incremental` (19G).
      - **Fatia 3c (2026-08-30) — preset por jogo/sistema no DB**: domain
        `ShaderChainStore` (upsert_preset / list_presets / set_assignment /
        clear_assignment) impl em `ShaderChainRepo` (troca a atribuição do
        escopo via DELETE+INSERT numa tx). +1 teste (cascata + replace).
        Builtins semeados no startup (`seed_builtin_shader_presets`).
        `set_shader(name, scope, rom_id)` — `scope`: session / `default` /
        `rom` (name vazio = limpar). `get_rom_shader(rom_id)` → preset
        resolvido + `from_rom`. `load_game` chama `apply_resolved_shader`
        (cascata rom→sistema→default, senão `plain`). `FrameProcessor.
        preset_source` pro dedup. Front: `SettingsVideo` persiste como
        `default`; `RomDetail` tem `<Select>` "Shader deste jogo".
      - **Fatia 4+5 (2026-08-30) — decoração / bezels**:
        `library_scan::scan_decoration_pack` + `viewport_for_image` (convenção
        Bezel Project/RetroBat + `.cfg` `custom_viewport_*`, 2 testes); domain
        `DecorationStore` impl em `DecorationRepo`; shell `decoration.rs`
        (`import_pack` mapeia stem→`rom_id`, `decode_png` RGB/RGBA8); comandos
        `import_decoration_pack`/`clear_decorations`; `load_game` →
        `apply_resolved_decoration` (cascata); **exclusão mútua** (pula se o
        shader ativo tem `includes_bezel`); `gpu.rs` passe de composição (jogo
        no viewport do `.cfg` ou centralizado + bezel PNG alpha-blend);
        `LoadedGame.aspect_ratio` vira a da moldura. Front: import/remover em
        Config › Vídeo.
    **Próxima fatia**: (6) parâmetros de shader ajustáveis na UI (os
    `#pragma parameter` já são lidos + buffer montado por nome).
  * **Falta / follow-up**: caso de dois controles idênticos (mesmo GUID SDL →
    colidem na porta — precisa usar o `GamepadId` do `gilrs`); saída de eixo
    analógico pro RetroPad (hoje só stick→d-pad).
- **2026-08-27 — Etapa 08 (`in-progress`)**: save states.
  * `EmuSession.loaded_core()` — rastreia o id do core carregado (states não
    são portáveis entre cores).
  * `apps/desktop/src-tauri/src/save_state.rs` — orquestração (testável):
    `save` grava o `.state` em disco (caminho determinístico por slot, troca o
    anterior no mesmo slot) + `record_state`; `load_bytes` valida `core_id`
    (`CoreMismatch`/`NoCore`/`NotFound`); `list`/`delete` (arquivo + registro).
    **4 testes** de integração (SQLite in-memory + dir temp).
  - Comandos Tauri `save_state`/`list_save_states`/`load_save_state`/
    `delete_save_state`; wrappers no `lib/tauri.ts`.
  * **2026-08-28** — save RAM (battery): `emu-session` carrega
    `<saves>/<stem>.srm` no core logo após o load e regrava a cada 10s + no
    unload/troca/shutdown (`DesktopCore::{save_ram,restore_save_ram}`). 2 testes.
    QuickSave/QuickLoad no menu de pausa da `PlayScreen`.
  * **2026-08-30** — shutdown limpo: `RunEvent::ExitRequested` (`lib.rs`) faz
    `session.unload()` ao fechar (X da janela / Alt+F4 / `quit_app`), garantindo
    o flush final da `.srm` — antes só o flush periódico de 10s cobria.
    `flush_save_ram` virou escrita atômica (`.srm.tmp` + rename).
  * **2026-08-30 — thumbnail + painel (etapa 08 `done`)**: `poll_frame` guarda
    uma cópia throttled (1×/500ms) do último frame em `AppState.last_frame`
    (`CachedFrame`); no `save_state`/QuickSave o `thumbnail_png` (nearest →
    320px → PNG via crate `png`) é gravado ao lado do `.state` como `.png`.
    `SaveStateMetadata.thumbnail_path` (schema já tinha a coluna); `delete`
    remove os dois. `SaveStateDto.has_thumbnail` + comando
    `read_save_thumbnail` (PNG por IPC → blob URL). Front:
    `components/SaveStateThumb.tsx`, `RomDetail` lista com miniatura + "Jogar
    daqui" (`/play?loadState=<id>` → `PlayScreen` carrega o state após o boot),
    e o menu de pausa ganhou a lista completa de estados (miniatura + carregar).
    +0 teste novo (os 4 de `save_state.rs` seguem, com `None` no arg novo).
- **2026-08-27 — Etapa 09 (`in-progress`)**: `crates/library-scan`.
  * `hash.rs` — `FileRomHasher` impl `domain::metadata::RomHashService`:
    CRC32 (`crc32fast`) + MD5 (`md-5`) do arquivo, com **skip do header iNES**
    (`.nes`, 16 bytes). 3 testes (valores conhecidos de "hello world", skip do
    header, arquivo comum).
  * `systems.rs` — extensão → `system_id` (nes/snes/gba/megadrive/...).
  * `scan.rs` — `scan_into(repo, dir, now)`: varre recursivo, dedup por
    `file_path`, pula extensões desconhecidas, popula `RomRepository`.
    `ScanReport { found, added, skipped_known, skipped_unrecognized, errors }`.
    2 testes de integração (SQLite in-memory).
  * `domain::library::RomRepository` ganhou `list()`.
  - App: comandos `list_roms` / `scan_library(path)`; tela Library agora
    escaneia um diretório e lista de verdade (via TanStack Query).
  * **2026-08-28** — capas via **thumbnails da libretro** (MVP, sem API key):
    `library_scan::libretro_boxart_url(system_id, título)` monta
    `https://thumbnails.libretro.com/<Sistema>/Named_Boxarts/<Nome>.png`
    (mapa de ~15 sistemas + sanitização de nome no padrão RetroArch). `RomDto`
    ganhou `boxart: Option<String>`; `<GameCard>` e `RomDetail` mostram como
    `<img>` com fallback pras iniciais no `onerror`. Casa melhor com ROMs
    No-Intro. +1 teste. URL verificada 200 no CDN.
  * **2026-08-30 — etapa 07 fechada**: `RomDetail` reescrito no estilo "página
    de jogo do Xbox" (hero com arte/capa da metadata + scrim, título grande,
    badges de sistema/ano/gênero, descrição, botão Jogar/Continuar grande,
    seções em painéis: shader do jogo, opções do core, save states, remover).
    Menu de pausa da `PlayScreen` migrado pro estilo Xbox (`usePauseStyles`,
    painel escuro + anel de foco próprio já que `/play/*` fica fora do
    `.xb-app`). `styles/xbox.ts` += `useDetailStyles` / `usePauseStyles`.
  * **2026-08-30 — MetadataProvider real (ScreenScraper)**: migration
    `0003_metadata.sql` (`scrape_matches.candidate_json`, índices únicos por
    rom, `metadata_config` singleton). domain: `GameMetadata`, `ScrapeQuery`,
    `PendingMatch`, `MetadataConfig`; trait `MetadataProvider::search` +
    `MetadataRepository` (get/upsert metadata, record_match, rom_ids_without_match,
    list_pending, resolve_pending). `db::MetadataRepo`. Shell `scraping.rs`:
    `query_screenscraper` (`jeuInfos.php` por CRC + systemeid, `ssid`/`sspassword`
    opcionais; parse título/synopsis/dates/genres/medias), `scrape_pending`
    (task em background, delay 1.2s, cancelável, `ScrapeProgress` atômico).
    **Hash exato → `auto_matched` + metadata aplicada; qualquer coisa por nome →
    `pending_review`** (Abordagem B respeitada). Comandos
    `get/set_metadata_config`, `start_metadata_scan`, `metadata_scan_progress`,
    `cancel_metadata_scan`, `get_rom_metadata`, `list_pending_matches`,
    `resolve_pending_match`. Front: aba **Configurações › Metadata**
    (`SettingsMetadata.tsx` — credenciais, botão escanear + progresso, lista de
    revisão aceitar/rejeitar); `RomDetail` mostra título/descrição/ano/gênero/
    capa da metadata quando existe. +1 teste db (21 agora).
  * **Falta**: multi-provider (IGDB/TheGamesDB) + cascata; rate-limit por
    provider; match por MD5 além de CRC; UI: badge de "N pendências" no rail.
- **2026-08-27 — Etapa 07 (`in-progress`)**: casca da UI.
  * `packages/app-desktop/src/`: `lib/tauri.ts` (wrappers de comando/evento,
    toleram fora do Tauri), `hooks/useFocusBridge`, `components/{ToastLayer,
    MenuOverlay,CoreOptionsPanel}`, `screens/{Library (mock),Settings (real)}`,
    `App.tsx` compõe HUD + overlay + toasts.
  * `CoreOptionsPanel` gera os controles do schema (`CoreOptionDefinition[]`).
  - Backend: `AppState.db` (SqlitePool, migrations rodam no startup em
    `<app_data_dir>/reemu.db` — **verificado**, 17 tabelas). Comandos
    `get_audio_config` / `update_audio_config` / `list_installed_cores`.
  * `pnpm build` + `oxlint` limpos; `cargo tauri dev` sobe com SQLite + video.
  * **2026-08-28** — reestruturação em **rotas (modelo launcher, opção A)**:
    `react-router-dom` v7 + `createHashRouter` (webview, sem servidor).
    - `layouts/`: `RootLayout` (Outlet + `BindingCapture` + `ToastLayer`),
      `AppShell` (rail Biblioteca/Configurações/Cores, **fundo opaco**),
      `SettingsLayout` (abas → sub-rotas).
    - Rotas: `/` Library · `/rom/:romId` RomDetail (core picker + save states +
      "Jogar") · `/settings/{audio,hotkeys,controllers,cores}` · `/play/:romId`
      PlayScreen (**transparente**, HUD, Esc → menu de pausa com QuickSave/Load/
      Sair; `loadGame` on mount / `unloadGame` on unmount).
    - `Settings.tsx` quebrado em `screens/settings/*`; `MenuOverlay`,
      `IdleScreen`, `App.tsx` removidos. `lib/toast.ts` (`sysToast`).
    - Backend: comando `unload_game` (limpa `current_rom` + `session.unload()`);
      `load_game` ganhou `rom_id`. `index.css`: `body` transparente, cada rota
      pinta seu fundo.
  * **2026-08-28** — pré-requisitos de backend do frontend:
    - Diretório de dados **único** (`data_dir()` no `lib.rs` → SQLite + cores +
      saves + system no mesmo lugar; `dirs_or_temp` removido; `AppState` ganhou
      `cores_dir`).
    - `list_installed_cores` agora **varre `<dados>/cores/`** (`discover_cores`)
      e cruza com `installed_cores` (render backend). DTO ganhou `name` +
      `extensions`. `SettingsCores` + o core-picker do `RomDetail` usam isso
      (ordena por extensão que casa com a ROM).
    - Core options: comandos `get_core_options` / `set_core_option` (fonte da
      verdade = core carregado, senão DB); `load_game` semeia os valores salvos
      antes do load e persiste o schema depois. `<CoreOptions>` (Select por
      opção) no `RomDetail`. `CoreOptionsPanel` antigo removido.
  * **2026-08-28** — visual "modo Xbox" + catálogo de cores + navegação por
    controle:
    - `styles/xbox.css` — linguagem visual (rail de ícones, topbar com relógio,
      cartões arredondados, anel de foco forte). `AppShell` reescrito;
      `<GameCard>` / `<ButtonHints>` / `useClock`. `Library` agrupada por
      sistema em grade estilo Xbox.
    - **Catálogo de cores (etapa 10)**: `core_catalog.rs` — cores do buildbot
      oficial (`<stem>.so.zip` → extrai o dylib pra `<dados>/cores/`). Comandos
      `list_core_catalog` / `download_core` / `remove_core` (deps `reqwest`
      rustls + `zip`). Aba **Cores** em Settings ("Instalados" + "Catálogo").
      **2026-08-30 — ampliado pra 68 cores** cobrindo ~todos os sistemas (NES→
      PS2, home computers, arcade, fantasy consoles). `CatalogEntry.hw`:
      `Software` (roda hoje) ou `OpenGl` (baixa, mas `load_game` recusa até o
      contexto GL da etapa 02 — badge "precisa de GPU" na UI, ordenados por
      último). Cores exclusivamente Vulkan ficam de fora até a etapa 12.
      **Etapa 10 → `done`.**
    - **Navegação por controle**: `useGamepadNav` — setas do teclado sempre +
      Gamepad API do navegador (lazy, só após `gamepadconnected` — o WebKitGTK
      reclama se pollarmos sem gamepad). Foco geométrico (bom pra grade), `A`
      clica, `B` volta. Montado no `AppShell`.
  * **Bug resolvido — a webview nunca renderizou nessa máquina** (duas causas):
    (1) Vite 8/Rolldown travava o dev server → **Vite 7.3 + plugin-react 5**;
    (2) a **child window X11 pro vídeo** + `GDK_BACKEND=x11` fazem o WebKitGTK
    2.52 (NVIDIA/XWayland) montar o DOM mas não pintar → **padrão agora é sem
    child window X11**, GTK usa Wayland nativo (`REEMU_X11_VIDEO=1` volta o
    esquema antigo). O vídeo do jogo passou a ser **`<canvas>` na webview**
    (comando `poll_frame` → RGBA8; `to_rgba8` moveu pro `domain`). Confirmado
    visualmente pelo usuário: launcher renderiza.
  - Falta: **confirmação visual do vídeo** (precisa de core + tela; já tem 3
    cores instalados: fceumm/snes9x/gambatte); scraping/metadata (09); shader
    (04); thumbnails de save state; polir RomDetail/PlayScreen no estilo Xbox;
    caçar o ruído do WebKit no dev.
- **2026-08-27 — Etapa 03 (`in-progress`)**: spine testável do shell.
  - Novo crate `crates/emu-session`: `EmuSession` roda o core numa **thread
    dedicada** (`emu-core-loop`) com API de comandos (`load`/`unload`/
    `set_paused`/`save_state`/`restore_state`, todos round-trip) e saída por
    buffers compartilhados (`take_latest_frame`, `drain_audio`, `frame_seq`).
  * `FocusController` implementa `domain::focus::FocusManager` — `toggle()` e
    `set()` pausam/resumem a `EmuSession` na transição `GameFocused ⇄
    MenuFocused` (o core congela, para de produzir áudio). 5 testes (frames
    avançam, pause congela, resume, save/restore, foco pausa).
  * `apps/desktop/src-tauri`: `commands.rs` — `AppState` (`EmuSession` +
    `Mutex<FocusController>` como managed state), comandos `toggle_focus`
    (emite evento `focus-changed`), `load_game` (spawn_blocking),
    `current_focus`, `session_state`. `cargo tauri dev` continua abrindo.
  - Novo crate `crates/video-surface`: `Renderer` wgpu — sobe o
    `SoftwareRawBuffer` numa textura (conversão RGB565/0RGB1555/XRGB8888 → RGBA8
    em `convert.rs`) e desenha um quad com letterbox (shader.wgsl). 4 testes:
    3 de conversão + 1 **render headless real** (upload → render p/ textura
    offscreen → readback → confere a cor). `examples/play.rs` — player
    standalone winit+wgpu (`cargo run -p video-surface --example play`), roda o
    core-fake e mostra a cor mudando; Espaço pausa. Rodou OK localmente.
  - Integração na janela do Tauri: `video-surface::WindowTarget` (surface a
    partir de raw handles) + `apps/desktop/src-tauri/src/video.rs` +
    render na thread principal a cada `RunEvent::MainEventsCleared` + resize.
    Janela agora é `transparent: true`. Feature `dev-autoload` (env
    `REEMU_DEV_CORE`/`REEMU_DEV_ROM`) pra testar sem UI de biblioteca.
  * **Descoberta Linux**: `wgpu::Surface` Vulkan na `wl_surface` da janela
    GTK (com ou sem webview) = `Gdk-Message: Error 71 (protocolo)`, crash — o
    GTK é dono da submissão de buffer daquela surface.
  * **Solução Linux — child window X11** (`video-surface::window_target` +
    `apps/desktop/src-tauri/src/video.rs` mod `x11`): sob X11/XWayland,
    `XCreateSimpleWindow` filha do XID do GTK + `XLowerWindow` (atrás da
    webview) + wgpu Surface nela. `main.rs` força `GDK_BACKEND=x11` +
    `WEBKIT_DISABLE_DMABUF_RENDERER=1`. **Verificado**: child window criada
    (`0x1400001`), wgpu Vulkan anexado, `cargo tauri dev --features
    dev-autoload` roda ~60s sem crash e sem erro de WebKit. `video-surface`
    usa present mode Mailbox/Immediate (não bloqueia a thread do event loop).
  - Windows/macOS: surface direto no handle da janela principal (webview
    transparente compõe) — código pronto, **não verificado**.
  * **2026-08-30 — readback com pipeline** (`gpu.rs`): o readback GPU→CPU fazia
    `device.poll(wait_indefinitely)` — bloqueava a thread do Tauri e serializava
    CPU/GPU (sem pipeline). Agora `ReadbackRing` com 2 staging buffers: o frame
    N copia pro slot N%2 + `map_async`, e lê o slot do frame anterior (já
    mapeado) sem bloquear (`poll(Poll)`). Fallback bloqueante só se o slot ainda
    não mapeou (raro). Atraso de exatamente 1 frame; sem stall no caminho
    normal. +1 teste (`pipelined_readback_has_one_frame_delay`, headless wgpu).
  * **2026-08-30 — Etapa 03 `done` (desvio aceito)**: a surface nativa não
    funciona no WebKitGTK+NVIDIA desse setup (`Gdk Error 71` na `wl_surface`;
    child X11 não pinta). O vídeo do desktop é um **`<canvas>` na webview**:
    `poll_frame` (RGBA8 por IPC, corpo vazio = sem frame novo) + `PlayScreen`
    com dois loops desacoplados (fetch async → ref; paint rAF sincronizado com
    vblank; 3ms rodando / 120ms pausado; freeze automático no menu). Resize =
    CSS. Critério de pronto atendido (SNES validado; foco pausa/resume áudio +
    frame). Caminho nativo fica no código (`REEMU_X11_VIDEO=1`, `#[cfg(not(
    linux))]` Win/macOS) sem verificação. Ver `docs/ai-context/03`.
    Follow-up não-bloqueante: medir latência canvas vs nativo; passo 4 da
    etapa 02 (contexto GL, `FrameOrigin::HardwareTexture` tem o encaixe).

## 2026-09-25 — Erros explicados e toasts com botão de fechar

- `lib/errors.ts` (`describeError`) traduz o erro cru em título ("o que aconteceu"), dica ("o que fazer") e, quando existe, um botão para a tela que resolve (Cores, BIOS, Metadados, Biblioteca). O texto original fica em "Detalhes técnicos" / "Copiar detalhes" (heurística 9 de Nielsen). Todos os toasts `Falha: ${e}` passaram para `errorToast`, a tela de erro do jogo e o diálogo de atualização usam o mesmo classificador.
- `core-host não respondeu (timeout)` era lido como falta de internet porque a regra de rede casava qualquer "timeout". Agora ele tem regra própria antes da de rede ("O emulador travou ao abrir o jogo"), e a regra de rede só casa timeouts de rede.
- Todo toast tem um X para fechar. Toast só informativo (sem botão nem progresso) some em no máximo 5 s.

## 2026-09-25 — Fundo dos temas com deriva lenta (só Windows)

- Os 4 brilhos do `AnimatedBackground` deslizam alguns vmax e mudam de escala em até 8%, em ciclos de 38 a 54 s que vão e voltam (`alternate`). As cores não mudam.
- A animação usa só `transform`, que o WebView2 anima direto no compositor da GPU, sem layout nem repintura. Fonte: web.dev, "Stick to compositor-only properties and manage layer count": só `transform` e `opacity` têm essa garantia, e cada camada extra custa memória de GPU. Por isso não há `will-change` nem blur.
- **Só no Windows.** No Linux o fundo continua estático, porque o WebKitGTK roda sem compositing com NVIDIA proprietário (`src-tauri/src/main.rs`) e repintaria a tela toda a cada quadro.
- A animação para com "reduzir movimento" do sistema (`prefers-reduced-motion`, MDN). Com a janela sem foco ou minimizada ela fica pausada com `animation-play-state: paused`, que retoma de onde parou (MDN).
- Na tela de jogo o fundo não existe: `/play` fica fora do `AppShell`, então o custo durante o jogo é zero.

## 2026-09-25 — Aparência: um card por tema, Switch "Claro" e tema Super Nintendo

- Os pares escuro/claro (Verde Xbox/Claro, Azul PlayStation, PlayStation Clássico, Alva, Super Nintendo) viraram **um card cada**, com um `Switch` "Claro" (Fluent 2) no rodapé. Os `ThemeId` salvos continuam os mesmos, só agrupados em `THEME_FAMILIES` (`styles/themes.ts`), com teste garantindo que cada tema está em uma família. Os cards são `Card` do Fluent.
- Cada card guarda o próprio modo. No card selecionado o Switch aplica na hora; nos outros ele só troca a prévia daquele card, e clicar no card aplica o tema no modo mostrado. Um card não muda por causa de outro.
- O papel de parede foi para uma coluna à direita dos temas (`Card` com prévia 16:9). Em janela estreita (até 960 px) ele desce para baixo dos temas.
- **Tema Super Nintendo** (`snes` / `snes-claro`): marca no roxo dos botões A/B do SNES americano, indo até o lavanda de X/Y. Os 4 brilhos do fundo são os botões do Super Famicom/PAL na posição do losango: X azul, A vermelho, B amarelo, Y verde. O escuro usa o grafite das peças escuras do console; o claro, o cinza do corpo. Referência das cores dos botões: artigo "Super Nintendo Entertainment System controller" (Nintendo Wiki/Fandom).

## 2026-09-25 — GBA (VBA-M) caía ao abrir; interface de log libretro

- **Sintoma:** "Sem conexão com a internet" ao abrir Ace Combat Advance. O texto era do classificador (corrigido antes). A mensagem real era `core-host não respondeu (timeout)`.
- **Causa:** o `vbam_libretro` morria com SIGSEGV no `retro_init`, reproduzido fora do app com o `open_core` do loader e um backtrace no gdb. Depois que o ReEmu passou a anunciar `GET_INPUT_BITMASKS` (hoje cedo), o VBA-M chama `log_cb(...)` sem checar se é nulo (`src/libretro/libretro.cpp`, `retro_init`). Como o ReEmu não entregava `GET_LOG_INTERFACE` (27), `log_cb` ficava nulo.
- **Correção:** `GET_LOG_INTERFACE` implementado (`struct retro_log_callback`, libretro.h). O callback é variádico (printf), e Rust estável não define função C variádica, então `src/log_shim.c` (compilado pelo `cc` no build.rs) formata a mensagem e chama `reemu_core_log`, que manda para o `log` com target `core` e o nível do `retro_log_level`. O core falso (`fixtures/testcore.c`) agora imita o VBA-M e usa o log sem checar, então qualquer teste que carregue o core falso quebra se a interface sumir.
- Quando o core-host morre durante o Load, a sessão agora diz "o core encerrou inesperadamente ao carregar o jogo (signal …)" em vez de "timeout", e o frontend mostra "O emulador fechou inesperadamente".

## 2026-09-25 — Tema Super Nintendo refeito com as cores do console; aba Aparência larga

- O tema SNES agora usa só as cores do console americano (foto de referência do usuário): marca no roxo-azulado das chaves POWER/RESET, fundo com o lavanda de X/Y, o roxo de A/B, o cinza das partes rebaixadas e o roxo das chaves. O claro é o cinza-lavanda do corpo; o escuro, o grafite do direcional.
- A aba Aparência entrou nas abas largas do `SettingsLayout` (antes limitada a 640 px, o que espremia os cards e cortava o Switch). A coluna do papel de parede quebra pela largura disponível (flex-wrap), não pela largura da janela.

## 2026-09-25 — Teste de fumaça do catálogo (fase 1) e 3 cores que não abriam

- **O teste:** `reemu-core-host --probe <pasta> <core>` abre o core sem jogo: API, `get_system_info`, `set_environment`, `retro_init` e as informações do sistema, sem `retro_deinit`. O VBA-M cai no `deinit` sem jogo, e o processo é descartável de qualquer jeito. O teste `catalog_smoke` (`core_catalog.rs`, `#[ignore]`) baixa cada core pelo mesmo `download` do app e roda o probe em processo separado, com limite de 60 s. A tabela sai no resumo do job, e há uma lista `KNOWN_BROKEN` para falhas explicadas. O workflow `catalog-smoke.yml` roda toda segunda-feira em Linux e Windows, e também dá para disparar à mão só com alguns cores.
- **Primeira rodada (Linux):** 123 de 127. Depois das correções abaixo: **126 de 127**.
- **Ordem de inicialização igual à do RetroArch** (`runloop.c`, `runloop_event_init_core` + `core_init_libretro_cbs`): `get_system_info` → `set_environment` → `retro_init` → demais `retro_set_*`. O libretro.h permite os callbacks antes do `init`, mas na prática os cores seguem o RetroArch. **RustyNES:** a `rust-libretro` só cria a instância no `get_system_info` e entra em pânico no `set_environment` sem ela. **Mesen:** caía num `set_video_refresh` chamado antes do `init`.
- **melonDS, pilha executável:** o binário do buildbot marca `PT_GNU_STACK` com `PF_X`, e a glibc ≥ 2.41 recusa o `dlopen` ("cannot enable executable stack"). Fontes: glibc NEWS 2.41/2.42 e o manual ("Dynamic Linking Tunables", `glibc.rtld.execstack`). O modo `glibc.rtld.execstack=2` é o de compatibilidade para `dlopen` de módulos que exigem pilha executável. O app lê o program header do core (System V gABI, ELF64 LE) e liga o tunable **só no processo filho** desse core (`core_loader_desktop::exec_stack_env`), nunca no app.
- **ep128emu:** continua caindo, numa thread de emulação que o próprio core sobe no `retro_init` sem jogo e sem as ROMs opcionais do Enterprise. O binário não tem símbolos; ficou em `KNOWN_BROKEN`.
- **Fase 2** (carregar ROMs de teste públicas) continua em TASKS.md.

## 2026-09-25 — Interop GL: fence no lugar do `glFinish`, e o interop virou padrão (Linux)

- **Antes:** o core GL no processo filho parava a thread em `glFinish` a cada quadro antes de entregar o `dma_buf`, e o interop inteiro era opt-in (`REEMU_GL_INTEROP=1`) por causa de uma tela preta com o parallel_n64.
- **Produtor (core-host):** fence nativa `EGL_ANDROID_native_fence_sync`. `eglCreateSyncKHR(EGL_SYNC_NATIVE_FENCE_ANDROID 0x3144, FD 0x3145 = -1)`, depois `glFlush` (o fd nasce no flush, pela spec), `eglDupNativeFenceFDANDROID` (a cópia é nossa) e `eglDestroySyncKHR`. Fonte: registro Khronos, `EGL/extensions/ANDROID/EGL_ANDROID_native_fence_sync.txt`. O fd de `sync_file` vai no `FrameReady` (`FrameKind::Hardware { sync }`), depois do fd do plano, pelo `SCM_RIGHTS`. Sem a extensão, volta ao `glFinish`. `REEMU_GL_SYNC=finish|fence|flush` continua para diagnóstico.
- **Consumidor (app):** o device wgpu abre com `VK_KHR_external_semaphore_fd` quando a GPU importa `SYNC_FD` (`vkGetPhysicalDeviceExternalSemaphoreProperties`), via `open_with_callback` do wgpu-hal 30. O fd é importado num semáforo binário com `vkImportSemaphoreFdKHR(TEMPORARY, SYNC_FD)`, porque esse tipo só aceita import temporário, com transferência por cópia, e o fd passa ao Vulkan no sucesso (spec Vulkan, capítulo de sincronização, "Importing Semaphore Payloads"). A espera vai para o próximo submit com `wgpu::hal::vulkan::Queue::add_wait_semaphore` (estágio `FRAGMENT_SHADER`). O semáforo só é reimportado depois que o submit terminou (`on_submitted_work_done`, VUID-vkImportSemaphoreFdKHR-semaphore-01142). Sem a extensão, o app faz `poll()` no fd; o `sync_file` fica `EPOLLIN` quando sinaliza (kernel, `drivers/dma-buf/sync_file.c`).
- **A tela preta de 2026-09-12 era outra coisa:** o filho só manda o plano `dma_buf` no primeiro quadro de cada slot. Se esse quadro era substituído em `latest_frame` antes de o vídeo pegar, o fd fechava junto, o slot nunca era importado e os quadros dele sumiam em silêncio. Reproduzido com mupen64plus_next e GoldenEye: metade dos quadros não saía. Corrigido com `Shared::orphan_planes`, que guarda o plano e o entrega no próximo quadro do mesmo slot, com teste de regressão.
- **Validação no RTX 3060** (teste `gl_core_real_rom`, core e ROM reais pela sessão completa): parallel_n64 e mupen64plus_next (GoldenEye) e flycast (Metropolis Street Racer), com interop e sem interop, dando as mesmas contagens de quadros e quadros com imagem. O teste ponta a ponta `dmabuf_from_gl_producer_imports_correctly_into_wgpu` agora passa a fence de verdade. **Não verificado:** execução com as camadas de validação do Vulkan (não instaladas nesta máquina) e GPUs AMD/Intel.
- O interop agora é o **padrão no Linux**; `REEMU_GL_INTEROP=0` força a cópia pela CPU. Windows sem mudança (sem dma_buf, continua na cópia pela CPU).
- **Também corrigido no caminho:** o teste de fumaça grava a saída do probe num arquivo (o pipe enchia e travava o `pcsx_rearmed` no CI), e o teste de interop ficou só-Linux (quebrava a compilação dos testes no Windows).
- **Correção no mesmo dia, pelo teste do usuário (flycast):** com o interop por padrão, o jogo aparecia cortado no canto de baixo à esquerda. O buffer compartilhado tem o tamanho **máximo** do core (flycast: 853x853), e o quadro ocupa só `w`×`h` a partir da origem do GL (640x480). O passe de inversão copiava o buffer inteiro, e o caminho sem inversão usava a textura inteira. Agora o passe recorta `w/tw × h/th` e inverte o Y só quando o core é bottom-left, num alvo do tamanho do quadro. Coordenadas conforme a spec WebGPU ("Coordinate Systems": NDC com y para cima, uv (0,0) na primeira linha). Os testes com N64 não pegaram porque ali o máximo é igual ao quadro (640x480) e o teste só contava pixels acesos. O teste novo `interop_crops_frame_smaller_than_the_buffer` pinta só um retângulo 40x24 num buffer 64x64 e exige a saída 40x24 inteira na cor, com e sem inversão. Ele falha sem a correção (735 pixels fora do lugar).

## 2026-09-25 — Teclado: gatilhos L2/R2 no mapa padrão

- No flycast, o teclado não tinha os gatilhos do Dreamcast: o controle de DC usa L2/R2 do RetroPad como gatilhos analógicos (`get_analog_trigger` com `JOYPAD_L2/R2` em `shell/libretro/libretro.cpp`), e o mapa fixo do teclado só ia até L1/R1. Em jogo de corrida não dava para acelerar nem frear. Agora **E = L2** e **R = R2**, na mesma fileira de Q/W = L1/R1. O `input_state` já responde `RETRO_DEVICE_INDEX_ANALOG_BUTTON` com 0x7fff quando o botão está pressionado.
- **Continua pendente** (TASKS.md): tela para remapear o teclado e o analógico pelo teclado.

## 2026-09-25 — flycast não lia teclado nem controle: portas de controle ligadas depois do load

- **Sintoma:** no flycast, nem teclado nem controle funcionavam. Medido com um contador temporário: o core chamava `input_poll`, mas **nunca** `input_state`.
- **Causa:** o ReEmu ignorava o `SET_CONTROLLER_INFO` e nunca chamava `retro_set_controller_port_device`. O RetroArch chama, para cada porta declarada, depois de carregar o conteúdo (`CMD_EVENT_CONTROLLER_INIT` → `command_event_init_controllers`, em `command.c`/`retroarch.c`), com `RETRO_DEVICE_JOYPAD` para cada usuário ativo. O flycast mantém `device_type[]` em -1 até receber essa chamada (`shell/libretro/libretro.cpp`).
- **Correção:** o ReEmu guarda quantas portas o core declarou (array de `retro_controller_info` terminado num elemento zerado, conforme o `libretro.h`, com teto de 16) e, depois do `retro_load_game`, chama `retro_set_controller_port_device(porta, RETRO_DEVICE_JOYPAD)` para as portas 0..min(declaradas, 4). Com isso o flycast passou a ler os botões (`JOYPAD_MASK`), os dois analógicos e os gatilhos analógicos em todas as portas.
- Teste `declared_ports_get_a_joypad_after_load`: o core falso declara 2 portas e registra quais o frontend ligou.

## 2026-09-25 — flycast: som que desafinava e secava depois de cada carregamento; gatilhos do DualSense

- **Medido** com o teste `gl_core_real_rom` + `REEMU_TEST_AUDIO=1 REEMU_AUDIO_DEBUG=1`, MSR copiado para o SSD (o HD externo não era a causa: as travas acontecem no mesmo ponto). Nas transições do jogo o `retro_run` do flycast leva ~100 ms várias vezes seguidas, e depois o core manda de uma vez o áudio atrasado, até 2,8× o esperado num segundo. É comportamento conhecido do core: `shell/libretro/audiostream.cpp` diz "flycast can stop rendering for arbitrary lengths of time, leading to multiple 'frames' worth of audio being uploaded in retro_run()".
- **Defeito do ReEmu:** o `RateEstimator` do sink media `amostras / tempo` em janelas de 0,5 s, e a rajada entrava na média. A taxa pulava de 44.100 para 49.392 Hz e levava ~8 s para voltar. Nesse tempo o áudio saía com a velocidade/afinação errada, e o buffer secava de 84% para 7% até faltar som.
- **Correção:** janela fora de ±3% da taxa declarada é tratada como rajada/trava e não entra na média, a menos que o desvio se sustente por 4 janelas seguidas no mesmo sentido (2 s), que é o caso de um core que mente sobre a taxa. O parallel_n64 (1,6%) continua dentro da faixa. Depois da correção: taxa medida 44.100–44.120 Hz, buffer estável em ~80–89% e nenhuma falta de amostra fora das rajadas. Continua havendo descarte e uma falha curta no próprio segundo da rajada, porque o core fica parado. Os testes `a_burst_does_not_skew_the_rate` (falha com o comportamento antigo: 9% de erro) e `a_sustained_offset_is_still_learned` cobrem isso.
- **Gatilhos:** o `gilrs` 0.11 só converte eixo em botão a partir de 75% do curso (solta abaixo de 65%, `GilrsBuilder` padrão). No DualSense, de curso longo, meia pressão não contava. Agora é `set_axis_to_btn(0.25, 0.15)`. **Não verificado com o controle na mão:** falta o usuário confirmar que o acelerador do MSR responde.

## 2026-09-25 — Release v0.1.2

- Versão 0.1.2 (`Cargo.toml` do workspace e `tauri.conf.json`). A tag `v0.1.2` dispara o `release.yml`, que monta os instaladores Linux e Windows num rascunho. Ao publicar o release, o job `updater-manifest` gera o `latest.json` do auto-update, e o site (`site/app.js`) passa a listar a versão pela API do GitHub.

## 2026-09-25 — Zona morta dos analógicos; diagnóstico de entrada; rotação validada

- **Zona morta:** o mapeamento SDL do `gilrs` não traz zona morta para todo controle. No DualSense ela é `0.0` (conferido com `Gamepad::deadzone`), e o stick parado mandava (2048, −3328), ou seja 6% e 10%, para o jogo. Agora há uma zona morta radial de 15% em `push_analog`, reescalada a partir da borda para não dar salto.
- **`REEMU_INPUT_DEBUG=1`:** o app registra cada evento bruto do `gilrs` (botão/eixo, valor, código evdev), e o core-host registra os botões que chegam em cada porta. O mapeamento do DualSense no `gilrs` 0.11 é simétrico (L2 = `ABS_Z`, R2 = `ABS_RZ`, os dois como botão analógico). O teclado R (R2) acelera no MSR, e o R2 do DualSense não; a causa segue em aberto até o log bruto do controle.
- **`SET_ROTATION`:** validado pelo usuário com um shooter vertical de arcade, em pé e sem espelhar.
- **Teste de jogo real:** `REEMU_TEST_OPTS` (opções de core no load) e `REEMU_TEST_SHOW_OPTS`. Medido no MSR: as travas de ~100 ms do flycast continuam iguais com `flycast_threaded_rendering=disabled` (a opção chega ao core), então não vêm da renderização em thread.

## 2026-09-25 — Etapa 12: flycast e mupen64plus_next em Vulkan in-process; hook do `vkCreateDevice`; mupen não derruba mais o app

- **flycast em Vulkan caía (SIGSEGV em `VulkanContext::init`), no Linux também.** O flycast só inicializa o despachante dinâmico do `vulkan.hpp` dentro do próprio `VkCreateDevice` (`core/rend/vulkan/vk_context_lr.cpp`), e o ReEmu não chamava esse `create_device`. O doc 12 dizia para não chamar, porque ele **ignora** o `required_device_extensions`. Resultado: todo ponteiro Vulkan do flycast ficava nulo. É provavelmente o mesmo crash do Windows (`STATUS_ACCESS_VIOLATION`).
- **Hook do `vkCreateDevice`** (`gpu/device_hook.rs`). Na negociação core-owned, o core recebe um `vkGetInstanceProcAddr` nosso, que repassa tudo ao verdadeiro **para sempre** (o despachante e o VMA do flycast carregam funções por ele durante toda a vida do core). A exceção é `"vkCreateDevice"`: o hook soma ao `VkDeviceCreateInfo` as extensões e as features que o wgpu exige e registra a lista final, e a adoção pelo wgpu passa a usar a lista **ligada**, não a pedida. Regras da spec Vulkan ("Devices and Queues"): VUID-VkDeviceCreateInfo-pNext-00373 (as features entram na `VkPhysicalDeviceFeatures2` do `pNext` se ela existir); VUID-…-03328 (KHR e EXT `buffer_device_address` nunca juntas); `vkGetInstanceProcAddr` com instância `NULL` só devolve comandos globais, então o `vkCreateDevice` verdadeiro é resolvido com a instância que o core passa. Os membros de `VkPhysicalDeviceFeatures` são todos `VkBool32`, então o OU é feito membro a membro.
- **Validado no RTX 3060** (teste `vk_core_real_rom`, core e ROM reais, quadros gravados em PNG): flycast (MSR, device com 9 extensões, 60 fps, imagem correta), Beetle PSX HW (40 Winks, 10 extensões, sem regressão) e mupen64plus_next com `mupen64plus-rdp-plugin=parallel` (GoldenEye, negociação v2, 31 extensões, 60 fps). **Não verificado:** camadas de validação do Vulkan (não instaladas aqui) e Windows.
- **Bug grave que já estava na v0.1.2 (Linux):** o `mupen64plus_next` estava na lista de roteamento automático para rodar in-process, mas com o plugin padrão (GLideN64, OpenGL) ele sobe a thread de emulação dentro do `retro_load_game` e desenha em GL antes de o `vulkan_only` rejeitar. **Abrir um jogo de N64 nele derrubava o app inteiro.** Agora a entrada da lista é condicional: o mupen só vai para in-process com `mupen64plus-rdp-plugin=parallel`, e com GLideN64 vai sempre para o processo filho, mesmo forçando. Teste `mupen_only_counts_as_vulkan_with_the_parallel_plugin`, e validado com o jogo: com GLideN64 vai para o filho, sem queda.
- O flycast em Vulkan continua opt-in (`REEMU_HW=vulkan`); sem isso, ele segue em GL no processo filho.

## 2026-09-25 — Camadas de validação do Vulkan; modificador DRM negociado; etapa 12 fase C

- **Validação:** `vulkan-validationlayers` instalado pelo usuário. O `vk_layer_settings.txt` liga a validação de núcleo e de **sincronização**. Para garantir que "zero erros" não era "zero relatos", fiz um controle: sem o `message_id_filter`, o aviso conhecido do naga aparece 12 vezes, então a camada relata.
- **Achado (interop GL, anterior a hoje):** o GBM alocava o dma_buf com `DRM_FORMAT_MOD_INVALID` ("o driver escolhe"), a NVIDIA dava `0x300000000e08014`, e o Vulkan recusa esse modificador para `R8G8B8A8_UNORM` amostrado (`VK_ERROR_FORMAT_NOT_SUPPORTED`, VUIDs 02251/00990/02273). Funcionava, mas era comportamento indefinido. **Correção pela negociação do `VK_EXT_image_drm_format_modifier`** (apêndice da extensão, "Negotiation"): `FrameProcessor::dmabuf_import_modifiers` consulta `vkGetPhysicalDeviceFormatProperties2` + `VkDrmFormatModifierPropertiesListEXT` e confirma cada candidato com `vkGetPhysicalDeviceImageFormatProperties2` + `VkPhysicalDeviceImageDrmFormatModifierInfoEXT` + dma_buf. Neste RTX 3060: `0x0300000000606010..15` e LINEAR. A lista vai para o filho no `ToChild::Load` (`dmabuf_modifiers`), e o GBM aloca só com ela (`gbm_bo_create_with_modifiers`); sem compatível, o interop desliga e o quadro vai pela cópia pela CPU. Resultado: o GBM escolhe `0x300000000606014`, e a validação fica limpa, também no semáforo `sync_file`.
- **Cores Vulkan sob validação:** Beetle limpo. mupen64plus_next (parallel) limpo durante o jogo; só no descarregamento aparecem 2 handles inválidos das threads de compilação do parallel-RDP (encerramento interno do core, junto do "Memory leaked in class allocator" dele). flycast: 2× `VUID-vkUpdateDescriptorSets-None-03047` na janela em que ele renderiza vários quadros num `retro_run`. O flycast submete com as próprias fences (`SubmitCommandBuffers(buffers, fence)` em `vk_context_lr.h`) e não usa o `wait_sync_index`, então o problema é interno a ele.
- **Fase C:** `submit_vulkan_cmds` submete os cmd buffers do core **sem fence e sem esperar**. A ordem na GPU vem da barreira que o core grava (`libretro_vulkan.h`: "vkCmdPipelineBarrier must be used to synchronize the core and frontend"); fora de render pass, o segundo escopo dela cobre "all commands that occur later in submission order" (spec, `vkCmdPipelineBarrier`). A conclusão de cada quadro é marcada em `VkFrameBridge::begin_frame` com uma submissão **vazia** com a fence do slot: o sinal de fence de `vkQueueSubmit` inclui "all commands that occur earlier in submission order" (spec, capítulo de fences), o que cobre os submits do próprio core, o `set_command_buffers` e a leitura do wgpu. O `wait_sync_index` espera essa fence só quando o slot volta, o que corresponde ao "waits on CPU for device activity for the current sync index" do `libretro_vulkan.h`. O `VkBlit` do Beetle passou a ter command buffer e fence por slot, esperados só antes de regravar aquele slot (a primeira barreira dele usa `srcStage` com FRAGMENT_SHADER, encadeando com a do core). Beetle, com validação: passo médio de 6,7–8,7 ms para 3,4 ms, sem `SYNC-HAZARD`.
- **Carga (fase C):** salvar e restaurar estado no meio do jogo com o core Vulkan in-process (`REEMU_TEST_SAVESTATE=1` no `vk_core_real_rom`), com validação de sincronização. Beetle (estado de 16 MB) e flycast (36 MB): nenhum VUID nem `SYNC-HAZARD`, e o jogo segue a 60 fps depois de restaurar. Redimensionar a surface nativa não foi coberto (precisa de janela real).

## 2026-09-25 — Base de idiomas: português, inglês e espanhol

- `i18next` 26 + `react-i18next` 17 em `packages/app-desktop/src/i18n/`. O `pt-BR` é a origem das chaves; `en` e `es` são tipados pelo formato dele (`Messages`), e as chaves do `t()` são tipadas (`i18next.d.ts`). O teste `locales.test.ts` garante as mesmas chaves e as mesmas variáveis `{{x}}` nos três idiomas, além da detecção do idioma do sistema.
- **Idioma:** preferência em Configurações › Aparência (Automático / Português (Brasil) / English / Español, cada idioma no próprio nome), guardada em `reemu.language`. `auto` segue `navigator.languages` pela língua-base (pt-PT → pt-BR, es-MX → es); sem correspondência, pt-BR. O `<html lang>` acompanha o idioma. A troca vale na hora, sem reiniciar.
- **Primeira leva migrada:** o rail (Início, Meus jogos, Configurações), o menu do perfil, o topo (voltar, busca, tela cheia, encerrar), as dicas de botão, o título e as abas de Configurações e a tela Aparência inteira, incluindo os nomes dos temas (`ThemeFamily.nameKey`) e os tamanhos da interface. O resto está listado no TASKS.md. Regra para textos novos no CLAUDE.md.

## 2026-09-26 — Tela preta nos cores de Atari, 2600 na biblioteca, core por sistema, BIOS de NAOMI

- **Tela preta até pausar (a5200, Stella):** o 1º `get_current_texture` voltava `Outdated` (surface criada em 1280x753, janela em 1366x768). O app reconfigurava e descartava o quadro, e cores que mandam quadro repetido com a tela parada nunca mandavam outro. A correção segue a doc do wgpu 30 (`CurrentSurfaceTexture`): em `Outdated`, `configure` e tenta de novo na hora. Se ainda falhar, o quadro fica pendente e o pump reapresenta a saída da chain sem esperar quadro novo.
- **Atari 2600 fora da biblioteca:** `.bin` só vale dentro da pasta de um sistema listado em `folder_only_exts`, e 2600 e 7800 não estavam lá. Extensões pelo `.info` oficial: stella `a26|bin`, prosystem `a78|bin|cdf`.
- **Core automático por sistema:** a tela do jogo ordenava só por extensão e nome, então um `.bin` de Mega Drive, 2600 ou PS1 ia para o a5200. `core_catalog::system_ids` e `rankCores` agora ordenam por sistema, extensão e nome.
- **Stella com "audio buffer overflow":** era uma ROM de Atari 5200 ("Adventure II") aberta no core de 2600. Com o Pitfall II (2600) não acontece.
- **BIOS de NAOMI/Atomiswave** na tela de BIOS (docs.libretro.com, library/flycast): `naomi.zip`, `naomi2.zip`, as de jogo específico (`hod2bios`, `f355bios`, `f355dlx`, `airlbios`) e `awbios.zip`, em `system/dc/`.

## 2026-09-26 — Interface VFS do libretro (v3)

- `RETRO_ENVIRONMENT_GET_VFS_INTERFACE` implementada em `core-loader-desktop/src/vfs.rs` sobre `std::fs`, com as 18 funções até a v3 (arquivo, `truncate`, `stat`, `mkdir` e pastas). Semântica do `libretro.h` de cada função. A tabela de modos do `open` é a da implementação de referência do libretro-common (`vfs/vfs_implementation.c`): `WRITE|UPDATE_EXISTING` equivale a `r+b`, que também lê. O flycast abre o `.cue` nesse modo. Core que pede versão acima de 3 recebe `false`.
- Motivo: o Stella 8 só reconhece a ROM pelo `stat` da VFS (`FSNodeLIBRETRO::setFlags`) e não abria nenhum jogo. Agora Pitfall II roda (300 quadros com imagem e áudio).
- Regressão conferida, já que todo core com `filestream` do libretro-common passa a fazer E/S por aqui:
  - fumaça em todos os 51 cores de cartucho do catálogo (depois de a ROM de SNES do teste subir para 128 KiB, porque o Snes9x 2010 recusa menos);
  - flycast/MSR e Beetle/40 Winks com jogo real, confirmando pelo log `vfs open` que o flycast lê BIOS, VMU, `.cue` e trilhas pela VFS.

## 2026-09-26 — Gatilhos analógicos, teclado configurável, Vulkan por padrão, fumaça fase 2, moldura com integer scaling

- **Gatilhos:** a pressão calibrada de L2/R2 vai ao core pelo `RETRO_DEVICE_INDEX_ANALOG_BUTTON`, na faixa `[0, 0x7fff]` do `libretro.h`. Sem pressão (teclado), vale o digital.
- **Teclado configurável:** a tabela `keyboard_bindings` guarda só o que o usuário muda sobre o padrão de `input_desktop::keymap`. Os alvos são os 16 botões e as direções dos dois analógicos. As setas também movem o analógico esquerdo, com Y+ para baixo como diz o `libretro.h`.
- **flycast em Vulkan por padrão:** com o loader em `vulkan_only`, o `GET_PREFERRED_HW_RENDER` responde Vulkan. O flycast escolhe o renderizador por essa resposta (`shell/libretro/libretro.cpp`). Validado sem `REEMU_HW`: flycast/MSR 638 quadros em 12 s, Beetle 591 em 10 s.
- **Textos do backend:** shaders prontos e notas de BIOS são traduzidos no frontend pelo id. O `hotkey-action` do QuickSave não tinha ouvinte e agora mostra aviso. O instalador NSIS tem PortugueseBR/English/Spanish (`tauri-utils` 2.9.3, `NsisConfig.languages`). O site ganhou `i18n.js`.
- **Fumaça fase 2:** `reemu-core-host --run` e as ROMs de `smoke_roms.rs`. O pcsx_rearmed travava no CI porque o `find_any_bios` (`frontend/libretro.c`) abre todo arquivo da pasta de sistema, e o probe usava o `/tmp` do runner, que tem FIFOs. Probe e run agora usam uma pasta própria. O Stella 8 exige a VFS do libretro (`FSNodeLIBRETRO::setFlags`) e está marcado como conhecido.
- **Integer scaling com moldura:** a composição ganha um zoom `k` perto de 1 (o menor `|ln k|` entre o piso e o teto de janela/nativa), e o jogo preenche a janela da moldura. O modo canvas recebe o zoom no cabeçalho do quadro (36 bytes).

## 2026-09-26 — Gatilhos do DualSense e teclado + controle

- **R2 não acelerava no MSR:** o DualSense do dono (driver `hid-playstation`, Bluetooth) informa `ABS_Z`/`ABS_RZ` em 168/173 de 0–255 com os gatilhos **soltos**, lido pelo `EVIOCGABS` do evdev. Com o limiar de 25% do gilrs (`btn_value` = valor/faixa), L2 e R2 contavam como apertados o tempo todo: no MSR, freio e acelerador juntos. O `GamepadPoller` agora decide L2/R2 pelo `ButtonChanged` com o zero calibrado (`TriggerCal`: o menor valor visto é o zero; o aperto é reescalado a partir dele, com histerese de 25%/15%) e ignora o press/release cru do gilrs para os gatilhos. Controle com zero em 0 continua igual.
- **Conclusão (defeito do controle):** com `scripts/dualsense-triggers.py` (evdev cru), `ABS_Z` e `ABS_RZ` subiam e desciam juntos (142/150 → 255/255 → ~202/204) apertando só o R2, e o zero ia caindo sozinho com o controle parado na mão. O hardwaretester.com também não via os gatilhos. Trocando de controle, tudo normal. A calibração fica (inofensiva com zero em 0), mas nenhum software separa dois gatilhos que o controle manda como o mesmo sinal. Para suspeita parecida, rode o script antes de mexer no app.
- **Teclado × controle:** cada um tem o próprio estado, combinado por OR no snapshot enviado ao core. O stick esquerdo não dobra mais como direcional quando o core lê o analógico (`ToParent::AnalogUsed` no filho; marca direta na rota in-process).

## 2026-09-26 — parallel_n64 derrubava o app na rota in-process

- Com `REEMU_HW=vulkan`, o parallel_n64 ia para a rota in-process e o app inteiro fechava no `LocalCore::load`. Fonte: `libretro/libretro.c` do repositório oficial libretro/parallel-n64. Com o padrão `parallel-n64-gfxplugin=auto`, o `retro_load_game` chama `retro_init_gl()` (o comentário diz que o GL "is assumed it always exists") e segue desenhando em GL mesmo com o `SET_HW_RENDER` recusado. Só o valor `parallel` pede `RETRO_HW_CONTEXT_VULKAN` (`retro_init_vulkan`). É o mesmo caso do mupen64plus_next com GLideN64.
- `GL_IN_LOAD_CORES` em `emu-session/session.rs` lista esses cores com a opção que escolhe o Vulkan. Sem ela, o core nunca vai para a rota local, nem com `REEMU_HW=vulkan`, e roda no processo filho.

## 2026-09-25 — Frontend inteiro em três idiomas

- Migradas para `t()` as telas que faltavam (Início, Biblioteca, plataforma, detalhe do jogo, tela de jogo, todas as abas de Configurações, onboarding) e os componentes (biblioteca de shaders e molduras, opções do core, mapeamento de controles, captura de atalho, gerenciar biblioteca, atualização e notificações, carrossel, diálogos de adicionar ROMs e de encerrar, erro de rota).
- `lib/errors.ts`: as regras têm um `id` e título/dica vêm de `errors.rules.<id>`; `errorToast`/`describeError` recebem a **chave** da ação (`actions.<chave>`) em vez de uma frase pronta, e montam "Não foi possível <ação>" no idioma ativo.
- Datas e números no idioma ativo com `Intl` (`toLocaleString(i18n.language)`): data de lançamento dos metadados (em UTC, para o fuso não mudar o dia), última vez jogado, data da versão no aviso de atualização, MB baixados. Plurais pelo sufixo `_one`/`_other` do i18next.
- Fica para depois (TASKS.md): textos gerados no Rust (shaders curados, notas de BIOS, eventos do backend) e o site/instalador.

## 2026-09-25 — Release v0.1.3

- Corrige o crash da v0.1.2 no Linux: abrir um jogo de N64 no mupen64plus_next com o plugin padrão (GLideN64) derrubava o app. Também entram o interop GL com modificador DRM negociado (validação limpa), a fase C dos cores Vulkan e a base de idiomas (pt-BR, en, es). A versão do `package.json` da raiz, que tinha ficado em 0.1.1, foi alinhada.

## 2026-10-01 — Vídeo nativo no Windows: janela filha acima do WebView2

- No Windows o jogo ia pelo `<canvas>`: o quadro saía da GPU, passava pela IPC e voltava para a GPU no WebGL. Agora é como no Linux. A chain desenha direto no swapchain de uma **janela filha** (`WS_CHILD`) do HWND principal, criada na thread principal pelo `VideoSurface::spawn` (`video.rs`, módulo `win`). A surface não pode ficar no HWND principal porque o WebView2 é uma janela filha dele e cobriria o jogo. A nossa vai para o topo das irmãs (`HWND_TOP`).
- Coreografia igual à da `wl_subsurface`: a máquina `VideoMenu` no `reemu-video-pump` esconde a janela no menu de pausa, no carregamento e sem jogo (`SWP_HIDEWINDOW`). O WebView2 aparece com o print de fundo.
- Decisões, pela documentação da Microsoft (Win32):
  - `WS_DISABLED`: "When a child window is disabled, the system passes the child's mouse input messages to the parent window" (*Window Features › Disabled Windows*). Clicar no jogo não tira o foco do teclado do WebView2, que recebe as teclas do jogo e o Esc.
  - `WS_CLIPSIBLINGS` (*Window Features › Child Windows*): a janela irmã não desenha na nossa área.
  - `SWP_ASYNCWINDOWPOS` (*SetWindowPos*): "the system posts the request to the thread that owns the window. This prevents the calling thread from blocking". O pump mexe na janela de outra thread sem esperar o event loop.
  - O deadlock de *DXGI overview › Multithread considerations* é de swapchain em tela cheia exclusiva. O do app é em janela (a tela cheia é janela sem borda).
  - Mostrar vem **depois** do present: a janela escondida guarda o último quadro, e mostrá-la antes piscaria o jogo anterior.
  - Posição em coordenadas de cliente do pai, sempre `(0, 0)`: `csd_offset` devolve zero no Windows.
- Padrão no Windows (`REEMU_NATIVE_VIDEO=0` volta para o canvas). Verificado aqui: `scripts/check-windows.sh` e os testes do Linux. **Falta validar numa máquina Windows.**

## 2026-10-01 — Frontend nas proporções do Xbox (960×540 epx)

- Referência: Microsoft, *Designing for Xbox and TV* e *Gamepad and remote control interactions*. O Xbox renderiza 1080p a 200%, e a interface é desenhada numa tela lógica de 960×540 epx que escala por igual.
- **Escala:** `lib/uiScale.ts` passa a calcular o zoom nativo do webview a partir do tamanho lógico da janela, com `zoom = min(L/960, A/540)`. 1 px do CSS vale 1 epx: zoom 2 em 1080p, 4 em 4K. O cálculo acompanha redimensionamento e troca de DPI.
  - Antes, cada componente escalava sozinho com `clamp(…vw…)`, e as proporções mudavam de tela para tela (rail com 26 epx em 1080p, abaixo do mínimo de 32).
  - A preferência de tamanho virou um multiplicador: Compacto 80%, Padrão, Grande 115% e Maior 130%.
- **Métricas** (`styles/metrics.ts`):
  - área segura de 48/27 epx (rail, topbar e dicas ficam dentro dela);
  - alvo de pelo menos 32 epx (31 botões, `Select` e `Input` saíram de `size="small"`, além de duas `TabList`);
  - texto de pelo menos 15/12 epx (tokens do tema);
  - 6 cards por fileira em 16:9, pela regra dos 6 cliques;
  - títulos na rampa do Windows (28/20/15).
- **Telas:**
  - Configurações com as categorias numa lista vertical, como no Xbox. Dez abas não cabiam em 960 epx.
  - Primeiro acesso com rolagem quando passa da altura.
  - Destaque da página do jogo dimensionado para o aviso de core não invadir as abas.
  - Destaque da Início com 208 epx, para a primeira fileira aparecer junto, como no dashboard.
- Verificado no Chrome headless com o backend simulado, em 16:9, 21:9 e 4:3. A experiência de tela cheia do Xbox no PC não tem medidas publicadas; os tamanhos vêm das regras oficiais acima e da tela de referência.

## 2026-10-01 — Protótipo no Windows: WebView2 transparente por cima do jogo (`REEMU_WIN_OVERLAY`)

- **Ideia:** no Windows o WebView2 aceita fundo transparente, ao contrário do WebKitGTK com NVIDIA. O jogo pode ficar *atrás* da interface: menu de pausa, avisos e HUD passam a ser camadas HTML sobre o jogo ao vivo, sem print de fundo e sem esconder e mostrar janela.
- **Base:** documentação do WebView2, `ICoreWebView2Controller2::put_DefaultBackgroundColor`: "In the case of a transparent DefaultBackgroundColor WebView will render hosting app content as the background" (só alfa 0 ou 255). No Tauri é `Webview::set_background_color(Color(0,0,0,0))`, e o wry repassa ao `DefaultBackgroundColor`. Só o WebView2 fica transparente; as telas com fundo opaco no CSS continuam opacas, e a `PlayScreen` já era transparente no vídeo nativo.
- **Duas variantes**, porque a documentação não detalha o que aparece por baixo no modo janela:
  - `REEMU_WIN_OVERLAY=1`: a surface vai no próprio HWND principal;
  - `REEMU_WIN_OVERLAY=child`: a janela filha fica no fundo das irmãs (`HWND_BOTTOM`), abaixo do WebView2.
- **Comportamento no modo overlay:**
  - sem jogo ou carregando, a surface é limpa de preto (`clear_surface`) em vez de escondida;
  - ao abrir o menu, não há captura nem esconder; a `PlayScreen` não busca o `pause_background`, e o véu do menu escurece o jogo.
- Sem a flag, nada muda (janela filha por cima, como antes). Verificado aqui: `scripts/check-windows.sh`, clippy e os testes no Linux. **Falta rodar no Windows.**

## 2026-10-02 — Windows: vídeo nativo validado; proporções do app Xbox (1366×768)

- **Vídeo nativo no Windows** (RTX 3060, wgpu em Vulkan):
  - O padrão (janela filha acima do WebView2) funcionou: o jogo aparece, o Esc abre o menu e volta, e clicar no jogo não tira o teclado.
  - O protótipo `REEMU_WIN_OVERLAY` deu tela preta nas duas variantes (`=1` e `=child`), mesmo com som e "1º frame apresentado". A causa provável é que o WebView2 no modo janela não deixa ver o que está por baixo. Isso não está verificado: a página da Microsoft "Windowed vs. Visual hosting" não trata do caso.
  - Às vezes o jogo anterior piscava na troca de jogo. O present é assíncrono (Vulkan, `vkQueuePresentKHR`: não garante a imagem visível quando a chamada retorna), então a janela filha podia reaparecer antes do 1º quadro novo, ainda com o último quadro guardado. Correção: o pump apresenta um quadro preto antes de esconder (`clear_surface`, só no Windows).
- **Proporções:** comparado com capturas do app Xbox do Windows na mesma resolução (1366×768), o ReEmu saía 1,42× maior (rail de 91 px contra 68, controles de 46 contra 31, texto de 21 contra 14). Isso vinha da tela lógica de 960×540 do console.
  - A referência passou a ser **1366×768** (`lib/uiScale.ts`).
  - As métricas foram medidas nas capturas (`styles/metrics.ts`): rail 68 com itens de 50×46 a cada 56; busca de 500; margem de 47; cards de 184 com espaço de 20; título de seção 24; corpo 14.
  - Conferido com um print do app rodando em 1366×768: rail, busca, botões da direita e as seis colunas de cards caem nas mesmas coordenadas da captura do Xbox.

- **Página do jogo** (`useDetailStyles`), medida contra a página de jogo do app Xbox em 1366×768:
  - capa de 128 (antes 96) logo abaixo da topbar;
  - título de 36 semibold (antes 28 extra-bold);
  - "Jogar" com 180×56 e texto de 14 regular, ações de 56×56 com 16 entre elas (antes 40 e 12);
  - destaque de 420 de altura que se dissolve no fundo por uma máscara no hero inteiro, em vez do corte reto em 340.
  - Conferido com print: capa, título e botões caem nas coordenadas do Xbox. A linha da plataforma acima do título e os selos de ano/gênero continuam, porque são do ReEmu.

## 2026-10-02 — Saves do flycast corrompidos ao sair do jogo

- Sintoma relatado: sair do jogo e voltar deixava o save (VMU) corrompido.
- Causa, pelo código do flycast (`core/hw/maple/maple_devs.cpp`, repositório flyinghead/flycast): o arquivo do VMU fica aberto o jogo inteiro (`fopen` no `OnSetup`, `fclose` só no destrutor). Cada bloco salvo é `fseek` + `fwrite`, **sem `fflush`**: o que estava no buffer do stdio só chegava ao disco no `fclose`, no encerramento do core.
- O `ChildProc::kill` mandava `Shutdown` e matava o `reemu-core-host` no mesmo instante. O core nunca recebia `retro_unload_game`/`retro_deinit` (a ordem do `libretro.h`), o `fclose` não acontecia, e o VMU ficava com parte dos blocos gravada e o índice inconsistente.
- Correção (`b98bb56`): o `kill` espera o filho sair sozinho depois do `Shutdown` (até 3 s; passou disso, mata com aviso no log).
- **Validado pelo usuário no Windows (2026-10-02):** com o VMU antigo (corrompido) apagado, o flycast criou um cartão novo; salvar, sair pelo menu e voltar manteve o save inteiro. O core-host encerrou sozinho ("reemu-core-host: encerrando"), sem o aviso de kill. Cópia do cartão antigo em `system/dc/vmu_save_A1.backup-2026-10-02.bin`.
- Observado junto: com `reicast_per_content_vmus` desligado (padrão do flycast), todos os jogos de Dreamcast dividem `system/dc/vmu_save_A1.bin`, que tem 200 blocos. Não existe `dc_nvmem.bin` (flash do Dreamcast), outro arquivo que o flycast só grava no encerramento.

## 2026-10-02 — Release v0.1.4

- Windows: vídeo nativo (janela filha acima do WebView2) validado, sem o quadro do jogo anterior na troca; o core no ritmo do monitor (sem o quadro perdido a cada poucos segundos) e latência 1 no present; o core-host encerra sozinho antes de ser morto, o que acaba com os saves corrompidos do flycast (VMU).
- Interface nas proporções do app Xbox do Windows (tela lógica de 1366×768), página do jogo refeita com tempo de jogo, anel de foco mais afastado, logo do site no ícone e na abertura (fade-in), e a categoria Configurações › Sistema (iniciar com o sistema, abrir em tela cheia, minimizar para a bandeja).
- Diagnóstico: `REEMU_PERF` mede o readback GL e a espera do present.

## 2026-10-02 — Revisão das regras de uso dos cores libretro

- **Situação:** o ReEmu é MIT e não distribui cores, shaders, molduras nem BIOS. O instalador leva só o app e o `reemu-core-host`. Os cores são baixados pelo usuário, sob demanda, do buildbot oficial, e o catálogo mostra a licença de cada um.
- **API libretro:** o `libretro.h` e o `libretro_vulkan.h` são MIT ("Copyright (C) 2010-2024/2020 The RetroArch team"), e a licença pede o aviso "in all copies or substantial portions". O `sys.rs` e o `vk_sys.rs` reproduzem as declarações sem o aviso. Corrigido: `THIRD_PARTY_NOTICES.md` (também empacotado no instalador) e referência no topo dos dois arquivos.
- **Cores não comerciais** (Genesis Plus GX, família Snes9x, FB Alpha/FBNeo, MAME 2000/2003/2003-Plus, PicoDrive e outros): "may not be sold, nor may they be used in a commercial product or activity without copyright holders' approval" (docs.libretro.com/development/licenses). O ReEmu é gratuito e não os distribui. Doação voluntária não vende o software. Se um dia houver versão paga, anúncio ou loja, esses cores precisam sair da oferta ou ter autorização dos autores.
- **GPL:** os cores GPL rodam no processo do core-host (e os Vulkan, no do app). Quem monta essa combinação é o usuário, na própria máquina, e o ReEmu não redistribui nenhum binário de core, então não há obrigação de fonte do lado do ReEmu.
- **Buildbot:** a documentação da libretro não tem regra publicada pra uso por outros frontends (não verificado além disso). Os downloads agora se identificam com `User-Agent: ReEmu/<versão> (+https://github.com/Kluis6/reemu)` (`http.rs`); antes saíam sem User-Agent.

## 2026-10-02 — flycast: "Detect Frame Rate Changes" ligado por padrão

- **Sintoma:** em jogos a 30 fps (Jet Set Radio), o áudio saía em dobro (176 mil amostras/s contra 88 mil).
- **Causa** (`shell/libretro/libretro.cpp` do flycast): com Threaded Rendering, o `retro_run` repete `emu.render()` até achar um quadro novo. Num jogo a 30 fps, cada chamada emula 2 refreshes.
- **Correção:** tabela `FRONTEND_DEFAULTS` em `core-loader-desktop/src/coreopts.rs`, aplicada ao schema quando o core declara as opções. Ela troca o padrão de `reicast_detect_vsync_swap_interval` para `enabled`, só se o core oferecer esse valor. O valor escolhido pelo usuário continua tendo prioridade. Valores e descrição conferidos em `shell/libretro/libretro_core_options.h` (flyinghead/flycast, master): `disabled`/`enabled`, padrão `disabled`, recomendado desligar em jogos de taxa instável.
- **Validado no Windows:** o flycast avisa `fps=29.973`, o core-host passa a 30 chamadas/s e o áudio fica em 88.284 amostras/s (~88.282 esperadas), sem quadros perdidos.

## 2026-10-02 — NAOMI/Atomiswave na biblioteca, "Atualizar biblioteca" e quadro antigo na troca de jogo

- **NAOMI não rodava.** A biblioteca do usuário vinha de uma varredura de antes de 26/09: os 127 `.zip` de NAOMI (formato do MAME) não estavam lá, os `.chd` dos sets apareciam como jogos e as cópias do Redump (`naomi/japan/*.7z`, `.cue`) estavam como `disc` ou `naomi`. O log mostra que o flycast abriu o `.cue` do Capcom vs. SNK 2 como disco de Dreamcast. Pela documentação do core (docs.libretro.com, library/flycast, "How to play NAOMI Games"), NAOMI só roda do `.zip` do MAME, com o `.chd` numa subpasta de mesmo nome nos jogos em GD-ROM.
- **Varredura:** imagem de disco em pasta de NAOMI/Atomiswave, solta ou dentro de `.zip`/`.7z`, não é mais catalogada, e a que já estava sai da biblioteca (`MAME_ONLY_SYSTEMS`/`is_unplayable_disc_dump` em `library-scan/src/scan.rs`). Na pasta real: 127 jogos de NAOMI e 25 de Atomiswave, todos com o sistema certo.
- **"Atualizar biblioteca"** (botão na biblioteca, comando `rescan_library`): o app passa a guardar as pastas adicionadas (tabela `library_folders`, migração 0013) e varre de novo todas elas. Bibliotecas antigas, sem pastas guardadas, têm as pastas deduzidas das ROMs (as mais externas). Pasta que não existe mais (disco externo desligado) é pulada sem mexer no que veio dela. O aviso no fim mostra quantos jogos entraram, saíram e foram corrigidos.
- **Remover pasta da biblioteca não funcionava no Windows:** `remove_under_dir` só casava caminhos com `/`. Agora aceita `/` e `\` (teste `remove_under_dir_handles_both_separators`), e a pasta removida sai da lista de pastas guardadas.
- Dois testes da varredura falhavam no Windows por montar caminhos com `/` (`dos_and_scummvm_games`, `naomi_and_atomiswave_mame_sets`); corrigidos.
- **Quadro do jogo anterior na troca:** a janela de vídeo aparecia logo após o 1º present do jogo novo, e o present é assíncrono (`vkQueuePresentKHR` não garante quando a imagem fica visível). No Windows, agora ela só aparece depois do 2º quadro (`SHOW_AFTER_PRESENTS`). A latência de quadro do wgpu é só uma sugestão ao driver (`desired_maximum_frame_latency`, wgpu-types 30), então isso **não está verificado** por documentação; a garantia exigiria `VK_KHR_present_wait`, que o wgpu não expõe.
- **C1 (Vulkan no Windows):** flycast e Beetle PSX HW rodaram em Vulkan dentro do app sem cair; o log da `VkImage` do core saía a cada quadro e agora sai só quando tamanho ou formato mudam.

## 2026-10-02 — PPSSPP em Vulkan travava o app (slot nunca liberado)

- **Sintoma:** com `REEMU_HW=vulkan`, o PPSSPP carregou em Vulkan dentro do app (device do core adotado, 10 extensões), mas nenhum quadro foi apresentado: o log repetia `vk wait_sync_index slot 0: compositor atrasado (gen 6)` até o app parar.
- **Causa:** um slot do anel de quadros em voo só era liberado quando o `VkImageFrame` daquela geração era largado pelo compositor. Num `retro_run` sem imagem entregue (sem `set_image`, sem `video_refresh` ou quadro repetido, o que o PPSSPP faz enquanto carrega), a geração não virava quadro e ninguém a liberava. RING quadros depois, o `wait_sync_index` do core esperava por ela para sempre, na thread que também roda o compositor.
- **Correção:** o `begin_frame` libera a geração anterior quando ela não foi entregue ao compositor (`taken`). Pelo `libretro_vulkan.h` (libretro-common), o core só precisa manter a imagem até o `wait_sync_index` do índice em que ela foi entregue; imagem não entregue não é lida pelo frontend. A atividade de GPU do slot continua coberta pela fence-marcador do `begin_frame`.
- **Validado no Windows (2026-10-02, RTX 3060, tela 3840×2160):** o PPSSPP roda em Vulkan dentro do app a 60 quadros/s, 0 perdidos (só 13 no 1º segundo, durante o carregamento), ~1,15 ms por quadro no app; nenhum aviso de "compositor atrasado".

## 2026-10-02 — C1: cores Vulkan no Windows com as camadas de validação

- Vulkan SDK instalado; o app subiu com `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` (o loader confirma no log). O device criado pelo core é da instância do app, então a validação cobre o core também.
- flycast (Dreamcast e um jogo de Atomiswave, que mudou pra 29,97 fps com o "Detect Frame Rate Changes") e Beetle PSX HW em Vulkan dentro do app, trocando de jogo sem fechar o app: **nenhuma** mensagem `VUID` nem `SYNC-HAZARD`, 0 quadros perdidos. Com cores em OpenGL (mupen64plus_next e parallel_n64 sem `parallel`), o compositor do app também passou sem erro.
- Custo do shader na tela 3840×2160: `crt-guest-advanced` ~4 ms por quadro com o core Vulkan, até ~16 ms no teste com core GL (com a validação ligada, que pesa).
- Falta o mupen64plus_next com `mupen64plus-rdp-plugin=parallel`.

## 2026-10-02 — C1 concluída: mupen64plus_next em Vulkan no Windows e validação limpa

- **Correção do registro anterior:** "nenhum VUID" para flycast e Beetle estava errado. As camadas de validação imprimem na saída do processo, não no arquivo de log que eu tinha olhado. Rodando o teste `vk_core_real_rom` no Windows com `VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation`, os dois davam `VUID-VkShaderModuleCreateInfo-pCode-08737` e `VUID-VkSemaphoreTypeCreateInfo-timelineSemaphore-03252`.
- **mupen64plus_next (ParaLLEl-RDP) derrubava o app (`STATUS_ACCESS_VIOLATION`).** Um tratador de exceção temporário no teste mostrou a falha dentro de `SwitchToFiber` (escrita em `0x1e18`). O mupen usa libco, que no Windows troca de contexto com fibers: o `retro_init` guarda a fiber da thread que o chamou (`co_active`) e `retro_run`, `retro_serialize`/`unserialize`, `retro_unload_game` e `retro_deinit` fazem `co_switch` (`libretro/libretro.c` do mupen64plus-libretro-nx). `SwitchToFiber` só vale numa thread já convertida em fiber (learn.microsoft.com, SwitchToFiber). Na rota in-process o load e o save state rodavam na thread da sessão e o `retro_run` na do compositor. No Linux o libco é assembly e não depende de thread, por isso passava.
  - **Correção:** `on_vk_thread` (`emu-session/src/session.rs`): carregar, salvar/restaurar estado e descarregar viram tarefas que a thread que chama `step_vk_local` executa, com o portão da `VkQueue` já segurado. Sem essa thread (modo canvas) ou se ela não pegar a tarefa em 2 s, roda na própria thread da sessão.
- **Depois disso, o processo caía ao descarregar:** o ParaLLEl-RDP só encerra o device e as threads dele no `context_destroy` (`parallel_deinit`), e o `Drop` do core só chamava `context_destroy` para OpenGL. Sem ele, as threads seguiam depois do `FreeLibrary`. Agora, num core Vulkan, ele é chamado antes do `retro_unload_game`, na ordem do RetroArch (`core_unload_game` em `runloop.c`: `video_driver_free_hw_context` antes do `retro_unload_game`); `libretro.h`: "A callback to be called before the context is destroyed in a controlled way by the frontend". A ordem do OpenGL ficou como estava.
- **`08737` (SPIR-V 1.6 sob Vulkan 1.2):** o wgpu-hal 30 escolhe a versão do SPIR-V pela versão do device físico (`device_api_version = properties.api_version`, 1.4 na RTX 3060), mas a instância da negociação era criada com `apiVersion` 1.2. Pela spec ("Extending Physical Device Core Functionality", cap. Initialization), funcionalidade nova só vale quando as duas versões chegam lá. A instância agora usa 1.3 (a mesma do `Instance::init` do wgpu-hal, "the max Vulkan API version supported by wgpu-hal") ou o que o core pedir, se for maior.
- **`VUID-StandaloneSpirv-None-10684`** (aparecia só depois do anterior, porque o spirv-val parava no erro de versão): o naga punha a struct de saída (`var o: VOut`) e um `array` indexado por `vertex_index` em variáveis de função com decoração de layout. Os shaders de `gpu/mod.rs` e `gpu/pipelines.rs` agora devolvem a struct pelo construtor de valor (WGSL, W3C) e montam o triângulo de tela cheia com `select`.
- **`03252` (timeline sem o recurso ligado):** o wgpu-hal usa semáforo timeline em device 1.2+, e o device criado pelo core não ligava `timelineSemaphore`. O hook do `vkCreateDevice` (`gpu/device_hook.rs`) liga o campo na `VkPhysicalDeviceVulkan12Features` ou na `VkPhysicalDeviceTimelineSemaphoreFeatures` que o core mandou, ou acrescenta a sua (nunca as duas, VUID-VkDeviceCreateInfo-pNext-02830), só se `vkGetPhysicalDeviceFeatures2` disser que o device suporta.
- **Resultado (Windows, RTX 3060, validação ligada, 10 s com save state):** flycast (Jet Set Radio), Beetle PSX HW (40 Winks) e mupen64plus_next com `parallel` (GoldenEye): ~58–60 quadros/s em Vulkan, estado salvo e restaurado, descarregamento limpo, **zero** mensagens de validação. O teste `vk_core_real_rom` passou a rodar também no Windows (dados em `%APPDATA%`), e conduz o core numa thread enquanto as chamadas bloqueantes da sessão rodam em outra, como o app.

## 2026-10-02 — Avaliação do save state por plataforma; FB Alpha 2012 não restaurava em sessão nova

- Teste novo `savestate_real_rom` (`gpu/tests.rs`, `#[ignore]`, core e ROM reais pela sessão completa, rota padrão do app): pausado, salva → restaura → salva de novo e compara os dois estados; despausa e confere os quadros; fecha o jogo, abre de novo e restaura o estado da sessão anterior.
- Resultado no Windows (um jogo da biblioteca por core instalado):

| core | estado | salvar→restaurar→salvar | 2ª sessão |
|---|---|---|---|
| a5200, prosystem, gearsystem, genesis_plus_gx, gpsp, vbam, mednafen_pce_fast, snes9x2002, mednafen_psx_hw, parallel_n64 | 49 KB a 16 MB | 100% igual | ok |
| stella | 1 KB | 99,4% | ok |
| mupen64plus_next | 16 MB | 99,0% | ok |
| ppsspp | 40 MB | 99,9% | ok |
| flycast | 36 MB | 94,3% | ok |
| fbalpha2012 | 259 KB | 100% igual | **falhava** |

  Os que não dão 100% guardam contadores/relógio no estado; em todos, o jogo segue com imagem depois de restaurar.
- **FB Alpha 2012:** o `state_size` dele começa em 0 e só é calculado no `retro_serialize_size`; o `retro_unserialize` recusa se `size != state_size` (`svn-current/trunk/src/burner/libretro/libretro.cpp`, libretro/fbalpha2012). O ReEmu restaurava sem perguntar o tamanho antes, então carregar um estado numa sessão em que ainda não se salvou falhava. O `restore_state` agora chama `retro_serialize_size` antes do `retro_unserialize`, como o RetroArch. Validado: restaura na 2ª sessão.

## 2026-10-02 — PPSSPP: "Game data is corrupted" no save do Tekken (MAC sorteado a cada sessão)

- **Sintoma:** no Tekken: Dark Resurrection, criar o save, sair e voltar dava "Game data is corrupted. Failed to load data from Memory Stick Duo".
- **O que não era:** o PPSSPP grava o save direto no disco (não pela VFS do ReEmu); o `PARAM.SFO` estava certo (`SAVEDATA_PARAMS` = `0x21`, modo 3, o mesmo da gravação); e o log não teve "Incorrect hash on save data" — o PPSSPP decifrou e conferiu o hash (`Core/Dialog/SavedataParam.cpp`), então o arquivo estava íntegro e quem recusava o conteúdo era o jogo.
- **Causa:** o MAC do PSP emulado vem das opções `ppsspp_change_mac_address01..12`; com todas em "0" (padrão), o core sorteia um MAC e pede ao frontend que guarde via `RETRO_ENVIRONMENT_SET_VARIABLE` (`libretro/libretro.cpp` do PPSSPP). O ReEmu não implementava esse pedido, então cada sessão sorteava um MAC novo (medido: `50:50:A7:5C:FB:19` numa sessão, `68:37:21:…` na seguinte). Um save amarrado ao console deixa de ser reconhecido. **Não verificado:** que o Tekken use o MAC especificamente; é a única identidade do console que mudava entre as sessões.
- **Correção:** `RETRO_ENVIRONMENT_SET_VARIABLE` (70) implementado conforme o `libretro.h` ("After changing a core option value with this callback, it will be reflected in the frontend and GET_VARIABLE_UPDATE will return true"; `data` nulo só pergunta se existe). O core-loader valida e aplica o valor e anota a troca; o core-host repassa ao pai (`ToParent::CoreOptionSet`, antes do `Loaded`); a rota in-process recolhe direto; o app grava como opção do core (escopo do core) no fim do `load_game` e no `unload_game`. Da 2ª sessão em diante o core recebe o MAC guardado e não sorteia outro.
- **Validado pelo usuário no Windows (2026-10-02):** com um save novo, sair e voltar ao Tekken carrega normal.
- O save criado antes da correção ficou amarrado a um MAC sorteado que não foi guardado; é preciso criar um save novo.

## 2026-10-02 — C2: Vulkan automático no Windows

- O usuário validou no app o GoldenEye em Vulkan com o ParaLLEl-RDP (salvar/restaurar estado, trocar de jogo), fechando o C1.
- `route_local_device` (`emu-session/src/session.rs`): a escolha automática da rota in-process para os cores de `VK_CAPABLE_CORES` passa a valer também no Windows. Antes só com `REEMU_HW=vulkan`, porque o flycast derrubava o app; as causas (libco em threads diferentes, `context_destroy`, versão da instância, `timelineSemaphore`) foram corrigidas no C1.
- Validado com `vk_core_real_rom` e `REEMU_TEST_NO_FORCE=1`: flycast, Beetle PSX HW e mupen64plus_next com `parallel` vão sozinhos para Vulkan in-process; o mupen64plus_next sem `parallel` (GLideN64) segue no processo filho.
- Continua valendo o risco conhecido: sem o isolamento do processo filho, um core desses que quebre leva a interface junto. O C3 (core Vulkan no processo filho) devolve o isolamento.

## 2026-10-02 — "fatal runtime error: out of TLS indexes" instalando cores

- **Sintoma:** instalando cores pelo catálogo (dice, desmume), o app fechou com `fatal runtime error: out of TLS indexes, aborting`.
- **Causa:** `discover_cores` espiava `retro_get_system_info` carregando **cada** DLL de core no processo do app (`LoadLibrary` + `FreeLibrary`), e o catálogo (`list_core_catalog`) e a lista de cores (`list_installed_cores`) chamavam isso a cada atualização da tela. Carregar uma DLL roda a inicialização dela; várias reservam índices de TLS e não devolvem ao descarregar. O Windows tem um número fixo de índices de TLS por processo; recarregando todas as DLLs a cada instalação, eles acabaram, e o runtime do Rust aborta quando não consegue um.
- **Correção** (`core-loader-desktop/src/discover.rs`): o catálogo usa `installed_core_ids`, que só lê os nomes dos arquivos; `discover_cores` guarda o que espiou por (caminho, tamanho, data de modificação) e carrega cada DLL uma vez por execução, de novo só se o arquivo mudar.
- **Ainda em aberto:** a rota Vulkan in-process carrega e descarrega a DLL do core a cada jogo; se o core vaza TLS, trocar de jogo muitas vezes numa sessão longa consome índices. O C3 (core Vulkan no processo filho) resolve.

## 2026-10-02 — Revisão dos 96 cores instalados no Windows

- **Teste de fumaça** (`reemu-core-host --probe`, um processo por core): os 96 abrem no Windows, nenhum cai nem trava.
- **Jogo real com save state** (`savestate_real_rom`) nos 67 cores com jogo na biblioteca. Passam (carregar, rodar, salvar, restaurar, restaurar com o jogo reaberto): atari800, blastem, bsnes (todas as variantes), clownmdemu, desmume2015, fceumm, gambatte, mame (atual), mednafen (gba, pce, psx, snes, supafaust, supergrafx, vb), melonds, melondsds, mesen, mgba, nestopia, noods, pcsx_rearmed, rustynes, smsplus, snes9x2005_plus, stella2023, swanstation, tgbdual, vba_next, virtualjaguar e os 15 da avaliação anterior.
- **RustyNES não carregava nada:** pede `RETRO_ENVIRONMENT_GET_GAME_INFO_EXT` (66) e desiste com `false` (o `libretro.h` diz que o core deveria seguir com o `retro_game_info` normal, mas o RetroArch implementa e o core conta com isso). Implementado: a `retro_game_info_ext` é montada antes do `retro_load_game` e apagada depois ("may only be called inside retro_load_game()"); com `.zip`, `archive_path`/`archive_file` e `name` = nome do `.zip`, como o RetroArch.
- **Isso quebrou o FCEUmm**, que registra por `RETRO_ENVIRONMENT_SET_CONTENT_INFO_OVERRIDE` (65) que quer o `.nes` na memória (`need_fullpath=false`, mesmo com `true` no `retro_system_info`) e lê o buffer do `GET_GAME_INFO_EXT` (`src/drivers/libretro/libretro.c`). O `libretro.h` garante o `GET_GAME_INFO_EXT` quando o override é aceito: os dois andam juntos. Implementado o override: extensão → `need_fullpath`/`persistent_data` (só a 1ª ocorrência vale); o loader aplica pela extensão do conteúdo, e com `persistent_data` o buffer fica no core até o `retro_deinit`. FCEUmm, RustyNES, Mesen e Nestopia validados com o mesmo jogo.
- Comandos de ambiente não tratados agora aparecem no log uma vez por processo ("environment N não tratado — respondido false").
- **Não são defeito do ReEmu:**
  - MAME 2000/2003/2003-plus/2003-midway/2010/2015/2016, HBMAME e FBNeo: o `1941.zip` da biblioteca é de outro conjunto de ROMs; cada versão do MAME pede o conjunto da sua época.
  - DICE: é para arcades de lógica discreta (Pong), não CPS1.
  - PCSX2: precisa da BIOS do PS2.
  - blueMSX: o jogo escolhido no teste era de Master System.
  - FixNES: não tem save state (o ReEmu já trata).
  - IroGB: declara `.zip`, mas não abre o `.zip` na memória; com o `.gb` solto funciona. O ReEmu segue a regra do RetroArch (core que lista `zip` recebe o arquivo como está).
  - PCSX ReARMed: na abertura do jogo quase todo quadro é repetido (duplicação de quadros ligada por padrão); o core roda a 60/s.
- **Sem sistema na biblioteca:** TIC-80 e Commodore 128 (exceção conhecida no teste do catálogo).
- **Sem jogo na biblioteca para testar:** 27 cores (Amiga, Amstrad CPC, CD-i, DOS, MSX, C64, ColecoVision, Lynx, Saturn, Neo Geo Pocket, PC-FX, WonderSwan, Neo Geo CD, Odyssey², Pokémon Mini, ScummVM).

## 2026-10-03 — PPSSPP com backend Vulkan caía ao carregar (0xc0000005)

- **Sintoma:** Tekken (PSP) não abria: "core-host: o core encerrou inesperadamente ao carregar o jogo (exit code: 0xc0000005)". O log mostrava `core ppsspp_libretro → rota processo filho (device=false, negotiator=false, known_non_vk=false)`.
- **Causa:** o PPSSPP do usuário está com `ppsspp_backend = vulkan`, mas o core não estava em `VK_CAPABLE_CORES`. A validação de 2026-10-02 foi com `REEMU_HW=vulkan` (forçado); sem a variável, o `route_local_device` mandava o core pro processo filho, que ainda não faz Vulkan (C3), e o core quebrava no `retro_load_game`.
- **Correção:** `("ppsspp", Some(("ppsspp_backend", "vulkan")))` em `VK_CAPABLE_CORES` (`crates/emu-session/src/session.rs`): com o backend Vulkan ele vai pra rota in-process, a mesma validada em 2026-10-02 (60 quadros/s, saves ok); em `auto`/`opengl` segue no filho. Teste em `vk_route_tests`.
- **Em aberto:** se a rota in-process não estiver disponível (sem device Vulkan publicado), o PPSSPP com backend Vulkan ainda vai pro filho e cai do mesmo jeito.

## 2026-10-03 — Prévia do shader em Configurações › Vídeo

- **O que é:** na aba Shaders, uma cena de exemplo (pixel art 320×240 gerada pelo próprio app em `commands/shader_preview.rs`: céu em faixas, sol, montanhas, tijolos, o robô do avatar e um baú — arte original, sem material de terceiros) aparece com o preset ativo e os mesmos parâmetros, num comparador "original / com shader" com slider. Atualiza ao trocar de preset ou mexer num parâmetro.
- **Como:** `render_shader_preview` usa um `FrameProcessor` só do preview (`AppState.shader_preview`, criado na 1ª vez), sem surface, então o viewport vem do pedido (960×720) e o processador do jogo não troca de preset nem de viewport. `FrameProcessor::render_still` roda a chain 4 vezes sobre a imagem parada (o readback tem 1 quadro de atraso e presets com histórico leem quadros anteriores) e o resultado vai como PNG.
- **Cena 3D (mesmo dia):** `commands/shader_preview_3d.rs` desenha uma paisagem no jeito de PS1/N64 com um rasterizador de triângulos em software (z-buffer, textura do chão com perspectiva correta, recorte no plano próximo, luz direcional e neblina): chão quadriculado com caminho de pedra, blocos, pilares, torre e pirâmide sob céu com sol, em 320×240. A prévia troca entre "Cena 2D" e "Cena 3D" por abas; o comparador com slider deu lugar a um botão sobre a imagem que liga/desliga o shader já carregado (alterna entre a imagem original e a renderizada, sem renderizar de novo).
- **Cenas mais detalhadas (2026-10-04):** a 2D virou `commands/shader_preview_2d.rs` — céu em degradê com dithering de Bayer, sol com halo, nuvens com volume, montanhas nevadas, pinheiros, lago com reflexo, plataforma de tijolos com moedas, o robô e o baú sombreados e um HUD (corações e SCORE). A 3D ganhou textura de tijolo pelas coordenadas do mundo, árvores, lago com reflexo e ondas, montanhas no horizonte, sombras no chão e saída em 15 bits com dithering 4×4, como o PS1. Tudo continua gerado pelo app, sem arte de terceiros.
- **Validado:** teste `shader_preview_renders_the_sample_scene` na RTX 3060 (o "plain" sai no tamanho nativo, o CRT em 960×720 e muda mais de 10% dos pixels); imagens conferidas a olho (scanlines, máscara e brilho do CRT).

## 2026-10-04 — Onboarding em três modais

- **O que é:** na primeira execução, três modais do mesmo tamanho (quase a tela toda) sobre o fundo animado do tema: (1) boas-vindas — "Bem-vindo" em 16 línguas aparecendo uma a uma com fade e a escolha do idioma, que já troca o texto; (2) nome e avatar (preset ou imagem própria); (3) "Tudo pronto, {nome}!" com o avatar escolhido e o botão Começar.
- **Como:** `screens/Onboarding.tsx` com o `Dialog` do Fluent seguindo a página de uso dele no Fluent 2: título em cada passo, ações no rodapé, um modal fecha antes do próximo abrir (sem aninhar) e `modalType="alert"` (não fecha com Esc nem clique fora — fluxo obrigatório). O perfil só é salvo no último passo: o `set_profile` marca `onboarded` e o `RootLayout` mandaria pra Home se salvasse antes. `ProfileForm` ganhou o modo sem botão/sem salvar (`onChange`, `showBio`), e o erro de nome só aparece depois que o campo é mexido.
- **Backlog:** passo de contas (redes sociais e lojas como a Steam) em `TASKS.md`.

## 2026-10-04 — Release v0.1.5

- PSP: o PPSSPP com backend Vulkan roda in-process (antes ia pro processo filho e caía com 0xc0000005 ao carregar).
- Interface: navbar e espaçamentos medidos contra o app Xbox, raio único de 4 px, abas com o mesmo estilo e comportamento em todo o app, menu de Configurações com ícones e só o conteúdo rolando; Configurações reorganizadas (Sistema com idioma, tamanho, integer scaling e informações do hardware; Aparência, Gerenciar biblioteca, Cores e Vídeo em abas/subpáginas; Vídeo › Shaders e Vídeo › Molduras como no Windows).
- Shaders: prévia do shader ativo na GPU numa cena 2D e numa 3D geradas pelo app, com botão pra ligar/desligar, lista em cards e ajustes do shader no lugar da lista.
- Onboarding novo em três modais (boas-vindas em várias línguas e idioma, nome e avatar, tudo pronto).
- Foco automático: Jogar na página do jogo e o 1º card da biblioteca.


## 2026-10-08 — Configurações em hub bento

- **O que é:** `/settings` deixou de abrir no Perfil com menu lateral: agora mostra só o título e uma grade bento com um card por categoria (ícone, nome e o que tem dentro; Perfil maior, com avatar e nome). A grade ocupa a área útil sem rolar. Clicar abre a categoria com o caminho "Configurações › Categoria" no topo (níveis de cima clicáveis; em Vídeo, "Configurações › Vídeo › Shaders") e o conteúdo dentro de um card que rola por dentro.
- **Como:** `screens/settings/SettingsHome.tsx` (rota índice) e `layouts/settingsSections.tsx` (lista única das categorias); `SettingsLayout` monta o caminho e o card; o `SettingsBreadcrumb` próprio do Vídeo saiu. Card clicável seguindo o Card do Fluent 2 (superfície inteira navega, hover/pressionado no fundo, elevação no hover), `<a>` pra ganhar o anel de foco do app. Transições: a página entra com o `RouteTransition` e os cards em cascata (30 ms entre eles, `backwards`, sem animação com `prefers-reduced-motion`).
- **Colunas pela proporção, não pela largura:** com o zoom da interface a tela lógica continua 16:9 e só encolhe (Maior = 1051×591); por largura caía em 3 colunas × 4 linhas e rolava. Agora 4 colunas (3 linhas) em tela larga, 3 em janela ≤ 3:2; em altura ≤ 640 o card fica em linha (ícone ao lado do texto). Medido sem rolagem em 1366×768, 1188×668, 1051×591, 1920×1080, 1024×768 e 1280×1024.

## 2026-10-08 — Janela de terminal ao abrir jogo no Windows

- **Sintoma:** no app instalado no Windows, cada jogo abria uma janela de terminal junto (no Linux não).
- **Causa:** o core roda no processo filho `reemu-core-host`, que é programa de console. O app instalado não tem console (subsistema "windows" no release), então o Windows criava um console novo — com janela — pro filho. No `tauri dev` não aparecia porque o filho herdava o terminal do dev.
- **Correção:** `emu-session` cria o filho com `CREATE_NO_WINDOW` (0x08000000), conforme "Process Creation Flags" (learn.microsoft.com/windows/win32/procthread/process-creation-flags). stdout/stderr herdados continuam funcionando.

## 2026-10-08 — Tela branca com bordas pretas ao abrir o app

- **Causa:** antes do JS montar a splash (escura), o WebView2 pinta o fundo padrão dele (branco), e a janela atrás, entrando em tela cheia, aparecia preta nas laterais.
- **Correção:** `backgroundColor: "#050506"` na janela do `tauri.conf.json` — "Set the window and webview background color" (docs.rs `tauri_utils::config::WindowConfig::background_color`; formato hex conferido no `FromStr` de `Color` da tauri-utils 2.10.1) — e o mesmo fundo num `<style>` do `index.html`, antes do CSS do bundle. Mesma cor da splash, então a abertura fica escura do 1º quadro até o app.

## 2026-10-08 — "banco de dados indisponível" ao trocar o shader (checksum de migration por quebra de linha)

- **Sintoma:** no `tauri dev`, toda configuração falhava com "banco de dados indisponível". O log: `migration 13 was previously applied but has been modified`.
- **Causa:** o sqlx guarda o SHA-384 do texto de cada migration aplicada (`sqlx-core` 0.8.6, `Migration::new`). No checkout do Windows (`core.autocrlf`) as migrations 0001–0012 estão em CRLF e a 0013 ficou em LF; o banco do usuário tinha as 13 aplicadas com CRLF. O binário do dev embutiu a 0013 em LF → checksum diferente → o sqlx recusa e o app segue sem banco.
- **Correção:** `db::run_migrations` corrige antes de migrar o checksum guardado que bate com o MESMO texto só com a outra quebra de linha (LF↔CRLF); mudança real de conteúdo continua recusada. Fixar uma quebra só no `.gitattributes` não resolveria: bancos do Windows têm checksums CRLF e os do Linux, LF. Testes em `crates/db/tests/migration_checksum.rs`; conferido abrindo uma cópia do banco real.

## 2026-10-08 — BIOS de todas as plataformas do catálogo (PS2 incluso)

- **Pedido:** a tela de BIOS não tinha o PS2; comparar com tudo que a libretro suporta.
- **Comparação:** os sistemas do app (mapa core → sistema do `core_catalog`) contra os `firmwareN_*` dos `.info` do libretro-core-info e o `dat/System.dat` do libretro-database (a lista que o RetroArch usa, com caminho e MD5). A tabela à mão cobria 14 sistemas; faltavam 21 inteiros (PS2, 3DO, Neo Geo CD, Intellivision, Odyssey², Lynx, ColecoVision, GB/GBC/GBA, NES/FDS, SNES — Satellaview, Sufami Turbo, Super Game Boy e chips DSP —, N64DD, Mega Drive/Master System/Game Gear, Pokémon Mini, CD-i, Atari 7800, ZX Spectrum, DOS/MT-32) e arquivos em outros (Amiga, Saturn, Dreamcast, MSX, Atari 8-bit, arcade).
- **Como:** `scripts/gen_bios_table.py` gera `crates/domain/src/bios_generated.rs` (133 entradas), só com o que algum core DO CATÁLOGO declara, sem bancos de dados de jogos, cartuchos de trapaça nem bibliotecas. Obrigatório só quando todos os cores do sistema marcam `opt = false`; o O2EM marca os 4 BIOS, mas a doc dele diz que o core só usa o `o2rom.bin` — os outros ficam opcionais. A tabela à mão (notas conferidas core a core) continua valendo e tem prioridade.
- **PS2:** dezenas de dumps equivalentes em `pcsx2/bios/` viraram uma entrada "qualquer um destes" (`BiosFile::any_of`): presente com qualquer dump; o import reconhece o dump pelo MD5 e grava com o nome oficial (um dump fora da lista mantém o nome — o core aceita qualquer BIOS válido). Remover limpa a pasta.
- **Pendente:** as notas dos arquivos gerados vêm em inglês (descrição do `.info`); as da tabela à mão e a do PS2 são traduzidas.

## 2026-10-08 — Metadados não baixavam: ScreenScraper sem credenciais de desenvolvedor

- **Diagnóstico (log do usuário):** 8712 ROMs na fila e toda consulta com `HTTP 403 — Erreur de login : Vérifier vos identifiants développeur !`. Segundo a doc da API (api.screenscraper.fr/webapi2.php), 403 = credenciais de DESENVOLVEDOR erradas/ausentes; o ReEmu mandava só `softname` e a conta do usuário, sem `devid`/`devpassword` (o dev pede as suas "via le forum"). Nenhum metadado vinha do ScreenScraper, e cada falha custava ~16 s (espera de 5 s + atrasos) — a fila levaria dias pra só falhar.
- **Feito:** `scraping.rs` envia `devid`/`devpassword` quando o build tem `REEMU_SS_DEVID`/`REEMU_SS_DEVPASSWORD` (`option_env!`; o `release.yml` repassa os secrets de mesmo nome). No 1º 403 a leva para de consultar o ScreenScraper: segue só pelo TheGamesDB se houver chave, senão encerra, e o progresso informa o motivo (`error: "ss_dev_login"`). Na UI: toast ao iniciar a busca, barra de progresso grossa com % na página de Metadados e um aviso explicando o 403.
- **Pendente (TASKS):** pedir as credenciais no fórum do ScreenScraper e cadastrar os secrets.

## 2026-10-08 — Ficha técnica do jogo na gaveta de informações

- **O quê:** a gaveta "Informações do jogo" ganhou desenvolvedora, publicadora, jogadores, modos de jogo (etiquetas), nota (/10) e classificação etária — cada linha só aparece quando o provedor traz o dado.
- **De onde vem:** campos do `jeuInfos.php` na doc da API v2 do ScreenScraper (api.screenscraper.fr/webapi2.php): `developpeur`, `editeur`, `joueurs`, `note` (0–20, guardada como 0–100), `classifications` (um item por órgão; PEGI primeiro, depois ESRB) e `modes` (nomes por idioma, como os gêneros). O parse aceita o campo como texto, `{ "text": .. }` ou lista por idioma, porque a doc descreve o formato XML e o JSON segue o padrão `{langue/region, text}` que o app já lia.
- **Como:** `domain::metadata::GameDetails` (com `serde(default)`: correspondências já gravadas continuam abrindo) dentro de `GameMetadata` e `ScrapeCandidate`; migration `0014_game_details.sql` com as colunas novas; `get_rom_metadata` devolve os campos. TheGamesDB fica sem ficha por enquanto: a doc da API dele (swagger) não abriu pra conferir quais `fields` a busca aceita.
- **Observação:** os dados só chegam depois que o ScreenScraper aceitar o app (credenciais de desenvolvedor, ver TASKS).

## 2026-10-08 — Testes do core-fake voltam a rodar no Windows

- **Causa:** o `fixtures/testcore.c` não marcava as funções com `__declspec(dllexport)`; no MSVC a DLL saía sem exports e todo teste que carrega o core-fake falhava em `GetProcAddress` (`load_run` do `core-loader-desktop` e os 9 de `emu-session/tests/session.rs`). O `cl.exe` também escrevia `testcore.obj` na pasta do crate, e ele estava versionado.
- **Correção:** `RETRO_API` no `testcore.c` com a mesma definição do `libretro.h` oficial (libretro-common): `__declspec(dllexport)` no Windows, visibilidade padrão no resto; `/Fo:` manda o `.obj` pro `OUT_DIR`; `*.obj` no `.gitignore`. `cargo test --workspace` e `cargo clippy --workspace --all-targets -D warnings` passam no Windows 11. Os testes de sessão sobem o processo filho de verdade, então também validam o `CREATE_NO_WINDOW` do mesmo dia.

## 2026-10-08 — Itens 6, 8 e 9 da lista de melhorias

- **Contraste nos temas claros (6):** a camada de dentro do card das Configurações escurecia nos temas claros (cinza sobre cinza). `liftTokens` (themes.ts) define `reemuSurface1..3` por modo — escuro/alto contraste misturam a cor do texto, claro mistura branco e fecha o card 4% — seguindo "Use lighter neutrals on surfaces" (Fluent 2 › Color). Medido na tela de BIOS: linha × card entre 1,18 e 1,35 em todos os temas, sempre mais clara que o card.
- **Busca automática (8):** depois de uma varredura que trouxe jogos novos, a busca de metadados começa sozinha (`useAutoMetadataScan`), com o mesmo toast de início.
- **Buscar de novo (9):** botão na gaveta de informações; comando `rescrape_rom` → `scraping::scrape_one`, a mesma cascata e a mesma gravação da leva (`record_result`, extraído do laço).
- **Achado no caminho:** os toasts tinham `z-index` 9999 e ficavam atrás dos portais do Fluent (gaveta, diálogos), que usam 1000000 (`usePortalMountNodeStyles`). Subiu pra 1000001.

## 2026-10-08 — Release v0.1.6

- **Configurações:** hub em cards (bento) no lugar do menu lateral; cada categoria abre num card próprio, com caminho "Configurações › …" no topo; camadas internas com contraste em qualquer tema (claro ou escuro); a página para acima da barra de dicas quando há controle conectado.
- **BIOS:** tabela gerada das fontes oficiais da libretro pra todos os sistemas do catálogo — PS2 e mais 20 sistemas que não tinham nada —, busca e filtro "só da minha biblioteca", uma coluna.
- **Metadados:** ficha técnica do jogo (desenvolvedora, publicadora, jogadores, modos, nota, classificação), aba Galeria (captura, tela de título, arte, logo, vídeo), "Buscar dados de novo" por jogo, busca automática depois de uma varredura, toast e barra de progresso. TheGamesDB com capa na mesma requisição (metade da cota). O ScreenScraper ainda depende das credenciais de desenvolvedor (ver TASKS).
- **Correções:** banco que não abria por causa da quebra de linha de uma migration; janela de terminal ao abrir jogo no Windows; tela branca na abertura; toasts atrás da gaveta e dos diálogos; erro "banco de dados indisponível" ao trocar o shader.
- **Interface:** prévia do shader maior e com o botão de ligar/desligar na barra das cenas; botões de baixar só com o ícone da nuvem; ícones de Cores/BIOS/Vídeo unificados com o site; teclado em Controles sem sobreposição em nenhuma resolução.
- **Qualidade:** testes do emulador voltam a rodar no Windows (o core de teste não exportava as funções da DLL); clippy do workspace inteiro limpo; `.gitattributes` com LF; `scripts/ui-smoke.py` (teste visual das Configurações em 5 tamanhos).

## 2026-10-11 — B1: detecção do interop GL → Vulkan no Windows

- **O quê:** o app (Vulkan) e o processo do core (OpenGL/WGL) passam a saber se dá pra compartilhar a imagem do quadro sem passar pela CPU. Só detecção e log; o quadro continua pelo readback até B2–B5.
- **Fontes (Khronos):** EXT_external_objects.txt (`GL_EXT_memory_object`, `GL_EXT_semaphore`, `DEVICE_UUID_EXT` 0x9597, `DRIVER_UUID_EXT` 0x9598, `NUM_DEVICE_UUIDS_EXT` 0x9596, `glGetUnsignedBytevEXT`/`glGetUnsignedBytei_vEXT`; os IDs "can be used to correlate devices and determine driver compatibility across process and API boundaries"), EXT_external_objects_win32.txt (as variantes `_win32`), e o refpage de `VkPhysicalDeviceIDProperties` ("deviceUUID and/or driverUUID must be used to determine whether a particular external object can be shared").
- **Como:** `domain::gpu_interop` (tipos + regra pura, 3 testes); `FrameProcessor::vk_interop_id` lê `VkPhysicalDeviceIDProperties` e as extensões `VK_KHR_external_memory_win32`/`external_semaphore_win32`; vai no `ToChild::Load` (`vk_interop`, em `Box` por causa do `large_enum_variant`); o `GlContext` do Windows consulta o WGL e registra "interop GL→Vulkan: possível…" ou o motivo.
- **Medido (RTX 3060, Windows 11):** as quatro extensões GL e as duas Vulkan presentes; deviceUUID `041cb70b2082bdc64f008b4a2126e828` e driverUUID `16edf6112967515eb44ea51f39e45424` idênticos nos dois lados. Próximo: B2 (anel de imagens exportáveis no app).

## 2026-10-11 — B2: anel de imagens exportáveis do app pro processo do core (Windows)

- **O quê:** o app aloca as imagens onde o core OpenGL vai desenhar e entrega ao processo do core o acesso a elas. O GL não exporta memória (EXT_external_objects só importa), então quem aloca é o Vulkan. Tudo atrás de `REEMU_GL_INTEROP=1` até o B5: sem a variável, nada muda.
- **Spec Vulkan seguida (docs.vulkan.org):** `VkExternalMemoryImageCreateInfo` na imagem; "handleType must have been included in VkExportMemoryAllocateInfo::handleTypes" (VUID-VkMemoryGetWin32HandleInfoKHR-handleType-00662); handle NT exportado "no more than once" por memória (00663) e do app — "must release ownership of them using the CloseHandle system call"; `EXPORTABLE` conferido em `vkGetPhysicalDeviceImageFormatProperties2` + `VkExternalImageFormatProperties`; alocação sempre dedicada (`VkMemoryDedicatedAllocateInfo`), que cobre o caso `DEDICATED_ONLY`.
- **Caminho:** o filho, com o B1 compatível e o interop pedido, manda `ToParent::InteropRequest { max_w, max_h }`; a sessão chama o provedor do app (`set_interop_provider`), que aloca 3 imagens (`FrameProcessor::install_interop_ring`), duplica cada handle pro processo do filho (`DuplicateHandle`) e responde `ToChild::InteropImages`; o filho guarda os handles (fecha num anel novo ou ao sair). O app guarda as texturas (donas da memória) pro B5.
- **Medido (RTX 3060, Windows 11):** 3 imagens 640×480 de 1.310.720 bytes; de ponta a ponta com o core-fake (`HW`, GL 3.3): pedido 64×48 → 3 handles duplicados → recebidos no filho. Camadas de validação (SDK 1.4.363): nenhum VUID. Próximo: B3 (importar no GL do filho).

## 2026-10-11 — B3: o core OpenGL desenha na memória das imagens Vulkan (Windows)

- **O quê:** o processo do core importa no OpenGL as imagens que o app alocou no B2 e o core passa a desenhar nelas (o FBO dele gira entre as 3 texturas). O quadro ainda chega ao app pelo readback; o B4 (semáforos) e o B5 (quadro pelo índice do slot) tiram a cópia. Tudo atrás de `REEMU_GL_INTEROP=1`.
- **Spec seguida (registry.khronos.org, EXT_external_objects.txt e EXT_external_objects_win32.txt):**
  - Ordem: `CreateMemoryObjectsEXT` → `MemoryObjectParameterivEXT(DEDICATED_MEMORY_OBJECT_EXT 0x9581, TRUE)`, que "must be set to TRUE when the external memory handle ... was created as a dedicated allocation" e tem de vir antes do import (depois, "INVALID_OPERATION ... if <memoryObject> is immutable") → `ImportMemoryWin32HandleEXT(mem, alloc_size, HANDLE_TYPE_OPAQUE_WIN32_EXT 0x9587, handle)` → `TEXTURE_TILING_EXT 0x9580 = OPTIMAL_TILING_EXT 0x9584` (a spec não diz o valor inicial; a imagem Vulkan é `OPTIMAL`; só vale se definido antes da alocação) → `TexStorageMem2DEXT(TEXTURE_2D, 1, RGBA8, w, h, mem, 0)`.
  - Handles: "Importing a Windows handle does not transfer ownership of the handle to the GL implementation"; o NT handle é fechado "when it is no longer needed". **Não verificado:** a spec não diz se dá pra fechar logo depois do import, então os handles ficam abertos até o anel sumir (texturas → memory objects → `CloseHandle`).
  - Lado Vulkan: issue 13, "Require all supported usage flags be specified by the application on the Vulkan side if the image is intended to alias with an OpenGL texture" → os usos saem das `optimalTilingFeatures` do formato; issue 17, com GL ≥ 4.3 do outro lado é preciso o "mutable format bit" → `VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT`. Se o export recusar todos os usos, cai para os mínimos e avisa no log.
- **Medido (RTX 3060, Windows 11):** `win32_interop_ring_imports_into_gl` mostra o anel exportado pelo wgpu, com os handles duplicados para o próprio processo, importado num contexto WGL 3.3; o clear vermelho volta certo das 3 texturas. Camadas de validação: nenhum VUID. Próximo: B4 (semáforos Win32 no lugar do `glFinish`).

## 2026-10-11 — B4: semáforos entre o OpenGL do core e o Vulkan do app (Windows)

- **O quê:** cada imagem do anel ganha dois semáforos compartilhados. Com eles, o OpenGL avisa ao app quando terminou o quadro, sem `glFinish`, e o app devolve a imagem quando terminou de ler. A troca do quadro pelo índice do slot é o B5; até lá o quadro do core segue pelo readback e os semáforos só são exercitados no teste.
- **Base oficial:**
  - Exemplo `open_gl_interop` do Vulkan-Samples (KhronosGroup): dois semáforos exportáveis (`gl_ready` Vulkan → GL, `gl_complete` GL → Vulkan), `glWaitSemaphoreEXT`/`glSignalSemaphoreEXT` com layouts, sem barreira de família de fila (a issue 9 da EXT_external_objects deixa isso em aberto).
  - EXT_external_objects: o `SignalSemaphoreEXT` "flushes the stream as if a Flush were inserted right after it"; o `WaitSemaphoreEXT` recebe o layout atual porque "an external API may have changed texture layouts"; esperar sem sinal pendente é indefinido.
  - Tabela 4.4: `GL_LAYOUT_SHADER_READ_ONLY_EXT` = `VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL`, valor 0x9591 do `glext.h` do registro.
  - EXT_external_objects_win32: `ImportSemaphoreWin32HandleEXT(uint, enum, void*)`; o handle continua da aplicação.
  - Vulkan (refpages): `VkExportSemaphoreCreateInfo` com `OPAQUE_WIN32`, conferido em `vkGetPhysicalDeviceExternalSemaphoreProperties` (`EXPORTABLE`); `vkGetSemaphoreWin32HandleKHR` uma vez por semáforo (VUID-VkSemaphoreGetWin32HandleInfoKHR-handleType-01127); o handle NT é do app e fecha com `CloseHandle`; `vkDestroySemaphore` só depois de "all submitted batches that refer to semaphore" terminarem, por isso os semáforos morrem no callback de drop da textura do wgpu.
- **Protocolo:** a imagem passa de um lado pro outro sempre em `SHADER_READ_ONLY_OPTIMAL`, que é o `TextureUses::RESOURCE` com que o wgpu a embrulha. O GL só espera `free` num slot que já entregou (o 1º uso de cada slot não tem sinal pendente).
- **Medido (RTX 3060, Windows 11):** `win32_interop_semaphores_round_trip` faz o GL desenhar vermelho/verde/azul nos 3 slots e sinalizar; o Vulkan espera, copia a imagem pra um buffer e lê as cores certas; o GL espera o `free` e redesenha amarelo, que o Vulkan lê de novo. Camadas de validação: nenhum VUID. Próximo: B5 (o app esperar `done` e sinalizar `free` no próprio quadro do wgpu e o quadro sair pelo índice do slot).

## 2026-10-11 — B5: o quadro do core OpenGL chega ao app sem cópia (Windows)

- **O quê:** no Windows, o quadro de um core OpenGL (N64, Dreamcast, PSP…) deixa de passar pela CPU (`glReadPixels` + cópia pelo pipe). O core desenha numa das 3 imagens compartilhadas e o processo do core manda só o número dela; o app mostra direto da memória da GPU. Ligado por padrão; `REEMU_GL_INTEROP=0` volta ao readback.
- **Protocolo:**
  1. Filho: `bind_write_slot` escolhe o próximo slot livre. Se o slot foi devolvido, espera o `free` com `glWaitSemaphoreEXT`.
  2. Depois do `retro_run`, o `hand_over_slot` sinaliza o `done` (`glSignalSemaphoreEXT`, que já faz flush) e manda `FrameReady { kind: Hardware }` com o índice.
  3. App: o `bind_interop_input` reivindica o quadro (`claim`) e põe na fila do wgpu-hal a espera do `done` e o sinal do `free`, na mesma submissão que amostra a imagem.
  4. O `flush_interop` garante a submissão (um `submit` vazio, se o quadro não chegou a submeter) e só então a sessão avisa o filho (`ToChild::InteropSlotFree`).
  5. Quadros que ninguém amostrou (substituídos na sessão, menus, falha da chain) voltam pela mesma via (`take_dropped_interop_slots`).
- **Base oficial:**
  - EXT_external_objects: "Calling WaitSemaphoreEXT on a semaphore that has not had a signal flushed or submitted since it was created or last waited on results in undefined behavior". Por isso o aviso só sai depois do submit do `free`, e o filho nunca espera um semáforo sem sinal.
  - Spec Vulkan (`vkQueueSubmit`): a operação de sinal de semáforo tem como 1º escopo os comandos do mesmo lote, então o `free` só dispara depois da leitura.
  - Exemplo `open_gl_interop` do Vulkan-Samples para o par de semáforos.
  - **Não verificado na spec:** o wgpu-hal aplica `add_wait_semaphore`/`add_signal_semaphore` na próxima submissão da fila (comportamento do código do wgpu-hal 30, o mesmo usado pelo `sync_file` do Linux).
- **Sem travar:** sem slot livre (o app atrasou 3 quadros), o filho desenha na textura própria e manda aquele quadro pelo readback síncrono. Qualquer falha antes disso (GPU sem as extensões, UUIDs diferentes, import recusado) também fica no readback.
- **Medido (RTX 3060, Windows 11, camadas de validação ligadas, nenhum VUID):**
  - `win32_interop_frames_flow_through_the_ring` (core-fake `HWD`, que limpa de magenta via GL): 120 quadros pelo anel, cor conferida na saída da chain.
  - `win32_interop_real_gl_core`:
    - mupen64plus_next com GoldenEye: 889 quadros pelo anel e 1 pelo readback em 15 s (~59 fps).
    - Mario Kart 64: tela de título conferida num PNG, com orientação e cores certas.
    - flycast com Soulcalibur: 700 quadros pelo anel, 0 pelo readback.
- **Próximo:** B6 (validação de sincronização, outras GPUs, medir contra a fase A).

## 2026-10-11 — B6: validação de sincronização e medição do interop GL → Vulkan (Windows, NVIDIA)

- **Validação:**
  - Camada `VK_LAYER_KHRONOS_validation` do SDK 1.4.363, com a validação de sincronização ligada. O nome da opção é `validate_sync` no `VkLayer_khronos_validation.json`; a variável `VK_KHRONOS_VALIDATION_VALIDATE_SYNC=true` vem do `Config/vk_layer_settings.sh` do próprio SDK, e também foi ligada pelo `VK_LAYER_ENABLES=VK_VALIDATION_FEATURE_ENABLE_SYNCHRONIZATION_VALIDATION_EXT`.
  - Resultado: nenhum aviso em todos os testes `win32_interop_*`, com mupen64plus_next (Mario Kart 64) e flycast (Soulcalibur).
  - Prova de que a camada estava ativa: uma 2ª cópia sem barreira, plantada de propósito no teste do B4, gerou `SYNC-HAZARD-WRITE-AFTER-WRITE` (alteração desfeita).
  - Limite: a camada só enxerga o lado Vulkan; o lado OpenGL não tem validador equivalente.
- **GPUs:** só a RTX 3060 (driver 32.0.16.1692) nesta máquina; AMD e Intel ficam no B6b.
- **Medição** (`REEMU_PERF`, `win32_interop_real_gl_core`, 15 s cada; `REEMU_GL_INTEROP=0` é o readback da fase A). O processo do core soma `retro_run` (que inclui o readback) e o envio; o app é o tempo do `process` por quadro.

  | Jogo | Readback (core / app) | Anel (core / app) |
  |---|---|---|
  | mupen64plus_next, Mario Kart 64, 640×480 | ~1,3–2,1 ms / 1,36 ms | ~0,7–1,5 ms / 0,84 ms |
  | flycast, Soulcalibur, 640×480 | ~1,7 ms / 1,36 ms | ~1,2 ms / 0,88 ms |
  | flycast, 1920×1440 | 4,45 ms / 5,12 ms | 1,27 ms / 2,14 ms |

  - Em 1920×1440, o `glReadPixels` sozinho custava 2,17 ms e o envio de 10,5 MB pelo pipe, 1,37 ms (525 MB/s a 50 fps).
  - O tempo do app inclui a leitura de saída do próprio teste, igual nos dois casos; a diferença é a subida da imagem pra GPU, que deixou de existir.
  - Além do custo, o anel tira 1 quadro de latência: o readback por PBO (fase A) entregava o quadro anterior.

## 2026-10-11 — Vazamento de TLS da rota Vulkan in-process, medido

- **Como:** teste `vk_core_tls_leak` (Windows, `--ignored`). Ele carrega e descarrega o core Vulkan no processo do app `REEMU_TEST_CYCLES` vezes, como ao trocar de jogo, rodando ~2 s por volta, e mede os índices livres depois de cada volta. A medição aloca com `TlsAlloc`/`FlsAlloc` até `TLS_OUT_OF_INDEXES`/`FLS_OUT_OF_INDEXES` e devolve tudo, porque a página oficial do `TlsAlloc` (learn.microsoft.com) não informa o limite por processo.
- **Resultado** (RTX 3060, 8 voltas por core, ~1005 índices de TLS e 4060 de FLS livres no início):

  | Core | TLS por jogo | FLS |
  |---|---|---|
  | flycast (Soulcalibur) | −1 | 0 |
  | mupen64plus_next com ParaLLEl-RDP (Mario Kart 64) | −1 | 0 |
  | Beetle PSX HW (40 Winks) | 0 (só −1 no 1º load) | 0 |

- **Leitura:** o Beetle passa pela mesma rota do ReEmu e não vaza, então o índice perdido deve ser reservado pela própria DLL do flycast e do mupen e não devolvido ao descarregar. **Não verificado:** a causa exata dentro dessas DLLs. Com essa taxa, o abort "out of TLS indexes" exigiria ~1000 trocas de jogo numa sessão. O C3 (core Vulkan no processo filho) elimina o problema: cada jogo ganha um processo novo.

## 2026-10-11 — C3a: core Vulkan no processo filho (device próprio, quadro por cópia)

- **O quê:** com `REEMU_VK_CHILD=1`, os cores da lista Vulkan (`VK_CAPABLE_CORES`) rodam no `reemu-core-host` em vez de dentro do app. Volta o isolamento de processo (um crash do core não derruba a interface), e a DLL do core não é mais carregada no app (fim do vazamento de TLS medido no `vk_core_tls_leak`). O quadro ainda é copiado pra CPU; o C3b troca a cópia pelo anel compartilhado.
- **Como:**
  - O `Load` leva `vulkan_in_child`, e o loader do filho responde Vulkan no `GET_PREFERRED_HW_RENDER`.
  - O device é do filho. Se o core registra `create_device` na negociação (Beetle, flycast), o filho chama esse `create_device` com a própria instância (`VkContext::from_core_negotiation`). "Device provided to frontend is owned by the frontend" (`libretro_vulkan.h`), então o device e a instância são do filho. O `destroy_device` do core roda antes de descarregar a DLL.
  - O `vk_present::VkReadback` faz o papel do compositor: submete os command buffers do core e, no mesmo lote, um passe de desenho que lê a imagem do core e escreve num RGBA8, que é copiado pra um buffer visível pela CPU.
- **Por que passe de desenho e não blit:** o `libretro_vulkan.h` pede `TRANSFER_SRC` e `SAMPLED` na imagem do `set_image`, mas o flycast entrega sem `TRANSFER_SRC` (a validação acusou VUID-vkCmdBlitImage-srcImage-00219). O fragment shader usa `textureLoad`, que vira `OpImageFetch` ("No sampler is used", spec Vulkan, Textures), só exige `SAMPLED`, converte o formato do core e não muda o layout da imagem. O descritor usa o `image_layout` informado pelo core, porque um layout diferente do real devolve "poison". O shader é WGSL compilado pelo naga 30 (a mesma versão do wgpu) pra SPIR-V 1.0 no load.
- **Também corrigido:**
  - A ponte passa a guardar os semáforos do `set_image` e o filho espera por eles quando não há `set_command_buffers` ("Semaphores provided by a single call to set_image will only be waited for once").
  - Registra no log se um core pedir transferência de ownership (`src_queue_family`).
  - O flycast liga `VK_KHR_swapchain` no device dele, o que exige `VK_KHR_surface` na instância (VUID de `vkCreateDevice`); a instância do filho liga a extensão quando existe.
- **Medido** (RTX 3060, teste `vk_core_in_child`, camadas de validação ligadas no filho, nenhum VUID; nenhum índice de TLS perdido no app em todas as trocas de jogo):

  | Core e jogo | Resultado |
  |---|---|
  | Beetle PSX HW, 40 Winks | 60 fps, abertura da Eurocom conferida |
  | mupen64plus_next com ParaLLEl-RDP, Mario Kart 64 | 60 fps, título conferido (640×240 do ParaLLEl) |
  | flycast, Soulcalibur | abertura conferida |

## 2026-10-11 — C3b: core Vulkan no processo filho sem cópia (anel compartilhado)

- **O quê:** com `REEMU_VK_CHILD=1`, o core Vulkan no processo filho desenha direto numa imagem do anel que o app alocou (o mesmo do GL, B2–B5) e manda só o índice. Sem slot livre, aquele quadro cai na cópia pra CPU do C3a.
- **Mesma GPU:** o filho escolhe o device físico pelo `deviceUUID`/`driverUUID` que o app manda (`VkPhysicalDeviceIDProperties`). É a condição do `VK_QUEUE_FAMILY_EXTERNAL`: "the same underlying ... physical device, and the same driver version".
- **Extensões no device do core:** `VK_KHR_external_memory_win32` e `VK_KHR_external_semaphore_win32` entram no `create` do filho e na lista do `create_device` do core. O flycast ignora essa lista, então um hook mínimo do `vkCreateDevice` (`vk_hook.rs`, a mesma ideia do `device_hook` do app) soma as duas extensões e confirma que foram ligadas. Também passou a valer o piso Vulkan 1.1 na instância: o Beetle pede 1.0.32, e numa instância 1.0 a validação acusava as extensões Win32 sem `VK_KHR_external_memory` e o import.
- **Import (spec Vulkan, capítulo Memory):**
  - A imagem é recriada com os mesmos parâmetros da do app: o app manda `flags`, `usage` e o `memoryTypeIndex`.
  - A alocação é dedicada (VUID-VkMemoryDedicatedAllocateInfo-image-01876) e usa o mesmo `allocationSize`/`memoryTypeIndex` (VUID-VkMemoryAllocateInfo-allocationSize-01743).
  - Os semáforos entram por `vkImportSemaphoreWin32HandleKHR`.
  - Importar não passa a posse do handle NT, então o filho fecha os handles quando o anel cai.
- **Posse entre as filas (spec Vulkan, Queue Family Ownership Transfer):**
  - Recurso compartilhado entre instâncias "must also explicitly manage ownership transfers ... regardless of the VkSharingMode".
  - O filho faz o *release* pra `VK_QUEUE_FAMILY_EXTERNAL` (`COLOR_ATTACHMENT_OPTIMAL` → `SHADER_READ_ONLY_OPTIMAL`) e sinaliza o `done`.
  - O app (`AcquireCmds`) espera o `done` em `ALL_COMMANDS` ("the only valid stage to wait on") e faz o *acquire* com os mesmos layouts, num lote próprio antes do lote do wgpu que amostra.
  - A volta pro filho não transfere, porque o conteúdo é descartado ("the ownership transfer should be skipped"; o render pass começa em `UNDEFINED`).
  - O GL (B5) segue sem *acquire*: a issue 9 da EXT_external_objects deixa isso em aberto.
- **Medido** (RTX 3060, `win32_interop_real_gl_core` com `REEMU_VK_CHILD=1`, 15 s cada, camadas de validação no app e no filho, nenhum VUID):

  | Core e jogo | Pelo anel | Pela cópia |
  |---|---|---|
  | flycast, Soulcalibur | 706 | 1 |
  | mupen64plus_next com ParaLLEl, Mario Kart 64 | 883 | 1 |
  | Beetle PSX HW, 40 Winks | 899 | 1 |

  Imagens do flycast e do mupen conferidas. Os 6 testes do anel GL seguem passando.

## 2026-10-11 — C3c: core Vulkan no processo filho vira o padrão no Windows

- **O quê:** no Windows, os cores da lista Vulkan (flycast, Beetle PSX HW, mupen64plus_next com ParaLLEl-RDP, PPSSPP com `ppsspp_backend=vulkan`) passam a rodar no `reemu-core-host`, com o quadro pelo anel compartilhado (C3b). `REEMU_VK_CHILD=0` volta pra rota in-process. No Linux continua opt-in (`REEMU_VK_CHILD=1`): lá o in-process já é zero-cópia por `dma_buf` e o anel Win32 não existe.
- **Por quê:** um crash do core volta a ficar isolado do app (a razão do processo filho); a DLL do core não é carregada no app (fim do vazamento de TLS do flycast e do mupen, ver `vk_core_tls_leak`); e a thread de vídeo do app deixa de emular o core.
- **Validação** (RTX 3060, camadas com a validação de sincronização nos dois processos, `VK_KHRONOS_VALIDATION_VALIDATE_SYNC=true`): nenhum VUID nem SYNC-HAZARD com flycast, mupen64plus_next e Beetle PSX HW. O PPSSPP (Tekken: Dark Resurrection) roda pelo anel, 865 quadros em 15 s, sem VUID. Antes ele caía no filho com `0xc0000005`. A imagem foi conferida (o PPSSPP escreve alfa 0, como já fazia in-process).
- **Medido contra o in-process** (sem validação, 15 s, mesmo jogo):

  | Core | Filho (padrão) | In-process |
  |---|---|---|
  | flycast | 50 fps; app 1,1 ms/quadro | 50 fps; passo do core 20,6 ms (inclui a espera interna do flycast) |
  | mupen64plus_next | 881 quadros, 60 fps; app 0,86 ms/quadro | 835 quadros; passo do core 5,8 ms |
  | Beetle PSX HW | 899 quadros; app 0,85 ms/quadro | 888 quadros; passo do core 2,9 ms |

  O passo do core in-process roda na thread de vídeo do app. No filho, ele roda no processo do core (`retro_run` de 1,2 a 4,9 ms lá), e o app só amostra. Mesma taxa de quadros; no mupen, mais quadros entregues no filho.

## 2026-10-11 — CAS: nitidez adaptativa da AMD como shader próprio

- **O quê:** o AMD FidelityFX CAS (Contrast Adaptive Sharpening), que o pacote `slang-shaders` da libretro não tem, virou um shader próprio do ReEmu embutido no app (`apps/desktop/src-tauri/shaders/cas`), oferecido como o preset curado "Nitidez adaptativa (AMD CAS)". Os shaders próprios são gravados em `<dados>/shaders/reemu` no startup (`bundled_shaders.rs`), fora da pasta do pacote, e funcionam sem baixar o pacote.
- **Fonte oficial:** `ffx_cas.h` 1.20190610 de GPUOpen-Effects/FidelityFX-CAS (MIT; aviso em `THIRD_PARTY_NOTICES.md` e na tela de licenças). O que foi seguido:
  - Caminho só de nitidez (`noScaling`): a ampliação do CAS é limitada a 4× de área (`CAS_AREA_LIMIT`), abaixo do que a emulação pede.
  - Build padrão do filtro: círculo de 5 amostras (sem `CAS_BETTER_DIAGONALS`) e pesos do verde (sem `CAS_SLOW`); `rcp`/`sqrt` exatos no lugar das aproximações de hardware.
  - "This is designed to be a linear filter. Running CAS on perceptual inputs will yield over-sharpening" → com entrada UNORM, a conversão documentada: quadrado na entrada (gama 2.0) e raiz na saída.
  - `sharpness` 0 a 1, padrão 0 ("0 := default (lower ringing)"), com peso `-1/lerp(8, 5, sharpness)`.
  - Uma guarda que a fonte não tem: vizinhança toda preta (máximo 0) não divide por zero.
- **Teste:** `cas_matches_the_reference_math` roda o preset na GPU com degraus de cinza e um ponto claro e compara cada pixel com uma referência em Rust das mesmas fórmulas (diferença ≤ 2 em 0–255); 101 dos 512 pixels saem realçados.

## 2026-10-11 — Saída HDR na janela do jogo

- **O quê:** com "Saída HDR" ligada (Configurações › Sistema › Vídeo), a janela do jogo sai em HDR quando o monitor e o sistema estão com HDR ligado; senão continua em SDR, sem mudança. Há também o "Realce de brilhos" (ligado por padrão), o "Brilho do branco" e o "Pico do realce", estes dois automáticos por padrão. Os shaders do pacote seguem rodando em SDR; o HDR é feito no passe final.
- **Formato (Microsoft, "Use DirectX with Advanced Color on high/standard dynamic range displays"):**
  - FP16 em scRGB, a "Option 1", que "is the only option that works for all types of Advanced Color displays", em que "scRGB (1.0, 1.0, 1.0) encodes the standard D65 white at 80 nits".
  - No wgpu 30: `TextureFormat::Rgba16Float` + `SurfaceColorSpace::ExtendedSrgbLinear`, conferido em `SurfaceCapabilities::format_capabilities`.
  - Num monitor SDR, a Microsoft diz que o DWM corta o que passa de [0, 1]; o ReEmu só liga o scRGB quando a superfície aceita.
- **Branco SDR:** "multiply the SDR color value by SdrWhiteLevelInNits / 80" (mesma página). O automático vem de `Surface::display_hdr_info` → `DisplayLuminance::sdr_white_nits`, que o wgpu lê do `DISPLAYCONFIG_SDR_WHITE_LEVEL` indicado pela Microsoft. Sem esse valor, usa 200 nits ("typically set to around 200 nits").
- **Realce de brilhos:** "there's no single standard algorithm" (Microsoft); a página mostra um Reinhard simplificado `x / (1 + x)`.
  - O ReEmu usa a inversa do Reinhard estendido que leva o branco SDR ao pico: `x = y / (1 − y·k)`, com `k = 1 − branco/pico`, aplicada sobre o maior canal pra não mudar o matiz.
  - Pico automático: `max_full_frame_nits` ("Prefer it over max_nits for large bright regions", wgpu `DisplayLuminance`), porque tela inteira clara é comum em jogo antigo.
  - O código de HDR do RetroArch não foi usado: é GPLv3, e o ReEmu é MIT.
- **Interface (Fluent 2):** Switch com rótulo substantivo e efeito imediato; Slider padrão sem passos visíveis ("brightness controls", "imprecise values"), com o valor atual à vista e "Usar automático"; o valor grava 300 ms depois do último movimento.
- **Medido nesta máquina** (RTX 3060, monitor HDR com HDR ligado no Windows):
  - O monitor informa pico de 1499 nits, 799 nits em tela cheia e branco SDR de 240 nits.
  - A superfície oferece `Rgba16Float` em `EXTENDED_SRGB_LINEAR` e `Rgb10a2Unorm` em `BT2100_PQ`.
  - `hdr_surface_diagnostics`: liga o scRGB numa janela real, apresenta quadros e volta pro SDR, sem VUID com as camadas de validação.
  - `hdr_blit_maps_sdr_to_scrgb`: preto fica em 0, cinza 128 vai a 61 nits e branco ao pico de 799 nits, iguais à referência na GPU.
- **Também:** o binding 0 do `comp.bgl` passou a ser visível no fragment shader (o passe HDR lê os parâmetros ali), e salvar o integer scaling manda a configuração de vídeo inteira (antes mandava só o campo, o que apagaria o HDR).

## 2026-10-11 — Movimento e elevação pelo Fluent 2; chevrons do tamanho do Xbox

- **Chevron "Mostrar tudo"** (cabeçalho das prateleiras na Início e na Biblioteca): medido num print do app Xbox a 3840×2160 (3,07 px por epx), o traço do chevron tem 6×12 epx, que é o `ChevronRight` de 20. O do ReEmu era o de 16, num botão de 24. Agora é ícone de 20 (`ICON`) num botão de 32 (`TARGET_MIN`), 6 epx depois do título, centrado nas maiúsculas.
- **Setas do destaque da Início:** de 32/16 para 40/24. A seta das prateleiras do Xbox tem 45 epx e um chevron de ~23 epx de altura.
- **Movimento** (<https://fluent2.microsoft.design/motion>):
  - Troca de tela só com fade ("Top level: use a quick fade. Avoid sliding large elements"). Antes, a tela subia 14 px ou vinha da esquerda no "voltar". Duração e curva do `Fade` do Fluent: `durationNormal` e `curveEasyEase`.
  - Durações e curvas soltas (150, 170, 190, 220, 240, 260, 320, 360, 380 e 800 ms, `ease-in`/`ease-out` e cúbicas próprias) passaram para os tokens do Fluent (`durationFaster` … `durationUltraSlow`, `curveDecelerate*`, `curveAccelerateMid`, `curveEasyEase`), com os valores conferidos no `@fluentui/tokens` instalado. A saída do menu de pausa ficou mais curta que a entrada e acelerando.
  - Faltavam guardas de `prefers-reduced-motion` em dois lugares, que agora têm: o zoom das setas do destaque e o deslocamento da página de jogo carregando.
  - A entrada do onboarding (fundo de 1,1 s, palavras de 700 ms) ficou como está, porque é o momento de abertura.
- **Elevação** (<https://fluent2.microsoft.design/elevation>): os cards já seguiam a escala (`shadow4` em cards, `shadow16` na capa sem borda, diálogos e popovers com as sombras padrão do Fluent). Os cards do hub de Configurações já subiam para `shadow8` no hover; agora também descem para `shadow2` quando pressionados. Os cards de jogo continuam sem sombra: no Windows, a orientação é usar borda em vez de sombra principal.

## 2026-10-11 — Temas Simple Vibes e Serene; botões da Início

- **Simple Vibes:** tema só claro, sem a opção de escuro. É todo em brancos e cinzas: casca `#F0F0F1`, superfícies brancas, e a "marca" é uma rampa cinza (matiz 220, 4% de saturação), então botões e seleção ficam em grafite.
- **Serene:** tema só escuro. Cinzas escuros levemente azulados com acento azul-ardósia apagado (matiz 212). O texto de marca usa o tom 100: o 90 dava 4,1:1 nas superfícies, abaixo do AA.
- **Fundo de uma cor só nos dois:** nova opção `solidBg` em `make()`. Os 4 brilhos dos cantos e o véu viram a cor do `Background1`, como no alto contraste, e a casca fica lisa. O papel de parede, se a pessoa escolher um, continua aparecendo na Início.
- **Família de tema só claro:** `ThemeFamily.lightOnly`, e `familyOf` devolve `light` para ela.
- **Testes** (`themes.test.ts`): fundo de uma cor só, contraste AA (texto, marca, item ativo, botão primário) e modo único de cada um.
- **Início:**
  - "Mostrar tudo" das prateleiras passou a `appearance="subtle"`, sem fundo em repouso, como no app Xbox.
  - "Todos os jogos" e "Adicionar ROMs" passaram a `outline`.

## 2026-10-11 — Anel de foco do 1º card das prateleiras

- O anel do card vai 9 epx para fora: 6 de afastamento mais 3 de traço.
- A folga lateral da prateleira (`SHELF_PAD`) era 8, e o `overflow-x: clip` cortava 1 px do traço à esquerda do 1º card. O mesmo valia para a direita do último.
- Medido numa captura com o card focado: 2 px à esquerda contra 3 nos outros lados.
- Com `SHELF_PAD` = 12 o traço fica inteiro. A posição dos cards não muda, porque a margem negativa do `.wrap` acompanha a constante.

## 2026-10-11 — Topbar alinhada ao conteúdo

- O botão de voltar e o relógio agora seguem as margens do conteúdo (`SAFE_X`, 47 epx dos dois lados). Antes a topbar era mais larga, como no app Xbox: 17 à esquerda e 13 à direita.
- Medido com os dados de teste: o voltar começa em x = 115, a borda esquerda do destaque e dos cards. O texto do relógio termina em x = 1319 a 1366 px de largura e em 1873 a 1920 px, a borda direita do destaque. A busca continua centralizada.

## 2026-10-11 — Configurações com a mesma altura com ou sem controle

- Com controle conectado, a página das Configurações encolhia 56 px para a barra de dicas (A/B…) não cobrir o pé dos cards.
- Agora a altura é sempre a mesma, e a barra flutua por cima do canto inferior direito do card, como no app Xbox. Foi o pedido do usuário, entre as duas opções oferecidas.

## 2026-10-11 — Botões da Início no estilo dos filtros do app Xbox

- "Todos os jogos" e "Adicionar ROMs" viraram pílulas como os filtros da biblioteca do app Xbox (`useBrowseStyles.pillBtn`). Medido num print a 3840×2160: 32 epx de altura, texto regular, preenchimento #3E434B sobre a página #1A1B1E e borda de 1 epx #6B7584.
- As cores saem do tema ativo, misturando o fundo da página (`colorNeutralBackground1`) com a cor do texto: 14% no preenchimento e 32% na borda, mais forte no hover e mais fraco ao pressionar. No escuro a pílula fica um cinza mais claro que o fundo; no claro (Simple Vibes), um cinza mais escuro.
- Ajuste no mesmo dia, a pedido do usuário: o preenchimento das pílulas passou a ser o dos outros botões do tema (`colorNeutralBackground2`, como o voltar, a tela cheia e os filtros da biblioteca). A borda continua derivada do tema.

## 2026-10-11 — Serene: branco como destaque e superfícies mais separadas

- **Destaque branco:** como no modo XBOX do app Xbox (print do usuário, 3840×2160), ficam brancos a aba selecionada, o anel de foco, o switch e o rádio marcados e o botão primário. Os tons 70–160 da rampa são brancos ou quase, e os 10–50 cinzas escuros. É o mapeamento do `createDarkTheme`: `colorBrandBackground` = 70, foreground e traço de marca = 100.
  - Ajustes à mão: o texto sobre o branco é escuro (`colorNeutralForegroundOnBrand` #111316, como o chip "Instalado" do Xbox, preto sobre #E9E9E9). O pressionado do botão primário, que seria o tom 40, escuro, vira cinza claro.
  - O item ativo do rail é cinza claro (#62666C) com ícone branco, como no Xbox (#67696C).
- **Superfícies:**
  - Cards, rail e botões estavam quase da cor da página: #1A1F25 sobre #15191E, cerca de 1,06:1. Agora seguem as proporções medidas no Xbox (página #1A1B1E, superfícies #25282C): página #1A1C20 e superfícies #262A2F, cerca de 1,2:1.
  - Fontes da Fluent 2: › Color, "Use lighter neutrals on surfaces to highlight areas of primary focus"; › Elevation, "Windows uses strokes instead of key shadows". As bordas dos controles subiram de 12% para 20% de branco, perto da borda dos filtros do Xbox.
  - O card das Configurações, que tinha a cor da página e só se separava pela sombra, sobe um degrau (#212429). Linhas e campos sobem mais um cada (nova opção `lift` em `make()`).
- **Testes:** `themes.test.ts` confere o destaque branco, o contraste do texto sobre o botão pressionado e a diferença mínima entre página, card, linha e campo.
- **Ajuste no mesmo dia (print do Perfil no Serene):**
  - O card das Configurações ainda ficava perto da página (#212429 sobre #1A1C20, 1,08:1). Agora usa a cor das superfícies (#262A2F, 1,18:1, a distância página → superfície do Xbox), e linhas e campos sobem mais um degrau de 1,18:1 cada.
  - O texto secundário foi para #A9B0B9, que passa AA também sobre os campos.
  - O card das Configurações ganhou um traço de 1 px (`colorNeutralStroke2`) em todos os temas. No escuro, o `shadow4` quase some sobre fundo escuro, e "Windows uses strokes instead of key shadows" (Fluent 2 › Elevation).
  - Obs.: os prints anteriores do usuário ainda mostravam o Serene antigo (#15191E, botão azul): a janela do `tauri dev` precisa ser recarregada (Ctrl+R) para ler o `themes.ts` novo.

## 2026-10-11 — Botão "Mostrar tudo" das prateleiras: espaço e foco

- Medido no print do usuário (app Xbox, "Jogar com mouse e teclado", 3840×2160): caixa de 24×24 epx, a 18 epx do fim do título, e fundo #4E535B sobre a página #1A1B1E no hover/foco.
- O ReEmu estava com 32 px e a 6 px do título. Agora tem 24 px, ícone de 20 e 18 px de distância (conferido: título termina em 303 e o botão começa em 321).
- No foco (teclado ou controle), o fundo acende como no hover. A cor é o fundo da página misturado a 22% com a cor do texto do tema, perto do #4E535B do Xbox no escuro.

## 2026-10-11 — Último card das prateleiras ("ver todos" 2×2) destacado do fundo

- O card 2×2 do fim de cada prateleira tinha fundo `colorNeutralBackground1`, a cor da própria página, e sumia, sobretudo nos temas de fundo liso.
- Agora usa o mesmo fundo dos cards de jogo ao lado (`Background4` → `Background3`), e as células sem capa ficam em `Background2`.
- Conferido em capturas nos temas Serene, Simple Vibes e Neon Circuit. Uma 1ª tentativa, misturando o fundo com a cor do texto, deixava o card cinza-escuro no tema claro, destoando dos cards claros ao lado.

## 2026-10-11 — "Adicionados recentemente" dentro de um card na Início

- Como "Transmita seu próprio jogo" no app Xbox. Medido no print do usuário (3840×2160, 3,07 px por epx):
  - o card passa 32 epx dos cards de jogo para cada lado, e o conteúdo continua alinhado com o resto da página;
  - 33 epx embaixo, o "T" do título a 18 epx do topo e cantos de 16;
  - o fundo é a página com um leve tom de cor (#1B282E sobre #1A1B1E).
- No ReEmu (`useBrowseStyles.featured`, prop `featured` da `Row` da Início), o fundo é o do tema com 8% da cor de destaque.
- Com 12%, o card do Serene (destaque branco) ficava da cor dos cards de jogo de dentro. Conferido em capturas no Serene, no Neon Circuit e no Simple Vibes.
- Os cantos de 16 fogem do raio único de 4 do app, de propósito: é o raio do card do Xbox, pedido pelo usuário.
- **Ajuste no mesmo dia (pedido do usuário):**
  - O card não passa mais das margens da página: vai de 115 a 1319 a 1366 px, as mesmas bordas do destaque.
  - O conteúdo fica com 32 de padding nos 4 lados. A prateleira mede a própria largura e reajusta quantos cards cabem e o tamanho deles.
  - Medido a 1366, 1051 e 1920 px: 32/32/32/32 em todas, com 5, 4 e 8 cards.
- **Fundo pela escala de elevação:** o card de "Adicionados recentemente" deixou o tom da cor de destaque e passou a usar a mesma superfície, traço e sombra do card das Configurações.
  - Superfície: `reemuSurface1`, um degrau acima da página e acompanhando o tema.
  - Sombra: `shadow4`, que a Fluent 2 indica para cards.
  - Traço: 1 px de `colorNeutralStroke2` ("Windows uses strokes instead of key shadows").
  - Os cards de jogo de dentro ficam mais um degrau acima. Conferido em capturas no Serene, no Neon Circuit, no Simple Vibes e no Alva claro.
- Borda tirada a pedido do usuário. Depois ganhou um degradê leve na diagonal: o canto de cima à esquerda puxa 10% para a cor de destaque do tema e some até 60% do card. Conferido no Neon Circuit (esverdeado), no Serene e no Alva claro (azulado).
- Degradê refeito a pedido do usuário: agora vai da cor do sidebar (o rail, `colorNeutralBackground2`) com 14% da cor de destaque, no canto de cima à esquerda, ao tom do rail puro aos 60%. O fundo do card passa a ser o tom do rail. Conferido no Neon Circuit (verde-escuro do rail, esverdeado no canto), no Simple Vibes (branco do rail) e no Serene.

## 2026-10-11 — Cor de destaque dos ícones e indicadores (claro e escuro)

- **Problema:** no escuro, o `createDarkTheme` do Fluent pinta switch, rádio, checkbox, traço de foco e de item selecionado com o tom 100 da rampa, e o texto de marca com o 100 ou o 90 (`@fluentui/tokens`, `alias/darkColor`). Em várias rampas esses tons ficam desbotados, longe da cor do tema.
  - O favorito usava `reemuBrandText` (tom 90 no escuro, 40, quase preto, no claro), e a aba ativa tinha o indicador sempre branco.
- **Correção:** token `reemuBrandAccent` para ícones e indicadores.
  - É o tom primário (80, o mesmo do `reemuBrandSolid` e do botão primário no claro) sempre que ele tem 3:1 com a página e as superfícies (WCAG 2.2, 1.4.11 "Non-text Contrast").
  - Senão, é o tom mais próximo que tenha: mais claro no escuro, mais escuro no claro (`pickAccent`).
  - Os tokens de indicador do Fluent (`colorCompoundBrand*`, `colorBrandStroke1`) apontam para ele.
  - O texto de marca continua com o tom AA (4,5:1).
- **Usam o destaque:** favorito na página do jogo, indicador da aba ativa, ícones do hub de Configurações, notificação não lida, switch, rádio, checkbox, foco e borda de item selecionado.
  - O coração sobre a capa dos cards usa o tom 80 puro, porque fica sempre sobre fundo escuro.
- **Resultado por tema:** 13 dos 15 usam o primário exato.
  - Ajustados: Green Modern claro (#52B72A → #3A7920, o verde não tinha 3:1 sobre o cinza claro) e Purple Mistique escuro (#5B4FCF → #7E74DC).
  - Teste em `themes.test.ts`: 3:1 em todos os temas e indicadores do Fluent iguais ao destaque.

## 2026-10-11 — Card de jogo: sem sombra; brilho na cor do tema no hover/foco (teste)

- Pedido do usuário como teste: tirar a sombra dos cards de jogo e, no hover/foco, mostrar um brilho leve na cor primária do tema, "como no menu do Xbox Series X".
- **Referência oficial:** "Reveal Focus" da Microsoft para Xbox/10-foot (<https://learn.microsoft.com/windows/uwp/ui-input/reveal-focus>): "an animated glow around the element's border", com borda principal de 2 px e o brilho por fora, na cor `SystemAccentColor` por padrão.
  - A busca não achou documentação específica do destaque do dashboard do Series X além do sistema de cores de destaque.
- **No ReEmu:**
  - Em repouso, `boxShadow: none` (o `<Card>` do Fluent punha `shadow4`).
  - No hover e no foco: `0 0 18px 2px` do `reemuBrandAccent` a 55%, junto do anel de foco que já existia (`useCardStyles.card`). Sem a animação de pulso.
  - Também vale para o card 2×2 do fim das prateleiras. Conferido em capturas no Neon Circuit, no Crimson Retro e no Simple Vibes.
- **Para voltar ao estado anterior:** `git revert` deste commit.

## 2026-10-11 — Cards de tema: anel de foco padrão e X do controle para claro/escuro

- **Anel:** os cards de tema em Configurações › Aparência usavam o anel do `<Card>` do Fluent (um `::after` com borda). Agora usam o anel padrão do app: 3 px na cor de destaque, afastado 4 px. Num card selecionável, o foco fica no checkbox escondido do Fluent (`.fui-Card__checkbox`), então o anel vai no card por `:has(> .fui-Card__checkbox:focus)`.
- **X do controle:**
  - Novo `NavPulse::Action`, botão West do gilrs, índice 3 em `gilrs_button_index`, emitido como `menu-nav` = `action`.
  - `useMenuNav` dispara o evento `reemu-action` no elemento focado, e ele borbulha. O card de tema escuta e troca claro/escuro (só nos temas que têm os dois modos).
  - A barra de dicas mostra "X Claro/escuro" nessa tela (pt-BR, en, es).
- **Conferido no navegador headless:** foco no card, anel 3 px a 4 px na cor do tema, e o evento do X liga e desliga o Switch "Claro". `cargo clippy -D warnings`, `cargo fmt --check` e os testes do `input-desktop` (23) passaram.
- **Não testado com controle físico.**

## 2026-10-11 — Tema Dream Sky (menu do Dreamcast)

- **Pesquisa:**
  - O menu do Dreamcast tem "a blue background that looks like the sky", com Play/File/Music/Settings (GameFAQs, top 10 boot menus).
  - O logo é a espiral laranja nos consoles americanos e azul nos PAL, por marca registrada (AVID wiki; GameSpot).
  - Não há códigos de cor oficiais publicados: as cores abaixo são escolhidas a partir dessas descrições, não amostradas de material oficial.
- **Tema "Dream Sky"** (sem a marca no nome, como os outros), escuro e claro:
  - Marca no laranja da espiral (matiz 26, na curva do "Personalizado").
  - Brilhos do fundo: céu azul, azul-claro de nuvem, azul fundo e laranja da espiral.
  - Escuro: céu noturno azul-marinho. Claro: céu claro com superfícies brancas.
- **Testes:** contraste AA de texto e marca nos dois modos. O destaque de ícones (`pickAccent`) passa 3:1 no teste geral.
- O "Personalizado" continua o último da lista de temas.

## 2026-10-11 — Release v0.1.7

- **GPU (Windows):**
  - Os cores OpenGL desenham direto nas imagens Vulkan do app, sem cópia (B1–B6: anel compartilhado, semáforos, validação de sincronização limpa). Medido a 1920×1440: 2,14 ms por quadro no app, antes 5,12.
  - Os cores Vulkan (flycast, Beetle PSX HW, mupen64plus/ParaLLEl, PPSSPP) rodam no processo filho com device próprio e anel compartilhado, como padrão. Isso tira do app o vazamento de TLS desses cores (C3a–C3c).
- **Vídeo:**
  - Saída HDR (scRGB) na janela do jogo, com branco SDR e pico lidos do monitor e realce de brilhos.
  - Shader AMD FidelityFX CAS embutido.
- **Interface:**
  - Movimento e elevação pelo Fluent 2 (troca de tela só com fade, tokens de duração e curva).
  - Tamanhos medidos no app Xbox: chevron "Mostrar tudo", setas do destaque, pílulas "Todos os jogos" / "Adicionar ROMs", topbar alinhada ao conteúdo.
  - "Adicionados recentemente" vem primeiro, num card com degradê na cor do tema.
  - Brilho na cor do tema no hover/foco dos cards de jogo.
  - Anel de foco inteiro no 1º card das prateleiras.
  - Dica do "Remover" que abre sozinha.
- **Temas:**
  - Novos: Simple Vibes (claro), Serene (escuro, destaque branco) e Dream Sky (menu do Dreamcast, escuro e claro).
  - Ícones e indicadores na cor primária do tema com 3:1 garantido (`reemuBrandAccent`), nos modos claro e escuro.
  - Superfícies mais separadas do fundo.
  - Aparência: aba "Temas" em até 4 colunas, anel de foco padrão e X do controle para claro/escuro.
- **Controle:** botão X chega à interface (`menu-nav` "action"). As Configurações têm a mesma altura com ou sem controle.
- **Linux:** release no Ubuntu 22.04 (glibc 2.35).

## 2026-10-12 — Anel de foco dos cards de jogo no estilo do dashboard do Xbox (teste)

- **Medido em prints do usuário** (Configurações do Xbox, 1926×1080, card "Enable 4k UHD resolution"), de fora para dentro:
  - brilho de ~10 px na cor de destaque;
  - borda de 3 px num verde mais claro (#4E9148) que o verde Xbox;
  - faixa escura de ~4 px (#1C3414) colada no card;
  - o card focado clareia (#303030 → #454545).
  - No menu-guia, o botão "Store" segue o mesmo padrão. São as três camadas do Reveal Focus da Microsoft: borda principal, secundária escura e brilho.
- **No ReEmu** (`useCardStyles.card`):
  - `box-shadow` em camadas, que segue o raio do card, no lugar do `outline` global: 3 px do fundo do tema escurecido, borda até 5 px no destaque com 25% de branco e brilho `0 0 8px 4px` do destaque a 45%.
  - Véu branco de 8% sobre a arte do card focado (`[data-art]::after`). Vale no hover e no foco.
  - Cabe na folga da prateleira (`SHELF_PAD` = 12).
- **Substitui** o brilho largo do teste anterior (`4961fe8`). Para voltar a ele: `git revert` deste commit.

## 2026-10-12 — Gaveta de informações do jogo com o degradê do menu-guia do Xbox

- **Referência:** print do menu-guia do Xbox enviado pelo usuário: cinza escuro até ~1/3 da altura (abaixo de "My games & apps"), depois desce para a cor de destaque, verde, com o texto branco por cima.
- **No ReEmu** (`useDetailStyles.infoDrawer`): o fundo da gaveta (`Background1`) até 35% da altura e, no pé, o mesmo fundo com 25% de `reemuBrandAccent`.
- **Por que 25%:** é o máximo em que o texto principal e o secundário seguem AA (4,5:1) em todos os temas. O mais apertado é o Serene, com 4,9:1 no secundário. Com 35–40%, Serene e os temas claros caem abaixo de 4,5. O Xbox usa um verde mais forte embaixo.
- Conferido em capturas no Neon Circuit, no Serene e no Alva claro.
- **Ajuste (pedido do usuário):** a faixa entre o card e o anel ficou transparente.
  - O anel virou `outline` de 2 px, afastado 3 px, num tom mais claro do destaque. Ele acompanha o raio do card no Chromium/WebView2 e no WebKit atual.
  - O brilho passou para um `::before` do tamanho da borda externa do anel, para o `box-shadow` não pintar o vão.
  - Conferido no Neon Circuit e no Simple Vibes.

## 2026-10-12 — Brilho do foco comparado com o Xbox (e o corte que o escondia)

- **Comparação medida,** com o ReEmu capturado na escala do print do Xbox (1080p sobre a tela de 768: ×1,406). Tile do Starfield na tela inicial do Xbox, 1920×1080, de fora para dentro:
  - brilho de ~13 px (G +5 → +35 sobre o fundo);
  - borda de 4 px (#40924A);
  - vão de ~4–5 px levemente esverdeado pelo brilho;
  - o card.
- **Achado:** o brilho dos testes anteriores nunca aparecia. O `<Card>` do Fluent põe `overflow: hidden` na raiz e cortava o `::before`, que fica fora da caixa. Agora o card fica com `overflow: visible`; quem recorta a arte é o `.art`.
- **Calibrado para o perfil do Xbox:**
  - `0 0 12px` do destaque a 45%, sem spread, some em ~13 px como no Xbox;
  - a folga da prateleira (`SHELF_PAD`) e o padding vertical subiram para 16, para o brilho não ser cortado no 1º/último card. A posição dos cards não muda.
- **Diferenças que ficam:**
  - a cor do anel é a do tema (pedido do usuário: o padrão do app);
  - o vão é transparente (pedido do usuário; no Xbox o brilho tinge o vão);
  - o tile do Xbox focado cresce e mostra o nome numa faixa, o do ReEmu não.

## 2026-10-12 — O foco do card de jogo vira o foco padrão do app

- **Pedido do usuário:** o mesmo efeito do card de jogo em todo o app. Constantes em `styles/xbox.ts`:
  - `FOCUS_OFFSET` foi de 4 para 3;
  - novo `FOCUS_GLOW`: `0 0 12px` do `colorBrandStroke1` a 45%.
- **Aplicado em:** regra global `.app a:focus, [tabindex]:focus` (rail, cards do hub, links), abas (`.fui-Tab` global e `useTabStyles`: antes 2 px a 5 px), botões do Fluent em foco de teclado, logo da rail, botões da página de jogo e da pausa e cards de tema da Aparência. O anel é sempre 3 px na cor de destaque, afastado 3 px, com o brilho.
- **Fora do card de jogo,** o brilho sai da borda do elemento e tinge de leve o vão até o anel, como no Xbox. Nos outros elementos não há um pseudo-elemento livre: o Fluent usa `::before`/`::after` em botões, abas e cards. No card de jogo o vão continua transparente, com o brilho no `::before`.
- **Conferido** em capturas (rail e aba focadas). Testes do frontend (99) e `ui-smoke` passaram.

## 2026-10-12 — Anel nos botões focados pelo controle

- **Problema:** o `<Button>` do Fluent só desenha foco quando ele vem do teclado (`data-fui-focus-visible`). O `moveFocus` do controle e das setas chama `.focus()`, que não conta, e os botões focados pelo controle (ex.: "Plataforma" na Biblioteca) ficavam sem anel.
- **Correção:**
  - `moveFocus` marca o `<html>` com `data-nav-focus`, e o primeiro `pointerdown` (mouse/toque) tira a marca.
  - Com a marca, o `.app` (`useShellStyles`) mostra o anel padrão em `button:focus`: 3 px na cor de destaque, a 3 px, com o brilho. O mouse continua sem deixar anel nos botões clicados.
- **Conferido no navegador headless:** setas ativam a marca, o botão focado mostra `solid 3px` a 3 px, e o clique desliga a marca.

## 2026-10-13 — Foco padrão em todo componente focável

- **Pedido do usuário,** depois de aprovar o efeito: o mesmo foco (anel de 3 px na cor de destaque, a 3 px, com brilho curto) em todo componente que recebe foco.
- **Varredura** (navegador headless: foca cada elemento focável de 16 telas no modo de controle e lê o anel). Faltavam:
  - campos de texto e busca, `Select`;
  - switch, rádio, slider;
  - cards selecionáveis de shader;
  - botões da rail ("Encerrar", sino);
  - os portais do Fluent (menus, diálogos, gavetas), que ficam fora do `.app` e não pegavam as regras do Griffel.
- **CSS global** (`index.css`, seção "foco"), que vale também nos portais, porque os tokens são variáveis CSS do `FluentProvider` e dos nós de portal:
  1. **Teclado:** o anel do próprio Fluent (`::after` sob `[data-fui-focus-visible]`/`[data-fui-focus-within]`, `createFocusOutlineStyle` do `@fluentui/react-tabster`) é restilizado para 3 px a 3 px com o brilho. Botões do Fluent no teclado (`.fui-Button[data-fui-focus-visible]`) também.
  2. **Controle/setas** (`html[data-nav-focus]`): anel no elemento focado ou, quando o foco cai num `<input>` escondido, no componente visível (switch, rádio, checkbox, slider, card selecionável).
  3. **Campos:** anel em volta do campo inteiro quando o foco é visível. A barra de baixo do Fluent continua.
- **Ajustes:**
  - switch/rádio/checkbox ganham o raio padrão;
  - o switch encolhe para o conteúdo no foco, porque esticava na linha do `<Field>`;
  - itens de menu/lista (`menuitem`, `option`, `treeitem`) usam o anel por dentro (−3 px), senão o `overflow` do popover cortava.
- **Exceções que continuam com o anel próprio:** cards de jogo (vão transparente) e cards de tema.
- Os campos de texto, que antes ficavam de fora a pedido do usuário, agora entram, pelo pedido novo de "todos os componentes".
- **Resultado:** a varredura não acha nenhum elemento sem o anel. No teclado (Tab) também conferido. 99 testes e `ui-smoke` (12/12) passaram.
- **Diálogos (ajuste do mesmo dia, print do diálogo "Encerrar"):** o anel das opções, que ocupam a largura toda, era cortado nas laterais pelo `overflow-y: auto` do `DialogContent`. O Fluent já reserva 2 px de respiro com margem negativa para o anel dele (`useDialogContentStyles`). O ReEmu usa o mesmo truque com 12 px, em todos os diálogos e sem mover o conteúdo. Conferido em captura.

## 2026-10-13 — Abas alinhadas com o conteúdo, como nas Configurações do Windows

- **Comparado** com o print das Configurações do Windows 11 (Personalização › Tela de fundo): título da seção, rótulos e controles começam todos no mesmo x (393).
- **No ReEmu,** o texto da 1ª aba ficava 15 px para dentro do conteúdo, por causa dos 13 px de padding da aba e dos 2 px do conteúdo do Fluent. Medido: texto em 155 com a grade em 140 (Aparência, Sistema) e texto em 130 com os cards em 115 (Biblioteca, página do jogo).
- **Correção:** `useTabStyles.tabs` com `marginLeft: -15px`. O texto agora coincide com o conteúdo em todas as telas com abas. O fundo do hover e o anel de foco da 1ª aba usam a margem.

## 2026-10-13 — Caminho das Configurações com o `Breadcrumb` do Fluent 2

- O caminho "Configurações › Vídeo › Molduras" era montado à mão (links + `Title2` + chevrons). Agora usa `Breadcrumb` / `BreadcrumbItem` / `BreadcrumbButton` / `BreadcrumbDivider` do Fluent.
- Segue a página de uso do Fluent 2 (fluent2.microsoft.design/components/web/react/core/breadcrumb/usage): `nav` com `aria-label`, lista ordenada, divisores fora do foco e da leitura, `aria-current="page"` no último item.
- **Tamanho:** o maior do Fluent é 16 px. O texto vai em `fontSizeHero700` (28 px, semibold), como o título-caminho das Configurações do Windows 11. Níveis de cima em `Foreground3`, que vai para `Foreground1` no hover; o atual em `Foreground1`.
- **Alinhamento,** comparado com o print do Windows Update, em que o caminho começa na mesma coluna do conteúdo: o botão tem 8 px de padding, então a lista volta 8 px. O texto "Configurações" fica em x = 116, na borda do card (115).
- Foco: varredura sem falhas (anel padrão). `ui-smoke` 12/12.

## 2026-10-13 — Revisão dos componentes: o que não usava o Fluent 2

- **Levantamento:** o app já usava o Fluent em quase tudo: Button, Field/Input, TabList, Card, Dialog, Menu, MessageBar, toasts, Accordion, Skeleton, Avatar, ColorPicker, Breadcrumb e outros. Elementos HTML soltos eram poucos.
- **Trocados pelo componente do Fluent:**
  1. **Carrossel da Início:** `Carousel` (`@fluentui/react-carousel`, no `react-components` 9.74), com `motion="fade"`, `layout="overlay-expanded"` (setas nas laterais, pontos embaixo), `autoplayInterval` de 7 s e o botão de pausar.
     - O Fluent 2 pede: "the user must always have the option of pausing the auto-play feature"; o antigo só pausava com o mouse em cima.
     - Anúncio do slide para leitor de tela (`announcement`, "Destaque N de M").
     - Os slides inativos ficam `inert`.
     - Visual mantido: altura, véu, texto branco, setas de 40 px.
     - O texto subiu para ficar acima da linha de controles. O anel do slide ativo vai por dentro, porque o carrossel corta o que passa da borda.
  2. **"Detalhes técnicos" no erro do jogo:** `Accordion` no lugar de `<details>`/`<summary>`.
  3. **Avatar da rail:** `Button` (transparente) no lugar de `<button>`.
  4. **Notas de versão (janela de atualização):** `List`/`ListItem` no lugar de `<ul>`/`<li>`, com os marcadores mantidos por CSS.
- **Mantidos próprios** (o Fluent não tem equivalente que sirva):
  - a rail estilo Xbox: o `Nav` do Fluent é gaveta com texto, sem modo só de ícones;
  - a prateleira (`Shelf`), sobre `Card`;
  - os cards de navegação das Configurações, que são links e o `Card` não vira link;
  - dicas do controle, relógio, splash, fundo animado, avatares prontos (exibidos dentro do `Avatar`), estados vazios e `<kbd>` nos atalhos.
- **Não conferidos na tela:** o "Detalhes técnicos" (o erro de core não sai com os mocks) e as notas de versão. Ficam pela checagem de tipos e pelo DOM do Fluent (`List` → `ul`, `ListItem` → `li`).
- **Ajuste (pedido do usuário):** o carrossel ficou sem o botão de pausar/retomar e avança sozinho.
  - O avanço liga por `enableAutoplay` do contexto do `Carousel` (`useCarouselContext_unstable`), o mesmo que o botão chama. O Fluent continua pausando com o mouse ou o foco em cima.
  - Fica registrado que o Fluent 2 recomenda o controle de pausa ("the user must always have the option of pausing the auto-play feature").
  - Conferido: slide 0 → 1 em 8 s → 2 em 15 s. O texto voltou para `bottom: 24px`.
- **Ajuste (pedido do usuário: nomes grandes encostavam nos controles):**
  - Os pontos do carrossel foram para o canto de baixo à direita, a 24 px da borda, como o texto à esquerda (o Fluent centraliza). O texto continua embaixo à esquerda, como no Xbox.
  - O título vai a no máximo 2 linhas com "…", porque um nome muito comprido subia até a seta da esquerda. O nome inteiro segue no texto do botão para o leitor de tela.
  - Conferido com um título de 2+ linhas: o texto termina em x = 885 e os pontos começam em 1151.

## 2026-10-13 — Cards que se separam da página em todos os temas

- **Print do usuário (Simple Vibes, Biblioteca):** os cards de jogo sem capa quase não se viam.
- **Medido em todos os temas** (contraste com a página): `Background3`/`Background4`, o degradê dos cards, ficavam a 1,04–1,11:1 nos claros e a 1,10–1,20:1 nos escuros de console. O card sem capa ainda terminava o degradê na própria cor da página.
- **Tokens novos** `reemuCardTop`/`reemuCardBottom`/`reemuCardStroke` (`cardSurface` em `themes.ts`), calculados a partir da página de cada tema:
  - **Escuro:** página clareada pela cor do texto, 14% no topo e 8% na base (~1,4 e ~1,2:1). Fluent 2 › Color, "Use lighter neutrals on surfaces".
  - **Claro:** a superfície branca do tema (`Background2`), indo 35% para a página na base, com traço de 1 px a 12% de preto. Fluent 2 › Elevation, "Windows uses strokes instead of key shadows", como os cards das Configurações do Windows.
  - **Alto contraste:** preto com traço branco.
- **Usados em:** arte do card de jogo (por trás da capa), card sem capa e card 2×2. As células do 2×2 passam a ter a cor da página, o que dá contraste com o card nos dois modos.
- **Teste novo:** escuro com topo ≥ 1,3:1 e base ≥ 1,15:1; claro com topo ≥ 1,08:1 e traço visível, em todos os temas.
- Conferido em captura no Simple Vibes, no Blue Modern e no Green Modern claro.

## 2026-10-13 — Fonte do Windows 11 e o texto serrilhado

- **Serrilhado:** no PC do usuário a opção do Windows "Suavizar bordas das fontes de tela" está desligada (`HKCU\Control Panel\Desktop\FontSmoothing = 0`, `SPI_GETFONTSMOOTHING = 0`).
  - O Chromium, e portanto o WebView2, segue essa opção e desenha o texto sem antialiasing: 0 tons intermediários na borda das letras, medido no app e no Chrome headless.
  - As Configurações do Windows (WinUI) suavizam por conta própria, por isso parecem lisas.
  - Não é o CSS do ReEmu: desligar `-webkit-font-smoothing`/`text-rendering` e trocar a fonte não mudou nada. A correção é ligar a opção no Windows.
- **Fonte:**
  - O Fluent usa "Segoe UI" (a estática do Windows 10). O Windows 11 usa a Segoe UI Variable (learn.microsoft.com/windows/apps/design/signature-experiences/typography: "When using HTML ... you will need to specify the Segoe UI Variable font in CSS").
  - No Chromium os nomes que casam são "Segoe UI Variable Text" e "Segoe UI Variable Display"; "Segoe UI Variable" sozinho cai na fonte de reserva (medido com `CSS.getPlatformFontsForNode`).
  - Agora `fontFamilyBase` começa com a Text, e os títulos (`Subtitle1` e acima, título de prateleira, caminho das Configurações, título do destaque) usam a Display (`--reemuFontDisplay`), como a rampa do Windows ("Display semibold" do Subtitle de 20 px para cima). A pilha do Fluent continua depois, para o Linux.
- **Tela cheia:** o ícone tinha 16 px. Agora tem 20 px, como o voltar e os outros botões de ícone da topbar (`navIconLg`).

## 2026-10-13 — Selawik como fonte no Linux

- **Problema:** no Linux não há Segoe, e a pilha do Fluent caía na `sans-serif` de cada distro (DejaVu, Noto, Cantarell…). O visual e a quebra das linhas mudavam de distro para distro.
- **Correção:** embutida a **Selawik** (Microsoft, <https://github.com/microsoft/Selawik>, release 1.01, SIL OFL 1.1). É a fonte aberta com as métricas da Segoe UI que a própria página de tipografia do Windows indica para apps fora do Windows.
  - Arquivos: os 5 pesos em `.woff2` (300/350/400/600/700, ~73 KB no total), sem modificação, com `OFL.txt` ao lado.
  - `@font-face` em `index.css`.
  - Pilha (`segoeVariable`): Segoe UI Variable Text/Display → Segoe UI → **Selawik** → resto do Fluent.
  - Créditos em `THIRD_PARTY_NOTICES.md` e em Sistema › Informações e licenças (pt-BR/en/es).
- **Conferido no Chrome:** no Windows a Segoe UI Variable continua vencendo e nenhum arquivo da Selawik é baixado. Com só a Selawik, uma frase com acentos do pt-BR, ç/ã/õ/ñ/¿/«» e travessão sai inteira nela (Regular e Semibold), sem fallback. O build do Vite inclui os `.woff2`.
- **Não testado num Linux de verdade.**

## 2026-10-13 — Elevação como no Fluent e no modo XBOX

- **Comparado** com prints do app Xbox (Biblioteca e menu suspenso "Plataforma"):
  - página #1A1B1E;
  - UMA superfície um degrau acima, #25282C (~1,2:1), para cards, rail, botão voltar, busca e o próprio menu suspenso, sem borda nem sombra;
  - o item selecionado clareia mais um degrau (#4E535B).
- **No ReEmu o problema era de semântica de token.** No Fluent o `colorNeutralBackground1` é a superfície mais à frente: branco no claro, grey[16] no escuro, e os números maiores vão para o fundo. Menus, Popover, Dialog, Input, Button secondary e Tooltip pintam o fundo com ele. Os temas do ReEmu tinham o `Background1` como PÁGINA, então o painel de notificações e os menus saíam da cor da página (print: popover #DCDAE1 com a página #DADBE6 e os cards #E5E5EB).
- **Correção** (`surfaceTokens` em `themes.ts`):
  - O `Background1` que o tema define vira o token `reemuPage`.
  - O `Background1` do Fluent passa a ser a superfície: o `Background2` do tema quando já fica ≥ 1,15:1 da página (claro: ≥ 1,08:1 e mais claro); senão, a página clareada pela cor do texto (escuro) ou puxada para o branco (claro).
  - Hover/pressionado/selecionado calculados a partir dela.
  - Os `Background2`–`4` dos temas não mudam.
  - Os usos que eram "página" passam para `var(--reemuPage)`: fundo animado, células do 2×2, selo do favorito, botão "Mostrar tudo", bordas das pílulas, fundo da tela de jogo.
- **Teste novo:** em todos os temas, superfície mais clara e ≥ 1,08:1 da página, com texto AA sobre ela.
- Conferido em captura: popover de notificações e diálogo "Encerrar" no Purple Mistique claro e no Neon Circuit. 131 testes e `ui-smoke` (12/12) passaram.
- Antes disso, a pedido do usuário: cards sem borda (o Xbox destaca só pela cor; o alto contraste mantém) e cards do hub de Configurações sem sombra.
- **Revisão da escada em todos os temas (mesmo dia):** os `Background2`–`4` dos temas não seguiam uma ordem. Nos escuros de console, o `2` (cards, rail, botões) ficava abaixo da superfície; nos claros, o `4` era mais escuro que a página.
  - Agora `surfaceTokens` gera a escada inteira: `Background1` = `Background2` = a superfície (uma só, como o #25282C do Xbox), e `3`/`4` são os degraus de realce.
    - **Escuro:** cada degrau clareia pela cor do texto, como os filtros #3E434B e o selecionado #4E535B do Xbox.
    - **Claro:** a superfície é branca e os realces escurecem, com ≥ 1,1:1 garantido.
    - Hover/pressionado/selecionado derivados de cada degrau.
  - Camadas do card das Configurações (`liftTokens`): o card é a própria superfície; linhas e campos clareiam no escuro e descem para a página no claro.
  - O card geral das Configurações ficou **sem borda e sem sombra**, a pedido do usuário. Separa-se só pela cor.
  - **Teste novo** em todos os temas: ordem página → superfície → realces, contraste mínimo entre os degraus e texto AA em todos.
  - Conferido em captura no Blue Modern, no Neon Circuit, no Simple Vibes e no Alva claro. 147 testes e `ui-smoke` (12/12) passaram.

## 2026-10-13 — Teclado na tela para a busca com o controle

- **Antes:** o Y do controle abria a busca, mas só dava para digitar com teclado físico.
- **Agora** (`components/OnScreenKeyboard.tsx`): o Y abre a busca já com o teclado na tela, no estilo do teclado do Xbox.
  - Grade QWERTY com números, ç e acentos do português (á à ã â é ê í ó õ ú), - : ', e uma linha de ações: Maiúsculas, Espaço, Apagar, Concluir e Fechar.
  - Teclas em `Button` do Fluent, na escada de elevação do tema, sem borda nem sombra.
  - A tecla em foco recebe o foco do DOM, com o anel padrão.
- **Controle:**
  - direcional anda na grade;
  - A digita;
  - B apaga, e com o campo vazio fecha;
  - X dá espaço, Y alterna maiúsculas, ☰ conclui.
  - Enquanto o teclado está aberto, ele recebe os pulsos antes da navegação normal (`useSearchStore.keyboardNav`, chamado no `useMenuNav`).
  - Ao fechar, o foco vai para o 1º resultado (ou para a busca).
  - A biblioteca filtra enquanto se digita, pela mesma busca do teclado físico. A barra de dicas fica escondida e o painel mostra as próprias dicas.
- **Por que próprio:** o Fluent não tem componente de teclado na tela. O teclado virtual do Windows (TabTip) é feito para toque, não abre com o controle e não existe no Linux.
- **Achado no teste:** com Maiúsculas, o array de teclas novo a cada render recriava o handler do controle, que se re-registrava na store e re-renderizava sem parar ("Maximum update depth"). As linhas maiúsculas agora são constantes.
- **Conferido no navegador** simulando os pulsos do controle (evento `menu-nav`): digitar "mario", alternar maiúsculas, apagar, fechar com ☰ e com B no campo vazio, e o foco voltando ao 1º card. Captura no Neon Circuit e no Simple Vibes. Textos em pt-BR/en/es. Não testado com controle físico.
- **Ajuste (print do usuário):**
  - O painel do teclado ganhou a sombra `shadow28`, a que o Fluent 2 › Elevation indica para "bottom sheets", porque ele flutua sobre a página.
  - O anel de foco das teclas era cortado pelas vizinhas: o anel vai 6 px para fora e o vão era de 4 px. O vão subiu para 8 px (`spacingHorizontalS`), e a tecla em foco fica por cima (`z-index`). Conferido em captura ampliada no Alva claro.

## 2026-10-13 — Foco nas abas

- **Problema** (print do usuário): com foco de teclado/controle, a aba ganhava um anel duplo e grosso e perdia o traço de aba selecionada.
  - Causa: a regra global de foco (`index.css`, regra 1) restiliza o `::after` dos elementos com `[data-fui-focus-visible]` como anel.
  - Na `Tab` do Fluent, o `::after` é o traço de selecionada, então virava um segundo anel por cima do anel da aba (`useTabStyles`).
- **Correção:** a regra 1 exclui `.fui-Tab`. A aba fica com o anel padrão (3 px a 3 px, com brilho) e o traço embaixo.
- Conferido com foco de teclado real (seta no `TabList`, que liga o `data-fui-focus-visible`). Os testes anteriores só focavam por `.focus()`, que não liga o atributo, por isso não pegavam o defeito.
- Antes, no mesmo dia: a aba selecionada em foco deixou de ter o fundo cheio na cor do tema; a estrela do estado vazio de Favoritos passou a caber inteira.
- **Foco nas abas como no app Xbox** (prints do usuário, medidos):
  - A aba selecionada e focada fica invertida: fundo branco (#FFFFFF), texto escuro e sem o traço de baixo.
  - A aba focada não selecionada ganha o fundo de realce (#3E434B, um degrau acima da superfície), e o traço continua na selecionada.
  - No ReEmu: selecionada+foco = fundo `colorNeutralForeground1` e texto `reemuPage`, invertido em qualquer tema, sem o traço; foco = `colorNeutralBackground3`. O anel é o padrão do app.
  - Conferido em captura com foco de teclado real nos dois casos.

## 2026-10-14 — Desempenho no Linux, medido com os jogos do Windows

- **Como:** os mesmos jogos da medição do B6 (Windows), 20 s cada com `REEMU_PERF=1`, numa RTX 3060 (driver NVIDIA 595.99.02, kernel 7.0, Wayland). Cores OpenGL pelo `gl_core_real_rom` (processo filho; `REEMU_GL_INTEROP=0` é a cópia pela CPU); cores Vulkan pelo `vk_core_real_rom` (dentro do processo do app). Os dois testes passaram a imprimir o tempo do app por quadro (`process` da chain + leitura do teste), a mesma medida do `drive_interop_session` do Windows.
- **Core** = mediana do `retro_run` + envio ao app nos segundos estáveis (fps no alvo e nenhum quadro acima do orçamento; os outros são o jogo carregando do disco, com `retro_run` de ~100 ms dentro do core).

  | Jogo | Cópia pela CPU (core / app) | Sem cópia, dma_buf (core / app) |
  |---|---|---|
  | mupen64plus_next, Mario Kart 64, 640×480 | 1,04 ms / 1,27 ms | 0,73 ms / 0,70 ms |
  | flycast, Soulcalibur, 640×480 | 1,48 ms / 1,28 ms | 1,24 ms / 0,73 ms |
  | flycast, Soulcalibur, 1920×1440 | 4,07 ms / 5,69 ms | 1,34 ms / 2,53 ms |
  | flycast, Metropolis Street Racer, 640×480 | 4,76 ms / 1,27 ms | 4,58 ms / 0,93 ms |
  | flycast, Metropolis Street Racer, 1920×1440 | 7,82 ms / 5,36 ms | 4,55 ms / 2,48 ms |

  - Todos no ritmo: p99 do intervalo entre quadros igual ao período (16,68 ms; 20,0 ms no Soulcalibur europeu, que passa a 50 Hz depois da abertura e o app acompanha).
  - Em 1920×1440 a cópia custa ~3,3 ms no core (o `glReadPixels` e ~1,35 ms de envio de 10,5 MB) e ~2,9 ms no app — a mesma diferença do Windows (3,2 ms e 3,0 ms).
- **Cores Vulkan (dentro do app):** Beetle PSX HW (40 Winks), mupen64plus_next com ParaLLEl-RDP (Mario Kart 64) e flycast (Soulcalibur), todos com o quadro em imagem Vulkan, a 60 fps (50 no Soulcalibur PAL); o app gasta 0,16–0,20 ms por quadro, porque amostra a imagem do core direto, sem sincronização externa.
- **Não medido:** GPUs AMD e Intel; a apresentação na tela (o pump do app real) e o uso de CPU por jogo.

## 2026-10-16 — Carrossel com o controle; botão "Mostrar tudo"

- **Carrossel** (print do usuário: slide deslocado para o lado e a página rolada para cima, com o destaque cortado embaixo da topbar).
  - O `moveFocus` chamava `.focus()` e `scrollIntoView`, que rolam TODOS os ancestrais, inclusive o `overflow: hidden` do `Carousel` do Fluent.
  - Agora o foco é `focus({ preventScroll: true })`. Dentro do carrossel, quem vai para a tela é o carrossel inteiro (`scrollIntoView` no `.fui-Carousel`); nas prateleiras, o card continua centralizando na horizontal.
  - A área de rolagem ganhou `scroll-padding-top` igual ao topo da página, para a rolagem do foco contar a topbar como coberta. Voltando para o destaque, a página rola até ele aparecer inteiro (medido: de 66 px rolados para 0, topo do carrossel em 116).
- **"Mostrar tudo"** (título das prateleiras): hover e foco passam a ter o mesmo fundo dos outros botões do tema (`colorNeutralBackground2Hover`/`Pressed`), no lugar da mistura própria.

## 2026-10-16 — Aparência e Decorações abrem em cards, como o hub

- **Pedido do usuário:** as opções de Aparência e Decorações em cards com a cara dos cards do menu principal das Configurações, SEM o card geral atrás. O card geral só aparece dentro da opção escolhida.
- `SettingsHubCards` (em `SettingsHome.tsx`): os mesmos cards do hub (ícone, nome e descrição), numa fileira de 180 px. A entrada de uma subcategoria (`HUB_SECTIONS` em `SettingsLayout`) renderiza sem o card geral.
- **Aparência:** as abas Temas e Papel de parede viraram páginas próprias, `/settings/aparencia/temas` e `/settings/aparencia/papel-de-parede`, com o caminho "Configurações › Aparência › Temas".
- **Decorações:** a lista de links (`SettingsLinkList`) foi trocada pelos mesmos cards; o `components/SettingsNav.tsx` ficou sem uso e saiu.
- `ui-smoke` em 2 tamanhos (10/10) e 147 testes passaram.

## 2026-10-16 — Revisão do foco em todos os componentes

- **Varredura** (navegador headless, modo controle) em 20 telas, foco a foco, conferindo o anel padrão: 3 px na cor de destaque, a 3 px, com o brilho de 12 px. Dois desvios:
  1. **Slide do carrossel:** o anel ficava por dentro e sem brilho, porque o carrossel corta o que passa da borda. Agora o anel padrão vai no próprio carrossel (`.fui-Carousel:has(slide em foco)`), que não é cortado, e o botão do slide fica sem anel.
  2. **Busca da BIOS:** estava sem anel, porque a regra que tira o anel da busca da topbar pegava qualquer `SearchBox`. Agora vale só para a busca da topbar (`reemu-topbar-search`).
- Depois disso, a varredura não acha desvio em nenhuma das 20 telas. No teclado (Tab) também conferido.
- **Exceções de propósito:**
  - itens de menu e de lista com o anel por dentro, porque o popover corta;
  - busca da topbar com a linha do Fluent no lugar do anel, a pedido do usuário;
  - cards de jogo com o vão transparente.

## 2026-10-16 — Todo foco igual ao do card de jogo

- **Pedido do usuário:** o foco do menu de Configurações não estava igual ao do card de jogo. Todos os focos iguais a ele.
- **Diferença:** nos outros componentes o brilho era um `box-shadow` no próprio elemento. Ele sai rente à borda e pinta o vão até o anel. No card de jogo, o vão é transparente e o brilho fica por fora do anel (um `::before` com −6 px).
- **Correção** (`index.css`, regra 4):
  - Todo elemento que mostra o anel (teclado, controle, links/`[tabindex]`, abas, switch/rádio/slider, campos, cards selecionáveis e o carrossel) ganha o mesmo `::before` de brilho, e o `box-shadow` rente saiu das regras do Griffel e do CSS.
  - A posição relativa vai em `:where` (especificidade 0), então quem já é `absolute` continua.
  - Enquanto em foco, `overflow: visible`, porque o Button, o Card e a Tab do Fluent cortavam o brilho.
  - O carrossel passou o recorte para o viewport.
  - Os cards do hub de Configurações também clareiam no foco, como o card de jogo, e a grade ganhou 20 px de folga para o brilho.
- **A varredura agora confere também:** brilho por fora, nenhum brilho rente e o elemento sem cortar o `::before`. Passou nas 20 telas. Capturas do card do hub, do botão "Plataforma" e do carrossel. 147 testes e `ui-smoke` 12/12.

## 2026-10-16 — Prévia dos shaders com os avatares do ReEmu

- **Pedido do usuário:** a cena 2D de teste dos shaders deve usar os personagens dos avatares, em pixel art. Depois pediu outra cena com um tema de jogo diferente, que ponha os avatares em contexto.
- **Primeiro, uma cena de plataforma com os avatares;** depois o usuário quis uma cena 2D só, com tema novo. A cena espacial substituiu a de plataforma (que está no histórico do git, commit 54baf8e), e a aba continua "Cena 2D".
- **Cena espacial:** um shoot 'em up de rolagem lateral no estilo Gradius/R-Type.
  - A nave do avatar é o jogador: o sprite foi girado para a direita (`rotate_cw`).
  - Os slimes formam a esquadrilha inimiga, o robô gigante é o chefe atrás de um escudo, e o controle e o joystick são power-ups em cápsulas.
  - Fundo: degradê e nebulosa em dithering, estrelas em 3 camadas, planeta com anel e asteroides.
  - O HUD fica numa faixa embaixo, como nos shmups horizontais.
  - É desenhada em código (`sample_scene_space`, 320×240), sem material de terceiros.
- **Referências do gênero:**
  - CPCWiki "Shoot them Up": HUD numa faixa e parallax em camadas.
  - Fórum do Defold sobre star field parallax: estrelas em várias camadas, com o planeta numa camada própria.
  - Pacotes Space Backgrounds da Ansimuz: camadas separadas para planetas e asteroides.
- **Conferência:** o teste `sample_scene_has_the_expected_size` grava a cena em PPM quando recebe `REEMU_DUMP_SCENE`, e ela foi conferida a olho.

## 2026-10-16 — Cena 2D da prévia a partir da imagem de referência

- **Pedido do usuário:** a cena 2D de teste dos shaders deve partir de uma imagem de referência que ele mandou, uma fase de plataforma ao pôr do sol. Os avatares não precisam aparecer.
- **O que mudou:** a cena espacial com os avatares saiu. A nova `sample_scene` (320×240) redesenha a referência pixel a pixel em código, porque a imagem original não tem grade de pixels exata e o teste de shader precisa de pixels nítidos. Ela tem:
  - céu em degradê com dithering, sol com halo, nuvens e montanhas em duas camadas;
  - a ilha com o castelo, o lago com o reflexo do sol e a margem com pinheiros;
  - a árvore, as plataformas de pedra com grama e tufos, a cachoeira, a água com espuma e a ponte;
  - a placa, os blocos (tijolo e "?"), as moedas, o herói de capa e o inimigo;
  - o HUD em cima: retrato ×03, corações, SCORE, moedas ×12 e STAGE 1-2.
- **Fonte do HUD:** ganhou T, A, G e "-".
- **Novo teste:** confere que todos os sprites são retangulares.
- **Molduras (Configurações › Decorações › Molduras), também a pedido:**
  - Os botões "Importar pasta de bezels…" e "Remover bezels" foram escondidos (`SHOW_BEZEL_PACK_TOOLS`). A importação ficou no backlog (TASKS.md).
  - O tamanho dos packs (100–600 MB por sistema) subiu do rodapé para o texto do topo, junto com as outras informações.

## 2026-10-16 — Cena 2D da prévia é a própria imagem de referência

- **Pedido do usuário:** usar a imagem enviada (fase de plataforma ao pôr do sol, 1536×1024) como a cena 2D de teste dos shaders, no lugar do redesenho em código.
- **Ajuste ao 4:3:** a cena da prévia é 320×240, igual à 3D, e a interface mostra a prévia em 4:3. Para caber sem cortar o HUD nem achatar a imagem:
  - a faixa de terra e água de baixo (64 px) foi repetida duas vezes, levando a imagem a 1536×1152;
  - depois ela foi reduzida 4,8× com filtro box para 320×240.
  - A grade da arte tem uns 320 px de largura, então a redução quase coincide com os pixels dela.
- **Arquivo:** `apps/desktop/src-tauri/assets/shader-preview-2d.png` (RGB, 138 KB). Fica embutido com `include_bytes!` e é decodificado com o crate `png`, que o app já usava.
- **Código:** o desenho procedural da cena 2D saiu de `shader_preview_2d.rs`.

## 2026-10-16 — Prévia de shader quebrava com alguns presets do pacote

- **Sintoma (relato do usuário):** ao escolher alguns presets do pacote, a prévia ficava com o ícone de imagem quebrada, mesmo depois de voltar para "Nenhum".
- **Causa:** erros de validação do WebGPU, ao montar o pipeline ou ao renderizar, iam para o handler de erro não capturado do wgpu, que entra em pânico. No front, a URL de blob anterior era revogada e continuava na tela.
- **Correções:**
  - **`set_preset` e `render_still`:** rodam dentro de `push_error_scope(Validation)` / `pop`, segundo a doc do wgpu 30 (`Device::push_error_scope`, `ErrorScopeGuard::pop`). Um preset recusado vira `Err` e o shader anterior continua; um render recusado vira "sem imagem". O `pollster` passou para as dependências gerais por causa do `block_on` do `pop`.
  - **Sampler `clamp_to_border`:** exige `Features::ADDRESS_MODE_CLAMP_TO_BORDER`, ou `ADDRESS_MODE_CLAMP_TO_ZERO` com `SamplerBorderColor::Zero` (docs do `wgpu-types` 30). Sem nenhuma das duas no device, cai para `ClampToEdge`, que é uma aproximação. A cor de borda do RetroArch não foi conferida.
  - **Último passe:** sai sempre no formato de saída de 8 bits, ignorando `float_framebuffer` e `srgb_framebuffer`, conforme `spec/SHADER_SPEC.md` e `README.md` do libretro/slang-shaders ("all filter chains terminate by rendering to the backbuffer"; "the final pass will not be an sRGB backbuffer"). A exceção é quando ele é lido como feedback. Antes, o readback copiava `Rgba16Float` com `bytes_per_row` de 4 bytes por pixel.
  - **Prévia no front:** sem buffer, volta a `null`, e o aviso de erro aparece dentro do quadro.
- **Teste novo:** `field_render_real_presets` (ignorado; precisa do pacote) monta e renderiza cada `.slangp`. Usa um `FrameProcessor` por preset, porque reaproveitar um só esgotava a memória da GPU no Mega Bezel.
- **Placar e o que ficou em aberto:** no TASKS.md (Vídeo / GPU).

## 2026-10-17 — Pesquisa: projetos de recomp e decomp de jogos

- **Pedido do usuário:** pesquisar os projetos de recompilação e decompilação de jogos, ver quais são de código livre e avaliar portar para Rust e trazer para o ReEmu. Ficou no backlog (TASKS.md › Funcionalidades), para avaliar no futuro.
- **Tipos de projeto:**
  - **Decomp:** C reescrito à mão até compilar idêntico à ROM, depois portado para PC.
  - **Recomp:** uma ferramenta traduz o código de máquina para C/C++, compilado junto com um runtime.
  - Em ambos, os assets vêm da ROM do usuário.
- **Licenças**, lidas na API do GitHub (`gh api repos/...`):
  - **MIT:** N64Recomp, RT64, XenonRecomp, XenosRecomp, libultraship, perfect_dark (fgsfds).
  - **GPL-3.0:** N64ModernRuntime (arquivo COPYING), Zelda64Recomp, UnleashedRecomp, PS2Recomp.
  - **CC0:** n64decomp/sm64, 2ship2harkinian.
  - **Licença própria, derivada do Xenia:** rexglue-sdk.
  - **Sem licença detectada pelo GitHub:** zeldaret/oot, zeldaret/mm, Shipwright, sm64ex, pret/pokeemerald, pret/pokered.
- **Avaliação:** não portar para Rust nem copiar código.
  - O volume é enorme, e cada port serve para um jogo só.
  - Traduzir código GPL obrigaria o ReEmu (MIT) a virar GPL; código sem licença não pode ser copiado.
  - Mesmo o código CC0/MIT reproduz jogos com direitos autorais.
  - Em Rust só existem experimentos: psprecomp (GPL-2.0, gera C++) e um GCRecomp para GameCube que geraria Rust.
- **Caminho proposto:** abrir o port como processo separado, como os cores (detalhes no item do TASKS).
- **Fontes:**
  - github.com/N64Recomp/N64Recomp e a wiki
  - github.com/N64Recomp/N64ModernRuntime
  - github.com/Zelda64Recomp/Zelda64Recomp
  - cdn.jsdelivr.net/gh/hedge-dev/XenonRecomp@main/README.md
  - sourceforge.net/projects/unleashed-recompiled.mirror
  - timeextension.com (2025/03, ferramenta de recompilação de Xbox 360)
  - readonlymemo.com/rexglue-xbox-360-recompilation-interview
  - github.com/n64decomp/sm64
  - nintendolife.com (2021/08, decomp do OoT)
  - github.com/wizardengineer/psprecomp
  - tomevault.io/tome/KaiserGranatapfel/GameCubeRecompiled

## 2026-10-20 — Pesquisa avançada: recomp e decomp (documento 15)

- **Pedido do usuário:** pesquisar a fundo os projetos de recompilação e decompilação de jogos e deixar documentado, para consultar no desenvolvimento futuro de launchers desses jogos no ReEmu e de versões em Rust.
- **Documento:** `docs/ai-context/15-ports-nativos-recomp-decomp.md`, linkado no README de ai-context e no item do backlog do TASKS. Cobre:
  - decomp × recomp × reimplementação;
  - o pipeline da recompilação estática, com detalhes do README do N64Recomp e do cps2recomp;
  - o catálogo de ferramentas e ports, com licenças conferidas na API do GitHub em 2026-10-20;
  - as regras de licença e direito autoral para o ReEmu;
  - o que existe em Rust: rabbitizer (MIT), decomp-toolkit e objdiff (Apache-2.0), spimdisasm, ppc750cl, além de experimentos GPL;
  - dois caminhos: launcher agora, recompilador próprio em Rust no longo prazo;
  - a proposta de arquitetura do launcher (modelo `NativePort`, fluxo, pontos de atenção);
  - perguntas em aberto e fontes.
- **Conclusão mantida:**
  - não traduzir nem copiar código dos ports;
  - o primeiro passo é abrir os executáveis oficiais como processos separados;
  - um recompilador próprio em Rust só depois do doc 14, com runtime escrito do zero.

## 2026-10-20 — Avaliação da GameKave / Alvanista

- **Pedido do usuário:** avaliar a gamekave.net para integração de dados de jogos e da rede social Alvanista.
- **Achados**, lidos na própria API em 2026-10-20 (`api.gamekave.net`, `/api/info`, OpenAPI):
  - plataforma da Tellurian Corp que alimenta a Alvanista;
  - API v0.1.0 com 68 endpoints, OAuth 2.0 + JWT;
  - serviços: `games` (assets e conquistas), `lists` (coleção), `gas` (conquistas por sinais HMAC), `social` e `trophies`;
  - 339 plataformas, com IDs do IGDB.
- **Conclusão:** promissora para integração social opcional (enviar tempo de jogo pelo GAS e sincronizar a coleção). Ainda não serve como fonte de metadados: não há busca por nome nem por hash, e faltam termos de uso e licença dos dados.
- **Próximo passo:** falar com a Tellurian antes de qualquer código. As perguntas e o escopo do protótipo estão no TASKS.md › Funcionalidades.

## 2026-10-20 — Cenas 2D e 3D da prévia de shaders trocadas por imagens novas

- **Pedido do usuário:** usar duas imagens novas como cenas de teste dos shaders:
  - 2D: fase de plataforma ao pôr do sol, com HUD de SCORE, LIVES, moedas e WORLD 1-1;
  - 3D: aventura no jeito de PS1/N64, com cachoeira, templo, ponte de corda e minimapa.
- **Preparo:** as duas originais têm 1448×1086, já em 4:3, então só foram reduzidas para 320×240 com filtro box, sem corte nem extensão.
- **Arquivos:** `apps/desktop/src-tauri/assets/shader-preview-2d.png` e `shader-preview-3d.png`, PNG RGB de 8 bits, embutidos com `include_bytes!` e decodificados com o crate `png`.
- **Código:** o rasterizador de triângulos em software da cena 3D (`shader_preview_3d.rs`, ~500 linhas) saiu. `sample_scene_3d()` mantém a mesma assinatura, então o comando da prévia e o teste da GPU não mudaram.
- **Testes:** os das duas cenas e `shader_preview_renders_the_sample_scene` (CRT na GPU, com as duas cenas) passaram; clippy sem avisos.

## 2026-10-24 — Release v0.1.8

- **Foco:**
  - O efeito do card de jogo (anel de 3 px a 3 px, vão transparente e brilho na cor do tema) virou o foco padrão de todos os componentes.
  - O controle e o mouse mostram o mesmo anel.
  - O carrossel não desloca o slide no foco.
- **Configurações:**
  - Aparência, Decorações (antigo Vídeo) e Sistema abrem em cards; o card geral só aparece dentro da opção escolhida.
  - O caminho no topo usa o Breadcrumb do Fluent 2, e o título e os cards ficam na mesma altura em todas as telas.
  - Cores e BIOS em linhas no estilo da Microsoft Store.
  - Perfil em duas colunas, com o avatar ativo grande.
  - "Plano de fundo" com a opção de aparecer só na tela inicial ou em todas as telas (nunca na página do jogo).
- **Temas:**
  - Elevação única: página → superfície → realces, sem bordas nem sombras nos cards, como no modo XBOX.
  - O tema ativo é marcado por um check depois do nome.
- **Controle:**
  - Teclado na tela para a busca, com o visual do teclado do Windows 11 com controle.
  - Barra de botões com os círculos A/B/X/Y preenchidos.
- **Fonte:** Segoe UI Variable no Windows 11 e Selawik (OFL) embutida para o Linux.
- **Início:** carrossel de destaque com o Carousel do Fluent 2, com avanço automático.
- **Página do jogo:**
  - Capa na altura do destaque da Início e arte de fundo mais alta.
  - Card sem capa igual ao da galeria.
  - Botões do hero com o foco padrão.
- **Shaders:**
  - Novas cenas 2D e 3D na prévia.
  - Itens do pacote iguais aos da lista do ReEmu.
  - Preset recusado pelo WebGPU agora vira aviso em vez de derrubar a prévia, e mais presets do pacote renderizam (`clamp_to_border` e o formato do último passe).
- **Molduras:** importar e remover packs escondidos (backlog).
- **Biblioteca:** menu de plataformas cabe na janela e rola por dentro.

## 2026-10-24 — Tela branca no PSP (God of War): alfa zero do core na superfície

- **Relato do usuário:** God of War: Ghost of Sparta no PPSSPP abre com a tela branca. Já tinha rodado antes (909 s de jogo registrados).
- **Investigação:**
  - O teste `win32_interop_real_gl_core` reproduziu o caso com o PPSSPP e o ISO do usuário.
  - Com um diagnóstico temporário (já removido), os quadros têm conteúdo escuro, de brilho médio 0–12, iguais no interop e na cópia pela CPU (`REEMU_GL_INTEROP=0`).
  - O **alfa é 0 em todos os quadros**.
  - O shader FSR do pacote não é a causa: `field_render_upscalers` gera imagem normal.
- **Causa:**
  - O blit SDR da superfície usava o `COMP_WGSL`, que repassa o alfa da textura.
  - A superfície era configurada com `caps.alpha_modes[0]`, que pode ser `Inherit`. Com a janela de vídeo por cima do WebView2, o compositor respeitava o alfa 0 e o jogo ficava transparente.
- **Fontes:**
  - `libretro.h`, `RETRO_PIXEL_FORMAT_XRGB8888`: "The most significant byte (the X) is ignored".
  - wgpu-types 30, `CompositeAlphaMode::Opaque`: "the alpha channel ... is ignored in the compositing process ... treated as if it has a constant alpha of 1.0".
  - wgpu-types 30, `SurfaceCapabilities::alpha_modes`: "at least one element, Opaque or Inherit".
- **Correção:**
  - A superfície usa `Opaque` quando a GPU oferece, e o modo escolhido vai para o log.
  - O blit SDR ganhou o `BLIT_WGSL`, que grava alfa 1, como o passe HDR já fazia.
  - Os mipmaps da chain continuam com o `COMP_WGSL` (`mip_blit_pipeline`), que preserva o alfa.
  - A moldura é composta antes do blit, então mantém a transparência.
- **Falta:** conferir no app com o jogo. O teste não cobre a superfície nem a composição do Windows.


## 2026-10-24 — Tauri atualizado para a última 2.x estável

- **Pedido do usuário:** atualizar o Tauri para a versão mais atual, sem a 3.0 alpha. Base: <https://tauri.app/release/>.
- **Versões:**
  - Crates: `tauri` 2.12.1 → 2.12.5; `tauri-build` 2.7.1 → 2.7.4; `tauri-runtime-wry` 2.12.1 → 2.12.3; `wry` 0.57.0 → 0.57.1; `tao` 0.37.1 → 0.37.2.
  - Plugins: `tauri-plugin-updater` 2.12.0 → 2.14.0; `tauri-plugin-dialog` 2.7.3 → 2.8.3; `tauri-plugin-log` 2.9.1 → 2.10.1; `tauri-plugin-fs` 2.5.2 → 2.7.1. O `tauri-plugin-autostart` já estava na última (2.7.0).
  - npm: `@tauri-apps/api` 2.11.1 → 2.12.4; `@tauri-apps/plugin-dialog` 2.7.2 → 2.8.3.
  - O CLI local foi para a última 2.x. O CI já instala com `--version "^2"`.
- **Cuidado com o lock:**
  - Um `cargo update --recursive` quebrou o `wgpu-hal`. O resolvedor passou o `gpu-allocator` para `windows` 0.61 enquanto o `wgpu-hal` usa 0.62: "multiple different versions of crate `windows`".
  - A atualização foi refeita só nos crates do Tauri.
  - `gpu-allocator` e `gilrs-core` voltaram para `windows` 0.62.2, como antes, para não mexer no backend do controle.
- **O que ajuda o ReEmu (changelogs oficiais):**
  - tauri 2.12.3: um `Channel` continua entregando mensagens depois que o `onmessage` lança erro. É o canal do progresso de download de molduras e pacotes de shader.
  - tauri 2.12.4: corrige um pânico no handler de IPC com URI de caminho vazio.
  - updater 2.14.0:
    - verifica os bytes do pacote antes de instalar;
    - no Windows, diz por que o instalador não abriu (por exemplo, UAC recusado);
    - no Linux, a atualização do AppImage não falha mais com `/tmp` em outro mount;
    - libera a memória de um download anterior.

## 2026-10-24 — Tela branca em todos os jogos: o WebView2 pintava por cima da janela de vídeo

- **Relato do usuário:** qualquer jogo abre com a tela branca, e só o menu aparece. No modo canvas (`REEMU_NATIVE_VIDEO=0`) o jogo aparece.
- **Investigação:**
  - Uma sonda externa (`EnumChildWindows` e `GetWindow` em ordem Z, mais `IsWindowVisible`) mostrou a `ReEmuVideo` visível e no topo das irmãs. O log também tinha "1º frame apresentado".
  - O `WRY_WEBVIEW` estava com o estilo `0x52000000`: WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN, **sem WS_CLIPSIBLINGS**.
  - Teste A/B com o jogo rodando (Jet Set Radio, com moldura), ligando e desligando o flag no `WRY_WEBVIEW` por fora:
    - com o flag, o jogo aparece;
    - sem o flag, a tela fica branca.
- **Causa:** o código supunha que WS_CLIPSIBLINGS na nossa janela impedia o WebView2 de desenhar por cima. O estilo recorta quem desenha, não quem é coberto.
- **Fontes:** Microsoft Learn:
  - Window Styles, WS_CLIPSIBLINGS: "If WS_CLIPSIBLINGS is not specified and child windows overlap, it is possible, when drawing within the client area of a child window, to draw within the client area of a neighboring child window";
  - SetWindowLongPtrW: "changes you make using SetWindowLongPtr will not take effect until you call the SetWindowPos function".
- **Correção:**
  - Ao criar a janela de vídeo acima do WebView2, `clip_siblings` (`video.rs`) liga WS_CLIPSIBLINGS nas irmãs.
  - Em seguida chama `SetWindowPos` com `SWP_FRAMECHANGED`.
  - Roda na thread principal, a dona das janelas.
- **Não explicado:** por que funcionava em 09 e 11/10. Talvez uma atualização do runtime do WebView2 ou do Windows (build 26300) tenha mudado como ele pinta. Não verificado.

## 2026-10-24 — Controles no Windows pelo SDL3 (o Windows.Gaming.Input deixou de ver o DualSense)

- **Relato do usuário:** o joystick é reconhecido, mas não funciona.
- **Investigação:** com o DualSense pareado por Bluetooth e conectado (`DEVPKEY_Device_IsConnected`):
  - o Windows o lista como "Controlador de jogo compatível com HID" (VID 054C, PID 0CE6);
  - a winmm o enxerga;
  - o Windows.Gaming.Input não lista nenhum controle (`RawGameControllers` vazio), nem com uma janela em primeiro plano.
  - Até 20/10 o gilrs, que usa o WGI no Windows, reconhecia o "PS5 Controller".
  - Com o DS4Windows, só aparecia o Xbox 360 virtual (ViGEm), sem eventos na sonda.
  - O backend `xinput` do gilrs não atende o DualSense.
  - Lido o HID cru pelo hidapi do SDL, o controle estava no relatório estendido do Bluetooth (0x31). O DS4Windows e o DSX ligam esse modo, e ele fica até o controle ser desligado. Provável motivo de a winmm só ver eixos no centro. Não verificado se é também a causa no WGI.
- **Decisão:** no Windows, o `GamepadPoller` lê o SDL3 em vez do gilrs, que continua no Linux e no macOS.
  - A lógica do poller (calibração, zona morta, portas, navegação, captura) não mudou.
  - A fonte dos eventos virou `input_desktop::pad_source`, com vocabulário próprio (`Button`, `Axis`, `PadEvent`).
- **Fontes:** headers do SDL 3.4.18 e wiki SDL3:
  - `SDL_Init`: "This function should only be called on the main thread". Chamado no `setup` do Tauri, antes do `AppState::new`.
  - `SDL_PollEvent`: "should only be called on the main thread". Por isso a fila de eventos não é usada, e os eventos de gamepad e joystick ficam desligados.
  - Por rodada, na thread do poller: `SDL_UpdateGamepads` ("Manually pump gamepad updates if not using the loop"), `SDL_GetGamepads` (o array vai para o `SDL_free`), `SDL_OpenGamepad`, `SDL_GetGamepadButton` e `SDL_GetGamepadAxis`. Todos "safe to call from any thread". Os eventos saem da diferença com a rodada anterior.
  - `SDL_GetGamepadAxis`: sticks de -32768 (cima/esquerda) a 32767; gatilhos de 0 a 32767. O Y é invertido para a convenção do poller (Y+ = cima).
  - `SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1`: sem janela do SDL, o padrão "0" descarta o input do app em segundo plano.
  - `SDL_HINT_JOYSTICK_WGI`: "WGI is not used (default)".
- **Build:**
  - `sdl3-sys` 0.7 (Zlib) com `build-from-source` e `link-static`, só `sdl-joystick` e `sdl-hidapi`. Precisa de CMake.
  - Na máquina do dono, o CMake do VS Build Tools vai por `[env] CMAKE` em `~/.cargo/config.toml`, fora do repositório. O `windows-latest` do CI tem CMake.
  - O `scripts/check-windows.sh` (cross-check no Linux) usa `DOCS_RS=1` para o `sdl3-sys` pular o build nativo.
  - SDL e HIDAPI (opção BSD) entraram no `THIRD_PARTY_NOTICES.md`, e o `licenses.json` foi refeito.
- **Gatilho preso:**
  - O DualSense do usuário manda L2 e R2 em 255 soltos no relatório 0x31. Os bytes 6 e 7 estão em `ff ff` no HID cru.
  - A calibração tratava "1º valor 1.0" como zero em 0, e os dois ficariam apertados para sempre.
  - Agora o 1º valor é sempre o repouso, e um gatilho segurado na conexão só conta depois de solto.
- **Testado no hardware:** pela sonda com a `pad_source`, chegaram face, d-pad, analógicos, L1/R1, Options e Share.
- **Pendente:**
  - O botão PS não apareceu na gravação.
  - L2 e R2 não variaram.
  - Validar no app.
- **Efeito colateral:** o GUID do controle mudou com o SDL, porque o GUID do HIDAPI tem CRC do nome e o driver `h`. Remapeamentos salvos para o DualSense precisam ser refeitos.

## 2026-10-24 — Escolha do driver de controle (Configurações › Controles, Windows)

- **Pedido do usuário:** poder escolher o driver do controle além da detecção automática do SDL.
- **Opções:**
  - **Automático:** padrão do SDL. HIDAPI para PlayStation, Switch e Xbox, mais XInput e DirectInput.
  - **Só XInput:** controles Xbox e virtuais (DS4Windows, DSX).
  - **Só DirectInput.**
  - **Windows.Gaming.Input:** o caminho de antes.
  - RawInput fica sempre no padrão "0".
  - Salvo em `app-settings.json` (`controllerDriver`), lido no `setup` antes do `SDL_Init`.
  - Fora do Windows a seção não aparece, porque o `gilrs` não tem escolha.
- **Troca com o app aberto:**
  - Os hints `SDL_HINT_XINPUT_ENABLED`, `SDL_HINT_JOYSTICK_DIRECTINPUT` e `SDL_HINT_JOYSTICK_WGI` "should be set before SDL is initialized" (SDL_hints.h 3.4.18). Por isso a troca faz `SDL_Quit`, aplica os hints e chama o `SDL_Init`.
  - A wiki não diz que dá para iniciar de novo depois do `SDL_Quit`. O fonte diz (SDL.c 3.4.18): ele zera a contagem dos subsistemas e chama `SDL_QuitHints()`, então os hints são postos de novo a cada início.
  - Os drivers leem os hints na própria inicialização: `SDL_xinputjoystick.c:49`, `SDL_dinputjoystick.c:406` e `SDL_windows_gaming_input.c:589`.
  - Antes do `SDL_Quit`, a thread do poller fecha os controles e avisa. A thread principal desliga `READY` e espera o `PARKED` da geração atual, por no máximo 1 s. Se o tempo esgotar, a troca desiste.
  - Se o driver novo não iniciar, a troca volta ao anterior.
  - O comando `set_controller_driver` é síncrono porque "Commands without the async keyword are executed on the main thread" (v2.tauri.app/develop/calling-rust), e o `SDL_Init`/`SDL_Quit` exigem a thread principal.
- **Testado no hardware** (DualSense por Bluetooth), trocando de driver com uma thread lendo o controle:

  | Driver | Resultado | Tempo da troca |
  |---|---|---|
  | XInput | nenhum controle (o DualSense não é XInput) | 5 ms |
  | DirectInput | "PS5 Controller" | 75 ms |
  | WGI | nenhum controle (como no gilrs) | 14 ms |
  | Automático | "DualSense Wireless Controller" de volta | 97 ms |

- **Interface:**
  - `ControllerDriver.tsx` no topo de Configurações › Controles, com `RadioGroup` vertical e a descrição de cada opção abaixo do nome, no mesmo padrão da tela Sistema.
  - Textos em pt-BR, en e es; nova ação de erro `setControllerDriver`.

## 2026-10-24 — Sistema › Opções em cards; foco dos botões-pílula da página inicial

- **Sistema › Opções (pedido do usuário):**
  - Idioma, Tamanho da interface, Inicialização e Vídeo viraram cards internos sem sombra, dentro do card geral, que continua.
  - Os cards usam o mesmo tom dos outros cards das Configurações: `colorNeutralBackground2`, `borderRadiusMedium`, padding L, 16 px entre cards.
  - A aba passou a usar a largura toda (até os 1120 px do `SettingsSystem`). Na coluna de 640 px, os cards ocupavam só a metade esquerda.
- **Botões "Todos os jogos" e "Adicionar ROMs" (pedido do usuário: "comportamento estranho" no foco):**
  - Medido no DOM: o anel (`outline`) acompanha o raio de pílula do botão (10000px), mas o brilho (`::before` da regra 4 do `index.css`) usava o raio dos botões retangulares (`Medium` + 6 = 10px). Dava um retângulo arredondado em volta do anel redondo.
  - O `pillBtn` agora põe `borderRadiusCircular` no `::before`.
  - Pelo controle, a borda cinza do botão continuava dentro do anel; no teclado o Fluent a esconde. Agora o foco do controle também a esconde em todos os botões do Fluent (`.fui-Button`, `.fui-MenuButton`, `.fui-ToggleButton`).

## 2026-10-24 — Diálogos: conteúdo não passa mais por baixo dos botões; barra de rolagem do app

- **Relato do usuário (print do Gerenciar biblioteca):** o conteúdo rolava por baixo dos botões Fechar e Salvar, e a barra de rolagem era a padrão do Windows.
- **Medido no DOM:**
  - A linha do conteúdo no grid do `DialogBody` tinha 484 px, e o `DialogContent` tinha 508 px, começando 12 px antes.
  - Era a regra global `margin: -12px` (index.css), que abre espaço para o anel de foco não ser cortado pela rolagem. Na vertical, ela fazia a caixa invadir o título e o rodapé.
- **Correção:**
  - A margem negativa ficou só na horizontal (`margin: 0 -12px`).
  - O padding de 12 px continua, então o anel do primeiro e do último item cabe nele.
  - O conteúdo agora ocupa exatamente a linha (142–626 px, com o rodapé em 634).
  - A barra de rolagem dos diálogos ganhou o estilo da `.scroll` do app: 6 px, polegar `colorNeutralStroke2` arredondado.

## 2026-10-25 — Teclado na tela em qualquer campo de texto (A do controle)

- **Pedido do usuário:** ao selecionar o campo Nome do onboarding com o joystick, o teclado na tela deve aparecer.
- **Antes:** o `OnScreenKeyboard` só servia à busca (Y). Ele editava o `useSearchStore.query` e era montado no `AppShell`, que não envolve o onboarding.
- **Agora:**
  - O A (confirmar) do controle num campo de texto focado abre o teclado para esse campo (`useMenuNav`). Valem `<input>` de texto, busca, e-mail, URL, telefone e senha, e `<textarea>`, desde que não sejam somente leitura nem desabilitados.
  - O `useSearchStore` ganhou `keyboardTarget` e `openKeyboardFor(campo)`. Sem campo, o teclado edita a busca, como antes.
  - Para escrever no campo controlado pelo React, o teclado usa o setter nativo de `value` e um evento `input` que borbulha; atribuir `el.value` direto o React ignora. Respeita o `maxLength`.
  - Ao fechar, o foco volta ao campo editado.
  - O teclado é montado no `RootLayout`. Com um Dialog do Fluent aberto, ele vai para dentro do `DialogSurface` por portal, porque o Dialog fica num portal por cima do app e prende o foco no próprio conteúdo.
  - O realce de "digitando" da busca só aparece quando o teclado edita a busca.
- **Testado** na página de teste, com uma simulação que guarda o `listen` do Tauri para disparar `menu-nav`:
  - onboarding: o A no Nome abre o teclado dentro do modal; Q, E, X (espaço), Y (maiúscula) e E dão "qe E" e habilitam o Continuar; o B apaga; o ☰ fecha e devolve o foco ao campo;
  - busca: o Y abre, digita e fecha.
  - Nota do teste: o unlisten do `@tauri-apps/api` 2.12 chama `window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` antes do `invoke`. Sem isso no mock, o StrictMode deixava dois ouvintes e cada pulso contava duas vezes.

## 2026-10-25 — Onboarding revisado (pedidos do usuário, com o `scripts/first-run.ps1`)

- **Primeira abertura sem tocar nos dados reais:** `scripts/first-run.ps1` abre o app com `REEMU_DATA_DIR` e `WEBVIEW2_USER_DATA_FOLDER` numa pasta temporária. O WebView2 usa a variável no lugar da pasta que o Tauri passa (Microsoft Learn, `CreateCoreWebView2EnvironmentWithOptions`).
- **Boas-vindas:**
  - duas colunas, com 64 px entre elas;
  - à esquerda, "Escolha o idioma" e os idiomas como botões grandes, num `radiogroup`. Os não escolhidos têm fundo `Background3`, porque o `secondary` some sobre o modal;
  - à direita, "Bem-vindo" grande no idioma escolhido, na cor primária, e a nuvem de "bem-vindo";
  - Continuar grande;
  - o modal tem até 1200×760, em 92vw×82vh.
- **Perfil:**
  - mesmo layout de Configurações › Perfil, sem a bio;
  - a coluna dos avatares fica no meio da altura, e a dica ficou mais afastada;
  - campo Nome grande, e Voltar e Continuar grandes;
  - o brilho do foco das opções de avatar ficou redondo;
  - o A do controle no Nome abre o teclado na tela (ver a entrada anterior).
- **Final:**
  - no lugar de "Tudo pronto", um modal com o avatar escolhido entrando (escala + fade), "Bem-vindo, {nome}!" grande e a frase de baixo, uma depois da outra;
  - o botão "Tudo pronto para começar" fica no centro, embaixo (`DialogActions` com `justify-self: center`);
  - o perfil é salvo ao continuar no passo do nome. O cache só marca `onboarded` no botão final, porque antes disso o `RootLayout` já tiraria do onboarding.
- **Controle no onboarding:**
  - as dicas ficam no centro, embaixo, num `Portal` próprio acima do Dialog. O nó do portal do Fluent é `z-index: 1000000`, e dentro do `DialogSurface` o `position: fixed` ficava preso ao modal;
  - A em todos os passos, B onde dá para voltar;
  - o B volta um passo (evento `reemu-onboarding-back`) em vez do `navigate(-1)`.
