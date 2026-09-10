-- Perfil local — um por instalação (sem multi-perfil), igual audio_config.
-- Futuro: ligado a uma rede social. Onboarding na 1ª execução define
-- name/bio/avatar e marca onboarded = 1.
--
-- avatar: "preset:1".."preset:5" (avatares embutidos) ou "file" (o usuário
-- escolheu uma imagem — copiada pra <dados>/profile/avatar.<ext>).

CREATE TABLE profile (
    id        INTEGER PRIMARY KEY CHECK (id = 1),
    name      TEXT NOT NULL DEFAULT '',
    bio       TEXT,
    avatar    TEXT NOT NULL DEFAULT 'preset:1',
    onboarded INTEGER NOT NULL DEFAULT 0
);

INSERT INTO profile (id) VALUES (1);
