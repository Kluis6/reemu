//! Dynamic Rate Control — lógica **pura** (nível de buffer → fator de
//! ajuste do resample). Testável sem hardware de áudio.
//!
//! Ideia: mantemos o buffer de saída perto de `target_fill`. Se ele está
//! esvaziando (emulador rodando devagar demais p/ o device), produzimos um
//! pouco mais de amostras por frame de entrada (fator > 1). Se enchendo,
//! um pouco menos (fator < 1). O ajuste é limitado a `±max_delta`.

#[derive(Debug, Clone, Copy)]
pub struct RateControl {
    target_fill: f32,
    max_delta: f32,
    enabled: bool,
}

impl RateControl {
    /// `max_delta` ex: 0.005 = ±0,5% (vem de `AudioConfig::rate_control_delta`).
    pub fn new(max_delta: f32, enabled: bool) -> Self {
        Self {
            target_fill: 0.5,
            max_delta: max_delta.clamp(0.0, 0.25),
            enabled,
        }
    }

    /// Fator multiplicativo da razão de resample base (`device_rate /
    /// core_rate`). `1.0` = sem ajuste. `fill` é a fração do buffer de saída
    /// preenchida agora (`0.0..=1.0`).
    pub fn factor(&self, fill: f32) -> f32 {
        if !self.enabled || self.max_delta == 0.0 {
            return 1.0;
        }
        // erro > 0 quando o buffer está abaixo do alvo (precisa produzir mais)
        let error = self.target_fill - fill.clamp(0.0, 1.0);
        // ganho 2.0: satura o ajuste quando o buffer está vazio/cheio de vez
        let adjust = (error * 2.0).clamp(-1.0, 1.0) * self.max_delta;
        1.0 + adjust
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rc() -> RateControl {
        RateControl::new(0.005, true)
    }

    #[test]
    fn no_adjust_at_target_fill() {
        assert!((rc().factor(0.5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn empty_buffer_produces_more() {
        let f = rc().factor(0.0);
        assert!(f > 1.0, "{f}");
        assert!((f - 1.005).abs() < 1e-6, "satura em +max_delta: {f}");
    }

    #[test]
    fn full_buffer_produces_less() {
        let f = rc().factor(1.0);
        assert!(f < 1.0, "{f}");
        assert!((f - 0.995).abs() < 1e-6, "satura em -max_delta: {f}");
    }

    #[test]
    fn adjustment_is_monotonic_in_fill() {
        let rc = rc();
        let a = rc.factor(0.2);
        let b = rc.factor(0.5);
        let c = rc.factor(0.8);
        assert!(a > b && b > c, "{a} {b} {c}");
    }

    #[test]
    fn disabled_or_zero_delta_is_identity() {
        assert_eq!(RateControl::new(0.005, false).factor(0.0), 1.0);
        assert_eq!(RateControl::new(0.0, true).factor(1.0), 1.0);
    }

    #[test]
    fn clamps_out_of_range_fill() {
        let rc = rc();
        assert_eq!(rc.factor(-1.0), rc.factor(0.0));
        assert_eq!(rc.factor(2.0), rc.factor(1.0));
    }
}
