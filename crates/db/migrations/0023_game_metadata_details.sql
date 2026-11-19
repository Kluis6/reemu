ALTER TABLE game_metadata ADD COLUMN gamekave_json TEXT;
CREATE TABLE game_metadata_edits (
    rom_id TEXT PRIMARY KEY REFERENCES roms(id) ON DELETE CASCADE,
    fields_json TEXT NOT NULL
);
