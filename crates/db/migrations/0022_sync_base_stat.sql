-- Cloud save: o tamanho, o mtime (nanossegundos Unix) e a linha de
-- `save_states` do arquivo quando a base foi gravada. Com os três iguais, o
-- hash da base vale e a passada não relê o arquivo (um save state pode ter
-- dezenas de MB); uma vaga gravada de novo é outra linha, mesmo com o mesmo
-- tamanho e mtime. NULL nas linhas de antes e nos game saves.
ALTER TABLE sync_base ADD COLUMN local_size INTEGER;
ALTER TABLE sync_base ADD COLUMN local_mtime_ns INTEGER;
ALTER TABLE sync_base ADD COLUMN local_row TEXT;
