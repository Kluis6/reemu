-- =========================================================================
-- Vídeo (integer scaling — backlog)
-- =========================================================================

-- Linha única (sem user_profile_id), mesmo padrão de audio_config.
CREATE TABLE video_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    integer_scaling INTEGER NOT NULL DEFAULT 0
);

INSERT INTO video_config (id, integer_scaling) VALUES (1, 0);
