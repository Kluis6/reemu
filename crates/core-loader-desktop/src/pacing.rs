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
/// Até quantos frames de atraso o acumulador tenta recuperar rodando quadros
/// colados. Era 4 (~66 ms a 60 fps): um core atrasado corria sem dormir esse
/// tempo todo — o jogo "acelerava" visivelmente e a CPU dava pico. 2 ainda
/// absorve um engasgo isolado do sistema (um quadro perdido) sem acelerar;
/// acima disso ressincroniza com o relógio.
const MAX_CATCH_UP_FRAMES: u32 = 2;

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

    /// Ajuste fino do período sem recomeçar a contar (o próximo prazo é o
    /// anterior + o período novo): casa o ritmo com o monitor (`RateMatch`)
    /// sem salto no meio do jogo.
    pub fn set_period(&mut self, budget: Duration) {
        self.budget = budget;
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
    /// um pouco é compensado nos seguintes; atrasado mais de
    /// `MAX_CATCH_UP_FRAMES`,
    /// desiste e ressincroniza (core lento demais pra máquina).
    pub fn pace(&mut self) -> PaceStats {
        let mut stats = PaceStats::default();
        self.next += self.budget;
        let now = Instant::now();
        if now >= self.next {
            stats.late = true;
            if now.duration_since(self.next) > self.budget * MAX_CATCH_UP_FRAMES {
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

/// Ritmo do core casado com o do monitor (tarefa A4 do TASKS).
///
/// O core roda no relógio próprio (`Pacer`, 1/fps), e o monitor tem o dele.
/// Com 60,0 fps num monitor de 59,8 Hz a fase desliza: os quadros chegam
/// cada vez mais cedo em relação ao refresh, a espera no present cresce até
/// um refresh inteiro e sobra 1 quadro a cada poucos segundos (medido em
/// 2026-10-02, com latência 1 e 2).
///
/// O pai mede o período do monitor e manda uma vez por segundo
/// (`ToChild::DisplayPeriod`). Se ele estiver a até
/// `RATE_MATCH_TOLERANCE` do período pedido pelo core, o `Pacer` passa a
/// usar o período do monitor: o deslize de fase para, e a diferença de
/// ritmo no áudio fica com o Dynamic Rate Control (±0,5% no padrão,
/// `AudioConfig::rate_control_delta`). Fora disso (monitor de 144 ou 50 Hz,
/// jogo PAL), o `Pacer` volta ao período do core.
///
/// O core nunca espera o monitor. A tentativa anterior (um quadro por tick
/// do present) travava o laço no tick e, com o acquire bloqueando só às
/// vezes, derrubava o jogo pra 48–56 fps.
#[derive(Debug, Clone, Copy)]
pub struct RateMatch {
    core: Duration,
    matched: bool,
}

/// Diferença máxima entre o período do monitor e o do core. Abaixo dos
/// ±0,5% que o Dynamic Rate Control do áudio compensa.
pub const RATE_MATCH_TOLERANCE: f64 = 0.004;

impl RateMatch {
    pub fn new(core: Duration) -> Self {
        Self {
            core,
            matched: false,
        }
    }

    /// O core pediu outro fps (load, `SET_SYSTEM_AV_INFO`): volta ao
    /// período dele até o próximo período do monitor.
    pub fn set_core_budget(&mut self, core: Duration) {
        self.core = core;
        self.matched = false;
    }

    pub fn matched(&self) -> bool {
        self.matched
    }

    /// Período pedido pelo core (1/fps).
    pub fn core_budget(&self) -> Duration {
        self.core
    }

    /// Período medido do monitor. Devolve o período que o `Pacer` deve usar
    /// e, se o estado mudou, `Some(casado agora?)`.
    pub fn on_display_period(&mut self, period: Duration) -> (Duration, Option<bool>) {
        let ratio = period.as_secs_f64() / self.core.as_secs_f64().max(f64::EPSILON);
        let ok = (ratio - 1.0).abs() <= RATE_MATCH_TOLERANCE;
        let change = (ok != self.matched).then_some(ok);
        self.matched = ok;
        (if ok { period } else { self.core }, change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE: Duration = Duration::from_micros(16_667);

    #[test]
    fn rate_matches_a_close_refresh() {
        // 59,8 Hz contra 60,0 fps: 0,33%, dentro da tolerância.
        let mut r = RateMatch::new(CORE);
        let monitor = Duration::from_micros(16_722);
        assert_eq!(r.on_display_period(monitor), (monitor, Some(true)));
        assert!(r.matched());
        // Mesma situação no segundo seguinte: sem mudança de estado.
        assert_eq!(r.on_display_period(monitor), (monitor, None));
    }

    #[test]
    fn rate_keeps_the_core_period_on_other_refresh_rates() {
        let mut r = RateMatch::new(CORE);
        // 144 Hz e 50 Hz: segue o core.
        assert_eq!(
            r.on_display_period(Duration::from_micros(6_944)),
            (CORE, None)
        );
        assert_eq!(
            r.on_display_period(Duration::from_micros(20_000)),
            (CORE, None)
        );
        assert!(!r.matched());
    }

    #[test]
    fn rate_unmatches_when_the_monitor_or_the_core_changes() {
        let mut r = RateMatch::new(CORE);
        r.on_display_period(Duration::from_micros(16_722));
        // Janela foi pra um monitor de 144 Hz.
        assert_eq!(
            r.on_display_period(Duration::from_micros(6_944)),
            (CORE, Some(false))
        );
        // E o core muda de fps: volta ao período novo dele.
        r.on_display_period(Duration::from_micros(16_722));
        let pal = Duration::from_millis(20);
        r.set_core_budget(pal);
        assert!(!r.matched());
        assert_eq!(
            r.on_display_period(Duration::from_micros(16_722)),
            (pal, None)
        );
    }

    #[test]
    fn set_period_keeps_the_phase() {
        // Trocar o período não recomeça a contar de agora (sem salto).
        let mut p = Pacer::new(Duration::from_millis(4));
        let next = p.next;
        p.set_period(Duration::from_millis(5));
        assert_eq!(p.next, next);
        assert_eq!(p.budget(), Duration::from_millis(5));
    }

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

    #[test]
    fn catches_up_at_most_max_frames_then_resyncs() {
        let budget = Duration::from_millis(10);
        let mut p = Pacer::new(budget);
        // ~3 quadros de atraso: acima do limite de 2 → ressincroniza, e o
        // pace seguinte já dorme normalmente. Com o limite antigo (4) ele
        // ficaria correndo colado pra "recuperar" esses quadros.
        std::thread::sleep(budget * 4);
        assert!(p.pace().late);
        assert!(!p.pace().late, "ressincronizou, não fica correndo colado");
    }
}
