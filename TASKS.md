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
| 12 | Vulkan HW Render Fase 2 | `done`: Beetle PSX HW, flycast e mupen64plus_next (Linux 2026-09-25, Windows 2026-10-02), a fase C e o C3 (Vulkan no processo filho, Windows, 2026-10-11) |

Desktop (01–10) fechado. Detalhe de cada etapa: `docs/historico.md` ›
"Etapas de implementação" e o "Estado atual" de cada doc em
`docs/ai-context/`.

## Em aberto

### Depende do dono do projeto

- [ ] `todo` — **Credenciais de desenvolvedor do ScreenScraper** (2026-10-08):
      sem `devid`/`devpassword` a API responde 403 em toda busca e nenhum
      metadado vem do ScreenScraper. Pedir no fórum do screenscraper.fr
      (apresentar o ReEmu) e cadastrar como secrets `REEMU_SS_DEVID` e
      `REEMU_SS_DEVPASSWORD` no GitHub — o `release.yml` já repassa ao build
      e o `scraping.rs` envia quando existem. Pra testar local: exportar as
      duas variáveis antes do `cargo tauri dev`.

### Validação (precisa de hardware/máquina que não é esta)

- [ ] `todo` — **Stella trava no Windows na fumaça do catálogo**
      (2026-10-14): desde 2026-09-28 o `catalog-smoke.yml` falha toda
      semana só no Windows porque `stella_libretro` (Stella 8) e
      `stella2023_libretro` (Stella 7) passam do limite de 60 s rodando a ROM
      de teste do Atari 2600 ("FALHA: travou"). No Linux os dois rodam os 180
      quadros, e o `stella2014_libretro` passa nos dois sistemas. A falha
      começou dois dias depois do VFS v3 (2026-09-26,
      `core-loader-desktop/src/vfs.rs`); suspeita, não confirmada, de um
      caminho do VFS específico do Windows. Em 2026-10-12 o `ppsspp_libretro` também travou
      no Windows, ainda no `retro_init` (> 60 s); precisa ver se repete.
      Reproduzir numa máquina Windows com `reemu-core-host` e a ROM do teste.

- [x] `done` — **Vídeo nativo no Windows** (janela filha acima do
      WebView2): validado pelo usuário em 2026-10-02 (RTX 3060, wgpu em
      Vulkan; vbam, gpsp, parallel_n64, fbalpha2012 vertical). O jogo
      aparece, Esc abre o menu e volta, clicar no jogo não tira o teclado,
      redimensionar e tela cheia acompanham. O jogo anterior piscava na
      troca, e isso foi corrigido limpando a surface de preto antes de
      esconder (`lib.rs`, pump). `REEMU_NATIVE_VIDEO=0` ainda volta pro
      `<canvas>`.
- [x] `done` — **Protótipo de overlay no Windows** (`REEMU_WIN_OVERLAY`):
      testado em 2026-10-02, tela preta nas duas variantes (`=1` e
      `=child`). O WebView2 em modo janela não deixa ver o que está por
      baixo; precisaria de visual hosting (DirectComposition), que o
      Tauri/wry não usa. O padrão (janela filha acima do WebView2) ficou.
- [x] `done` — **Windows ponta a ponta**: validado pelo usuário em 2026-10-08. Antes, só os testes do `core-ipc` rodavam
      numa máquina Windows real. Falta `cargo tauri dev` completo, `video.rs`
      no caminho `#[cfg(not(linux))]`, paths do buildbot de cores, instalador.
      Conferir que as capas aparecem (URL `http://cover.localhost/<id>` no
      Windows, `covers::cover_url`, corrigida sem teste em máquina real).
- [x] `done` — **Publicar uma versão com auto-update**: superado pela
      `v0.1.4`, publicada em 2026-10-02 (instaladores assinados e
      `latest.json` com a 0.1.4 nas seis variantes).
- [x] `done` — **Chaveiro no Windows** (validado pelo usuário em 2026-10-08): `cargo test -p reemu-desktop --lib
      os_keyring -- --ignored` (Credential Manager).
- [x] `done` — `SET_ROTATION` com um jogo vertical real: validado pelo
      usuário em 2026-09-25 (shooter vertical de arcade em pé, sem espelhar).

### Atualização automática

- [ ] `todo` — Validar ponta a ponta com duas Releases publicadas: instalar
      a antiga pelo AppImage e pelo NSIS, atualizar pelo modal e conferir o
      aviso "atualizado" depois do reinício. O `.deb` usa `pkexec`.
- [x] `done` — Capturas do site em `site/screens/` (2026-10-02): `jogo`,
      `biblioteca`, `pagina-jogo`, `jogo-moldura`, `pausa` e
      `configuracoes` .png, mandadas pelo usuário. As vagas de tela
      inicial e de catálogo de cores saíram da galeria (sem captura).
- [x] `done` — Logo e favicon do site (`site/logo.webp`, `favicon-*.png`,
      `icon-512.png`), 2026-10-01.

### Vídeo / GPU

