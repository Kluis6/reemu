# TASKS — Progresso do ReEmu

O que está **em aberto**, alinhado com `docs/ai-context/`. O histórico
completo (como cada etapa foi feita, decisões, investigações, itens já
fechados) está em [`docs/historico.md`](docs/historico.md). Ao pedir pra uma
IA continuar o projeto, aponte pra este arquivo primeiro ("veja o TASKS.md e
continue da próxima tarefa em aberto").

**Status**: `todo` · `in-progress` · `blocked` · `done`

---

## Etapas (docs/ai-context/01 a 12)

| # | Etapa | Status |
| --- | --- | --- |
| 01 | Domain + DB (repositórios sqlx) | `done` |
| 02 | Core Loader Desktop (software + GL HW render, interop dma_buf) | `done` |
| 03 | Tauri Desktop Shell (surface nativa `wl_subsurface`) | `done` |
| 04 | Shader Chain + Decoração (slang 99,7% dos presets) | `done` |
| 05 | Input, Hotkeys, UI de Binding | `done` |
| 06 | Áudio (Dynamic Rate Control) | `done` |
| 07 | Frontend React (modo Xbox) | `done` |
| 08 | Save States e Save RAM | `done` |
| 09 | Scraping de Metadata (ScreenScraper) | `done` (MVP) |
| 10 | Catálogo e Download de Cores | `done` |
| 11 | Port Android | `todo`: adiado pelo usuário (2026-08-30) |
| 12 | Vulkan HW Render Fase 2 | `in-progress`: Beetle PSX HW ok; faltam 2º/3º core e Fase C |

Desktop (01–10) fechado. Detalhe de cada etapa: `docs/historico.md` ›
"Etapas de implementação" e o "Estado atual" de cada doc em
`docs/ai-context/`.

## Em aberto

### Validação (precisa de hardware/máquina que não é esta)

- [ ] `todo` — **Surface nativa de vídeo no Windows**: hoje o padrão lá é o
      `<canvas>` (a surface no HWND fica atrás do WebView2). Precisa de uma
      janela filha acima do WebView2 + esconder/mostrar pro menu de pausa,
      equivalente ao `wl_subsurface` do Linux.
- [ ] `todo` — **Windows ponta a ponta**: só os testes do `core-ipc` rodaram
      numa máquina Windows real. Falta `cargo tauri dev` completo, `video.rs`
      no caminho `#[cfg(not(linux))]`, paths do buildbot de cores, instalador.
      Conferir que as capas aparecem (URL `http://cover.localhost/<id>` no
      Windows, `covers::cover_url`, corrigida sem teste em máquina real).
- [ ] `todo` — **Publicar e instalar a `v0.1.1`**: o draft no GitHub já tem os
      4 instaladores assinados. Publicar, instalar no Windows (`-setup.exe`) e
      no Linux (AppImage) — é a 1ª versão com auto-update.
- [ ] `todo` — **Chaveiro no Windows**: `cargo test -p reemu-desktop --lib
      os_keyring -- --ignored` (Credential Manager).
- [x] `done` — `SET_ROTATION` com um jogo vertical real: validado pelo
      usuário em 2026-09-25 (shooter vertical de arcade em pé, sem espelhar).

### Atualização automática

- [ ] `todo` — Validar ponta a ponta com duas Releases publicadas: instalar
      a antiga pelo AppImage e pelo NSIS, atualizar pelo modal e conferir o
      aviso "atualizado" depois do reinício. O `.deb` usa `pkexec`.
- [ ] `todo` — Capturas do site em `site/screens/` (`inicio`, `biblioteca`,
      `jogo`, `pausa`, `cores` .png).
- [ ] `todo` — Salvar a logo oficial em `site/logo.png` (a página já
      aponta pra ela; sem o arquivo a imagem some e o resto aparece).

### Vídeo / GPU

- [ ] `todo` — **Validar o OpenGL por hardware no Windows** (WGL, feito em
      2026-09-25 sem máquina Windows): abrir um jogo de PS1 no Beetle PSX HW,
      Dreamcast no flycast e N64 no mupen64plus/parallel e conferir a imagem.
- [ ] `todo` — Vulkan in-process no Windows: desligado por padrão depois
      que o `flycast` derrubou o app (`STATUS_ACCESS_VIOLATION`). Causa
      provável achada no Linux em 2026-09-25 (despachante do flycast nulo /
      extensões não ligadas — corrigido pelo hook do `vkCreateDevice`);
      falta validar num Windows antes de religar.

- [x] `done` — Etapa 12: flycast e mupen64plus_next rodam em Vulkan
      in-process no Linux (2026-09-25, `REEMU_HW=vulkan`; mupen com
      `mupen64plus-rdp-plugin=parallel`). Ver docs/historico.md.
- [x] `done` — Etapa 12, fase C (2026-09-25): sem espera de CPU no submit
      dos cmd buffers do core nem no blit; fence-marcador de fim de quadro
      esperada só no `wait_sync_index`. Validado com as camadas de validação
      (sync incluída). Ver docs/historico.md.
- [x] `done` — flycast em Vulkan como padrão (2026-09-26): a rota in-process
      responde Vulkan ao GET_PREFERRED_HW_RENDER; flycast e Beetle PSX HW
      rodam em Vulkan no Linux sem `REEMU_HW`.
- [x] `done` — Interop GL: `glFinish` trocado por fence `sync_file` →
      semáforo Vulkan, e o interop virou padrão no Linux (2026-09-25; ver
      docs/historico.md). Validado com as camadas de validação em
      2026-09-25 (corrigido o modificador DRM fora da negociação).
- [x] `done` — Integer scaling com moldura (2026-09-26): a moldura ganha um
      zoom perto de 1 e o jogo preenche a janela dela no múltiplo inteiro —
      sem faixa preta entre os dois.
- [ ] `todo` — CAS (AMD FidelityFX, licença MIT) não vem no pacote de shaders
      do libretro — portar como `.slang` se fizer falta (FSR, RCAS e NIS já
      estão nos presets recomendados). HDR / tonemapping.
- [ ] `todo` — Shader: `test/format.slangp` (textura de inteiros) é a única
      falha de preset que sobrou por limitação do pipeline.

### Desempenho do caminho do core

Medir antes de otimizar: `REEMU_PERF=1 scripts/dev.sh` com um jogo pesado
(N64/PS1) e anotar as linhas `perf core`/`perf vídeo` (ver STEP_BY_STEP §4).
Feito em 2026-09-24 (ver `docs/historico.md`): pump acorda no frame
(perdia 12% dos frames), margem de spin adaptativa (33 → 6 ms/s de CPU),
áudio sem alocação por frame, canvas sem a 2ª cópia do frame.

Sem alocação por quadro no caminho software desde 2026-09-25 (pool de buffers
no `emu-session`, ver `docs/historico.md`).

### Funcionalidades

- [ ] `blocked` — Metadata: IGDB como 3º provedor. Bloqueado por falta de
      fonte pública confiável pros ids de plataforma do IGDB (a lista só
      sai da API autenticada) — não implementar de memória. Alternativa
      avaliada: o banco do LaunchBox (`gamesdb.launchbox-app.com/Metadata.zip`,
      ~108 MB, atualizado diariamente) — avaliado em 2026-09-24: o site não
      publica termos de uso (só política de privacidade, sobre contas), então
      não há autorização pra usar o banco num app distribuído. Só com
      permissão escrita do LaunchBox.
- [ ] `todo` — Validar o TheGamesDB com uma chave real (o parser foi testado
      com JSON no formato que o ES-DE lê, não com resposta capturada).

- [x] `done` — **Teclado configurável** (2026-09-26): seção Teclado em
      Configurações › Controles (botões e direções dos dois analógicos); as
      setas também movem o analógico esquerdo por padrão.
- [x] `done` — **Gatilhos analógicos** (2026-09-26): pressão real de L2/R2
      chega ao core (`RETRO_DEVICE_INDEX_ANALOG_BUTTON`).

- [x] `done` — **Idiomas (pt-BR, en, es) no frontend** (2026-09-25): todas
      as telas, componentes, toasts, erros (`lib/errors.ts`) e diálogos
      nativos passam por `t()`; datas e números usam `Intl` no idioma ativo.
- [x] `done` — **Idiomas: textos que vêm do Rust** (2026-09-26): shaders
      prontos, notas de BIOS, aviso do QuickSave por atalho, instalador NSIS
      e o site (seletor PT/EN/ES).

### Infra / qualidade

- [x] `done` — **Teste de fumaça do catálogo, fase 1** (2026-09-25):
      workflow semanal `catalog-smoke.yml` (Linux e Windows) baixa cada core
      e abre com `reemu-core-host --probe` (`retro_init` + system info, sem
      jogo). Ver docs/historico.md.
- [x] `done` — **Fumaça do catálogo, fase 2** (2026-09-26): ROMs mínimas
      geradas pelo teste (NES, SNES, GB, GBA, Mega Drive, Master System, PC
      Engine, Atari 2600) rodam 180 quadros em cada core compatível; o
      travamento do pcsx_rearmed no CI (FIFO no /tmp) foi resolvido.
- [ ] `todo` — **VFS do libretro** (`RETRO_ENVIRONMENT_GET_VFS_INTERFACE`,
      v1–v3): o Stella 8 (`stella_libretro`) só reconhece a ROM com ela e
      hoje não abre nenhum jogo no ReEmu (o Stella 2014 funciona).

- [ ] `todo` — Etapa 11 (Android): `apps/mobile`, `packages/app-mobile`,
      `packages/ui`, `packages/shared`. Os pacotes compartilhados só nascem
      quando o mobile for o 2º consumidor. Esta máquina ainda precisa de NDK,
      JDK 17, `cmdline-tools` e targets Rust `*-linux-android*`.

## Projeto futuro: emuladores nativos em Rust

`blocked` — só começa quando o ReEmu for **totalmente compatível com cores
libretro** (critérios na Fase 0 do plano). Plano completo, levantamento de
licenças e ordem: [`docs/ai-context/14-emuladores-nativos-rust.md`](docs/ai-context/14-emuladores-nativos-rust.md).

Resumo: recriar em Rust os emuladores a partir de fontes **livres e
permissivas** (o ReEmu é MIT — traduzir código GPL obrigaria a virar GPL),
substituindo os cores libretro sistema a sistema, sempre com o libretro
como alternativa. Base principal: **ares** (ISC), que cobre quase todos os
sistemas. Ordem, do mais fácil pro mais difícil:

1. ColecoVision + SG-1000 → 2. Master System/Game Gear → 3. Game Boy/Color
→ 4. NES (adaptar o TetaNES, Rust) → 5. Atari 2600 → 6. PC Engine →
7. WonderSwan → 8. Neo Geo Pocket → 9. ZX Spectrum/MSX → 10. Atari 5200 →
11. Mega Drive → 12. GBA → 13. SNES → 14. Neo Geo → 15. Sega CD/32X/PCE CD
→ 16. Amiga → 17. C64 → 18. PlayStation → 19. N64 → 20. Saturn →
21. DS → 22. PS2. Dreamcast, PSP, GameCube/Wii, 3DS, DOS e ScummVM ficam
com libretro (só existem fontes GPL).

## Como atualizar

Ao concluir uma tarefa:

1. Tire a linha daqui (ou marque a etapa como `done` na tabela).
2. Registre o que foi feito, com o porquê e como foi validado, no topo de
   **Notas de progresso** em `docs/historico.md`.
3. Se algo ficou pra trás ou foi simplificado, vira uma linha nova aqui.
4. Se depende de algo externo (decisão, hardware), marque `blocked` com o
   motivo.
