-- "Continuar jogando" (Jump back in) — timestamp do último load do jogo.
ALTER TABLE roms ADD COLUMN last_played_at INTEGER;
CREATE INDEX idx_roms_last_played ON roms (last_played_at);
