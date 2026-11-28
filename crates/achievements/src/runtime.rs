//! Runtime em tempo real de uma partida com RetroAchievements:
//! - coordena a sequência `login_with_token` -> `load_game` exigida pelo `rcheevos`;
//! - dirige `do_frame` / `idle` / `reset` sem duplicar código entre `reemu-core-host`
//!   e `emu-session::LocalCore`;
//! - anexa e destaca os bytes de progresso do `rcheevos` no rodapé dos save states
//!   sem quebrar cores que validam `size == retro_serialize_size()`.

use crate::client::{
    is_incompatible_game_id, AchievementClient, AchievementEntry, AchievementEvent, AsyncOutcome,
    GameSummary, LeaderboardInfo, OutboundHttpRequest, RaError,
};
use crate::ffi;
use crate::memory::LibretroMemory;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, Instant};

const PROGRESS_MAGIC: &[u8; 8] = b"RCHV0001";
const PROGRESS_FOOTER_LEN: usize = 12;
const RICH_PRESENCE_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
const RICH_PRESENCE_SAMPLE_FRAMES: u32 = 300;

/// Anexa o estado de contadores/priming do `rcheevos` ao final do save state do core.
pub fn pack_state_with_progress(mut core_state: Vec<u8>, progress: Option<&[u8]>) -> Vec<u8> {
    let Some(prog) = progress.filter(|p| !p.is_empty()) else {
        return core_state;
    };
    let Ok(len_u32) = u32::try_from(prog.len()) else {
        return core_state;
    };
    core_state.reserve(prog.len() + PROGRESS_FOOTER_LEN);
    core_state.extend_from_slice(prog);
    core_state.extend_from_slice(&len_u32.to_le_bytes());
    core_state.extend_from_slice(PROGRESS_MAGIC);
    core_state
}