- [ ] `backlog` — **Menus e avisos por cima do jogo em movimento (Windows)**,
      pedido do dono do projeto (2026-10-28): fica para depois. Por ora seguem
      como estão: o menu de pausa esconde o vídeo e usa a foto do quadro, e os
      avisos e as conquistas da fase 4 esperam esta decisão.
      - **Testado em 2026-10-28:** o WebView2 transparente em "Window to
        Visual" (`COREWEBVIEW2_FORCED_HOSTING_MODE=COREWEBVIEW2_HOSTING_MODE_WINDOW_TO_VISUAL`,
        com `REEMU_WIN_OVERLAY=child`) deu **tela branca**, sem o jogo.
        - O modo mudou de fato: só sobrou a janela de hospedagem do WebView2
          no nosso processo.
        - Antes, no modo janela, deu tela preta (2026-10-02).
        - Provável causa, não verificada: aparece o fundo da própria janela do
          WebView, não a janela do jogo embaixo.
      - **O Tauri 2.12.5 e o wry 0.57.1 não mudam isso:** os changelogs só
        trazem transparência para macOS e iOS. O modo visual completo
        (composition controller) exigiria mudar o wry e encaminhar mouse e
        teclado à mão.
      - **Opções levantadas:**
        1. Recorte na janela do jogo (`SetWindowRgn`) onde fica o aviso
           HTML. Rápido, mas o aviso fica opaco.
        2. Avisos ou menu desenhados no wgpu (texto com `glyphon`, ou `egui`).
           Interface duplicada fora do React.
        3. Segunda janela transparente, sem borda e sempre por cima, com um
           WebView próprio. O menu fica em HTML sobre o jogo ao vivo, mas é
           preciso acompanhar posição, minimização e foco. Sugestão para o
           menu.
        4. Modo canvas (`REEMU_NATIVE_VIDEO=0`), que já existe, com o custo da
           cópia de cada quadro.
        5. `TextureStream` do WebView2: só proposta, sem API estável.

- [ ] `todo` — Shaders do pacote que o WebGPU recusa na prévia (2026-10-16,
      `field_render_real_presets`: monta os pipelines e renderiza a cena 2D;
      o `field_validate_real_presets` só compila e não pega isto). Placar
      depois das correções do dia: fora do Mega Bezel ~86% (1006/1164 antes
      do ajuste do último passe); **Mega Bezel 0/1494**. Erros, por grupo:
      - `Parser recursion limit exceeded` do naga ao criar o shader module
        (487, quase todo Mega Bezel);
      - Mega Bezel: "passe 28 não conseguiu resolver uma textura de bind
        group" (alias/histórico na chain) — o quadro é pulado;
      - vertex com mais saídas que `max_inter_stage_shader_variables` (~200);
      - fragment lê `location` que o vertex não escreve (90; o Vulkan aceita,
        o WebGPU não — ex.: `downsample/mixed-res-4x-crt-hyllian`);
      - último passe lido como feedback em `Rgba16Float` × blit `Rgba8Unorm`
        (`blurs/dual_filter_bloom*`).
      Hoje esses presets voltam como erro (o shader anterior continua e a
      prévia mostra o aviso) em vez de derrubar o wgpu.

- [x] `done` — **OpenGL por hardware no Windows** (WGL): validado pelo
      usuário em 2026-10-01 pelos logs — flycast (GL 3.2), Beetle PSX HW
      (3.3) e parallel_n64 (3.0 compat) abriram contexto e renderizaram.
- [x] `done` — Vulkan in-process no Windows: feito e automático (fase C,
      C1 e C2, 2026-10-02; reconferido no código em 2026-10-08:
      `route_local_device` escolhe in-process no Windows os cores de
      `VK_CAPABLE_CORES`). Falta só o C3 (Vulkan no processo filho).

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
- [x] `done` — CAS (2026-10-11): AMD FidelityFX CAS portado do `ffx_cas.h`
      oficial (GPUOpen-Effects/FidelityFX-CAS, MIT) como shader próprio
      embutido (`apps/desktop/src-tauri/shaders/cas`, preset curado
      "Nitidez adaptativa (AMD CAS)", não depende do pacote). Teste
      `cas_matches_the_reference_math`: a saída da GPU bate pixel a pixel com a
      referência das fórmulas da AMD.
- [x] `done` — HDR (2026-10-11): saída HDR na janela do jogo
      (Configurações › Sistema › Vídeo): superfície `Rgba16Float` +
      `ExtendedSrgbLinear` (scRGB) do wgpu 30, branco SDR e pico
      automáticos lidos do monitor (`display_hdr_info`) ou escolhidos, e
      realce de brilhos (inversa do Reinhard estendido). Testes:
      `hdr_blit_maps_sdr_to_scrgb` (matemática na GPU),
      `hdr_surface_diagnostics` (janela real, monitor HDR desta máquina),
      `video_config_hdr_roundtrip_and_defaults`. Falta conferir a olho no app.
- [x] `decidido` — Shader: `test/format.slangp` (textura de inteiros) fica
      sem suporte (2026-10-11). É um preset de teste do RetroArch: o passe 0
      grava em `R32_UINT` e o 1 lê com `usampler2D` + `texelFetch`. No pacote
      inteiro, `usampler2D`/`isampler2D` só aparecem nele; o outro
      `#pragma format R32_UINT` (`nes_raw_palette/.../int_stock.slang`) grava
      `vec4` comum e nenhum `.slangp` o usa (o `cgwg-famicom-geom.slangp`
      funciona). Suportar exigiria `usampler2D` → `utexture2D` no
      `shader-slang` (hoje `contains("sampler2D")` casa com `usampler2D` e o
      declara como float — erro do glslang no `unpackUnorm4x8`) e bind groups
      com tipo de amostra Uint, sem filtragem nem blending no wgpu. Reabrir se
      um preset de verdade passar a usar.

