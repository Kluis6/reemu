//! Estimativa do período real de atualização do monitor.

/// Estima o período do monitor (tarefa A4 do TASKS). Quando o
/// `get_current_texture` bloqueia, ele só volta quando o present engine
/// libera uma imagem, ou seja, num refresh. "Calls to
/// `Surface::get_current_texture()` will block until there is a spot in the
/// queue" (docs.rs wgpu 30, `PresentMode::Fifo`); a spec Vulkan
/// (`VkPresentModeKHR`) diz que, no FIFO, a fila anda um pedido por
/// "vertical blanking period". O instante em que um acquire bloqueado volta
/// é então uma amostra do refresh.
///
/// O período sai de somas: cada intervalo entre dois acquires bloqueados
/// seguidos vale `n` refreshes (`n` arredondado pela estimativa atual), e o
/// período é `soma(intervalos) / soma(n)`. O jitter de cada amostra se
/// dilui com o tempo. A média móvel da 1ª versão oscilava ~0,4%, a mesma
/// ordem da diferença entre core e monitor que queremos casar.
///
/// Não verificado: que o driver libera a imagem exatamente no refresh numa
/// janela composta pelo DWM (a doc da Khronos não cobre). Um atraso fixo
/// some na diferença entre amostras; intervalos que não fecham um número
/// inteiro de refreshes (travada, acquire que não bloqueou) ficam de fora.
#[derive(Default)]
pub(super) struct VblankEstimator {
    last: Option<std::time::Instant>,
    /// Estimativa corrente (s), usada pra arredondar `n`.
    coarse: Option<f64>,
    sum_dt: f64,
    sum_n: f64,
    /// Intervalos seguidos que não fecharam: muitos = o monitor mudou.
    rejects: u32,
}

impl VblankEstimator {
    /// Acquire que esperou menos que isso não foi parado pelo refresh.
    const MIN_BLOCK: std::time::Duration = std::time::Duration::from_millis(2);
    /// Refreshes somados antes de publicar o período (~2 s a 60 Hz).
    const MIN_REFRESHES: f64 = 120.0;
    /// Acima disto as somas caem pela metade (~1 min a 60 Hz): a estimativa
    /// acompanha uma mudança lenta sem esquecer a precisão.
    const MAX_REFRESHES: f64 = 3600.0;
    /// Intervalos seguidos que não fecham antes de recomeçar do zero.
    const MAX_REJECTS: u32 = 60;

    pub(super) fn observe(&mut self, started: std::time::Instant, returned: std::time::Instant) {
        if returned.duration_since(started) < Self::MIN_BLOCK {
            // Não bloqueou: este instante não é um refresh, e o intervalo a
            // partir da amostra anterior também deixa de valer.
            self.last = None;
            return;
        }
        if let Some(prev) = self.last {
            let dt = returned.duration_since(prev).as_secs_f64();
            match self.coarse {
                // 20–250 Hz.
                None if (0.004..0.05).contains(&dt) => self.coarse = Some(dt),
                None => {}
                Some(p) => {
                    let n = (dt / p).round();
                    if (1.0..=4.0).contains(&n) && (dt - n * p).abs() < 0.2 * p {
                        self.sum_dt += dt;
                        self.sum_n += n;
                        self.rejects = 0;
                        if self.sum_n > Self::MAX_REFRESHES {
                            self.sum_dt /= 2.0;
                            self.sum_n /= 2.0;
                        }
                        self.coarse = Some(if self.sum_n >= 10.0 {
                            self.sum_dt / self.sum_n
                        } else {
                            p * 0.8 + dt / n * 0.2
                        });
                    } else {
                        self.rejects += 1;
                        if self.rejects >= Self::MAX_REJECTS || self.sum_n < 10.0 {
                            // Começou num quadro duplo ou o monitor mudou.
                            *self = Self {
                                coarse: (0.004..0.05).contains(&dt).then_some(dt),
                                ..Self::default()
                            };
                        }
                    }
                }
            }
        }
        self.last = Some(returned);
    }

    /// `None` até somar `MIN_REFRESHES` refreshes.
    pub(super) fn period(&self) -> Option<std::time::Duration> {
        (self.sum_n >= Self::MIN_REFRESHES)
            .then(|| std::time::Duration::from_secs_f64(self.sum_dt / self.sum_n))
    }
}
