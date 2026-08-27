-- Migration inicial: todos os schemas definidos no design do projeto.
-- Nenhuma tabela tem user_profile_id — app sem multi-perfil no MVP
-- (configs de hotkey, áudio etc são únicas por instalação).

-- =========================================================================
-- Biblioteca / Scraping de metadata
-- =========================================================================

CREATE TABLE roms (
    id TEXT PRIMARY KEY,
    file_path TEXT NOT NULL UNIQUE,
    crc32 TEXT NOT NULL,
    md5 TEXT NOT NULL,
    system_id TEXT NOT NULL,
    added_at INTEGER NOT NULL
);

CREATE INDEX idx_roms_crc32 ON roms (crc32);

CREATE TABLE scrape_matches (
    id TEXT PRIMARY KEY,
    rom_id TEXT NOT NULL REFERENCES roms(id),
    provider TEXT NOT NULL,
    external_id TEXT NOT NULL,
    confidence_score REAL,
    -- 'auto_matched' só ocorre com hash exato (decisão: Abordagem B)
    status TEXT NOT NULL CHECK (status IN ('auto_matched', 'pending_review', 'user_confirmed', 'no_match'))
);

CREATE TABLE game_metadata (
    id TEXT PRIMARY KEY,
    rom_id TEXT REFERENCES roms(id),
    title TEXT NOT NULL,
    description TEXT,
    cover_url TEXT,
    release_date TEXT,
    genre TEXT,
    provider_source TEXT
);

-- =========================================================================
-- Cores
-- =========================================================================

CREATE TABLE installed_cores (
    core_id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    installed_at INTEGER NOT NULL,
    -- Detectado em runtime no primeiro load (decisão: Opção a.1),
    -- não curado manualmente.
    render_backend TEXT CHECK (render_backend IN ('software', 'opengl', 'vulkan')),
    gl_version_min TEXT,
    gl_profile TEXT,
    needs_depth_stencil INTEGER
);

CREATE TABLE core_options_schema (
    id TEXT PRIMARY KEY,
    core_id TEXT NOT NULL REFERENCES installed_cores(core_id),
    option_key TEXT NOT NULL,
    display_name TEXT NOT NULL,
    option_type TEXT NOT NULL CHECK (option_type IN ('combo', 'bool', 'range')),
    choices TEXT, -- JSON
    default_value TEXT,
    UNIQUE(core_id, option_key)
);

CREATE TABLE core_options_values (
    id TEXT PRIMARY KEY,
    core_id TEXT NOT NULL,
    option_key TEXT NOT NULL,
    value TEXT NOT NULL,
    UNIQUE(core_id, option_key)
);

-- Status experimental/estabilidade NÃO tem tabela: fica em arquivo estático
-- versionado no repo do app (decisão: Opção b.1), não no banco do usuário.

-- =========================================================================
-- Shader chain / decoração
-- =========================================================================

CREATE TABLE shader_presets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source_path TEXT NOT NULL,
    format TEXT NOT NULL CHECK (format IN ('slang')), -- reshade_fx: backlog
    is_builtin INTEGER NOT NULL DEFAULT 0,
    includes_bezel INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE shader_chain_assignments (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK (scope IN ('default', 'system', 'rom')),
    system_id TEXT,
    rom_id TEXT,
    preset_id TEXT NOT NULL REFERENCES shader_presets(id),
    UNIQUE(scope, system_id, rom_id)
);

CREATE TABLE shader_parameter_overrides (
    id TEXT PRIMARY KEY,
    assignment_id TEXT NOT NULL REFERENCES shader_chain_assignments(id),
    parameter_key TEXT NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE decoration_packs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('bundled', 'user_imported')),
    base_path TEXT NOT NULL
);

CREATE TABLE decoration_assignments (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK (scope IN ('default', 'system', 'rom')),
    system_id TEXT,
    rom_id TEXT,
    pack_id TEXT NOT NULL REFERENCES decoration_packs(id),
    asset_path TEXT NOT NULL,
    UNIQUE(scope, system_id, rom_id)
);

-- =========================================================================
-- Input
-- =========================================================================

-- layout_json: serialização de Vec<ControllerLayoutEntry> (domain::input),
-- cada entrada com um trigger (combinação de RawInputEvent) -> RetroPadButton.
-- Suporta combinação hold+press igual a system_hotkeys.trigger_json
-- (decisão revisada: combinação vale pros dois casos, não só hotkeys).
CREATE TABLE controller_mappings (
    guid TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    layout_json TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('sdl_game_controller_db', 'bundled_android', 'user_override'))
);

CREATE TABLE device_port_assignment (
    guid TEXT PRIMARY KEY REFERENCES controller_mappings(guid),
    port_index INTEGER NOT NULL
);

-- Suporta combinação hold+press (decisão: Abordagem B pra hotkeys)
CREATE TABLE system_hotkeys (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    trigger_json TEXT NOT NULL, -- lista de RawInputEvent serializados
    device_guid TEXT
);

-- =========================================================================
-- Áudio
-- =========================================================================

-- Linha única (sem user_profile_id)
CREATE TABLE audio_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    output_device_id TEXT,
    output_device_name TEXT,
    rate_control_enabled INTEGER NOT NULL DEFAULT 1,
    rate_control_delta REAL NOT NULL DEFAULT 0.005,
    sample_rate_preference INTEGER
);

INSERT INTO audio_config (id, rate_control_enabled, rate_control_delta)
VALUES (1, 1, 0.005);

-- =========================================================================
-- Save states / save RAM
-- =========================================================================

CREATE TABLE save_states (
    id TEXT PRIMARY KEY,
    rom_id TEXT NOT NULL REFERENCES roms(id),
    core_id TEXT NOT NULL,
    slot INTEGER,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    created_at INTEGER NOT NULL,
    play_time_at_save INTEGER
);

CREATE TABLE save_ram (
    id TEXT PRIMARY KEY,
    rom_id TEXT NOT NULL REFERENCES roms(id),
    core_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(rom_id, core_id)
);
