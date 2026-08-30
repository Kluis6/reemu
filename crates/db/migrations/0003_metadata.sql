-- Scraping de metadata (etapa 09).

-- Candidato proposto por um provedor, guardado junto do match pra preview na
-- tela de revisão (o schema base de scrape_matches não tinha onde pôr a
-- metadata proposta). JSON de domain::metadata::ScrapeCandidate.
ALTER TABLE scrape_matches ADD COLUMN candidate_json TEXT;

-- Um match por ROM (o scrape troca o anterior).
CREATE UNIQUE INDEX ux_scrape_matches_rom ON scrape_matches (rom_id);

-- game_metadata: um registro por ROM.
CREATE UNIQUE INDEX ux_game_metadata_rom ON game_metadata (rom_id);

-- Config do scraper (singleton, igual audio_config).
CREATE TABLE metadata_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    provider TEXT NOT NULL DEFAULT 'screenscraper',
    -- credenciais opcionais do usuário (screenscraper.fr) — melhoram o limite
    -- de requisições; anônimo funciona pra volume baixo.
    screenscraper_user TEXT,
    screenscraper_password TEXT
);
INSERT INTO metadata_config (id) VALUES (1);
