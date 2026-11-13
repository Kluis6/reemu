-- Cloud save: a base local (o que já foi sincronizado, por chave) e os conflitos
-- que a máquina viu e ainda não resolveu. Nada aqui guarda token.
CREATE TABLE sync_base (
    key               TEXT PRIMARY KEY,
    kind              TEXT NOT NULL,
    remote_save_id    TEXT NOT NULL,
    remote_version_id TEXT NOT NULL,
    remote_hash       TEXT NOT NULL,
    local_hash        TEXT NOT NULL,
    local_path        TEXT NOT NULL,
    rom_crc32         TEXT,
    synced_at         INTEGER NOT NULL,
    wiped_at          INTEGER,
    -- Onde o arquivo foi parar quando a remoção no site foi aceita.
    backup_path       TEXT
);

CREATE TABLE sync_conflicts (
    key                 TEXT PRIMARY KEY,
    local_hash          TEXT NOT NULL,
    local_mtime         INTEGER NOT NULL,
    local_size          INTEGER NOT NULL,
    remote_save_id      TEXT NOT NULL,
    remote_version_id   TEXT NOT NULL,
    remote_hash         TEXT NOT NULL,
    remote_device_label TEXT,
    remote_client_mtime INTEGER,
    remote_size         INTEGER NOT NULL,
    -- 1 quando o lado da nuvem é uma remoção feita no site.
    remote_deleted      INTEGER NOT NULL DEFAULT 0,
    rom_id              TEXT,
    seen_at             INTEGER NOT NULL
);
