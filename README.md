# ReEmu — Monorepo

Frontend emulador para cores libretro (Tauri v2 + React 19 + SQLite +
Fluent Design). Estrutura inicial gerada a partir do design de arquitetura
(ver `resumo-arquitetura-reemu.md`).

## Estrutura

```
apps/
  desktop/    Projeto Tauri v2 (Windows + Linux) — ainda não inicializado
  mobile/     Projeto Tauri v2 mobile (Android) — ainda não inicializado

crates/
  domain/     Regras de negócio puras — traits/portas, zero I/O de plataforma
  db/         Schema SQLite (migrations) + repositórios (a implementar)
  # core-loader-desktop/  (a criar) — libloading, cpal, gilrs
  # core-loader-mobile/   (a criar) — cores empacotados, JNI

packages/
  ui/             Componentes Fluent, tokens de design
  shared/         Hooks, cliente IPC, state management (Zustand)
  app-desktop/    Entrypoint React desktop
  app-mobile/     Entrypoint React mobile (layout touch)
```

## Regra de dependência (hexagonal)

`domain` nunca importa crates de I/O de plataforma. Tudo que toca hardware,
sistema de arquivos ou rede vive nos adapters (`core-loader-*`, `db`), que
implementam as traits definidas em `domain`.

## Status atual

- [x] Estrutura de workspace (Cargo + pnpm)
- [x] Traits do `domain` para todas as portas definidas no design
- [x] Migration SQL inicial com todos os schemas decididos
- [ ] `apps/desktop` — inicializar projeto Tauri v2 real (`cargo tauri init`)
- [ ] `apps/mobile` — inicializar target Android do Tauri v2
- [ ] `crates/core-loader-desktop` — primeira implementação real (libloading + GL)
- [ ] Implementação dos repositórios em `crates/db` (sqlx)
- [ ] `packages/ui`, `packages/shared` — setup React 19 + Fluent + Zustand

## Próximo passo recomendado

Implementar `core-loader-desktop` para o caminho GL de negociação de
hardware render (decisão: GL antes de Vulkan por-core), validando o
`FrameSource` com um core software-only simples primeiro (ex: um core NES),
antes de partir pra cores hardware-accelerated.
