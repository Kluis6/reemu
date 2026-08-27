# TASKS — Progresso do ReEmu

Checklist de execução, alinhado 1:1 com `docs/ai-context/`. Atualize o
status ao final de cada sessão de trabalho (manual ou com IA) — isso é o
que permite que uma sessão nova retome sem reler todo o código pra
descobrir onde parou.

**Status**: `todo` · `in-progress` · `blocked` · `done`

Ao pedir pra uma IA continuar o projeto, aponte pra este arquivo primeiro
("veja o TASKS.md e continue da próxima etapa `todo`") — evita que ela
refaça trabalho já feito ou pule pré-requisito.

---

## Fundação (feito neste scaffold)

- [x] `done` — Estrutura de workspace (Cargo + pnpm)
- [x] `done` — Traits do `domain` para todas as portas definidas no design
- [x] `done` — Migration SQL inicial com todos os schemas decididos
- [x] `done` — `.gitignore` e `TASKS.md`

## Setup local (manual, fora deste ambiente — ver STEP_BY_STEP.md)

- [ ] `todo` — `git init` + primeiro commit
- [ ] `todo` — Toolchain instalada (Rust, pnpm, Tauri CLI)
- [ ] `todo` — `cargo check --workspace` passa sem erro
- [ ] `todo` — `apps/desktop`: `cargo tauri init` executado
- [ ] `todo` — `packages/app-desktop`: Vite + React + Fluent + Zustand instalados
- [ ] `todo` — `cargo tauri dev` abre janela vazia sem erro

## Etapas de implementação (docs/ai-context/01 a 12)

| # | Etapa | Status | Depende de |
|---|---|---|---|
| 01 | Domain + DB — repositórios sqlx | `todo` | Setup local |
| 02 | Core Loader Desktop — caminho GL | `todo` | 01 |
| 03 | Tauri Desktop Shell — surface nativa | `todo` | 02 |
| 04 | Shader Chain + Decoração | `todo` | 03 |
| 05 | Input, Hotkeys, UI de Binding | `todo` | 03 |
| 06 | Áudio — Dynamic Rate Control | `todo` | 03 |
| 07 | Frontend React — Fluent/Zustand/Toast | `todo` | 03 |
| 08 | Save States e Save RAM | `todo` | 02 |
| 09 | Scraping de Metadata | `todo` | 01 |
| 10 | Catálogo e Download de Cores | `todo` | 01 |
| 11 | Port Android | `todo` | 03–10 completas no desktop |
| 12 | Vulkan HW Render Fase 2 | `blocked` | Gatilho de maturidade — ver doc 12, não iniciar antes das condições lá |

## Como atualizar

Ao concluir uma etapa:
1. Marque a linha correspondente como `done` nesta tabela
2. Se algo ficou pra trás/foi simplificado, anote em uma linha de nota
   logo abaixo da tabela (ex: "Etapa 04: Mega Bezel funcional, mas sem
   suporte a preset com sub-diretórios aninhados — ver issue X")
3. Se uma etapa não pode prosseguir por dependência externa (ex: aguarda
   decisão sua), marque `blocked` com o motivo

## Notas de progresso

_(vazio — preencher conforme o projeto avança)_
