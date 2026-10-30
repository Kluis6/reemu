-- "Entrar com Alvanista" (apps/desktop/src-tauri/src/alvanista/).
-- Os tokens ficam no chaveiro do SO; `secret_fallback` só recebe o par quando
-- o chaveiro falha (mesmo papel das colunas de segredo do scraper).
--
-- `IF NOT EXISTS`: builds de pré-lançamento criaram estas duas tabelas como
-- migration 0017 ou 0018; `forget_prerelease_alvanista` (pool.rs) tira a
-- linha delas do `_sqlx_migrations` e esta migration fica com as tabelas e o
-- que houver nelas.
CREATE TABLE IF NOT EXISTS secret_fallback (
    account TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);

-- Pares chave/valor da integração: `device_id` (gerado uma vez), `mirror`
-- (último perfil lido, JSON, pra mostrar sem rede) e `session`, o marcador de
-- sessão: sem ele o app não lê o chaveiro nem chama a rede.
CREATE TABLE IF NOT EXISTS alvanista_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Banco de um build de pré-lançamento, que não tinha o marcador: se a
-- integração já foi usada nele, marca a sessão para o app conferir o chaveiro
-- uma vez (sem par guardado, o marcador sai na primeira leitura). Num banco
-- que nunca teve a integração as tabelas acabaram de nascer vazias e nada
-- entra.
INSERT OR IGNORE INTO alvanista_state (key, value)
SELECT 'session', 'migrated'
WHERE EXISTS (SELECT 1 FROM alvanista_state WHERE key IN ('device_id', 'mirror'))
   OR EXISTS (SELECT 1 FROM secret_fallback WHERE account = 'alvanista');
