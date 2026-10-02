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

- [x] `done` — **Vídeo nativo no Windows** (janela filha acima do
      WebView2): validado pelo usuário em 2026-10-02 (RTX 3060, wgpu em
      Vulkan; vbam, gpsp, parallel_n64, fbalpha2012 vertical). O jogo
      aparece, Esc abre o menu e volta, clicar no jogo não tira o teclado,
      redimensionar e tela cheia acompanham. O jogo anterior piscava na
      troca, e isso foi corrigido limpando a surface de preto antes de
      esconder (`lib.rs`, pump). `REEMU_NATIVE_VIDEO=0` ainda volta pro
      `<canvas>`.
- [ ] `todo` — **Testar o protótipo de overlay no Windows**
      (`REEMU_WIN_OVERLAY`, 2026-10-01): WebView2 transparente por cima do
      jogo. Rodar `REEMU_WIN_OVERLAY=1` (surface no HWND principal) e
      `REEMU_WIN_OVERLAY=child` (janela filha abaixo do WebView2) e, em
      cada um, conferir: o jogo aparece na PlayScreen; Esc abre o menu POR
      CIMA do jogo (escurecido, sem print); avisos/toasts aparecem durante o
      jogo; biblioteca e configurações continuam opacas; trocar de jogo não
      mostra o anterior. A variante que funcionar vira o padrão.
      **2026-10-02, `=1` (HWND principal): tela preta** com flycast e
      vbam (RTX 3060, wgpu em Vulkan). O log mostra o swapchain
      apresentando ("1º frame apresentado"), mas nada aparece através do
      WebView2 transparente. A causa provável é que, no modo janela, o
      WebView2 não deixa ver o swapchain do HWND pai. Isso não está
      verificado: a página da Microsoft "Windowed vs. Visual hosting" não
      trata desse caso. **`=child`: tela preta também** (vbam e gpsp, com
      som e "1º frame apresentado"). Nos dois casos o WebView2 do Tauri,
      no modo janela, não deixa ver nada por baixo. Para o protótipo
      funcionar seria preciso hospedar o WebView2 como Visual
      (DirectComposition), o que o Tauri/wry não faz hoje. O padrão sem a
      flag (janela filha acima do WebView2) funcionou e segue como padrão.
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
      `jogo`, `pausa`, `cores`, `pagina-jogo` .png). 2026-10-02: o usuário
      mandou `biblioteca`, `jogo` e `pagina-jogo` pelo chat, mas as imagens
      coladas não viraram arquivo: falta salvar na pasta.
- [x] `done` — Logo e favicon do site (`site/logo.webp`, `favicon-*.png`,
      `icon-512.png`), 2026-10-01.

### Vídeo / GPU

- [x] `done` — **OpenGL por hardware no Windows** (WGL): validado pelo
      usuário em 2026-10-01 pelos logs — flycast (GL 3.2), Beetle PSX HW
      (3.3) e parallel_n64 (3.0 compat) abriram contexto e renderizaram.
- [ ] `todo` — Vulkan in-process no Windows: ver "GPU dos cores no
      Windows" abaixo (fase C).

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

### GPU dos cores no Windows (OpenGL, Vulkan, interop)

Situação em 2026-10-02 (RTX 3060, wgpu em Vulkan):
- **OpenGL (WGL):** funciona. Os cores rodam no `reemu-core-host` (processo
  filho), e o quadro sai por `glReadPixels` síncrono
  (`gl_context.rs::read_pixels`): GPU → CPU, linhas invertidas na CPU, cópia
  pro anel de memória compartilhada (`core-ipc/shm_ring_win.rs`) e upload
  pro wgpu no app. Esse é o **caminho lento**.
- **Vulkan dos cores:** desligado por padrão. Todo core vai pro processo
  filho (`route_local_device` só escolhe in-process sozinho no Linux), e lá a
  `VkImage` não cruza o processo. No Windows, um core que prefere Vulkan
  (flycast) roda em GL.
- O interop do Linux (`dma_buf` + `sync_file`) não existe no Windows. O
  equivalente oficial está abaixo, na fase B.

Regra do projeto: cada decisão de GPU com a documentação oficial (CLAUDE.md).
As fontes já conferidas estão citadas em cada item; o resto fica marcado "a
conferir".

**Fase A — caminho lento mais rápido (sem interop, vale pra qualquer GPU)**

- [x] `done` — A0. Medição de base, `REEMU_PERF=1`, linha `perf readback`
      no core-host (RTX 3060, 2026-10-02). Médias por quadro:

      | | 640×480 (1,2 MB) | 1920×1440 (10,5 MB) |
      |---|---|---|
      | `glFinish` | 0,6–2 ms | 1,3–3,7 ms |
      | `glReadPixels` | 0,5–1,8 ms | 2,1–3,1 ms |
      | flip na CPU | 0,1 ms | 0,75 ms |
      | envio (cópia pro anel) | 0,17 ms | 1,1 ms |
      | `retro_run` inteiro (de 16,7) | 2,5–5 ms | 7–14 ms |

      Em 640×480 o caminho lento não pesa. Em 1920×1440 ele leva ~5–8 ms,
      metade do quadro: com o `retro_run` em 12–14 ms apareceram 7–13
      quadros/s acima do orçamento. Decisão: fazer A1 e A2.
