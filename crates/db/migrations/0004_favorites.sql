-- Aba "Favoritos" da biblioteca (modo Xbox).
ALTER TABLE roms ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_roms_favorite ON roms (is_favorite);
