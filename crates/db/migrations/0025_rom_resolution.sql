-- Resolução de cada ROM pelo catálogo do Alvanista (`/api/partner/v1/games/resolve`
-- e escolha manual em `/api/partner/v1/games/claims`). `game_uuid` liga a ROM
-- ao jogo do catálogo e ao cache de mídia por jogo em `<dados>/media/<game_uuid>/`.
CREATE TABLE rom_resolution (
    rom_id TEXT PRIMARY KEY REFERENCES roms(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('found', 'pending', 'unknown', 'manual')),
    game_uuid TEXT,
    matched_by TEXT,
    reason TEXT,
    retry_after INTEGER,
    media_pending INTEGER NOT NULL DEFAULT 0,
    answer_json TEXT,
    resolved_at INTEGER NOT NULL
);

CREATE INDEX idx_rom_resolution_game_uuid ON rom_resolution(game_uuid);

ALTER TABLE game_metadata ADD COLUMN game_uuid TEXT;

