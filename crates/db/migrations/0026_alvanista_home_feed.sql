-- Novidades curadas do Alvanista para o HeroCarousel da tela inicial
-- (`GET /api/partner/v1/home?lang=<lang>`). A preferencia para ligar ou
-- desligar os destaques (`home.highlights_enabled`) fica em `alvanista_state`.
CREATE TABLE alvanista_home_feed (
    id         TEXT NOT NULL,
    lang       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('article', 'review')),
    title      TEXT NOT NULL,
    summary    TEXT NOT NULL DEFAULT '',
    image_url  TEXT,
    url        TEXT NOT NULL,
    starts_at  TEXT,
    ends_at    TEXT,
    sponsored  INTEGER NOT NULL DEFAULT 0,
    position   INTEGER NOT NULL,
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (lang, id)
);

CREATE INDEX idx_alvanista_home_feed_lang_pos
    ON alvanista_home_feed(lang, position);

CREATE INDEX idx_alvanista_home_feed_id
    ON alvanista_home_feed(id);