### GPU dos cores no Windows (OpenGL, Vulkan, interop)

Situação em 2026-10-11 (RTX 3060, wgpu em Vulkan):
- **OpenGL (WGL):** os cores rodam no `reemu-core-host` (processo filho) e,
  desde o B5, o quadro chega ao app **sem cópia**: o core desenha num anel de
  imagens Vulkan compartilhadas (`EXT_external_objects_win32`) e o filho
  manda só o índice. GPU sem suporte, import recusado ou nenhum slot livre
  → readback (fase A). `REEMU_GL_INTEROP=0` força o readback.
- **Vulkan dos cores:** desde o C3c, os cores de `VK_CAPABLE_CORES` rodam
  no processo filho com device próprio e o quadro chega ao app pelo mesmo
  anel compartilhado do GL (sem cópia). Um crash do core não derruba mais a
  interface. `REEMU_VK_CHILD=0` volta pra rota in-process (C1/C2).

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
- [x] `done` — A1. Readback assíncrono com PBO (2026-10-02): dois PBOs
      em revezamento. O `glReadPixels` do quadro N vai pra um (byte
      offset no buffer, refpage `glReadPixels`), e o outro, com o N-1, é
      mapeado com `glMapBufferRange`, que só volta "once all pending
      operations on the buffer object have completed" (refpage): isso já
      sincroniza, sem `glFinish`. 1 quadro de latência; o 1º quadro do jogo
      não sai. GL/GLES ≥ 3.0; `REEMU_GL_PBO=0` ou falha no mapeamento volta
      ao síncrono. Teste `pbo_readback_delivers_the_previous_frame` (cor e
      orientação do quadro anterior) passou num WGL 3.3 real. Falta medir
      (A3).
- [x] `done` — A2. Inversão das linhas na GPU (2026-10-02): em vez de
      `flip_rows_in_place` na CPU (~0,75 ms/quadro em 1920×1440), o
      `read_pixels` faz um `glBlitFramebuffer` com o destino de cabeça pra
      baixo pra um FBO à parte e lê dele ("If either the source or
      destination rectangle specifies a negative dimension, the image is
      reversed", EXT_framebuffer_blit; FBO separado porque blit no mesmo
      buffer com sobreposição é indefinido). Só com GL/GLES ≥ 3.0 (refpage
      do `glBlitFramebuffer`); abaixo disso, ou se o FBO falhar, continua na
      CPU. Teste `fbo_clear_and_readback` estendido pra conferir a
      orientação; passou num contexto WGL 3.3 real (RTX 3060). Saiu de outro
      jeito que o planejado (`flip_y` até o wgpu): assim o protocolo entre
      os processos não muda.
- [x] `done` — A3. Medição depois da A1 e da A2 (2026-10-02, Jet Set
      Radio em 1920×1440, 10,5 MB/quadro, RTX 3060): `glFinish` 0,00 ms,
      flip na CPU 0,00 ms, leitura ~2,0–2,5 ms (só a cópia do PBO já
      pronto). O caminho lento caiu de ~5–8 ms pra ~2–2,5 ms por quadro. O
      resto (leitura + ~1,3 ms de envio) é a cópia de CPU de 10,5 MB; só a
      fase B tira. 0 quadros perdidos, intervalo cravado.
