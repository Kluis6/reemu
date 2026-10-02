-- Pastas que o usuário adicionou à biblioteca. "Atualizar biblioteca"
-- varre de novo cada uma (jogos novos, regra de identificação nova, arquivo
-- que deixou de ser jogo).
CREATE TABLE library_folders (
    path     TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);
