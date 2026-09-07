-- Core options por jogo: override que vence o valor por core (`core_options_values`)
-- só quando aquela ROM está carregada. Tabela separada pra não recriar
-- `core_options_values` (que tem `UNIQUE(core_id, option_key)` inline). Sem FK:
-- o override sobrevive à reinstalação do core, igual à decisão de
-- `core_options_values`.
CREATE TABLE core_option_overrides (
    id TEXT PRIMARY KEY,
    rom_id TEXT NOT NULL,
    core_id TEXT NOT NULL,
    option_key TEXT NOT NULL,
    value TEXT NOT NULL,
    UNIQUE(rom_id, core_id, option_key)
);

CREATE INDEX idx_core_option_overrides_rom
    ON core_option_overrides (rom_id, core_id);
