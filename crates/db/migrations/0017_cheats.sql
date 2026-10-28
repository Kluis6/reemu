-- Cheats por código de cada jogo (docs/ai-context/17-cheats-rewind.md,
-- fase 1). `position` é a ordem da lista e o `index` do `retro_cheat_set`.
CREATE TABLE cheats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rom_id TEXT NOT NULL REFERENCES roms(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    code TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL
);

CREATE INDEX idx_cheats_rom ON cheats (rom_id, position);
