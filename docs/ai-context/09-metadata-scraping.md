# 09 — Scraping de Metadata da Biblioteca

## Objetivo desta etapa

Implementar `RomHashService` e um ou mais `MetadataProvider`, com fila de
jobs em background pra catalogar milhares de ROMs sem travar a UI.

## Decisões relevantes

- **Matching por hash (CRC32/MD5), nunca por nome de arquivo** — nome é
  pouco confiável (tags de região, revisão, scene groups).
- **Match automático só com hash exato** (`ScrapeCandidate.exact_hash_match
  = true`) — decisão explícita (Abordagem B): qualquer resultado vindo de
  busca heurística/por nome do provedor, mesmo com score de confiança
  alto declarado pela API externa, vai sempre pra `status =
  'pending_review'`, nunca `'auto_matched'`. Não implemente threshold de
  confiança como critério de automação.
- **Provedor configurável pelo usuário** — `MetadataProvider` é uma trait
  com múltiplas implementações possíveis (IGDB, ScreenScraper,
  TheGamesDB); a UI deixa escolher/priorizar quais ficam ativos.
  Múltiplos provedores ativos = cascata (primeiro que retornar hash exato
  vence, tenta o próximo se o atual não tiver a ROM catalogada).
- **Rate limiting por provedor**: cada API externa tem limite próprio —
  implemente isso no adapter de cada `MetadataProvider`, não numa camada
  genérica compartilhada (limites são diferentes entre provedores).

## Estado atual (2026-08-27 — `in-progress`)

`crates/library-scan`:
- `FileRomHasher` impl `domain::metadata::RomHashService` — CRC32
  (`crc32fast`) + MD5 (`md-5`), com skip do header iNES (`.nes`). Testado
  contra valores conhecidos.
- `system_for_extension` — palpite de `system_id` pela extensão.
- `scan_into(repo, dir, now)` — varre recursivo, dedup por `file_path`,
  popula `RomRepository`; `ScanReport`. 2 testes de integração.
- App: comandos `list_roms` / `scan_library(path)`; tela Library escaneia
  e lista de verdade.

**Falta**: `MetadataProvider` (IGDB/ScreenScraper/TheGamesDB), fila de jobs
em background, tabelas `scrape_matches`/`game_metadata`. A política já está
travada (auto só com hash exato) — falta plugar os provedores.

## Fila de jobs

```
ScrapeJobQueue (background, não bloqueia a UI)
  - um job por ROM não catalogada
  - progresso consultável via comando Tauri (polling ou evento de progresso)
  - jobs continuam mesmo se o usuário navegar pra outra tela
```

## Estrutura sugerida

```
crates/core-loader-desktop/src/scraping/     -- ou um crate próprio, se preferir
  rom_hash.rs           -- implementa RomHashService (CRC32/MD5)
  providers/
    igdb.rs
    screenscraper.rs
  job_queue.rs            -- fila com rate limiting por provedor
```

## Depende de

`01-domain-db.md` (repositório de `roms`/`scrape_matches`/`game_metadata`).

## Critério de pronto

- Biblioteca de milhares de ROMs é escaneada sem travar a UI
- Nenhum match sem hash exato vira `auto_matched`, mesmo com score alto
- Trocar o provedor ativo nas configurações reflete na próxima leva de
  scraping sem precisar reiniciar o app