- [ ] `todo` — A1. Readback assíncrono com PBO: `glReadPixels` num
      `GL_PIXEL_PACK_BUFFER` (2–3 buffers em anel) + `glFenceSync`; ler o
      quadro N−1 enquanto a GPU faz o N. Mapear o PBO e copiar direto pro
      slot do anel compartilhado, sem o `Vec` intermediário. A conferir:
      OpenGL Wiki "Pixel Buffer Object", refpages `glReadPixels`,
      `glMapBufferRange` e `glFenceSync`. Custo: um quadro de latência.
      Medir se compensa contra o A0.
- [ ] `todo` — A2. Tirar o flip de linhas da CPU (`flip_rows_in_place`):
      mandar `flip_y` junto do quadro e inverter na amostragem do wgpu,
      como o caminho `Hardware { flip_y }` já faz.
- [ ] `todo` — A3. Repetir a medição do A0 e registrar no histórico.

- [ ] `in-progress` — A4. Ritmo core × monitor (achado no A0, não é do
      readback). Medido em 2026-10-02 com o `perf vídeo` dividido
      (`e912b1c`): ~15,6 dos ~16,7 ms do "render" são espera no
      `get_current_texture`; shaders + submit + present ficam abaixo de 1
      ms. Igual com Mailbox/latência 2 e com latência 1. Pela spec Vulkan
      (`VkPresentModeKHR`), o Mailbox não deveria esperar o refresh; aqui
      espera. A forma como o driver NVIDIA apresenta uma janela composta
      no Windows não está na doc da Khronos (não verificado).
      **Causa: deslize de fase.** O core-host roda no relógio próprio
      (60,0 fps) e o monitor a 59,8 Hz (o pump faz 59,8 voltas/s). Os
      quadros chegam cada vez mais cedo em relação ao refresh, a espera
      cresce até um refresh inteiro e então sobra 1 quadro ("1 perdidos"),
      e o ciclo recomeça a cada ~5 s. A queda da espera coincide com os
      segundos de quadro perdido. Pro jogador: tranco a cada ~5 s e
      latência que oscila entre ~1 e ~2 quadros, em qualquer modo de
      present.
      **Correção proposta:** core no ritmo do monitor. (1) Medir o refresh
      real (Windows: conferir na doc da Microsoft a API certa, provável o
      timing de composição do DWM). (2) Core-host roda nesse período; a
      diferença de ~0,3% no áudio é absorvida pelo Dynamic Rate Control da
      etapa 06. (3) Alinhar a fase: gerar o quadro logo depois do refresh,
      pra espera no acquire ficar perto de zero.
      **Implementado em 2026-10-02, falta validar:** o pump mede o período
      do monitor pelos acquires que bloquearam (`VblankEstimator`) e manda
      `ToChild::VsyncTick` depois de cada present. O core-host
      (`VsyncLock`, em `pacing.rs`) prende o ritmo ao monitor quando o
      período fica a até 0,4% do budget por 30 ticks seguidos, e volta ao
      `Pacer` fora disso ou sem tick por 1,5 quadro.
      `REEMU_VSYNC_PACING=0` desliga.
      **1º teste (2026-10-02, Mailbox + latência 1, sem querer: a variável
      do teste 2 ficou no PowerShell):** com latência 1 o acquire quase não
      bloqueia (~0,03 ms) e o pump não perdeu quadro em ~60 s. O lock
      travava com ticks esparsos e esperava 1,5 quadro pelo tick: intervalo
      de 36 ms a cada 5–9 s. Corrigido: só trava com tick sem buraco de 2
      quadros; travado, espera só até o prazo normal (+1 ms) e destrava
      depois de 3 faltas; tick atrasado não gera quadro extra. Falta
      validar de novo, e comparar latência 1 × 2 com as variáveis limpas
      (pode ser que latência 1 deva virar o padrão).
      **2º teste (latência 1 de novo):** fora do lock o vídeo ficou liso
      (acquire ~0,03 ms, core 16,68 ms). O lock engatou e funcionou (60
      quadros a 16,72 ms, 0 perdidos), mas caía depois de cada travada
      interna do flycast (`retro_run` de 76–83 ms): a fila de present
      esvaziava, os acquires não bloqueavam e 3 faltas destravavam, com
      23–24 ms a cada entrada e saída. Agora destrava só depois de 30
      faltas. Falta testar o caso com latência 2 (variável limpa).
