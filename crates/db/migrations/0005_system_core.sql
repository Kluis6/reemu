-- Core preferido por plataforma (aba "Gerenciar biblioteca"). Usado como
-- default do seletor de core no RomDetail; o override por ROM continua vencendo.
CREATE TABLE system_core_prefs (
  system_id TEXT PRIMARY KEY,
  core_id   TEXT NOT NULL
);
