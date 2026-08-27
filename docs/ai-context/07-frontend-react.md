# 07 — Frontend React (Fluent UI, Overlay, Foco, Toast)

## Objetivo desta etapa

Montar a UI React 19 + Fluent Design: overlay de menu, indicadores de
foco, sistema de toast, e telas de biblioteca/configuração.

## Decisões relevantes

- **Zustand só pra estado de UI/cliente decidido fora do React** — o
  exemplo canônico é o foco (`InputFocus`), espelhado via evento Tauri
  (`listen("focus-changed", ...)`). Não duplique dados que já vivem no
  SQLite em Zustand.
- **Dados do backend (biblioteca, configs, cores instalados)** passam por
  IPC (`invoke`) + cache de query (TanStack Query) — não Zustand.
- **`ToastLayer` é independente da state machine de foco**: um componente
  de fila renderizado sempre por cima de tudo (`GameFocused` ou
  `MenuFocused`), nunca captura input, nunca dispara pause. Não modele
  toast como um terceiro valor de `InputFocus`.
- Erro bloqueante (ex: falha irrecuperável de core) **não é toast** — é
  uma tela dedicada, com transição forçada pra `MenuFocused`.
- Telas de configuração que dependem de schema dinâmico de core
  (`CoreOptionsStore`) devem ser **geradas automaticamente** a partir de
  `CoreOptionDefinition` — não crie tela customizada por core.

## Stores Zustand sugeridas (pequenas, específicas — não uma store global)

```typescript
useFocusStore     // InputFocus espelhado do Rust
useToastStore      // fila de ToastItem
useBindingCaptureStore  // estado transitório da captura de binding (05)
```

## Estrutura sugerida

```
packages/app-desktop/src/
  stores/
    useFocusStore.ts
    useToastStore.ts
  components/
    MenuOverlay.tsx
    ToastLayer.tsx
    CoreOptionsPanel.tsx    -- gerado a partir do schema dinâmico
  screens/
    Library.tsx
    Settings.tsx
```

## Depende de

`03-tauri-desktop-shell.md` (eventos de foco já sendo emitidos) e
`06-audio.md` (se a tela de configuração de áudio for feita nesta etapa).

## Critério de pronto

- Menu overlay reage corretamente ao evento de foco emitido pelo Rust
- Múltiplos toasts em sequência aparecem em fila, não sobrepostos
- Tela de opções de um core (ex: PS2) é gerada dinamicamente sem código
  específico pra esse core