/// Separa `(bytes_do_core, progresso_rcheevos)` de um save state. Se não houver
/// rodapé `RCHV0001`, devolve `(data, None)` intacto.
pub fn unpack_state_with_progress(data: &[u8]) -> (&[u8], Option<&[u8]>) {
    if data.len() < PROGRESS_FOOTER_LEN {
        return (data, None);
    }
    let magic_start = data.len() - 8;
    if &data[magic_start..] != PROGRESS_MAGIC {
        return (data, None);
    }
    let len_start = magic_start - 4;
    let prog_len = u32::from_le_bytes([
        data[len_start],
        data[len_start + 1],
        data[len_start + 2],
        data[len_start + 3],
    ]) as usize;
    if prog_len > len_start {
        return (data, None);
    }
    let core_end = len_start - prog_len;
    (&data[..core_end], Some(&data[core_end..len_start]))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaSessionConfig {
    pub console_id: u32,
    pub rom_path: String,
    pub rom_hash: Option<String>,
    pub username: String,
    pub token: String,
    pub hardcore: bool,
    pub unofficial: bool,
    pub encore: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaLoadedSnapshot {
    pub game_id: u32,
    pub title: String,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    #[serde(default)]
    pub hardcore_downgraded: bool,
    #[serde(default)]
    pub incompatible_version: bool,
    pub summary: GameSummary,
    pub achievements: Vec<AchievementEntry>,
    #[serde(default)]
    pub leaderboards: Vec<LeaderboardInfo>,
    #[serde(default)]
    pub rich_presence: Option<String>,
}

#[derive(Debug, Default)]
pub struct RuntimeDrain {
    pub http_requests: Vec<OutboundHttpRequest>,
    pub loaded: Option<RaLoadedSnapshot>,
    pub events: Vec<AchievementEvent>,
    pub warnings: Vec<String>,
    pub reset_core: bool,
}

enum Phase {
    LoggingIn {
        console_id: u32,
        rom_path: String,
        rom_hash: Option<String>,
    },
    LoadingGame,
    Active,
    Inactive,
}

pub struct LiveRuntime {
    client: AchievementClient,
    phase: Phase,
    console_id: u32,
    has_valid_memory: bool,
    hardcore: bool,
    unofficial: bool,
    last_rich_presence: Option<String>,
    last_rp_sample: Option<Instant>,
    rp_frames_since_sample: u32,
}

impl LiveRuntime {
    pub fn start(
        cfg: RaSessionConfig,
        memory: LibretroMemory,
    ) -> Result<(Self, RuntimeDrain), RaError> {
        let has_valid_memory = memory.has_valid_region();
        let console_id = cfg.console_id;
        let mut client = AchievementClient::new();
        client.set_hardcore(cfg.hardcore);
        client.set_unofficial(cfg.unofficial);
        client.set_encore(cfg.encore);
        client.attach_libretro_memory(memory);
        client.begin_login_with_token(&cfg.username, &cfg.token)?;

        let mut rt = Self {
            client,
            phase: Phase::LoggingIn {
                console_id: cfg.console_id,
                rom_path: cfg.rom_path,
                rom_hash: cfg.rom_hash,
            },
            console_id,
            has_valid_memory,
            hardcore: cfg.hardcore,
            unofficial: cfg.unofficial,
            last_rich_presence: None,
            last_rp_sample: None,
            rp_frames_since_sample: 0,
        };
        let drain = rt.collect_drain();
        Ok((rt, drain))
    }

    pub fn console_id(&self) -> u32 {
        self.console_id
    }

    pub fn has_valid_memory(&self) -> bool {
        self.has_valid_memory
    }

    /// Atualiza o mapa de memória em tempo de execução (por exemplo, quando
    /// cores como `dolphin_libretro` publicam `SET_MEMORY_MAPS` no primeiro
    /// `retro_run` após `retro_load_game`).
    pub fn update_memory(&mut self, memory: LibretroMemory) {
        self.has_valid_memory = memory.has_valid_region();
        self.client.attach_libretro_memory(memory);
    }

    #[cfg(test)]
    fn start_with_test_memory(
        cfg: RaSessionConfig,
        memory: Vec<u8>,
    ) -> Result<(Self, RuntimeDrain), RaError> {
        let console_id = cfg.console_id;
        let mut client = AchievementClient::new();
        client.set_hardcore(cfg.hardcore);
        client.set_unofficial(cfg.unofficial);
        client.set_encore(cfg.encore);
        client.attach_test_memory(memory);
        client.begin_login_with_token(&cfg.username, &cfg.token)?;

        let mut rt = Self {
            client,
            phase: Phase::LoggingIn {
                console_id: cfg.console_id,
                rom_path: cfg.rom_path,
                rom_hash: cfg.rom_hash,
            },
            console_id,
            has_valid_memory: true,
            hardcore: cfg.hardcore,
            unofficial: cfg.unofficial,
            last_rich_presence: None,
            last_rp_sample: None,
            rp_frames_since_sample: 0,
        };
        let drain = rt.collect_drain();
        Ok((rt, drain))
    }

    #[cfg(test)]
    fn write_test_memory(&mut self, offset: usize, bytes: &[u8]) {
        self.client.write_test_memory(offset, bytes);
    }

    pub fn on_http_response(
        &mut self,
        request_id: u64,
        status_code: i32,
        body: &[u8],
    ) -> RuntimeDrain {
        self.client
            .complete_http_request(request_id, status_code, body);
        self.collect_drain()
    }

    pub fn do_frame(&mut self) -> RuntimeDrain {
        if matches!(self.phase, Phase::Active) {
            self.client.do_frame();
            self.rp_frames_since_sample = self.rp_frames_since_sample.saturating_add(1);
            let mut drain = self.collect_drain();
            self.maybe_sample_rich_presence(&mut drain.events);
            drain
        } else {
            RuntimeDrain::default()
        }
    }

    pub fn idle(&mut self) -> RuntimeDrain {
        if !matches!(self.phase, Phase::Inactive) {
            self.client.idle();
            let mut drain = self.collect_drain();
            if matches!(self.phase, Phase::Active) {
                self.maybe_sample_rich_presence(&mut drain.events);
            }
            drain
        } else {
            RuntimeDrain::default()
        }
    }

    fn current_rich_presence(&self) -> Option<String> {
        if self.client.has_rich_presence() {
            self.client.rich_presence()
        } else {
            None
        }
    }

    fn maybe_sample_rich_presence(&mut self, events: &mut Vec<AchievementEvent>) {
        let due_by_time = self
            .last_rp_sample
            .is_none_or(|t| t.elapsed() >= RICH_PRESENCE_SAMPLE_INTERVAL);
        let due_by_frames = self.rp_frames_since_sample >= RICH_PRESENCE_SAMPLE_FRAMES;
        if !due_by_time && !due_by_frames {
            return;
        }
        self.last_rp_sample = Some(Instant::now());
        self.rp_frames_since_sample = 0;
        let next = self.current_rich_presence();
        if next != self.last_rich_presence {
            self.last_rich_presence = next.clone();
            events.push(AchievementEvent::RichPresenceChanged(next));
        }
    }

    pub fn pack_save_state(&self, core_state: Vec<u8>) -> Vec<u8> {
        if !matches!(self.phase, Phase::Active) || self.hardcore {
            return core_state;
        }
        let progress = self.client.serialize_progress();
        pack_state_with_progress(core_state, progress.as_deref())
    }

    pub fn restore_progress(&mut self, progress: Option<&[u8]>) {
        if !matches!(self.phase, Phase::Active) || self.hardcore {
            return;
        }
        if let Some(bytes) = progress {
            let _ = self.client.deserialize_progress(bytes);
        }
    }

    fn collect_drain(&mut self) -> RuntimeDrain {
        let mut drain = RuntimeDrain::default();

        for outcome in self.client.take_outcomes() {
            match outcome {
                AsyncOutcome::Login(Ok(_)) => {
                    if let Phase::LoggingIn {
                        console_id,
                        rom_path,
                        rom_hash,
                    } = std::mem::replace(&mut self.phase, Phase::LoadingGame)
                    {
                        let begin = match rom_hash {
                            Some(hash) => self.client.begin_load_game_by_hash(&hash),
                            None => self.client.begin_identify_and_load_game(
                                console_id,
                                Path::new(&rom_path),
                                None,
                            ),
                        };
                        if let Err(e) = begin {
                            self.phase = Phase::Inactive;
                            drain.warnings.push(format!("RetroAchievements: {e}"));
                        }
                    }
                }
                AsyncOutcome::Login(Err(e)) => {
                    self.phase = Phase::Inactive;
                    drain
                        .warnings
                        .push(format!("RetroAchievements (login): {e}"));
                }
                AsyncOutcome::GameLoad(Ok(game)) => {
                    self.phase = Phase::Active;
                    let warnings = self.client.runtime_warnings();
                    let incompatible_version =
                        is_incompatible_game_id(game.id) || warnings.unsupported_version;
                    let hardcore_downgraded = self.hardcore && warnings.unknown_emulator;
                    let effective_hardcore =
                        self.hardcore && !hardcore_downgraded && !incompatible_version;
                    self.hardcore = effective_hardcore;
                    // Os memrefs do Rich Presence so sao populados no 1o `do_frame`,
                    // entao agendamos a 1a amostragem para logo apos o 1o quadro ativo.
                    self.last_rich_presence = None;
                    self.last_rp_sample = None;
                    self.rp_frames_since_sample = RICH_PRESENCE_SAMPLE_FRAMES;
                    drain.loaded = Some(RaLoadedSnapshot {
                        game_id: game.id,
                        title: game.title,
                        badge_url: game.badge_url,
                        hardcore: effective_hardcore,
                        hardcore_downgraded,
                        incompatible_version,
                        summary: game.summary,
                        achievements: self.client.achievements(self.unofficial),
                        leaderboards: if incompatible_version {
                            Vec::new()
                        } else {
                            self.client.leaderboards()
                        },
                        rich_presence: None,
                    });
                }
                AsyncOutcome::GameLoad(Err(RaError::Api { code, .. }))
                    if code == ffi::RC_NO_GAME_LOADED =>
                {
                    // Jogo sem conjunto cadastrado no RetroAchievements: caso comum.
                    self.phase = Phase::Inactive;
                }
                AsyncOutcome::GameLoad(Err(e)) => {
                    self.phase = Phase::Inactive;
                    drain
                        .warnings
                        .push(format!("RetroAchievements (jogo): {e}"));
                }
            }
        }

        for ev in self.client.take_events() {
            if matches!(ev, AchievementEvent::ResetRequested) {
                drain.reset_core = true;
                self.client.reset();
                self.rp_frames_since_sample = RICH_PRESENCE_SAMPLE_FRAMES;
            }
            drain.events.push(ev);
        }

        drain.http_requests = self.client.take_http_requests();
        drain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_state_progress_round_trips_and_leaves_plain_states_untouched() {
        let plain = vec![1u8, 2, 3, 4, 5];
        assert_eq!(unpack_state_with_progress(&plain), (&plain[..], None));

        let prog = b"rc_prog_payload";
        let packed = pack_state_with_progress(plain.clone(), Some(prog));
        assert!(packed.len() > plain.len());
        let (core_slice, prog_slice) = unpack_state_with_progress(&packed);
        assert_eq!(core_slice, &plain[..]);
        assert_eq!(prog_slice, Some(&prog[..]));
    }

    #[test]
    fn live_runtime_sequences_login_then_load_and_packs_progress() {
        let cfg = RaSessionConfig {
            console_id: crate::console::RC_CONSOLE_SUPER_NINTENDO,
            rom_path: "dummy.sfc".into(),
            rom_hash: Some("0123456789abcdef0123456789abcdef".into()),
            username: "edward".into(),
            token: "tok_123".into(),
            hardcore: false,
            unofficial: false,
            encore: false,
        };
        let (mut rt, mut drain) =
            LiveRuntime::start_with_test_memory(cfg, vec![0u8; 4096]).unwrap();
        assert_eq!(drain.http_requests.len(), 1);
        assert!(drain.http_requests[0]
            .post_data
            .as_deref()
            .unwrap_or_default()
            .contains("r=login2"));

        while !drain.http_requests.is_empty() {
            let batch = std::mem::take(&mut drain.http_requests);
            for req in batch {
                let body = req.post_data.as_deref().unwrap_or(&req.url);
                let reply: &[u8] = if body.contains("r=login2") {
                    br#"{"Success":true,"User":"edward","Token":"tok_123","Score":100,"SoftcoreScore":50,"Messages":0}"#
                } else if body.contains("r=gameid") {
                    br#"{"Success":true,"GameID":999}"#
                } else if body.contains("r=achievementsets") {
                    br#"{"Success":true,"GameId":999,"Title":"Runtime Test","ConsoleId":3,"ImageIconUrl":"https://media.retroachievements.org/Images/001234.png","RichPresenceGameId":999,"RichPresencePatch":"","Sets":[{"AchievementSetId":1111,"GameId":999,"Title":null,"Type":"core","ImageIconUrl":"https://media.retroachievements.org/Images/001234.png","Achievements":[{"ID":501,"MemAddr":"0xH0010=42","Title":"First Step","Description":"Reach 42","Points":10,"Author":"tester","Modified":1700000000,"Created":1700000000,"BadgeName":"12345","Flags":3,"Type":"progression"}],"Leaderboards":[]}]}"#
                } else if body.contains("r=startsession") {
                    br#"{"Success":true,"HardcoreUnlocks":[],"Unlocks":[],"ServerNow":1700000100}"#
                } else {
                    br#"{"Success":true}"#
                };
                let step = rt.on_http_response(req.id, 200, reply);
                if step.loaded.is_some() {
                    drain.loaded = step.loaded;
                }
                drain.events.extend(step.events);
                drain.http_requests.extend(step.http_requests);
            }
        }

        let loaded = drain.loaded.expect("game loaded");
        assert_eq!(loaded.game_id, 999);
        assert_eq!(loaded.achievements.len(), 1);

        let packed = rt.pack_save_state(vec![9, 8, 7]);
        let (core_bytes, prog) = unpack_state_with_progress(&packed);
        assert_eq!(core_bytes, &[9, 8, 7]);
        assert!(prog.is_some());
        rt.restore_progress(prog);
    }

    #[test]
    fn live_runtime_samples_and_emits_rich_presence_changes() {
        let cfg = RaSessionConfig {
            console_id: crate::console::RC_CONSOLE_SUPER_NINTENDO,
            rom_path: "dummy.sfc".into(),
            rom_hash: Some("0123456789abcdef0123456789abcdef".into()),
            username: "edward".into(),
            token: "tok_123".into(),
            hardcore: true,
            unofficial: false,
            encore: false,
        };
        let mut init_mem = vec![0u8; 4096];
        init_mem[0x0040] = 1;
        init_mem[0x0041] = 5;
        let (mut rt, mut drain) = LiveRuntime::start_with_test_memory(cfg, init_mem).unwrap();

        while !drain.http_requests.is_empty() {
            let batch = std::mem::take(&mut drain.http_requests);
            for req in batch {
                let body = req.post_data.as_deref().unwrap_or(&req.url);
                let reply: &[u8] = if body.contains("r=login2") {
                    br#"{"Success":true,"User":"edward","Token":"tok_123","Score":100,"SoftcoreScore":50,"Messages":0}"#
                } else if body.contains("r=gameid") {
                    br#"{"Success":true,"GameID":999}"#
                } else if body.contains("r=achievementsets") {
                    br#"{"Success":true,"GameId":999,"Title":"RP Runtime Test","ConsoleId":3,"ImageIconUrl":"https://media.retroachievements.org/Images/001234.png","RichPresenceGameId":999,"RichPresencePatch":"Format:Rings\nFormatType=VALUE\n\nLookup:Zone\n1=Green Hill\n2=Marble Zone\n\nDisplay:\n@Zone(0xH0040) | @Rings(0xH0041) rings","Sets":[{"AchievementSetId":1111,"GameId":999,"Title":null,"Type":"core","ImageIconUrl":"https://media.retroachievements.org/Images/001234.png","Achievements":[],"Leaderboards":[]}]}"#
                } else if body.contains("r=startsession") {
                    br#"{"Success":true,"HardcoreUnlocks":[],"Unlocks":[],"ServerNow":1700000100}"#
                } else {
                    br#"{"Success":true}"#
                };
                let step = rt.on_http_response(req.id, 200, reply);
                if step.loaded.is_some() {
                    drain.loaded = step.loaded;
                }
                drain.events.extend(step.events);
                drain.http_requests.extend(step.http_requests);
            }
        }

        let loaded = drain.loaded.expect("game loaded");
        assert_eq!(loaded.game_id, 999);

        // No 1o `do_frame`, os memrefs sao lidos e o Rich Presence inicial e emitido.
        let first_frame = rt.do_frame();
        assert_eq!(
            first_frame.events,
            vec![AchievementEvent::RichPresenceChanged(Some(
                "Green Hill | 5 rings".into()
            ))]
        );

        // Muda a RAM para Zona 2 com 42 aneis e roda 300 quadros (~5s a 60fps).
        rt.write_test_memory(0x0040, &[2, 42]);
        let mut rp_events = Vec::new();
        for _ in 0..RICH_PRESENCE_SAMPLE_FRAMES {
            let step = rt.do_frame();
            rp_events.extend(step.events);
        }
        assert_eq!(
            rp_events,
            vec![AchievementEvent::RichPresenceChanged(Some(
                "Marble Zone | 42 rings".into()
            ))]
        );
    }
}
