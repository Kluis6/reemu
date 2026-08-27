# 00 — Visão Geral do Projeto (ReEmu)

## O que é

**ReEmu** — frontend emulador para cores libretro (suporte amplo, dezenas
de cores, estilo RetroArch). Stack: Tauri v2 + React 19 + TypeScript +
SQLite + Fluent Design.

## Princípios arquiteturais (não negociáveis)

- **Arquitetura hexagonal**: crate `domain` (`crates/domain`) contém só
  traits e tipos puros — NUNCA importa crates de I/O de plataforma
  (libloading, cpal, gilrs, wgpu, tauri). Toda implementação concreta vive
  em crates adapter (`core-loader-desktop`, `core-loader-mobile`, `db`).
- **SOLID** como princípio orientador. Sem MVC.
- **Monorepo**: Cargo workspace (`crates/`, `apps/*/src-tauri`) + pnpm
  workspace (`packages/`).
- **Sem multi-perfil no MVP**: nenhuma tabela tem `user_profile_id`. Config
  é única por instalação.

## Onde está a fonte de verdade

- Traits/portas: `crates/domain/src/*.rs` (um módulo por porta)
- Schema do banco: `crates/db/migrations/0001_init.sql`
- Decisões e histórico completo: `resumo-arquitetura-emulador-libretro.md`
  (raiz do repo)

**Ao gerar código, sempre leia esses arquivos antes de assumir um formato
de dado — não invente campo/tabela que não esteja lá. Se precisar de algo
que não existe no schema, pare e sinalize em vez de inventar.**

## Plataformas e suas divergências

| Aspecto | Desktop | Android |
|---|---|---|
| Carregamento de core | `libloading` (dlopen), sem restrição | `dlopen` também funciona — `targetSdkVersion` baixo (ex: 28), decisão tomada porque o app **não** é distribuído na Google Play |
| Surface de vídeo | Child window nativa sobre a webview Tauri | `SurfaceView`/`TextureView` via JNI |
| Input | `gilrs` + SDL_GameControllerDB | API nativa Android; cobertura MVP limitada a Xbox/PlayStation via Bluetooth |
| Áudio | `cpal` | Oboe (via JNI) |

## Convenções de código

- Rust: `Result<T, E>` com `thiserror` para erros de domínio, nunca `unwrap()`
  fora de testes.
- Toda porta em `domain` é uma trait `Send + Sync` (ou `Send` quando só
  usada em um contexto de thread única, como `LoadedCoreHandle`).
- SQL: tabelas sem `user_profile_id`; scopes de resolução em cascata usam
  sempre o padrão `scope IN ('default', 'system', 'rom')` já estabelecido
  em `shader_chain_assignments` e `decoration_assignments`.

## O que NÃO fazer

- Não adicionar suporte a ReShade FX (é backlog, não MVP)
- Não implementar negociação de HW render Vulkan por-core ainda (fase 2,
  GL vem primeiro — ver `12-vulkan-hw-render-fase2.md`)
- Não adicionar campo de perfil de usuário em nenhuma tabela
- Não fazer o `domain` depender de nenhum crate de I/O
