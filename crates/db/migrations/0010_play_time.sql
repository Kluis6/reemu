-- Tempo de jogo acumulado por ROM, em segundos — só o tempo com o jogo
-- rodando (pausa/menu não contam). Somado pelo relógio do app
-- (`apps/desktop/src-tauri/src/play_clock.rs`).
ALTER TABLE roms ADD COLUMN play_time_secs INTEGER NOT NULL DEFAULT 0;
