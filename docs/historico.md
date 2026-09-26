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

## 2026-09-26 — Gatilhos do DualSense e teclado + controle

- **R2 não acelerava no MSR:** o DualSense do dono (driver `hid-playstation`, Bluetooth) informa `ABS_Z`/`ABS_RZ` em 168/173 de 0–255 com os gatilhos **soltos**, lido pelo `EVIOCGABS` do evdev. Com o limiar de 25% do gilrs (`btn_value` = valor/faixa), L2 e R2 contavam como apertados o tempo todo: no MSR, freio e acelerador juntos. O `GamepadPoller` agora decide L2/R2 pelo `ButtonChanged` com o zero calibrado (`TriggerCal`: o menor valor visto é o zero; o aperto é reescalado a partir dele, com histerese de 25%/15%) e ignora o press/release cru do gilrs para os gatilhos. Controle com zero em 0 continua igual.
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
