# 20 — Mapa das travas de `session.rs`

**Estado: só leitura (2026-10-31).** Item 2 da fase 8, primeira metade. Nenhum código foi
alterado e nada aqui foi medido em tempo de espera: as conclusões vêm da leitura de
`crates/emu-session/src/session.rs` e das capturas de desempenho.

## Linhas de execução

| Thread | Onde nasce | Papel |
|---|---|---|
| Vídeo (`video pump`) | `apps/desktop/src-tauri/src/lib.rs` | Espera, processa e apresenta o quadro |
| `emu-core-loop` | `session.rs`, `core_loop` | Comandos, eventos do filho, áudio |
| `emu-core-host-reader` | `session.rs`, `spawn` do filho | Lê o canal do processo filho |
| `emu-gamepad` | `session.rs`, `gamepad_loop` | Controle e pulsos de navegação |
| `reemu-srm-writer` | `session.rs`, `core_loop` | Grava a `.srm` fora do caminho do jogo |
| Comandos Tauri | `apps/desktop/src-tauri/src/commands` | Chamadas vindas da interface |

## Rotas de core

- **Processo filho:** software, OpenGL e, no Windows, os cores da lista `VK_CAPABLE_CORES`.
- **No próprio processo:** core Vulkan fora dessa rota. Usa `vk_local`, `vk_jobs` e
  `vk_render_gate`. No Linux é o padrão; no Windows, só para o que não vai ao filho ou com
  `REEMU_VK_CHILD=0`.

## Travas

**Frequência:** "quadro" = a cada quadro nessa rota; "contínuo" = laço periódico; "raro" = load,
pausa, comando ou troca de dispositivo.

| Trava | Quem usa | Frequência |
|---|---|---|
| `latest_frame` + `frame_ready` | `handle_event`, `wait_for_frame`, `take_latest_frame` | Quadro |
| `spare_bufs` | `recycle`, `take_spare` | Quadro (software) |
| `orphan_planes` | `recycle`, `reconstruct_frame`, limpeza no load | Quadro (GPU, caso raro dentro dele) |
| `interop_dropped` | `take_dropped_interop_slots`, `answer_interop_request` | Quadro (Windows) |
| `child_channel` | `release_interop_slot`, `report_display_period`, `answer_interop_request`, `core_loop` | Quadro (Windows) |
| `audio` | `handle_event`, `core_loop`, `drain_audio` | Contínuo, só sem `AudioSink` |
| `vk_local` | `step_vk_local`, `teardown_vk_local`, `core_loop` | Quadro (no próprio processo) |
| `vk_jobs` | `on_vk_thread`, `step_vk_local` | Quadro (no próprio processo) |
| `vk_local_audio` | `step_vk_local`, `teardown_vk_local`, `core_loop` | Quadro (no próprio processo) |
| `vk_render_gate` | `lock_vk_queue` (thread de vídeo), `on_vk_thread` | Quadro |
| `nav` | `gamepad_loop`, `take_nav_pulses` | Contínuo |
| `captured_inputs` | `gamepad_loop`, `take_captured_inputs` | Contínuo |
| `gamepads`, `gamepad_connected`, `gamepad_disconnected` | `gamepad_loop` e leituras do app | Contínuo |
| `state` | `set_state`, `state` | Raro |
| `loaded_core`, `loading_core` | `core_loop`, `note_core_set_option`, `loaded_core` | Raro |
| `child_pid` | `core_loop`, `answer_interop_request`, `debug_child_pid` | Raro |
| `core_set_options` | `note_core_set_option`, `take_core_set_options` | Raro |
| `dmabuf_modifiers`, `vk_interop`, `interop_provider` | Configuração e load | Raro |
| `vulkan_shared_device`, `vulkan_negotiator` | Configuração e `route_local_device` | Raro |

Fora de `Shared`: `slot` (`on_vk_thread`) e `plane`, `sync_fd`, `returns` (quadro de GPU do filho).

## O que a leitura encontrou

Nenhum item abaixo foi reproduzido como falha. São riscos, em ordem de atenção.

1. **`release_interop_slot` envia ao filho com `child_channel` travada** (a cada quadro, Windows).
   Se a escrita no canal demorar, a thread de vídeo espera, e quem precisar da trava também.
   `report_display_period` já clona o canal e envia fora da trava, e o transporte do Windows tem
   trava de escrita própria. `answer_interop_request` faz o mesmo envio travado, mas só no load.
2. **`on_vk_thread` pode derrubar a thread com `expect`.** Se a tarefa saiu da fila e terminou
   sem responder, por exemplo com pânico dentro dela, `rx.recv().expect(...)` também entra em pânico.
3. **`vk_local` fica travada durante `run_frame`** (o quadro inteiro do core). Na rota no próprio
   processo, gravar a `.srm`, pausar e ler opções esperam o fim do quadro. A espera é limitada, e
   quem espera é o `core_loop`.
4. **Toda trava recupera envenenamento** (`unwrap_or_else(|p| p.into_inner())`). O app não cai
   depois de um pânico, mas segue com o dado como estava naquele instante.
5. **O `core_loop` da rota no próprio processo dorme 8 ms a cada volta**, o que limita a frequência
   de drenagem do áudio e dos comandos. Não é trava, mas é vizinho do caminho do quadro.

## O que este mapa não explica

**O pico de ~160 ms no Vulkan não parece vir destas travas.** Nas capturas, o pior intervalo é quase
igual ao pior `retro_run` (159,72 e 159,64 ms), medidos no processo do core. Essa lista de travas é
do processo do app. A causa provável está dentro da chamada do core ou no que ela aguarda, o que
não foi investigado.

## Decisão

Sem separar travas agora: as capturas mantêm a velocidade certa e não há medida de disputa. Se
voltar ao assunto, a ordem é medir a espera em `child_channel` e depois decidir o item 1, que seria
a menor mudança. A 2ª metade do item, medir e separar, e o item 3 ficam parados até haver um motivo.