- [ ] `todo` — Testes de integração do `emu-session` no Windows: o core de
      teste (`testcore_path`) carrega sem `retro_set_environment`
      ("GetProcAddress failed"), e os 9 testes de `tests/session.rs`
      falham. Já falhavam antes de 2026-10-02 (conferido com `git stash`).
      Provável: a DLL de teste não exporta os símbolos no Windows.
- [ ] `todo` — A5. Áudio em dobro (achado no A0): no flycast em 1920×1440,
      depois de o core travar a ~6 fps (05:54:42), o core-host passou a
      contar ~176 mil amostras/s contra ~88 mil esperadas. Ver se o core
      mudou a taxa sem `SET_SYSTEM_AV_INFO` ou se o contador está errado.

**Fase B — interop GL → Vulkan no Windows (zero cópia de CPU)**

Fontes conferidas: `GL_EXT_memory_object_win32` / `GL_EXT_semaphore_win32`
(registry.khronos.org, `EXT_external_objects_win32.txt`: importa memória e
semáforos de handles Win32; tipo `HANDLE_TYPE_OPAQUE_WIN32_EXT` = 0x9587;
importar não transfere a posse do handle, e quem importa fecha o handle NT)
e `VK_KHR_external_memory_win32` (docs.vulkan.org: `vkGetMemoryWin32HandleKHR`
transfere a posse do handle pra aplicação, que chama `CloseHandle`; o import
exige memória "created on the same underlying physical device"). Nenhuma das
duas trata de handle vindo de outro processo: isso é com o Win32
(`DuplicateHandle`, a conferir em learn.microsoft.com).

- [ ] `todo` — B1. Detectar suporte nos dois lados: extensões GL
      `GL_EXT_memory_object`, `GL_EXT_memory_object_win32`,
      `GL_EXT_semaphore` e `GL_EXT_semaphore_win32` no contexto WGL do filho;
      `VK_KHR_external_memory_win32` e `VK_KHR_external_semaphore_win32` no
      device do wgpu. Conferir se é a mesma GPU (UUID do device no GL e
      `VkPhysicalDeviceIDProperties` no Vulkan; a conferir na
      `EXT_external_objects.txt`). Sem suporte, segue no readback.
- [ ] `todo` — B2. App (Vulkan) aloca um anel de 2–3 imagens exportáveis
      (`VkExportMemoryAllocateInfo`, tipo opaco Win32), pega o handle NT de
      cada uma, duplica pro processo do core-host (`DuplicateHandle`) e
      manda o valor pelo pipe. Hoje o pipe não passa handle inline
      (`transport_win.rs`), então vai uma mensagem nova em `ToChild`.
      Importar no wgpu como textura (o mesmo `texture_from_raw` do
      `dma_buf` do Linux).
- [ ] `todo` — B3. Core-host (GL) importa cada handle
      (`glImportMemoryWin32HandleEXT` + `glTexStorageMem2DEXT`), usa como
      color attachment do FBO do core e fecha o handle depois de importar.
- [ ] `todo` — B4. Sincronização sem `glFinish`: semáforos exportados pelo
      Vulkan (`VK_KHR_external_semaphore_win32`) e importados no GL
      (`glImportSemaphoreWin32HandleEXT`). O GL sinaliza no fim do quadro
      (`glSignalSemaphoreEXT`) e o wgpu espera antes de amostrar. Mesmo
      papel do `sync_file` → semáforo do Linux.
- [ ] `todo` — B5. Quadro pelo pipe como `FrameKind::Hardware` com o
      índice do slot (sem fd). Fallback automático pro readback em qualquer
      falha, como no Linux. `REEMU_GL_INTEROP=0` força o readback.
- [ ] `todo` — B6. Validar com as camadas de validação do Vulkan (sync
      incluída), na NVIDIA e, se der, numa AMD/Intel. Medir contra a fase A.

**Fase C — cores Vulkan no Windows**

- [ ] `todo` — C1. Reproduzir com `REEMU_HW=vulkan` + flycast no Windows.
      O crash de 2026-09-25 (`STATUS_ACCESS_VIOLATION`) tem causa provável
      já corrigida no Linux (despachante nulo / extensões não ligadas, hook
      do `vkCreateDevice`). Rodar com as camadas de validação.
- [ ] `todo` — C2. Se passar, ligar a escolha automática no Windows
      (`route_local_device`: `auto` hoje é `cfg!(target_os = "linux")`) e
      validar flycast, Beetle PSX HW e mupen64plus_next com `parallel`.
- [ ] `todo` — C3. (Depois da B) Core Vulkan no processo filho: o core-host
      cria o próprio device, renderiza em imagens exportadas pelo app
      (`VK_KHR_external_memory_win32`) e devolve só o índice + semáforo.
      Devolve o isolamento de processo aos cores Vulkan: um crash do core não
      derruba a interface. Exige o mesmo device físico nos dois lados.

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
- [x] `done` — **VFS do libretro v3** (2026-09-26): `core-loader-desktop/src/vfs.rs`;
      o Stella 8 voltou a abrir jogos (Pitfall II testado).

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
