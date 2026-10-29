-- "Entrar com Alvanista" (apps/desktop/src-tauri/src/alvanista/).
-- Os tokens ficam no chaveiro do SO; `secret_fallback` só recebe o par quando
-- o chaveiro falha (mesmo papel das colunas de segredo do scraper).
CREATE TABLE secret_fallback (
    account TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

-- Pares chave/valor da integração: `device_id` (gerado uma vez) e `mirror`
-- (último perfil lido, JSON, pra mostrar sem rede).
CREATE TABLE alvanista_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
