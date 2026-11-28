//! Crate `achievements`: integração do ReEmu com RetroAchievements sobre o
//! `rcheevos` `v12.5.0` oficial (licença MIT, ver `THIRD_PARTY_NOTICES.md`).
//!
//! Compartilhado entre `reemu-core-host` (avaliação por quadro na memória do
//! core) e `reemu-desktop` (login trocando senha por `connect_token`, consulta
//! de conquistas da página do jogo e hash oficial de ROMs).

mod client;
pub mod console;
pub mod disc_io;
pub mod ffi;
mod memory;
mod runtime;

pub use client::{
    is_incompatible_game_id, is_warning_achievement_id, AchievementClient, AchievementEntry,
    AchievementEvent, AchievementInfo, AchievementKind, AsyncOutcome, GameSummary,
    LeaderboardFormat, LeaderboardInfo, LeaderboardRankInfo, LeaderboardState, LoadedGame,
    LoggedInUser, OutboundHttpRequest, RaError, RaRuntimeWarnings, RA_INCOMPATIBLE_GAME_ID_BASE,
    RC_CLIENT_ACHIEVEMENT_WARNING_ID,
};
pub use console::{
    console_id_for_core, console_id_for_system, console_name, hash_buffer, hash_file,
    is_disc_console,
};
pub use disc_io::{hash_path_for_console, is_disc_like_path, resolve_console_and_hash};
pub use memory::{LibretroMemory, MemoryDescriptorInput};
pub use runtime::{
    pack_state_with_progress, unpack_state_with_progress, LiveRuntime, RaLoadedSnapshot,
    RaSessionConfig, RuntimeDrain,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_systems_and_hashes_rom_buffer() {
        let snes = console_id_for_system("snes").expect("snes console id");
        assert_eq!(snes, console::RC_CONSOLE_SUPER_NINTENDO);
        assert_eq!(console_name(snes), "Super Nintendo Entertainment System");
        assert_eq!(
            console_id_for_system("gamecube"),
            Some(console::RC_CONSOLE_GAMECUBE)
        );
        assert_eq!(console_id_for_system("chip8"), None);

        let rom = vec![0x42u8; 32 * 1024];
        let hash = hash_buffer(snes, &rom).expect("hash generated");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn login_game_load_and_frame_trigger_work_with_mock_server() {
        let mut client = AchievementClient::new();
        client.set_hardcore(true);
        assert!(client.user_agent_clause().starts_with("rcheevos/"));

        client
            .begin_login_with_token("edward", "tok_test_123")
            .unwrap();
        let reqs = client.take_http_requests();
        assert_eq!(reqs.len(), 1);
        let post = reqs[0].post_data.as_deref().unwrap_or_default();
        assert!(post.contains("r=login2"));
        client.complete_http_request(
            reqs[0].id,
            200,
            br#"{"Success":true,"User":"edward","Token":"tok_test_123","Score":1500,"SoftcoreScore":200,"Messages":0}"#,
        );

        let outcomes = client.take_outcomes();
        assert_eq!(
            outcomes,
            vec![AsyncOutcome::Login(Ok(LoggedInUser {
                username: "edward".into(),
                display_name: "edward".into(),
                token: "tok_test_123".into(),
                score: 1500,
                score_softcore: 200,
                avatar_url: Some("https://media.retroachievements.org/UserPic/edward.png".into()),
            }))]
        );

        // Memória simulada de 4 KB: conquista dispara quando 0x0010 == 0x2A.
        client.attach_test_memory(vec![0u8; 4096]);
        client
            .begin_load_game_by_hash("0123456789abcdef0123456789abcdef")
            .unwrap();

        // Fluxo do rcheevos: gameid / patch + startsession.
        loop {
            let pending = client.take_http_requests();
            if pending.is_empty() {
                break;
            }
            for req in pending {
                let body = req.post_data.as_deref().unwrap_or(&req.url);
                if body.contains("r=gameid") {
                    client.complete_http_request(req.id, 200, br#"{"Success":true,"GameID":999}"#);
                } else if body.contains("r=patch")
                    || body.contains("r=gamesets")
                    || body.contains("r=achievementsets")
                {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"GameId":999,"Title":"ReEmu Test Game","ConsoleId":3,"ImageIconUrl":"https://media.retroachievements.org/Images/001234.png","RichPresenceGameId":999,"RichPresencePatch":"","Sets":[{"AchievementSetId":1111,"GameId":999,"Title":null,"Type":"core","ImageIconUrl":"https://media.retroachievements.org/Images/001234.png","Achievements":[{"ID":501,"MemAddr":"0xH0010=42","Title":"First Step","Description":"Reach 42","Points":10,"Author":"tester","Modified":1700000000,"Created":1700000000,"BadgeName":"12345","Flags":3,"Type":"progression"}],"Leaderboards":[]}]}"#,
                    );
                } else if body.contains("r=startsession") {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"HardcoreUnlocks":[],"Unlocks":[],"ServerNow":1700000100}"#,
                    );
                } else if body.contains("r=awardachievement") {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"AchievementID":501,"AchievementsRemaining":0,"Score":1510,"SoftcoreScore":200}"#,
                    );
                } else {
                    client.complete_http_request(req.id, 200, br#"{"Success":true}"#);
                }
            }
        }

        let load_outcomes = client.take_outcomes();
        assert!(
            matches!(load_outcomes.as_slice(), [AsyncOutcome::GameLoad(Ok(_))]),
            "load_outcomes={load_outcomes:?}"
        );
        assert!(client.is_game_loaded());
        let list = client.achievements(false);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "First Step");
        assert!(!list[0].unlocked);

        // Quadro 1: condição falsa.
        client.do_frame();
        assert!(client.take_events().is_empty());

        // Quadro 2: 0x0010 recebe 42 -> conquista dispara!
        client.write_test_memory(0x0010, &[42]);
        client.do_frame();
        let events = client.take_events();
        assert!(
            events.iter().any(|e| matches!(
                e,
                AchievementEvent::AchievementTriggered {
                    id: 501,
                    hardcore: true,
                    ..
                }
            )),
            "esperava AchievementTriggered, veio {events:?}"
        );

        // Serialização de progresso para save states.
        let progress = client.serialize_progress().expect("progress bytes");
        assert!(client.deserialize_progress(&progress));
    }

    #[test]
    fn filters_synthetic_warning_achievements_and_surfaces_runtime_warnings() {
        let mut client = AchievementClient::new();
        client.set_hardcore(true);
        client
            .begin_login_with_token("edward", "tok_test_123")
            .unwrap();
        let reqs = client.take_http_requests();
        client.complete_http_request(
            reqs[0].id,
            200,
            br#"{"Success":true,"User":"edward","Token":"tok_test_123","Score":100,"SoftcoreScore":50,"Messages":0}"#,
        );
        let _ = client.take_outcomes();

        client.attach_test_memory(vec![0u8; 4096]);
        client
            .begin_load_game_by_hash("faebcb2ef1f3831b2fc1dbd39d36517c")
            .unwrap();

        loop {
            let pending = client.take_http_requests();
            if pending.is_empty() {
                break;
            }
            for req in pending {
                let body = req.post_data.as_deref().unwrap_or(&req.url);
                if body.contains("r=gameid") {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"GameID":1100011718}"#,
                    );
                } else if body.contains("r=patch")
                    || body.contains("r=gamesets")
                    || body.contains("r=achievementsets")
                {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"GameId":1100011718,"Title":"Unsupported Game Version","ConsoleId":25,"ImageIconUrl":"https://media.retroachievements.org/Images/000001.png","RichPresenceGameId":1100011718,"RichPresencePatch":"","Sets":[{"AchievementSetId":9999,"GameId":1100011718,"Title":null,"Type":"core","ImageIconUrl":"https://media.retroachievements.org/Images/000001.png","Achievements":[{"ID":101000001,"MemAddr":"0xH0010=42","Title":"Warning: Unknown Emulator","Description":"Hardcore unlocks cannot be earned using this emulator.","Points":0,"Author":"RA","Modified":1700000000,"Created":1700000000,"BadgeName":"00000","Flags":3,"Type":"progression"},{"ID":101000002,"MemAddr":"0=1","Title":"Unsupported Game Version","Description":"This version of the game has not been tested.","Points":0,"Author":"RA","Modified":1700000000,"Created":1700000000,"BadgeName":"00000","Flags":3,"Type":"progression"}],"Leaderboards":[]}]}"#,
                    );
                } else if body.contains("r=startsession") {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"HardcoreUnlocks":[],"Unlocks":[],"ServerNow":1700000100}"#,
                    );
                } else {
                    client.complete_http_request(req.id, 200, br#"{"Success":true}"#);
                }
            }
        }

        let _ = client.take_outcomes();
        let warnings = client.runtime_warnings();
        assert!(warnings.unknown_emulator);
        assert!(warnings.unsupported_version);
        assert!(
            client.achievements(false).is_empty(),
            "avisos sintéticos não devem entrar na lista de conquistas"
        );

        // Quando `101000001` dispara no C, vira `RuntimeWarning`, nunca `AchievementTriggered`.
        client.do_frame();
        client.write_test_memory(0x0010, &[42]);
        client.do_frame();
        let events = client.take_events();
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, AchievementEvent::AchievementTriggered { .. })),
            "aviso sintético não pode disparar AchievementTriggered: {events:?}"
        );
        assert!(events.iter().any(|e| matches!(
            e,
            AchievementEvent::RuntimeWarning {
                id: 101_000_001,
                ..
            }
        )));
    }

    #[test]
    fn libretro_memory_maps_snes_wram_mirror() {
        let mut wram = vec![0u8; 0x20000];
        wram[0x0123] = 0x77;
        let descriptors = vec![MemoryDescriptorInput {
            flags: 0,
            ptr: wram.as_mut_ptr() as usize,
            offset: 0,
            start: 0x7E0000,
            select: 0,
            disconnect: 0,
            len: wram.len(),
            addrspace: Some("WRAM".into()),
        }];
        let mem = unsafe {
            LibretroMemory::init(console::RC_CONSOLE_SUPER_NINTENDO, &descriptors, |_id| {
                (std::ptr::null_mut(), 0)
            })
        };
        assert!(mem.total_size() >= 0x20000);
        let mut out = [0u8; 1];
        assert_eq!(mem.read(0x0123, &mut out), 1);
        assert_eq!(out[0], 0x77);
    }

    #[test]
    fn leaderboards_indicators_and_rich_presence_work_end_to_end() {
        let mut client = AchievementClient::new();
        client.set_hardcore(true);
        client
            .begin_login_with_token("edward", "tok_test_123")
            .unwrap();
        let reqs = client.take_http_requests();
        client.complete_http_request(
            reqs[0].id,
            200,
            br#"{"Success":true,"User":"edward","Token":"tok_test_123","Score":1500,"SoftcoreScore":200,"Messages":0}"#,
        );
        let _ = client.take_outcomes();

        // Memoria de teste:
        // - 0x0010: progresso medido (M:0xH0010=10)
        // - 0x0020 / 0x0021: desafio ativo / primed (A:0xH0020=1_T:0xH0021=1)
        // - 0x0030..0x0033: placar (STA:0xH0030=1::CAN:0xH0031=1::SUB:0xH0032=1::VAL:0xH0033)
        // - 0x0040 / 0x0041: Rich Presence (zona + moedas)
        let mut ram = vec![0u8; 4096];
        ram[0x0040] = 1;
        ram[0x0041] = 3;
        client.attach_test_memory(ram);
        client
            .begin_load_game_by_hash("fedcba9876543210fedcba9876543210")
            .unwrap();

        loop {
            let pending = client.take_http_requests();
            if pending.is_empty() {
                break;
            }
            for req in pending {
                let body = req.post_data.as_deref().unwrap_or(&req.url);
                if body.contains("r=gameid") {
                    client.complete_http_request(req.id, 200, br#"{"Success":true,"GameID":777}"#);
                } else if body.contains("r=patch")
                    || body.contains("r=gamesets")
                    || body.contains("r=achievementsets")
                {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"GameId":777,"Title":"Phase 6 Test","ConsoleId":3,"ImageIconUrl":"https://media.retroachievements.org/Images/007777.png","RichPresenceGameId":777,"RichPresencePatch":"Format:Coins\nFormatType=VALUE\n\nLookup:World\n1=World 1-1\n2=World 1-2\n\nDisplay:\n@World(0xH0040) - @Coins(0xH0041) coins","Sets":[{"AchievementSetId":2222,"GameId":777,"Title":null,"Type":"core","ImageIconUrl":"https://media.retroachievements.org/Images/007777.png","Achievements":[{"ID":601,"MemAddr":"M:0xH0010=10","Title":"Collector","Description":"Collect 10 orbs","Points":15,"Author":"tester","Modified":1700000000,"Created":1700000000,"BadgeName":"60101","Flags":3,"Type":"progression"},{"ID":602,"MemAddr":"0xH0020=1_T:0xH0021=1","Title":"No Hit Boss","Description":"Defeat the boss while primed","Points":25,"Author":"tester","Modified":1700000000,"Created":1700000000,"BadgeName":"60202","Flags":3,"Type":"missable"}],"Leaderboards":[{"ID":9001,"Title":"Stage 1 Speedrun","Description":"Clear Stage 1 fast","Mem":"STA:0xH0030=1::CAN:0xH0031=1::SUB:0xH0032=1::VAL:0xH0033","Format":"SCORE","LowerIsBetter":false,"Hidden":false}]}]}"#,
                    );
                } else if body.contains("r=startsession") {
                    client.complete_http_request(
                        req.id,
                        200,
                        br#"{"Success":true,"HardcoreUnlocks":[],"Unlocks":[],"ServerNow":1700000100}"#,
                    );
                } else {
                    client.complete_http_request(req.id, 200, br#"{"Success":true}"#);
                }
            }
        }

        let _ = client.take_outcomes();
        assert!(client.has_leaderboards());
        assert!(client.has_rich_presence());

        let lbs = client.leaderboards();
        assert_eq!(lbs.len(), 1);
        assert_eq!(lbs[0].id, 9001);
        assert_eq!(lbs[0].title, "Stage 1 Speedrun");
        assert_eq!(lbs[0].state, LeaderboardState::Active);
        assert_eq!(lbs[0].format, LeaderboardFormat::Score);
        assert!(!lbs[0].lower_is_better);

        // Quadro inicial: popula memrefs base (incluindo Rich Presence).
        client.do_frame();
        let _ = client.take_events();
        assert_eq!(
            client.rich_presence().as_deref(),
            Some("World 1-1 - 3 coins")
        );

        // 1) Progresso medido (3/10 -> 30%) + Desafio primed.
        client.write_test_memory(0x0010, &[3]);
        client.write_test_memory(0x0020, &[1]);
        client.do_frame();
        let evs = client.take_events();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                AchievementEvent::ProgressIndicatorShow {
                    id: 601,
                    measured_progress,
                    badge_name,
                    ..
                } if measured_progress == "3/10" && badge_name == "60101"
            )),
            "esperava ProgressIndicatorShow 3/10, veio {evs:?}"
        );
        assert!(
            evs.iter().any(|e| matches!(
                e,
                AchievementEvent::ChallengeIndicatorShow {
                    id: 602,
                    points: 25,
                    badge_name,
                    ..
                } if badge_name == "60202"
            )),
            "esperava ChallengeIndicatorShow 602, veio {evs:?}"
        );

        let achs = client.achievements(false);
        let collector = achs.iter().find(|a| a.id == 601).expect("ach 601");
        assert_eq!(collector.measured_progress.as_deref(), Some("3/10"));
        assert!((collector.measured_percent - 30.0).abs() < 0.1);

        // Atualiza progresso para 6/10 e cancela estado primed do desafio.
        client.write_test_memory(0x0010, &[6]);
        client.write_test_memory(0x0020, &[0]);
        client.do_frame();
        let evs = client.take_events();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                AchievementEvent::ProgressIndicatorUpdate {
                    id: 601,
                    measured_progress,
                    ..
                } if measured_progress == "6/10"
            )),
            "esperava ProgressIndicatorUpdate 6/10, veio {evs:?}"
        );
        assert!(
            evs.iter()
                .any(|e| matches!(e, AchievementEvent::ChallengeIndicatorHide { id: 602 })),
            "esperava ChallengeIndicatorHide 602, veio {evs:?}"
        );

        // 2) Placar: inicia, atualiza tracker, falha, reinicia e submete com ranking!
        client.write_test_memory(0x0030, &[1]);
        client.write_test_memory(0x0033, &[50]);
        client.do_frame();
        let evs = client.take_events();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                AchievementEvent::LeaderboardStarted { id: 9001, tracker_text: Some(txt), .. }
                    if txt.contains("50")
            )),
            "esperava LeaderboardStarted, veio {evs:?}"
        );
        assert!(
            evs.iter().any(|e| matches!(
                e,
                AchievementEvent::LeaderboardTrackerUpdate { display, .. } if display.contains("50")
            )),
            "esperava LeaderboardTrackerUpdate inicial, veio {evs:?}"
        );
        assert_eq!(client.leaderboards()[0].state, LeaderboardState::Tracking);

        // Atualiza valor do tracker para 125.
        client.write_test_memory(0x0033, &[125]);
        client.do_frame();
        let evs = client.take_events();
        assert!(
            evs.iter().any(|e| matches!(
                e,
                AchievementEvent::LeaderboardTrackerUpdate { display, .. } if display.contains("125")
            )),
            "esperava LeaderboardTrackerUpdate 125, veio {evs:?}"
        );

        // Cancela tentativa do placar -> LeaderboardFailed + LeaderboardTrackerHide.
        client.write_test_memory(0x0030, &[0]);
        client.write_test_memory(0x0031, &[1]);
        client.do_frame();
        let evs = client.take_events();
        assert!(
            evs.iter()
                .any(|e| matches!(e, AchievementEvent::LeaderboardFailed { id: 9001, .. })),
            "esperava LeaderboardFailed, veio {evs:?}"
        );
        assert!(
            evs.iter()
                .any(|e| matches!(e, AchievementEvent::LeaderboardTrackerHide { .. })),
            "esperava LeaderboardTrackerHide, veio {evs:?}"
        );

        // Nova tentativa e envio ao servidor com resposta de scoreboard:
        // em `lboard.c`, apos `CANCELED` precisa de 1 quadro com `STA=0` para rearmar `ACTIVE`.
        client.write_test_memory(0x0031, &[0]);
        client.do_frame();
        let _ = client.take_events();

        client.write_test_memory(0x0030, &[1]);
        client.write_test_memory(0x0033, &[200]);
        client.do_frame();
        let _ = client.take_events();

        client.write_test_memory(0x0032, &[1]);
        client.do_frame();
        let submit_evs = client.take_events();
        assert!(
            submit_evs.iter().any(|e| matches!(
                e,
                AchievementEvent::LeaderboardSubmitted {
                    id: 9001,
                    formatted_score,
                    rank_info: None,
                    ..
                } if formatted_score.contains("200")
            )),
            "esperava LeaderboardSubmitted local, veio {submit_evs:?}"
        );

        let lb_reqs = client.take_http_requests();
        assert_eq!(lb_reqs.len(), 1);
        assert!(lb_reqs[0]
            .post_data
            .as_deref()
            .unwrap_or_default()
            .contains("r=submitlbentry"));
        client.complete_http_request(
            lb_reqs[0].id,
            200,
            br#"{"Success":true,"Response":{"Score":200,"BestScore":250,"RankInfo":{"Rank":4,"NumEntries":"88"},"TopEntries":[{"User":"champ","Rank":1,"Score":300}]}}"#,
        );
        let sb_evs = client.take_events();
        assert!(
            sb_evs.iter().any(|e| matches!(
                e,
                AchievementEvent::LeaderboardSubmitted {
                    id: 9001,
                    formatted_score,
                    rank_info: Some(LeaderboardRankInfo {
                        best_score,
                        new_rank: 4,
                        num_entries: 88,
                    }),
                    ..
                } if formatted_score.contains("200") && best_score.contains("250")
            )),
            "esperava LeaderboardSubmitted com rank_info, veio {sb_evs:?}"
        );

        // 3) Rich Presence muda quando a RAM do jogo muda no proximo quadro.
        client.write_test_memory(0x0040, &[2, 19]);
        client.do_frame();
        let _ = client.take_events();
        assert_eq!(
            client.rich_presence().as_deref(),
            Some("World 1-2 - 19 coins")
        );
    }
}
