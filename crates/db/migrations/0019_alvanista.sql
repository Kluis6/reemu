-- "Entrar com Alvanista" (apps/desktop/src-tauri/src/alvanista/). Os tokens ficam
-- só no chaveiro do SO, nunca aqui.
--
-- Pares chave/valor da integração: `device_id` (gerado uma vez), `mirror`
-- (último perfil lido, JSON, pra mostrar sem rede) e `session`, o marcador de
-- sessão: sem ele o app não lê o chaveiro nem chama a rede.
CREATE TABLE alvanista_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
