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

- [ ] `todo` — **Windows ponta a ponta**: só os testes do `core-ipc` rodaram
      numa máquina Windows real. Falta `cargo tauri dev` completo, `video.rs`
      no caminho `#[cfg(not(linux))]`, paths do buildbot de cores, instalador.
      Suspeita a conferir: o `boxart` é `cover://localhost/<id>`
      (`commands/library.rs`), mas no Windows o Tauri 2 serve protocolo
      custom em `http://cover.localhost/` — capas podem não aparecer.
- [ ] `todo` — **Pipeline de release no GitHub Actions**: nunca rodou lá.
      Disparar `release.yml` por `workflow_dispatch` ou tag `v0.1.0-rc1`
      (sai como Release draft).
- [ ] `todo` — **Chaveiro no Windows**: `cargo test -p reemu-desktop --lib
      os_keyring -- --ignored` (Credential Manager).
- [ ] `todo` — `SET_ROTATION` com um jogo vertical real (FBNeo). Direção
      assumida = anti-horário; flipar se sair espelhado.
- [ ] `todo` — Dois controles idênticos ao mesmo tempo (fix por
      `GamepadId` sem teste automatizado possível).

### Vídeo / GPU

- [ ] `todo` — Etapa 12: flycast e mupen como 2º/3º alvo Vulkan; Fase C
      (tirar a espera de CPU do blit/submit, validar sob carga). Ver doc 12.
- [ ] `todo` — Interop GL: trocar `glFinish` por semáforo cross-API e tirar
      o gate `REEMU_GL_INTEROP`.
- [ ] `todo` — `SET_GEOMETRY`/`SET_SYSTEM_AV_INFO` em runtime propagam só
      timing; a proporção nova não pega. Baixa prioridade.
- [ ] `todo` — Integer scaling com moldura: quando o fator cai no meio do
      caminho entre dois inteiros, sobra barra preta grossa (aceito por ora).
- [ ] `todo` — Presets FSR 1.0 / RCAS / CAS (espaciais; temporais não servem
      pra emulação). HDR / tonemapping.
- [ ] `todo` — Canvas WebGL (`texImage2D`) em vez de `putImageData` no
      caminho `<canvas>`.
- [ ] `todo` — **Shaders do upstream regrediram pra 92,9%** (2470/2658,
      medido 2026-09-24 contra `libretro/slang-shaders@afb1416`). Não é
      código nosso: o mesmo código passa 173/179 no koko-aio de antes do sync
      e 2/2 no vectorscale antigo. Causas:
      (1) sync do koko-aio 1.9.101 (upstream `637d7bb`, 2026-09-16) →
      `shaders-ng/avglum_pass.slang` dá `'#if' : unexpected` no glslang
      (linha 1703 do fonte pré-processado), 176 presets;
      (2) reescrita do vectorscale (upstream `b61e1ee`, 2026-09-09) →
      `resolve-crossings.slang` falha na validação do naga, 4 presets.
      Afeta quem usa "Baixar pacote de shaders", que baixa o upstream atual.
      Atualizar também o `docs/shaders/working-presets.txt` depois.
- [ ] `todo` — Shader: `test/format.slangp` (textura de inteiros) é a única
      falha de preset que sobrou por limitação do pipeline.

### Desempenho do caminho do core

- [ ] `todo` — Cópias de frame restantes: buffer nativo do core, RGBA do
      `to_rgba8`, header de 8 bytes do `pack_frame`.
- [ ] `todo` — `latest_frame: Mutex<Option<Frame>>` → `triple_buffer`/`ArcSwap`.
- [ ] `todo` — Áudio: `push_samples` aloca `Vec` por frame; `drain_audio`
      faz `mem::take`. Resample direto do `&[i16]`; reservar capacidade.
      Resampler linear → `rubato` se a qualidade não bastar.
- [ ] `todo` — Spin de pacing (~0,5 ms CPU/frame); rebase do `next_deadline`
      só a >4 frames; sem prioridade de thread no `emu-core-loop`.

### Funcionalidades

- [ ] `todo` — Tema de alto contraste; persistir o tema no lado Rust (hoje
      `localStorage`).
- [ ] `todo` — Tempo de jogo: `SaveStateMetadata.play_time_at_save` sempre
      `None`.
- [ ] `todo` — Metadata: multi-provider (IGDB / TheGamesDB) + cascata,
      rate-limit por provider, match por MD5, badge de pendências no rail.
- [ ] `todo` — BIOS dos sistemas novos (Amiga Kickstart, Atari 5200, MSX,
      firmware do DS) em `domain::bios`.
- [ ] `todo` — DOS e ScummVM (jogos são pastas/`.zip` sem extensão própria).
- [ ] `todo` — PSP: baixar a pasta `assets` do PPSSPP (GPL) em
      `<system>/PPSSPP/`, se os jogos mostrarem problema sem ela.
- [ ] `todo` — `GET_INPUT_BITMASKS` não anunciado (cores caem no query por
      id; funciona, perde a otimização).

### Infra / qualidade

- [ ] `todo` — Teste intermitente
      `emu-session/tests/session.rs::pause_freezes_emulation_then_resume`: um
      `FrameReady` já no canal chega depois do `SetPaused(true)`.
- [ ] `todo` — Etapa 11 (Android): `apps/mobile`, `packages/app-mobile`,
      `packages/ui`, `packages/shared`. Os pacotes compartilhados só nascem
      quando o mobile for o 2º consumidor. Esta máquina ainda precisa de NDK,
      JDK 17, `cmdline-tools` e targets Rust `*-linux-android*`.

## Como atualizar

Ao concluir uma tarefa:

1. Tire a linha daqui (ou marque a etapa como `done` na tabela).
2. Registre o que foi feito, com o porquê e como foi validado, no topo de
   **Notas de progresso** em `docs/historico.md`.
3. Se algo ficou pra trás ou foi simplificado, vira uma linha nova aqui.
4. Se depende de algo externo (decisão, hardware), marque `blocked` com o
   motivo.
