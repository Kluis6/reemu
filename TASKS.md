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
- [ ] `todo` — **Instalador do `v0.1.0-rc2`** (Release draft no GitHub —
      o pipeline passou em Linux e Windows): instalar e abrir no Windows.
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
- [ ] `todo` — Shader: `test/format.slangp` (textura de inteiros) é a única
      falha de preset que sobrou por limitação do pipeline.

### Desempenho do caminho do core

Medir antes de otimizar: `REEMU_PERF=1 scripts/dev.sh` com um jogo pesado
(N64/PS1) e anotar as linhas `perf core`/`perf vídeo` (ver STEP_BY_STEP §4).
Feito em 2026-09-24 (ver `docs/historico.md`): pump acorda no frame
(perdia 12% dos frames), margem de spin adaptativa (33 → 6 ms/s de CPU),
áudio sem alocação por frame, canvas sem a 2ª cópia do frame.

- [ ] `todo` — Cópias de frame que restam: buffer nativo do core → anel
      (inevitável entre processos) e o caminho de CPU sem GPU (`to_rgba8` +
      `pack_frame`, só roda sem adapter wgpu).
- [ ] `todo` — Rebase do acumulador de pacing só a >4 frames de atraso
      (core lento roda sem dormir por até ~66 ms antes de ressincronizar);
      thread do core sem prioridade elevada. Decidir com dados do
      `REEMU_PERF=1` num jogo pesado.

### Funcionalidades

- [ ] `todo` — Metadata: multi-provider (IGDB / TheGamesDB) + cascata,
      rate-limit por provider, match por MD5, badge de pendências no rail.
- [ ] `todo` — DOS e ScummVM (jogos são pastas/`.zip` sem extensão própria).
- [ ] `todo` — PSP: baixar a pasta `assets` do PPSSPP (GPL) em
      `<system>/PPSSPP/`, se os jogos mostrarem problema sem ela.
- [ ] `todo` — `GET_INPUT_BITMASKS` não anunciado (cores caem no query por
      id; funciona, perde a otimização).

### Infra / qualidade

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
