# 05 — Input, Hotkeys e UI de Binding

## Objetivo desta etapa

Implementar `InputManager`, `ControllerMappingResolver` e `HotkeyResolver`
(desktop), mais o componente de UI de captura de binding compartilhado
entre hotkey de sistema e mapeamento de controle.

## Decisões relevantes

- Desktop: `gilrs` pra enumeração/eventos de gamepad, tradução usando o
  banco **SDL_GameControllerDB** — implemente uma camada de tradução
  SDL-layout → `RetroPadButton` (não assuma que os índices já vêm
  corretos).
- **`controller_mappings` funciona como cache com override do usuário**:
  se o `guid` do dispositivo já tem entrada salva localmente (bundled ou
  editada), usa essa; senão cai no SDL_GameControllerDB; senão cai no
  fluxo manual de binding.
- **Hotkeys de sistema têm prioridade sobre input de jogo** — a resolução
  de `SystemAction` (via `HotkeyResolver`) deve ser checada ANTES de
  rotear qualquer evento pra resolução de RetroPad.
- **UI de captura de binding é um componente único**, e **os dois `target`s
  aceitam combinação** (janela hold+press, ~300ms — primeiro evento que
  continua pressionado quando o segundo chega vira combinação):
  - `target = 'system_hotkey'` → grava em `HotkeyBinding.trigger`
  - `target = 'controller_mapping'` → grava em
    `ControllerLayoutEntry.trigger` (decisão revisada: originalmente só
    hotkey suportava combinação; mapeamento de controle usava tecla
    única — isso mudou, agora é o mesmo comportamento nos dois casos)
  - Combinação em mapeamento de controle é incomum pra ação de jogo — trate
    como opção avançada na UI, não como fluxo sugerido por padrão
- Teclado e gamepad são normalizados num formato único antes de salvar
  (`domain::input::RawInputEvent`) — não trate como tipos separados na UI.

## Estado atual (2026-08-28 — `in-progress`)

- `core-loader-desktop`: `RetroPadState` (global, atômico, por porta) → o
  callback `retro_input_state_t` lê dele (RetroPad digital). `retropad()`.
- `crates/input-desktop`:
  - `sdl_db` — parser SDL_GameControllerDB (`parse_mapping`/`parse_db`),
    swap Nintendo↔Xbox, `GamepadSource::{Button,Hat,Axis}`.
  - `hotkeys::ComboHotkeyResolver` — `HotkeyResolver` com combinação
    (recebe o conjunto segurado; combo vence tecla única).
  - `keymap` — `web_code_to_retropad(code)` (o que a webview manda) +
    `KeyboardMap`.
  - `gamepad::GamepadPoller` — poll de gamepad físico via `gilrs`. `Button`
    (já normalizado pelo SDL_GameControllerDB embutido) → RetroPad na
    convenção libretro; 1ª controle conectada = porta 0. Botão `Mode` =
    candidato a toggle de menu. Roda numa thread própria de `emu-session`
    (`SessionConfig.enable_gamepad`), reflete no `RetroPadState` global.
  - `capture` — flag global de modo de captura. Ligada = `GamepadPoller` e
    `input_key` param de rotear pro jogo e o evento bruto vai pro frontend.
- App: comando `input_key(code, pressed)` + hook `useKeyboardInput`; Escape/F1
  alterna o menu, o resto vai pro RetroPad só em `GameFocused`.
  `FocusController` limpa o pad ao entrar no menu.
- App (captura de binding): `start_binding_capture` / `cancel_binding_capture`
  / `save_binding` (`target` = `system_hotkey` | `controller_mapping`, ambos
  com combinação hold+press) / `list_system_hotkeys` / `clear_system_hotkey`
  / `list_controller_mappings`. Eventos brutos vão pro frontend por
  `emit("raw-input-captured", RawInputEvent)`. Teclado: FNV-1a do
  `KeyboardEvent.code` (a web não expõe scancode real). Gamepad: os eventos
  capturados na thread do poller sobem pelo loop de `RunEvent`.
- App (persistência): `db::SystemHotkeysRepo` (uma linha por `SystemAction`,
  `trigger` em JSON) e `db::ControllerMappingsRepo` (`layout` em JSON, upsert
  por `guid`). Portas: `domain::hotkeys::SystemHotkeyRepository` e
  `domain::input::ControllerMappingRepository`.
