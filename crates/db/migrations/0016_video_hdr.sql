-- Saída HDR (Configurações › Sistema › Vídeo). Ver domain::video::VideoConfig.
-- 0 nos nits = automático (valores do Windows/monitor).
ALTER TABLE video_config ADD COLUMN hdr INTEGER NOT NULL DEFAULT 0;
ALTER TABLE video_config ADD COLUMN hdr_paper_white_nits INTEGER NOT NULL DEFAULT 0;
ALTER TABLE video_config ADD COLUMN hdr_highlights INTEGER NOT NULL DEFAULT 1;
ALTER TABLE video_config ADD COLUMN hdr_peak_nits INTEGER NOT NULL DEFAULT 0;
