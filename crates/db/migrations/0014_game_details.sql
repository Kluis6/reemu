-- Ficha técnica do jogo (ScreenScraper jeuInfos.php: developpeur, editeur,
-- joueurs, note, classifications, modes). Ver domain::metadata::GameDetails.
ALTER TABLE game_metadata ADD COLUMN developer TEXT;
ALTER TABLE game_metadata ADD COLUMN publisher TEXT;
ALTER TABLE game_metadata ADD COLUMN players TEXT;
-- nota de 0 a 100
ALTER TABLE game_metadata ADD COLUMN rating INTEGER;
ALTER TABLE game_metadata ADD COLUMN age_rating TEXT;
ALTER TABLE game_metadata ADD COLUMN modes TEXT;
