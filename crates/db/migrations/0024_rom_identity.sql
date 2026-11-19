-- Identidade da ROM no scan: o SHA1 sai da mesma leitura do CRC32/MD5, e o
-- nome e o tamanho são os do que foi lido (a entrada de dentro do .zip/.7z, o
-- arquivo em si nos demais). `serial` é o do disco, preenchido na leitura da
-- imagem. `identified_at` em segundos Unix. NULL nas linhas de antes: o passo
-- em segundo plano completa, o scan não relê a biblioteca.
ALTER TABLE roms ADD COLUMN sha1 TEXT;
ALTER TABLE roms ADD COLUMN serial TEXT;
ALTER TABLE roms ADD COLUMN ident_name TEXT;
ALTER TABLE roms ADD COLUMN ident_size INTEGER;
ALTER TABLE roms ADD COLUMN identified_at INTEGER;
