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
