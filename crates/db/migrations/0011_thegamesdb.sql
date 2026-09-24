-- Chave de API do TheGamesDB (provedor de reserva do scraping, quando o
-- ScreenScraper não acha o jogo). Como a senha do ScreenScraper, normalmente
-- fica no chaveiro do SO e esta coluna só é usada de reserva quando não há
-- chaveiro (ver `apps/desktop/src-tauri/src/credentials.rs`).
ALTER TABLE metadata_config ADD COLUMN thegamesdb_api_key TEXT;

-- O ScreenScraper passou a cobrir todos os sistemas do scan (antes só 15) e
-- ganhou o TheGamesDB de reserva: reenfileira o que tinha dado "não
-- encontrado" (`external_id` vazio). Sugestões REJEITADAS pelo usuário também
-- ficam `no_match`, mas guardam o id do candidato — essas não voltam.
DELETE FROM scrape_matches WHERE status = 'no_match' AND external_id = '';
