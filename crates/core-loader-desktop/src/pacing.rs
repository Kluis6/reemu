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

/// Ritmo do core preso ao refresh do monitor (tarefa A4 do TASKS).
///
/// O core roda no relógio próprio (`Pacer`, 1/fps), e o monitor tem o dele.
/// Com 60,0 fps num monitor de 59,8 Hz, a fase desliza: os quadros chegam
/// cada vez mais cedo em relação ao refresh, a espera no present cresce até
/// um refresh inteiro e sobra 1 quadro a cada ~5 s (medido em 2026-10-02).
///
/// O pai manda um `VsyncTick` logo depois de cada present que esperou o
/// refresh, com o período medido do monitor. Se esse período estiver a no
/// máximo `TOLERANCE` do budget do core por `LOCK_AFTER` ticks seguidos, o
/// laço do filho passa a rodar um quadro por tick (o ritmo e a fase vêm do
/// monitor), e a diferença de ritmo no áudio fica com o Dynamic Rate
/// Control (±0,5% no padrão, `AudioConfig::rate_control_delta`). Fora da
/// tolerância (monitor de 144 ou 50 Hz) ou sem tick por `TIMEOUT_FRAMES`
/// quadros (menu, janela escondida), volta ao `Pacer`.
#[derive(Debug, Default)]
pub struct VsyncLock {
    locked: bool,
    /// Ticks seguidos com período compatível.
    good: u32,
    /// Chegou tick que ainda não virou quadro.
    pending: bool,
    /// Quando o laço começou a esperar o próximo tick.
    wait_since: Option<Instant>,
    /// Início do último quadro rodado no ritmo do monitor.
    last_run: Option<Instant>,
}

/// Diferença máxima entre o período do monitor e o do core. Abaixo dos
/// ±0,5% que o Dynamic Rate Control do áudio compensa.
pub const VSYNC_TOLERANCE: f64 = 0.004;
/// Ticks compatíveis seguidos antes de travar (~0,5 s a 60 Hz).
const LOCK_AFTER: u32 = 30;
/// Sem tick por este tanto de quadros, destrava e volta ao `Pacer`.
const TIMEOUT_FRAMES: f64 = 1.5;
/// Nunca roda dois quadros mais perto que isto, mesmo com tick (protege
/// contra tick adiantado).
const MIN_SPACING: f64 = 0.9;

/// Mudança de estado do `VsyncLock`, pra log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VsyncChange {
    Locked,
    Unlocked,
}

impl VsyncLock {
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Tick do pai com o período medido do monitor.
    pub fn on_tick(&mut self, period: Duration, budget: Duration) -> Option<VsyncChange> {
        let ratio = period.as_secs_f64() / budget.as_secs_f64().max(f64::EPSILON);
        let compatible = (ratio - 1.0).abs() <= VSYNC_TOLERANCE;
        let mut change = None;
        if compatible {
            self.good = self.good.saturating_add(1);
            if !self.locked && self.good >= LOCK_AFTER {
                self.locked = true;
                change = Some(VsyncChange::Locked);
            }
        } else {
            self.good = 0;
            if self.locked {
                self.unlock();
                change = Some(VsyncChange::Unlocked);
            }
        }
        if self.locked {
            self.pending = true;
        }
        change
    }

    /// Travado e sem tick pendente: o laço deve esperar o próximo tick até
    /// o prazo devolvido.
    pub fn wait_deadline(&mut self, budget: Duration) -> Option<Instant> {
        if !self.locked || self.pending {
            return None;
        }
        let since = *self.wait_since.get_or_insert_with(Instant::now);
        Some(since + budget.mul_f64(TIMEOUT_FRAMES))
    }

    /// O prazo de `wait_deadline` passou sem tick: volta ao `Pacer`.
    pub fn on_timeout(&mut self) -> Option<VsyncChange> {
        if !self.locked {
            return None;
        }
        self.unlock();
        self.good = 0;
        Some(VsyncChange::Unlocked)
    }

    /// Antes de rodar um quadro travado: espera o espaçamento mínimo desde
    /// o anterior, se o tick veio adiantado. Devolve quanto dormiu.
    pub fn space_out(&mut self, budget: Duration) -> Duration {
        let Some(last) = self.last_run else {
            return Duration::ZERO;
        };
        let min = budget.mul_f64(MIN_SPACING);
        let since = last.elapsed();
        if since < min {
            let d = min - since;
            std::thread::sleep(d);
            d
        } else {
            Duration::ZERO
        }
    }

    /// Um quadro rodou no ritmo do monitor (consome o tick).
    pub fn frame_ran(&mut self, started: Instant) {
        self.pending = false;
        self.wait_since = None;
        self.last_run = Some(started);
    }

    /// Load, pausa, troca de fps: recomeça a contar.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn unlock(&mut self) {
        self.locked = false;
        self.pending = false;
        self.wait_since = None;
        self.last_run = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGET: Duration = Duration::from_micros(16_667);

    fn feed(lock: &mut VsyncLock, period: Duration, n: u32) -> Option<VsyncChange> {
        let mut last = None;
        for _ in 0..n {
            if let Some(c) = lock.on_tick(period, BUDGET) {
                last = Some(c);
            }
        }
        last
    }

    #[test]
    fn vsync_locks_on_a_close_refresh() {
        // 59,8 Hz contra 60,0 fps: 0,33%, dentro da tolerância.
        let mut l = VsyncLock::default();
        assert_eq!(
            feed(&mut l, Duration::from_micros(16_722), LOCK_AFTER - 1),
            None
        );
        assert!(!l.locked());
        assert_eq!(
            feed(&mut l, Duration::from_micros(16_722), 1),
            Some(VsyncChange::Locked)
        );
        assert!(l.locked());
    }

    #[test]
    fn vsync_ignores_other_refresh_rates() {
        let mut l = VsyncLock::default();
        // 144 Hz e 50 Hz: nunca trava.
        assert_eq!(feed(&mut l, Duration::from_micros(6_944), 100), None);
        assert_eq!(feed(&mut l, Duration::from_micros(20_000), 100), None);
        assert!(!l.locked());
    }

    #[test]
    fn vsync_unlocks_when_the_refresh_changes() {
        let mut l = VsyncLock::default();
        feed(&mut l, BUDGET, LOCK_AFTER);
        assert!(l.locked());
        // Janela foi pra um monitor de 144 Hz.
        assert_eq!(
            l.on_tick(Duration::from_micros(6_944), BUDGET),
            Some(VsyncChange::Unlocked)
        );
        assert!(!l.locked());
    }

    #[test]
    fn vsync_waits_for_ticks_and_times_out() {
        let mut l = VsyncLock::default();
        feed(&mut l, BUDGET, LOCK_AFTER);
        // O tick que travou fica pendente: roda um quadro sem esperar.
        assert!(l.wait_deadline(BUDGET).is_none());
        l.frame_ran(Instant::now());
        // Agora espera o próximo tick, com prazo de 1,5 quadro.
        let deadline = l.wait_deadline(BUDGET).expect("travado espera tick");
        assert!(deadline > Instant::now());
        assert_eq!(l.on_timeout(), Some(VsyncChange::Unlocked));
        assert!(!l.locked());
        assert!(l.wait_deadline(BUDGET).is_none());
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
