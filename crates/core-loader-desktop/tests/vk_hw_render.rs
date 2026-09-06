//! HW render Vulkan (etapa 12) ponta a ponta contra o core de teste oficial
//! do libretro (`libretro-samples/video/vulkan/vk_rendering` — triângulo
//! girando).
//!
//! Compile o core antes:  `scripts/build-vk-test-core.sh`
//! Rode:  `cargo test -p core-loader-desktop --test vk_hw_render -- --ignored`
//!
//! `#[ignore]` porque precisa de uma ICD Vulkan (GPU ou lavapipe). Pra forçar
//! o rasterizador de software:
//! `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json cargo test ...`
//!
//! O que isto prova (critério de pronto da fase A):
//!   - o core declara `RETRO_HW_CONTEXT_VULKAN` e nós aceitamos;
//!   - `setup_vk_context` cria `VkInstance`/`VkDevice` e publica a
//!     `retro_hw_render_interface_vulkan`;
//!   - o `context_reset` do core acha a interface em `GET_HW_RENDER_INTERFACE`
//!     e monta os recursos dele (senão ele loga "Failed to get HW rendering
//!     interface!" e nunca entrega frame);
//!   - a cada `retro_run` o core chama `wait_sync_index`/`get_sync_index`,
//!     entrega `set_image` + `set_command_buffers`, e nós submetemos na
//!     `VkQueue` sem erro de validação.

use core_loader_desktop::DesktopCoreLoader;
use domain::core_loader::{CoreId, LoadedCore, RenderBackend};
use domain::frame_source::FrameSource;
use std::path::PathBuf;

/// `target/vk-test-core/testvulkan_libretro.so` (ou `$REEMU_VK_TEST_CORE`).
fn test_core_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("REEMU_VK_TEST_CORE") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .to_path_buf();
    let p = root.join("target/vk-test-core/testvulkan_libretro.so");
    p.is_file().then_some(p)
}

#[test]
#[ignore = "precisa de ICD Vulkan + scripts/build-vk-test-core.sh"]
fn vulkan_test_core_negotiates_and_delivers_frames() {
    let Some(core_path) = test_core_path() else {
        panic!(
            "core de teste Vulkan não encontrado — rode `scripts/build-vk-test-core.sh` \
             (ou aponte $REEMU_VK_TEST_CORE)"
        );
    };

    // O core é `SET_SUPPORT_NO_GAME` e ignora o conteúdo, mas o loader lê o
    // arquivo (need_fullpath = false) — então precisa existir.
    let rom = std::env::temp_dir().join(format!("reemu-vk-test-{}.bin", std::process::id()));
    std::fs::write(&rom, b"").unwrap();

    let tmp = std::env::temp_dir();
    let ldr = DesktopCoreLoader::new(tmp.clone(), tmp.clone(), tmp);
    let mut core = ldr
        .open_core(
            &CoreId(core_path.to_string_lossy().into_owned()),
            rom.to_str().unwrap(),
        )
        .expect("carregar o core de teste Vulkan");

    assert_eq!(
        core.render_requirements().render_backend,
        RenderBackend::Vulkan,
        "o core devia ter negociado RETRO_HW_CONTEXT_VULKAN"
    );

    // Mais frames que o RING (3) — exercita a volta do fence ring e o
    // `wait_sync_index` no slot já submetido.
    const FRAMES: usize = 10;
    for _ in 0..FRAMES {
        // Fase A não entrega `Frame` (o caminho pro compositor é a fase B);
        // o que importa é rodar sem crash/erro de submissão.
        let _ = core.next_frame();
    }

    let submitted = core
        .vk_frames_submitted()
        .expect("DesktopCore devia ter a ponte Vulkan");
    assert_eq!(
        submitted, FRAMES as u64,
        "o core devia ter entregue um frame por `retro_run` \
         (set_image + set_command_buffers submetidos)"
    );

    let _ = std::fs::remove_file(&rom);
}
