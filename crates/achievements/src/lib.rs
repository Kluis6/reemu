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
    AchievementClient, AchievementEntry, AchievementEvent, AchievementKind, AsyncOutcome,
    GameSummary, LoadedGame, LoggedInUser, OutboundHttpRequest, RaError,
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
}
