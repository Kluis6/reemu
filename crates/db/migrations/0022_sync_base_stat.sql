-- Cloud save: o tamanho e o mtime (nanossegundos Unix) do arquivo quando a
-- base foi gravada. Com os dois iguais no disco, o hash da base vale e a
-- passada não relê o arquivo (um save state pode ter dezenas de MB). NULL nas
-- linhas de antes e nos game saves.
ALTER TABLE sync_base ADD COLUMN local_size INTEGER;
ALTER TABLE sync_base ADD COLUMN local_mtime_ns INTEGER;
