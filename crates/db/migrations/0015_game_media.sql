-- Mídias do jogo pra galeria (ScreenScraper jeuInfos.php, `medias`: ss,
-- sstitle, fanart, wheel, video). URLs do provedor. Ver
-- domain::metadata::GameDetails.
ALTER TABLE game_metadata ADD COLUMN screenshot_url TEXT;
ALTER TABLE game_metadata ADD COLUMN title_screen_url TEXT;
ALTER TABLE game_metadata ADD COLUMN fanart_url TEXT;
ALTER TABLE game_metadata ADD COLUMN logo_url TEXT;
ALTER TABLE game_metadata ADD COLUMN video_url TEXT;
