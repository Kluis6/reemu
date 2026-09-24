//! Tempo de jogo por ROM (`roms.play_time_secs`, migration 0010).
//!
//! Uma thread amostra a sessão 1×/s e só conta o tempo com o jogo RODANDO
//! (`SessionState::Running`) — pausa, menu e biblioteca não entram. Grava no
//! banco a cada `FLUSH_EVERY` e na troca/descarga de ROM, que ela detecta
//! sozinha pela mudança de `AppState::current_rom`: nenhum ponto do fluxo de
//! load/unload precisa lembrar de avisar o relógio. Numa queda do app perde
//! no máximo `FLUSH_EVERY`.

use std::time::{Duration, Instant};

use domain::library::RomRepository;
use tauri::Manager;

use crate::commands::AppState;

const TICK: Duration = Duration::from_secs(1);
const FLUSH_EVERY: Duration = Duration::from_secs(10);

/// Acumulador puro: decide QUANTO somar e QUANDO gravar.
#[derive(Default)]
pub struct PlayClock {
    rom: Option<String>,
    pending: Duration,
}

impl PlayClock {
    /// Avança `dt`. Devolve `(rom, segundos)` a gravar no banco, se for hora:
    /// a ROM trocou/foi descarregada (grava o que sobrou da anterior), o jogo
    /// parou de rodar (pausa/unload — senão uns segundos ficavam pendentes
    /// até a próxima troca, e se perdiam ao fechar o app) ou o pendente passou
    /// de `FLUSH_EVERY`. Só segundos inteiros saem; a fração fica pra próxima.
    pub fn tick(
        &mut self,
        current: Option<&str>,
        running: bool,
        dt: Duration,
    ) -> Option<(String, u64)> {
        if self.rom.as_deref() != current {
            let flush = self.take_whole_secs(true);
            self.rom = current.map(str::to_owned);
            self.pending = Duration::ZERO;
            if running && self.rom.is_some() {
                self.pending = dt;
            }
            return flush;
        }
        if running && self.rom.is_some() {
            self.pending += dt;
        }
        if self.pending >= FLUSH_EVERY || (!running && self.pending >= Duration::from_secs(1)) {
            return self.take_whole_secs(false);
        }
        None
    }

    /// Segundos ainda não gravados da ROM `rom` (pro save state somar ao que
    /// já está no banco).
    pub fn pending_secs(&self, rom: &str) -> u64 {
        if self.rom.as_deref() == Some(rom) {
            self.pending.as_secs()
        } else {
            0
        }
    }

    fn take_whole_secs(&mut self, all: bool) -> Option<(String, u64)> {
        let rom = self.rom.clone()?;
        let secs = self.pending.as_secs();
        if all {
            self.pending = Duration::ZERO;
        } else {
            self.pending -= Duration::from_secs(secs);
        }
        (secs > 0).then_some((rom, secs))
    }
}

pub fn spawn(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("reemu-play-clock".into())
        .spawn(move || {
            let mut last = Instant::now();
            loop {
                std::thread::sleep(TICK);
                let now = Instant::now();
                let dt = now - last;
                last = now;
                let state = app.state::<AppState>();
                let current = state
                    .current_rom
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                let running = matches!(state.session.state(), emu_session::SessionState::Running);
                let flush = state
                    .play_clock
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .tick(current.as_deref(), running, dt);
                if let (Some((rom, secs)), Some(pool)) = (flush, state.db.clone()) {
                    let r = tauri::async_runtime::block_on(
                        db::RomsRepo::new(pool).add_play_time(&rom, secs),
                    );
                    if let Err(e) = r {
                        log::warn!("tempo de jogo: não gravou {secs}s de {rom}: {e}");
                    }
                }
            }
        })
        .expect("spawn reemu-play-clock");
}

/// Tempo total de jogo da ROM: o que está no banco + o que o relógio ainda
/// não gravou.
pub async fn total(state: &AppState, rom: &str) -> Option<u64> {
    let pool = state.db.clone()?;
    let saved = db::RomsRepo::new(pool).play_time(rom).await.ok()?;
    let pending = state
        .play_clock
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .pending_secs(rom);
    Some(saved + pending)
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn counts_only_while_running_and_flushes_when_it_stops() {
        let mut c = PlayClock::default();
        assert_eq!(c.tick(Some("a"), true, S), None);
        c.tick(Some("a"), true, S * 2);
        // pausou/descarregou: grava o que tinha, e o tempo parado não conta
        assert_eq!(c.tick(Some("a"), false, S * 5), Some(("a".into(), 3)));
        assert_eq!(c.tick(Some("a"), false, S * 5), None);
        c.tick(Some("a"), true, S * 2);
        assert_eq!(c.pending_secs("a"), 2);
    }

    #[test]
    fn flushes_every_ten_seconds_keeping_the_fraction() {
        let mut c = PlayClock::default();
        c.tick(Some("a"), true, Duration::from_millis(500));
        let flush = c.tick(Some("a"), true, Duration::from_millis(9_700));
        assert_eq!(flush, Some(("a".into(), 10)));
        assert_eq!(c.pending, Duration::from_millis(200));
    }

    #[test]
    fn rom_change_flushes_the_previous_one() {
        let mut c = PlayClock::default();
        c.tick(Some("a"), true, S * 4);
        let flush = c.tick(Some("b"), true, S);
        assert_eq!(flush, Some(("a".into(), 4)));
        assert_eq!(c.pending_secs("b"), 1);
        assert_eq!(c.pending_secs("a"), 0);
        // descarregou o jogo: grava o que sobrou de "b"
        c.tick(Some("b"), true, S * 2);
        assert_eq!(c.tick(None, false, S), Some(("b".into(), 3)));
    }

    #[test]
    fn nothing_to_flush_without_a_rom() {
        let mut c = PlayClock::default();
        assert_eq!(c.tick(None, true, S * 30), None);
        assert_eq!(c.pending, Duration::ZERO);
    }
}
