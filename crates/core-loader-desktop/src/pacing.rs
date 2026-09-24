//! Pacing do loop do core: um `retro_run` por `frame_budget` (1/fps).
//!
//! Dorme o grosso e cobre o resto em spin até o prazo — `sleep` sozinho
//! acorda atrasado (no Linux ~50–100 µs; no Windows o timer de alta
//! resolução do `std` erra mais), e isso vira jitter de frame. A margem de
//! spin era fixa em 600 µs: no Linux isso queimava ~0,5 ms de CPU por frame
//! à toa (3,2% de um núcleo medido com `REEMU_PERF=1`). Agora ela se ajusta
//! ao atraso REAL do `sleep` nesta máquina: sobe na hora se o sono atrasou
//! mais que a margem, desce devagar quando sobra.
//!
//! Usado pelo `reemu-core-host` (cores software/GL no processo filho) e pelo
//! caminho Vulkan in-process do `emu-session`.

use std::time::{Duration, Instant};

const MIN_MARGIN: Duration = Duration::from_micros(150);
const MAX_MARGIN: Duration = Duration::from_millis(2);
const INITIAL_MARGIN: Duration = Duration::from_micros(600);
/// Folga acima do pior atraso visto, pra um sono um pouco mais lento que o
/// habitual ainda não estourar o prazo.
const SAFETY: Duration = Duration::from_micros(100);

/// Quanto um `pace()` dormiu, girou em spin, e se o frame já chegou atrasado.
#[derive(Default, Debug, Clone, Copy)]
pub struct PaceStats {
    pub slept: Duration,
    pub spun: Duration,
    pub late: bool,
}

pub struct Pacer {
    budget: Duration,
    next: Instant,
    margin: Duration,
}

impl Pacer {
    pub fn new(budget: Duration) -> Self {
        Self {
            budget,
            next: Instant::now(),
            margin: INITIAL_MARGIN,
        }
    }

    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// Troca o ritmo (core mudou de fps) e recomeça a contar de agora.
    pub fn set_budget(&mut self, budget: Duration) {
        self.budget = budget;
        self.reset();
    }

    /// Recomeça a contar de agora (load, resume) — sem isso o acumulador
    /// tentaria "recuperar" o tempo parado rodando frames colados.
    pub fn reset(&mut self) {
        self.next = Instant::now();
    }

    /// Margem de spin atual (diagnóstico/testes).
    pub fn margin(&self) -> Duration {
        self.margin
    }

    /// Espera até o prazo do próximo frame. Acumulador: um frame que atrasou
    /// um pouco é compensado nos seguintes; atrasado mais de 4 frames,
    /// desiste e ressincroniza (core lento demais pra máquina).
    pub fn pace(&mut self) -> PaceStats {
        let mut stats = PaceStats::default();
        self.next += self.budget;
        let now = Instant::now();
        if now >= self.next {
            stats.late = true;
            if now.duration_since(self.next) > self.budget * 4 {
                self.next = now;
            }
            return stats;
        }
        if let Some(coarse) = (self.next - now).checked_sub(self.margin) {
            std::thread::sleep(coarse);
            let woke = Instant::now();
            self.adapt((woke - now).saturating_sub(coarse));
            stats.slept = woke - now;
        }
        let spin_start = Instant::now();
        while Instant::now() < self.next {
            for _ in 0..64 {
                std::hint::spin_loop();
            }
        }
        stats.spun = spin_start.elapsed();
        stats
    }

    /// Sobe na hora se o sono atrasou mais que a margem (senão o frame
    /// estoura o prazo); desce 1/32 da diferença por frame quando sobra —
    /// um pico isolado não deixa a margem alta pra sempre.
    fn adapt(&mut self, overshoot: Duration) {
        let target = (overshoot + SAFETY).clamp(MIN_MARGIN, MAX_MARGIN);
        self.margin = if target > self.margin {
            target
        } else {
            self.margin - (self.margin - target) / 32
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_requested_rate() {
        let budget = Duration::from_millis(4);
        let mut p = Pacer::new(budget);
        let t0 = Instant::now();
        for _ in 0..50 {
            p.pace();
        }
        let per_frame = t0.elapsed() / 50;
        assert!(
            per_frame >= budget && per_frame < budget + Duration::from_micros(500),
            "média de {per_frame:?} por frame pra um budget de {budget:?}"
        );
    }

    #[test]
    fn margin_stays_within_bounds() {
        let mut p = Pacer::new(Duration::from_millis(3));
        for _ in 0..200 {
            p.pace();
        }
        assert!(p.margin() >= MIN_MARGIN && p.margin() <= MAX_MARGIN);
    }

    #[test]
    fn adapt_grows_immediately_and_decays_slowly() {
        let mut p = Pacer::new(Duration::from_millis(16));
        p.adapt(Duration::from_micros(1500));
        assert_eq!(p.margin(), Duration::from_micros(1600));
        p.adapt(Duration::ZERO);
        let after_one = p.margin();
        assert!(after_one < Duration::from_micros(1600));
        assert!(
            after_one > Duration::from_micros(1500),
            "desce devagar: {after_one:?}"
        );
        p.adapt(Duration::from_millis(50));
        assert_eq!(p.margin(), MAX_MARGIN, "teto");
    }

    #[test]
    fn late_frame_is_reported_and_far_behind_resyncs() {
        let budget = Duration::from_millis(2);
        let mut p = Pacer::new(budget);
        std::thread::sleep(budget * 10);
        assert!(p.pace().late);
        // ressincronizou: o próximo não chega atrasado
        assert!(!p.pace().late);
    }
}
