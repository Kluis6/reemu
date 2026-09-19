# 09 — Scraping de Metadata da Biblioteca

## Objetivo desta etapa

Implementar `RomHashService` e um ou mais `MetadataProvider`, com fila de
jobs em background pra catalogar milhares de ROMs sem travar a UI.

## Decisões relevantes

- **Matching primário por hash (CRC32/MD5)** — nome de arquivo sozinho é
  pouco confiável (tags de região, revisão, scene groups).
- **Match automático com hash exato OU nome de arquivo exato** —
  `ScrapeCandidate::auto_matches()` (`exact_hash_match ||
  exact_filename_match`). **Abordagem B revisada, 2026-09-13** (decisão
  explícita do usuário — a Abordagem B original só aceitava hash): o 2º
  critério é `exact_filename_match`, que só é `true` quando o `file_stem`
  local bate, byte a byte (sem extensão, case-insensitive), com o nome de
  arquivo que o PRÓPRIO provedor devolveu pro candidato que ele encontrou
  (ex.: `rom.romfilename` do ScreenScraper) — não é fuzzy, não é
  "parecido", é igualdade exata contra o nome canônico que o provedor já
  reconhece (cobre bem dumps No-Intro/Redump com nome intacto). Continua
  proibido: threshold de confiança da API, busca por texto livre, ou
  qualquer comparação aproximada de nome como critério de automação —
  esses continuam indo sempre pra `pending_review`.
- **Provedor configurável pelo usuário** — `MetadataProvider` é uma trait
  com múltiplas implementações possíveis (IGDB, ScreenScraper,
  TheGamesDB); a UI deixa escolher/priorizar quais ficam ativos.
  Múltiplos provedores ativos = cascata (primeiro que retornar hash exato
  vence, tenta o próximo se o atual não tiver a ROM catalogada).
- **Rate limiting por provedor**: cada API externa tem limite próprio —
  implemente isso no adapter de cada `MetadataProvider`, não numa camada
  genérica compartilhada (limites são diferentes entre provedores).

## Estado atual (2026-09-19 — `done` no MVP)

`crates/library-scan`:
- `hash.rs` — `FileRomHasher` (CRC32 + MD5, pula o header iNES).
- `systems.rs` — `system_id` por extensão e por nome de pasta (inclusive os
  nomes no estilo No-Intro/RetroArch); `disc_sniff.rs` identifica o sistema
  de imagens de disco pelo conteúdo; `archive.rs` lê `.zip`/`.7z`.
- `scan.rs` — `scan_into` varre recursivo, com dedup por `file_path`.

`apps/desktop/src-tauri/src/scraping.rs`:
- Um provider: **ScreenScraper** (`api.screenscraper.fr`), busca por CRC.
  Credenciais do usuário são opcionais (aumentam o limite de requisições).
- `scrape_pending` roda em background com progresso consultável
  (`metadata_scan_progress`, `start_metadata_scan`, `cancel_metadata_scan`);
  a UI não trava.
- A política continua: só hash exato vira `auto_matched`; o resto vai pra
  revisão manual (`set_rom_metadata`).
- Capas: vêm do `thumbnails.libretro.com`, com cache local servido pelo
  protocolo `cover://` (`covers.rs`) — funcionam sem internet depois da 1ª
  vez.
- Telas: `SettingsMetadata` (credenciais) e `SettingsLibrary` (pastas e
  scan).

**Falta** (backlog do `TASKS.md`): IGDB/TheGamesDB como providers extras em
cascata, com limite de requisições por provider — hoje não há como trocar o
provider ativo, então o 3º item do critério de pronto não se aplica ainda.

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
- Nenhum match sem hash exato OU nome de arquivo exato (ver
  `ScrapeCandidate::auto_matches`) vira `auto_matched`, mesmo com score alto
  declarado pela API — nunca por score de confiança ou nome aproximado
- Trocar o provedor ativo nas configurações reflete na próxima leva de
  scraping sem precisar reiniciar o app
