//! Diagnóstico de desempenho do `reemu-video-pump` (surface nativa), 1 linha
//! por segundo no log sob `REEMU_PERF=1`. Par do diagnóstico do core-host
//! (`crates/core-host-desktop`): lá mede o core, aqui o que chega na tela.
//!
//! `recebidos` vem do `EmuSession::frame_seq` (frames que o core entregou);
//! `apresentados` são os que o pump desenhou. A diferença são frames
//! sobrescritos no `latest_frame` antes do pump pegar — descartados sem
//! nunca aparecer. Voltas sem frame novo são o pump acordando cedo demais.

use std::time::{Duration, Instant};

pub fn enabled() -> bool {
    std::env::var_os("REEMU_PERF").is_some()
}

pub struct PumpDiag {
    since: Instant,
    seq_at_start: u64,
    last_tick: Option<Instant>,
    ticks: u64,
    tick_worst: Duration,
    presented: u64,
    empty: u64,
    render: Duration,
    render_worst: Duration,
}

impl PumpDiag {
    pub fn new(frame_seq: u64) -> Self {
        Self {
            since: Instant::now(),
            seq_at_start: frame_seq,
            last_tick: None,
            ticks: 0,
            tick_worst: Duration::ZERO,
            presented: 0,
            empty: 0,
            render: Duration::ZERO,
            render_worst: Duration::ZERO,
        }
    }

    /// Início de uma volta do pump.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(prev) = self.last_tick {
            self.tick_worst = self.tick_worst.max(now - prev);
        }
        self.last_tick = Some(now);
        self.ticks += 1;
    }

    pub fn presented(&mut self, render: Duration) {
        self.presented += 1;
        self.render += render;
        self.render_worst = self.render_worst.max(render);
    }

    /// Jogando, mas sem frame novo nesta volta.
    pub fn empty(&mut self) {
        self.empty += 1;
    }

    pub fn maybe_report(&mut self, frame_seq: u64) {
        let elapsed = self.since.elapsed();
        if elapsed < Duration::from_secs(1) {
            return;
        }
        let secs = elapsed.as_secs_f32();
        let received = frame_seq.saturating_sub(self.seq_at_start);
        // Sem jogo rodando não há o que medir — só reinicia a janela.
        if received > 0 || self.presented > 0 {
            let ms = |d: Duration| d.as_secs_f32() * 1000.0;
            log::info!(
                "perf vídeo 1s: {} recebidos do core, {} apresentados, {} perdidos | \
                 {} voltas do pump ({:.1}/s, maior intervalo {:.1} ms), {} sem frame novo | \
                 render méd {:.2} pior {:.2} ms",
                received,
                self.presented,
                received.saturating_sub(self.presented),
                self.ticks,
                self.ticks as f32 / secs,
                ms(self.tick_worst),
                self.empty,
                ms(self.render) / self.presented.max(1) as f32,
                ms(self.render_worst),
            );
        }
        let last_tick = self.last_tick;
        *self = Self::new(frame_seq);
        self.last_tick = last_tick;
    }
}