- Frontend: `useBindingCaptureStore` (Zustand, transitório — janela de ~300ms
  que agrupa a combinação), `<BindingCapture>` (diálogo único p/ os dois
  `target`), seção "Atalhos de sistema" em `Settings` (redefinir/limpar).
- **`HotkeyResolver` alimentado pelo DB em runtime**: `input_desktop::held`
  (conjunto segurado global, teclado + gamepad); `AppState.hotkeys`
  (`Mutex<ComboHotkeyResolver>`) semeado do `system_hotkeys` no startup
  (default `ToggleMenuOverlay` = `F1`; `Esc` continua hardcoded como rede de
  segurança independente do DB). `commands::poll_hotkeys` roda a cada frame no
  loop de eventos ANTES do roteamento pro jogo (prioridade correta), dispara
  1×/aperto: toggle de menu direto, `QuickSave`/`QuickLoad` viram evento
  `hotkey-action`. `save_binding`/`clear_system_hotkey` recompõem o resolver;
  `FocusController` limpa o `held` na transição de foco.

- **Mapeamento de controle do DB em runtime**: `input_desktop::mappings`
  (override global, `set`/`resolve`, lido pela thread de gamepad — que não tem
  DB). O `GamepadPoller` recompõe o RetroPad por porta a cada evento a partir
  dos índices físicos segurados (`down` por gamepad, diff contra `applied`):
  usa o override do `guid` se existir, senão o mapa fixo do `gilrs`. Isso trata
  combinação e release de forma uniforme. `list_gamepads` /
  `clear_controller_mapping`; `save_binding` e o startup republicam o override.
  UI: `<ControllerMappings>` (seção "Controles" em Settings) junta gamepad
  conectado + mapa salvo, grade de 16 botões RetroPad com rebind.

- **Stick esquerdo → d-pad**: `GamepadPoller` trata `EventType::AxisChanged`
  (`LeftStickX/Y`, limiar 0.5, `gilrs` já aplicou deadzone), alimentando a
  mesma recomposição de RetroPad dos botões.
- **QuickSave/QuickLoad**: `AppState.current_rom` (setado por `load_game`, que
  ganhou `rom_id`), slot fixo `QUICK_SLOT = 0`. `poll_hotkeys` dispara uma task
  async que grava/restaura via `save_state.rs` e devolve o resultado por
  `hotkey-action {action, ok, message}` (toast no frontend).
- **`device_port_assignment`**: `DevicePortRepository` + `db::DevicePortsRepo`
  (cria uma linha vazia em `controller_mappings` pro FK antes de atribuir).
  `input_desktop::mappings::{set_ports, port_for}` — o poller consulta a
  atribuição fixa antes da ordem de conexão. Comandos
  `set_device_port`/`clear_device_port`/`list_device_ports`; `<Select>` de
  porta por controle na seção "Controles".

**Falta / follow-up**: validar em hardware; dois controles idênticos colidem na
porta (mesmo GUID SDL — precisa do `GamepadId` do `gilrs`); saída de eixo
analógico pro RetroPad (hoje só stick→d-pad digital).

## Fluxo da captura de binding

1. Frontend chama comando Tauri `start_binding_capture(target, target_key)`
2. `InputManager` muda de modo: próximo evento bruto vai por
   `emit("raw-input-captured", ...)` em vez de ser resolvido normalmente
3. Frontend mostra estado "aguardando input..." (Zustand, estado
   transitório, não persiste)
4. Ao capturar, grava na tabela correspondente
   (`system_hotkeys`/`controller_mappings`) via comando Tauri, sai do
   modo de captura

## Depende de

`03-tauri-desktop-shell.md` (precisa do loop de eventos e do padrão de
comandos Tauri já estabelecido).

## Critério de pronto

- Um controle Xbox/PlayStation comum é reconhecido automaticamente sem
  passar pelo fluxo manual
- Hotkey de toggle de menu funciona mesmo com o core capturando input
  normalmente (prioridade correta)
- Componente de captura de binding funciona nos dois `target`, ambos
  suportando combinação hold+press