- [x] `done` — A4. Ritmo core × monitor (achado no A0, não é do
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
      **3º teste e decisão (2026-10-02):** travado, o core caía pra 48–56
      fps com intervalos de ~25 ms (tick intermitente com latência 1). Fora
      do lock, latência 1 deu 60 fps cravados, 0 perdidos e acquire de
      ~0,03 ms; com latência 2 (teste anterior) eram ~15,6 ms e 1 perdido a
      cada ~5 s. **Latência 1 virou o padrão e o lock ficou opcional**
      (`REEMU_VSYNC_PACING=1`). Pendente: validar latência 1 em outra GPU
      (AMD/Intel) e no Linux/Wayland; o deslize de fase de 0,3% continua
      (o Mailbox descarta o quadro sobrando sem tranco visível).
      **4º teste:** com latência 1 e o binário novo, o acquire voltou a
      esperar ~14,5 ms com 1 quadro perdido a cada 2–4 s. A latência 1 só
      muda quando o deslize aparece, não resolve.
      **Troca de abordagem (2026-10-02): casar o ritmo em vez de travar no
      tick.** O `VsyncLock` saiu. O pump manda o período do monitor uma vez
      por segundo (`ToChild::DisplayPeriod`), medido por soma de refreshes
      inteiros (`VblankEstimator`; teste com jitter de ±0,8 ms: erro abaixo
      de 0,05%). O core-host (`RateMatch`) usa esse período no `Pacer` se
      estiver a até 0,4% do pedido pelo core (`Pacer::set_period`, sem
      salto de fase), senão o do core. O core nunca espera o monitor.
      `REEMU_VSYNC_PACING=0` desliga. Falta validar no jogo: o log do
      core-host mostra `ritmo: casado com o monitor`, e o `perf vídeo` deve
      ficar sem quadros perdidos.
      **Validado no Windows (2026-10-02, flycast, RTX 3060):** `alvo 59.8`
      (casado), 0 quadros perdidos em todos os segundos, espera no acquire
      estável em 14,5–14,7 ms (sem o serrote de 4 → 16,7 ms) e intervalo
      do core cravado em 16,73 ms. Pendente fora daqui: validar numa
      AMD/Intel e no Linux/Wayland.
- [x] `done` — Testes de integração do `emu-session` no Windows (corrigido em
      2026-10-08: `RETRO_API` = `__declspec(dllexport)` no `testcore.c`, como o
      `libretro.h` oficial; workspace inteiro passa no Windows). Era: o core de
      teste (`testcore_path`) carrega sem `retro_set_environment`
      ("GetProcAddress failed"), e os 9 testes de `tests/session.rs`
      falham. Já falhavam antes de 2026-10-02 (conferido com `git stash`).
      Provável: a DLL de teste não exporta os símbolos no Windows.
- [x] `done` — A5. Áudio em dobro no flycast em jogos a 30 fps. **Causa
      (fonte do flycast, `shell/libretro/libretro.cpp`):** com Threaded
      Rendering, o `retro_run` repete `emu.render()` até achar um quadro
      novo; num jogo a 30 fps, cada chamada emula 2 refreshes e o áudio sai
      em dobro. **Correção:** o ReEmu liga por padrão a opção
      `reicast_detect_vsync_swap_interval` ("Detect Frame Rate Changes";
      `FRONTEND_DEFAULTS` em `core-loader-desktop/src/coreopts.rs`), e o
      core avisa por `SET_SYSTEM_AV_INFO` quando o jogo muda de taxa.
      Validado no Jet Set Radio (2026-10-02): 30 chamadas/s e 88.284
      amostras/s de áudio contra ~88.282 esperadas. Jogo de taxa instável
      (Ecco, Unreal Tournament) pode desligar a opção por jogo.
- [x] `done` — B1 (2026-10-11): `domain::gpu_interop::interop_compatible`
      (4 extensões GL + 2 Vulkan + deviceUUID e driverUUID iguais); o app
      manda o `VkInteropId` no `Load`, o filho compara com o contexto WGL e
      registra no log. RTX 3060: tudo presente e UUIDs idênticos — interop
      possível. Testes `wgl_interop_caps_read_this_gpu` e
      `vk_interop_id_reads_this_device` (`--ignored`, GPU real). Texto
      original: Detectar suporte nos dois lados: extensões GL
      `GL_EXT_memory_object`, `GL_EXT_memory_object_win32`,
      `GL_EXT_semaphore` e `GL_EXT_semaphore_win32` no contexto WGL do filho;
      `VK_KHR_external_memory_win32` e `VK_KHR_external_semaphore_win32` no
      device do wgpu. Conferir se é a mesma GPU (UUID do device no GL e
      `VkPhysicalDeviceIDProperties` no Vulkan; a conferir na
      `EXT_external_objects.txt`). Sem suporte, segue no readback.
- [x] `done` — B2 (2026-10-11, atrás de `REEMU_GL_INTEROP=1` até o B5):
      `gpu/win_interop.rs` abre o device com as extensões Win32, aloca 3
      imagens exportáveis (memória dedicada, handle NT opaco), embrulha no
      wgpu e duplica os handles pro filho; `ToParent::InteropRequest` →
      provedor do app (`EmuSession::set_interop_provider`) →
      `ToChild::InteropImages`. Testado na RTX 3060 de ponta a ponta
      (`win32_interop_ring_reaches_the_core_process`) e com as camadas de
      validação, sem nenhum VUID. Texto original: App (Vulkan) aloca um anel de 2–3 imagens exportáveis
      (`VkExportMemoryAllocateInfo`, tipo opaco Win32), pega o handle NT de
      cada uma, duplica pro processo do core-host (`DuplicateHandle`) e
      manda o valor pelo pipe. Hoje o pipe não passa handle inline
      (`transport_win.rs`), então vai uma mensagem nova em `ToChild`.
      Importar no wgpu como textura (o mesmo `texture_from_raw` do
      `dma_buf` do Linux).
- [x] `done` — B3 (2026-10-11, atrás de `REEMU_GL_INTEROP=1`): o
      `GlContext` do Windows importa o anel no próximo quadro
      (`glCreateMemoryObjectsEXT` → `DEDICATED_MEMORY_OBJECT_EXT` →
      `glImportMemoryWin32HandleEXT` → `TEXTURE_TILING_EXT` ótimo →
      `glTexStorageMem2DEXT(GL_RGBA8)`) e o `bind_write_slot` gira o color
      attachment do FBO entre as texturas; o quadro ainda sai por readback.
      Os handles ficam abertos até o anel sumir (a spec do GL não diz que
      dá pra fechar logo depois do import). Do lado Vulkan, a imagem passou
      a ter todos os usos do formato e `MUTABLE_FORMAT` (issues 13 e 17 da
      EXT_external_objects). Teste: `win32_interop_ring_imports_into_gl`
      (RTX 3060, camadas de validação sem VUID).
- [x] `done` — B4 (2026-10-11): dois semáforos binários exportáveis por
      imagem (molde do `open_gl_interop` do Vulkan-Samples): `done` (o GL
      sinaliza no fim do quadro, `GlContext::hand_over_slot`, com a textura
      em `GL_LAYOUT_SHADER_READ_ONLY_EXT`) e `free` (o app sinaliza ao
      terminar de ler; o `bind_write_slot` espera antes de redesenhar no
      slot). Vão no `InteropImages` com a memória; o filho importa com
      `glImportSemaphoreWin32HandleEXT`. Teste de ida e volta
      `win32_interop_semaphores_round_trip` (RTX 3060, sem VUID). Em uso
      real só a partir do B5 (o `hand_over_slot` ainda não é chamado no
      quadro do core).
- [x] `done` — B5 (2026-10-11): o quadro do core GL sai pelo pipe como
      `FrameKind::Hardware` com o índice do slot do anel; o app amostra a
      imagem compartilhada esperando o `done` e sinalizando o `free` na
      mesma submissão do wgpu (`add_wait_semaphore`/`add_signal_semaphore`)
      e, depois de submeter, avisa o filho (`ToChild::InteropSlotFree`) —
      o GL só espera um `free` já enviado. Quadros descartados devolvem o
      slot do mesmo jeito (`take_dropped_interop_slots` → `flush_interop`).
      Sem slot livre, o quadro vai pelo readback (nunca trava). **Ligado
      por padrão**; `REEMU_GL_INTEROP=0` força o readback. Medido na RTX
      3060: mupen64plus_next (GoldenEye, Mario Kart 64) ~59 fps pelo anel,
      flycast (Soulcalibur) 700 quadros sem nenhum readback; orientação e
      cores conferidas num PNG; camadas de validação sem VUID.
- [x] `done` — B6 (2026-10-11, NVIDIA): camadas de validação com a de
      sincronização (`VK_KHRONOS_VALIDATION_VALIDATE_SYNC=true`, do
      `Config/vk_layer_settings.sh` do SDK 1.4.363) sem nenhum aviso em
      todos os testes do interop, com mupen64plus_next e flycast; um
      conflito plantado de propósito gerou `SYNC-HAZARD-WRITE-AFTER-WRITE`,
      o que prova que a camada estava ativa. Medido contra o readback da fase A:
      flycast em 1920×1440, processo do core 4,45 → 1,27 ms/quadro, app
      5,12 → 2,14 ms/quadro, 10,5 MB/quadro a menos e 1 quadro a menos
      de latência (o PBO entregava o quadro anterior).
- [ ] `todo` — B6b. Repetir o B6 numa GPU AMD e numa Intel (esta máquina só
      tem a RTX 3060): `win32_interop_*` com as camadas de validação e
      `REEMU_TEST_GL_CORE`/`REEMU_TEST_GL_ROM`.

**Fase C — cores Vulkan no Windows**

- [x] `done` — C1. Cores Vulkan in-process no Windows (2026-10-02, RTX 3060).
      flycast, Beetle PSX HW, PPSSPP (forçado) e mupen64plus_next com
      ParaLLEl-RDP rodam, salvam/restauram estado e descarregam; com as
      camadas de validação, **nenhum** `VUID`/`SYNC-HAZARD` (teste
      `vk_core_real_rom`, agora também no Windows). Correções que isso
      exigiu: todas as entradas do core na mesma thread (`on_vk_thread`, o
      libco do mupen usa fibers), `context_destroy` antes de descarregar um
      core Vulkan, instância 1.3, `timelineSemaphore` no hook do
      `vkCreateDevice` e dois shaders WGSL. Ver `docs/historico.md`.
- [x] `done` — C2. Vulkan automático no Windows (2026-10-02): a rota
      in-process escolhe sozinha os cores da lista (`VK_CAPABLE_CORES`:
      flycast, Beetle PSX HW, mupen64plus_next com `parallel`), como no
      Linux. Validado com `vk_core_real_rom` sem `REEMU_HW`, e o mupen sem
      `parallel` segue no processo filho. Falta só conferir no app, sem a
      variável.
- [x] `done` — Medir o vazamento de índices de TLS da rota Vulkan
      in-process (2026-10-11, teste `vk_core_tls_leak`, 8 trocas de jogo
      por core): flycast e mupen64plus_next (ParaLLEl) perdem **1 índice de
      TLS por jogo**; Beetle PSX HW, nenhum; FLS, nenhum nos três. Partindo
      de ~1005 livres, o app abortaria ("out of TLS indexes") depois de
      ~1000 trocas numa sessão com esses dois cores. Risco distante, mas
      real — mais um motivo pro C3.
- [x] `done` — C3a (2026-10-11, opt-in `REEMU_VK_CHILD=1`): core Vulkan
      da lista no **processo filho**, com device próprio. O filho cria o
      device (ou chama o `create_device` do core, caso do Beetle e do
      flycast), submete o trabalho do core e copia a imagem por um passe de
      desenho (`textureLoad`, só exige `SAMPLED` — o flycast não dá
      `TRANSFER_SRC`) pra RGBA8 e dali pra CPU; o quadro vai pelo anel de
      memória, como o readback do GL. Teste `vk_core_in_child`: Beetle PSX HW
      e mupen64plus_next (ParaLLEl) a 60 fps, flycast rodando, imagens
      conferidas, camadas de validação sem VUID, e **0 índice de TLS
      perdido no app** trocando de jogo.
- [x] `done` — C3b (2026-10-11, ainda atrás de `REEMU_VK_CHILD=1`):
      zero-cópia do core Vulkan no filho. O filho escolhe a GPU do app pelo
      UUID, liga as extensões Win32 (também no device que o flycast cria,
      pelo hook `vk_hook`), importa o anel do app (mesmos parâmetros de
      imagem, `allocationSize` e `memoryTypeIndex`), desenha no slot, faz o
      *release* pra `VK_QUEUE_FAMILY_EXTERNAL` e sinaliza o `done`; o app
      faz o *acquire* (`AcquireCmds`) antes de amostrar. Teste
      `win32_interop_real_gl_core` com `REEMU_VK_CHILD=1`: flycast,
      mupen64plus_next (ParaLLEl) e Beetle PSX HW pelo anel, imagens
      conferidas, camadas de validação dos dois lados sem VUID.
- [x] `done` — C3c (2026-10-11): processo filho é o **padrão no Windows**
      pros cores Vulkan da lista (`REEMU_VK_CHILD=0` volta pro in-process; no
      Linux segue opt-in). Validação com sync nos dois processos sem nenhum
      aviso (flycast, mupen64plus_next com ParaLLEl, Beetle PSX HW); PPSSPP
      com `ppsspp_backend=vulkan` também roda (antes caía no filho). Medido
      contra o in-process: mesma taxa de quadros, e a thread de vídeo do app
      só amostra o quadro (~0,8–1,1 ms) em vez de também emular o core.

### Biblioteca

- [x] `done` — Validado no Windows (validado pelo usuário em 2026-10-08): "Atualizar biblioteca"
      tira as 25 cópias do Redump e os `.chd` de `naomi/`, e adiciona os 127
      `.zip` de NAOMI e os 25 de Atomiswave (contagem da varredura nova na
      pasta real, num banco à parte); e os jogos de NAOMI abrem no flycast.
- [x] `done` — Validado no Windows (validado pelo usuário em 2026-10-08): trocar de jogo não mostra mais o último
      quadro do anterior (`SHOW_AFTER_PRESENTS` em `lib.rs`, também na rota
      Vulkan).

### Desempenho do caminho do core

Medir antes de otimizar: `REEMU_PERF=1 scripts/dev.sh` com um jogo pesado
(N64/PS1) e anotar as linhas `perf core`/`perf vídeo` (ver STEP_BY_STEP §4).
Feito em 2026-09-24 (ver `docs/historico.md`): pump acorda no frame
(perdia 12% dos frames), margem de spin adaptativa (33 → 6 ms/s de CPU),
áudio sem alocação por frame, canvas sem a 2ª cópia do frame.

Sem alocação por quadro no caminho software desde 2026-09-25 (pool de buffers
no `emu-session`, ver `docs/historico.md`).

### Funcionalidades

- [ ] `todo` — **Conquistas (RetroAchievements)**, pedido do dono do projeto
      (2026-10-28). Plano completo em
      [`docs/ai-context/16-retroachievements.md`](docs/ai-context/16-retroachievements.md):
      rcheevos (MIT) compilado num crate nosso; `rc_client` no processo do
      core; HTTPS pelo app com o User-Agent do ReEmu.
      - **Antes do código:** registrar o User-Agent com a equipe do RA. Sem
        isso o servidor rebaixa o hardcore para softcore. Tarefa do dono do
        projeto.
      - **Primeiro passo de código:** tratar o
        `RETRO_ENVIRONMENT_SET_MEMORY_MAPS`, que hoje cai em "não tratado".
- [ ] `todo` — **Cheats e rewind**, pedido do dono do projeto (2026-10-28).
      Plano em
      [`docs/ai-context/17-cheats-rewind.md`](docs/ai-context/17-cheats-rewind.md).
      - **Cheats:** código pelo `retro_cheat_set`; banco `.cht` do
        libretro-database (CC-BY-SA 4.0, baixado sob demanda com crédito);
        depois cheats de memória.
      - **Rewind:** buffer circular de estados com delta comprimido, no
        processo do core.
      - **Fase 0 feita (2026-10-28):**
        - quirks e `GET_SAVESTATE_CONTEXT` tratados;
        - estado de core `SINGLE_SESSION` não vai mais para disco;
        - estados medidos em 10 cores, de 5 KB (NES) a 83 MB (Dreamcast).
          O PPSSPP fica para medir dentro do app.
      - **Próximo:** fase 1 (cheats por código).
      - No hardcore do RetroAchievements, os dois ficam proibidos.

- [ ] `todo` — Avaliar integração **GameKave / Alvanista** (pedido do dono
      do projeto, 2026-10-20). A GameKave (Tellurian Corp) é a
      "infraestrutura de identidade gamer" por trás da Alvanista, rede social
      brasileira de jogos. API pública em OpenAPI 3.0
      (`https://api.gamekave.net/api-docs/openapi.json`), **v0.1.0**, 68
      endpoints, OAuth 2.0 (authorization code e client credentials) + JWT.
      - **Serve:** `games/{id}/assets` (capa, hero, logo, screenshots,
        manuais, trilha, vídeos; conquistas de Steam e RetroAchievements),
        `lists` (sincronizar a biblioteca com a coleção do perfil), `gas`
        (o app envia sinais de jogo assinados com HMAC, com limite por minuto,
        e o jogador ganha conquistas e nível no perfil).
      - **Não serve como fonte de metadados por enquanto:** não tem busca por
        nome nem por hash de ROM (só por ID); sem termos de uso, licença dos
        dados nem limites publicados; dados de jogos vêm do IGDB
        (`igdb_id`), então os termos do IGDB também contam. Inconsistência:
        a documentação diz que tudo exige token, mas
        `GET /games/filters/platforms` respondeu sem token.
      - **Antes de codar, perguntar à Tellurian:**
        1. termos de uso e licença dos dados (inclusive o que vem do IGDB);
        2. como registrar o ReEmu como cliente OAuth e como app do GAS, e se
           o client ID pode ir embutido num app de código aberto;
        3. limites de uso;
        4. se vai haver busca por nome e por hash de ROM;
        5. estabilidade da API (a 0.1.0 deve mudar).
      - **Escopo do protótipo (só se os termos permitirem):** opção
        "Conectar conta Alvanista" em Configurações › Perfil, via OAuth
        (token no chaveiro do sistema, como a chave do TheGamesDB),
        desligada por padrão, com consentimento explícito e botão para
        desconectar (LGPD). Começar só pelo que o ReEmu **envia**: sessões e
        tempo de jogo pelo GAS. Depois, hero e logo de jogos já identificados
        para o carrossel e a página do jogo. ScreenScraper e TheGamesDB
        continuam como fontes principais, e o app funciona sem a GameKave.
- [ ] `todo` — Avaliar no futuro: **ports nativos (recomp/decomp)** como
      launcher externo (pedido do dono do projeto, 2026-10-17). Pesquisa e
      licenças conferidas no GitHub:
      - Ferramentas MIT: N64Recomp, RT64, XenonRecomp/XenosRecomp,
        libultraship. Runtime GPL-3.0: N64ModernRuntime. Ports GPL-3.0:
        Zelda64Recomp, UnleashedRecomp, PS2Recomp. CC0: n64decomp/sm64,
        2ship2harkinian. MIT: perfect_dark (fgsfds). Sem licença detectada
        (conferir no repositório): zeldaret/oot, zeldaret/mm, Shipwright,
        sm64ex, pret/*.
      - **Não portar pra Rust nem copiar código pro ReEmu**: centenas de
        milhares de linhas por jogo, código GPL obrigaria o ReEmu (MIT) a
        virar GPL, os sem licença não podem ser copiados, e mesmo o código
        CC0/MIT reproduz a lógica de jogos com direitos autorais (a Nintendo
        já agiu contra mods feitos sobre a decomp do SM64).
      - Proposta: o ReEmu abre o executável do port como um processo
        separado, como faz com os cores (executar um programa GPL à parte
        não contamina o ReEmu). Cadastro manual do executável e associação
        ao jogo da biblioteca (pelo hash da ROM, que os ports já exigem);
        botão Jogar com a opção "Port nativo"; tempo de jogo e volta ao
        frontend quando o processo fecha; opcional: baixar as versões
        oficiais publicadas no GitHub. Compilar o port a partir da ROM fica
        de fora.
      - Em Rust só existem experimentos (psprecomp, GPL-2.0, gera C++; um
        GCRecomp que geraria Rust). **Pesquisa completa, catálogo com
        licenças, como a recompilação funciona, o que dá pra usar em Rust e a
        arquitetura proposta do launcher:**
        [`docs/ai-context/15-ports-nativos-recomp-decomp.md`](docs/ai-context/15-ports-nativos-recomp-decomp.md)
        (2026-10-20).
- [ ] `todo` — Molduras: repensar a importação de packs de bezels por pasta.
      Os botões "Importar pasta de bezels…" e "Remover bezels"
      (Configurações › Decorações › Molduras) estão escondidos
      (`SHOW_BEZEL_PACK_TOOLS` em `SettingsVideo.tsx`) a pedido do dono do
      projeto (2026-10-16); os comandos `import_decoration_pack` e
      `clear_decorations` e os textos do i18n continuam. Decidir o fluxo
      antes de voltar a mostrar.
- [ ] `blocked` — Metadata: IGDB como 3º provedor. Bloqueado por falta de
      fonte pública confiável pros ids de plataforma do IGDB (a lista só
      sai da API autenticada) — não implementar de memória. Alternativa
      avaliada: o banco do LaunchBox (`gamesdb.launchbox-app.com/Metadata.zip`,
      ~108 MB, atualizado diariamente) — avaliado em 2026-09-24: o site não
      publica termos de uso (só política de privacidade, sobre contas), então
      não há autorização pra usar o banco num app distribuído. Só com
      permissão escrita do LaunchBox.
- [x] `done` — Validar o TheGamesDB com uma chave real (2026-10-08, chave do
      usuário, 1 requisição): `/v1/Games/ByGameName` com `fields=overview,
      players,rating,coop` e `include=boxart` devolve o formato do spec.yaml
      oficial (api.thegamesdb.net/spec.yaml) — o parser bate. Cota: 1000
      requisições/mês por chave; com a capa no `include`, ~1 por jogo.
- [ ] `todo` — **Onboarding: passo de contas** (backlog, pedido em
      2026-10-04): um modal a mais no onboarding, do mesmo tamanho dos outros
      três, para entrar em redes sociais ou lojas de jogos (Steam e outras).
      Entra entre "nome e avatar" e "tudo pronto"; pular deve ser possível.
- [ ] `todo` — **Presets de qualidade nos emuladores** (backlog, pedido em
      2026-10-08): Desempenho, Padrão, Qualidade, Ultra e Personalizado, com
      os valores que a comunidade libretro costuma usar já configurados.
      - **Cores 3D**, onde a resolução interna faz diferença: flycast
        (Dreamcast/NAOMI), Beetle PSX HW, mupen64plus_next / parallel_n64
        (ParaLLEl-RDP) e PPSSPP.
      - **Cores 2D** (acrescentado em 2026-10-08): sem resolução interna, mas
        com opções de core que corrigem a imagem conforme o hardware original.
        Num preset "Qualidade" pra 2D:
        - **Correção de cor dos portáteis** — GBA/GBC (mGBA, Gambatte): as
          telas originais eram escuras e os jogos saturavam as cores pra
          compensar; sem a correção, estouram num monitor atual.
        - **Mistura de quadros / LCD ghosting** (mGBA, Gambatte…): jogos que
          piscavam sprites pra simular transparência/sombra contavam com a
          tela lenta; sem isso, pisca-pisca.
        - **Remover o limite de sprites por linha** (NES, Mega Drive, PC
          Engine): tira a cintilação com muitos objetos na linha (poucos
          jogos usavam o efeito de propósito).
        - **Paleta do NES** (Nestopia, Mesen, FCEUmm): trocar a padrão por uma
          mais fiel (as "FBX", por exemplo).
        - **Transparências da TV**: filtro NTSC do Genesis Plus GX (dithering
          do Mega Drive vira transparência, ex. cachoeiras de Sonic) e
          "Hi-Res Blending" do Snes9x (Kirby's Dream Land 3, Jurassic Park).
        - **Cortar overscan** onde o core oferece.
        Os shaders (CRT, NTSC, xBR/ScaleFX, sharp-bilinear, LCD) e o integer
        scaling, que o ReEmu já tem, ficam fora dos presets: são gosto, não
        qualidade/desempenho. Nomes das opções citados de memória — conferir
        no `libretro_core_options.h` de cada core antes de montar a tabela.
      - **Base já existe:** opções por core e por jogo no banco
        (`set_core_option`/`reset_core_options`, `CoreOptionsRepo`), aplicadas
        na hora quando o core está carregado; `FRONTEND_DEFAULTS` em
        `core-loader-desktop/src/coreopts.rs`. Um preset = um conjunto de
        valores gravado de uma vez no escopo do core ou do jogo; "Personalizado"
        aparece sozinho quando uma opção do preset é mudada à mão.
      - **Chaves e valores só da fonte oficial** de cada core
        (`libretro_core_options.h`), nunca de memória: flyinghead/flycast
        `shell/libretro/`, libretro/beetle-psx-libretro (raiz),
        libretro/mupen64plus-libretro-nx `libretro/`, hrydgard/ppsspp
        `libretro/`, libretro/parallel-n64 `libretro/`. Já conferido no PPSSPP
        (2026-10-08): `ppsspp_internal_resolution` (480x272 … 4800x2720),
        `ppsspp_mulitsample_level` (Disabled/x2/x4/x8, com o erro de grafia
        do próprio core), `ppsspp_texture_anisotropic_filtering`,
        `ppsspp_texture_scaling_level`/`_type`, `ppsspp_texture_filtering`,
        `ppsspp_frameskip`, `ppsspp_lower_resolution_for_effects`. Nos outros
        quatro o formato do arquivo é diferente (macros/arrays por idioma) e
        o extrator ainda não leu — fazer de novo antes de montar as tabelas.
      - **Pesquisa da comunidade** (a fazer): documentação de cada core em
        docs.libretro.com, fórum forums.libretro.com e guias da comunidade —
        anotar a fonte de cada valor escolhido.
      - **Cuidados:** não mexer em opção que quebra save state (tipo de
        emulação da CPU, plugin de vídeo do N64); avisar "aplica no próximo
        jogo" nas que o core só lê ao carregar; Ultra pode ser pesado em GPU
        fraca (opcional: sugerir o preset inicial pela GPU que o wgpu
        detecta). Validar cada core jogando um título pesado em cada nível.

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

- [ ] `todo` — **CI no Ubuntu 26**: os workflows ficaram presos em
      `ubuntu-24.04` (2026-10-02), porque o `ubuntu-latest` passa a ser o
      Ubuntu 26 a partir de 19/10/2026 (aviso do GitHub Actions,
      actions/runner-images#14748) e o build Linux instala pacotes de
      sistema por nome (WebKitGTK, GTK, appindicator…). Migrar quando der
      pra validar: trocar pra `ubuntu-26.04`, conferir os nomes dos pacotes
      e rodar o Release por `workflow_dispatch`. **Exceção: o build Linux do
      Release fica no `ubuntu-22.04`** (2026-10-11) — o AppImage/.deb exige
      a glibc do sistema de build ou mais nova, e a doc do Tauri manda
      "build using the oldest base system you intend to support". No 24.04
      os binários exigiam GLIBC 2.39 (fora Ubuntu 22.04, Debian 12, Mint 21);
      no 22.04 exigem GLIBC_2.34 (run 38130076079, passo "glibc mínima").
      Só os checks de CI migram; o Release só sobe de base quando o 22.04
      sair dos runners.

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
