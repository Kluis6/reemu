use super::*;
use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, SoftwarePixelFormat};

#[test]
fn curated_wire_ids_resolve_and_are_unique() {
    let mut ids: Vec<&str> = CURATED.iter().map(|c| c.id).collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n, "id curado duplicado");
    assert!(curated_by_wire("curated:xbr").is_some());
    assert!(curated_by_wire("curated:desconhecido").is_none());
    assert!(curated_by_wire("xbr").is_none(), "exige o prefixo curated:");
    assert!(CURATED.iter().all(|c| c.relpath.ends_with(".slangp")));
}

#[test]
fn curated_without_pack_errors_with_hint() {
    // SHADER_ROOT não setada neste processo de teste → sem pacote.
    let e = match build_specs("curated:ntsc") {
        Ok(_) => panic!("deveria falhar sem o pacote"),
        Err(e) => e,
    };
    assert!(e.contains("pacote de shaders"), "mensagem: {e}");
}

#[test]
fn mip_level_count_matches_floor_log2_plus_one() {
    assert_eq!(mip_level_count(1, 1), 1);
    assert_eq!(mip_level_count(2, 1), 2);
    assert_eq!(mip_level_count(64, 64), 7); // 64,32,16,8,4,2,1
    assert_eq!(mip_level_count(1024, 1024), 11);
    // não-quadrado: usa o maior lado (igual ao glslang_num_miplevels).
    assert_eq!(mip_level_count(1920, 1080), mip_level_count(1920, 1920));
}

#[test]
fn downsample_rgba8_averages_2x2_blocks() {
    #[rustfmt::skip]
    let src: [u8; 16] = [
        255, 0,   0,   255, // (0,0) vermelho
        0,   255, 0,   255, // (1,0) verde
        0,   0,   255, 255, // (0,1) azul
        255, 255, 0,   255, // (1,1) amarelo
    ];
    let (out, w, h) = downsample_rgba8(&src, 2, 2);
    assert_eq!((w, h), (1, 1));
    // média inteira (truncada) de cada canal dos 4 pixels.
    assert_eq!(out, vec![127, 127, 63, 255]);
}

#[test]
fn downsample_rgba8_handles_odd_dimensions() {
    // 3×1 → 1×1: não deve estourar índice (clamp do pixel repetido).
    let src: [u8; 12] = [10, 10, 10, 255, 20, 20, 20, 255, 30, 30, 30, 255];
    let (out, w, h) = downsample_rgba8(&src, 3, 1);
    assert_eq!((w, h), (1, 1));
    assert_eq!(out.len(), 4);
}

fn grey_frame(w: u32, h: u32, val: u8) -> Frame {
    let mut data = vec![0u8; (w * h * 4) as usize];
    for px in data.chunks_mut(4) {
        px[0] = val; // B
        px[1] = val; // G
        px[2] = val; // R
        px[3] = 0xFF; // X
    }
    Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data,
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: w as f32 / h as f32,
            rotation_degrees: 0,
        },
    }
}

/// Etapa 12 §Beetle (fatia D1): o `FrameProcessor` ADOTA uma
/// `VkInstance`/`VkDevice` criados fora do wgpu (aqui, à mão com `ash` —
/// estruturalmente o que o `libretro_create_device` do Beetle devolve) e
/// roda a chain de shader nela. Prova o caminho `wgpu-hal`
/// `Instance::from_raw` → `expose_adapter` → `device_from_raw` →
/// `wgpu::…::from_hal` antes de fiar o Beetle de verdade (D2..D5).
///
/// `#[ignore]`: precisa de um ICD Vulkan (GPU ou lavapipe).
#[test]
#[ignore = "precisa de ICD Vulkan"]
fn from_adopted_vulkan_runs_the_chain() {
    use ash::vk;

    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Ok(entry) = (unsafe { ash::Entry::load() }) else {
        eprintln!("sem loader Vulkan — pulando");
        return;
    };
    let app = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_2);
    let Ok(instance) = (unsafe {
        entry.create_instance(
            &vk::InstanceCreateInfo::default().application_info(&app),
            None,
        )
    }) else {
        eprintln!("sem ICD Vulkan — pulando");
        return;
    };

    let gpus = unsafe { instance.enumerate_physical_devices() }.unwrap_or_default();
    let Some(&gpu) = gpus.first() else {
        eprintln!("sem GPU Vulkan — pulando");
        unsafe { instance.destroy_instance(None) };
        return;
    };
    let fams = unsafe { instance.get_physical_device_queue_family_properties(gpu) };
    let Some(qfi) = fams.iter().position(|q| {
        q.queue_flags
            .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
    }) else {
        eprintln!("sem queue GRAPHICS+COMPUTE — pulando");
        unsafe { instance.destroy_instance(None) };
        return;
    };
    let qfi = qfi as u32;

    let prio = [1.0f32];
    let qci = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(qfi)
        .queue_priorities(&prio)];
    let device = unsafe {
        instance.create_device(
            gpu,
            &vk::DeviceCreateInfo::default().queue_create_infos(&qci),
            None,
        )
    }
    .expect("create_device");

    let adopted = AdoptedVulkan {
        entry,
        instance,
        physical_device: gpu,
        device,
        queue_family_index: qfi,
        queue_index: 0,
        instance_api_version: vk::API_VERSION_1_2,
        instance_extensions: vec![],
        device_extensions: vec![],
        features: wgpu::Features::empty(),
    };
    let mut fp = unsafe { FrameProcessor::from_adopted_vulkan(adopted) }
        .expect("from_adopted_vulkan devolveu None");

    // readback com pipeline: 1º frame prima (pode vir None), 2º entrega.
    fp.process(&grey_frame(64, 48, 0xC0));
    let (w, h, rgba) = fp
        .process(&grey_frame(64, 48, 0xC0))
        .expect("a chain devia entregar um frame no device adotado");
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    assert!(
        rgba.chunks(4).any(|p| p[0] > 0x80 && p[2] > 0x80),
        "esperava pixel claro da chain 'plain' no device adotado"
    );
    // `fp` dropa aqui; a `VkInstance`/`VkDevice` vazam de propósito (o
    // `drop_callback` é `None` — quem seria o dono é o core; no teste o
    // processo sai logo).
}

/// Etapa 12 ponta-a-ponta (fase B): o core de teste `vk_rendering` adota o
/// `VkDevice` do compositor, renderiza o triângulo numa `VkImage`, e o
/// `FrameProcessor` a amostra com `texture_from_raw` (zero cópia) + roda a
/// chain. Confere que sai pixel colorido (o core limpa pra 0.8,0.6,0.2).
///
/// `#[ignore]`: precisa de GPU + `scripts/build-vk-test-core.sh`.
#[test]
#[ignore = "precisa de ICD Vulkan + scripts/build-vk-test-core.sh"]
fn vk_hw_render_core_frame_reaches_the_chain() {
    use core_loader_desktop::DesktopCoreLoader;
    use domain::core_loader::CoreId;
    use domain::frame_source::FrameSource;

    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let core_path = {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/vk-test-core/testvulkan_libretro.so");
        if !root.is_file() {
            eprintln!("core de teste ausente — rode scripts/build-vk-test-core.sh");
            return;
        }
        root
    };
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let Some(shared) = fp.vulkan_shared_device() else {
        eprintln!("backend wgpu não-Vulkan — pulando");
        return;
    };

    let tmp = std::env::temp_dir();
    let rom = tmp.join(format!("reemu-vkchain-{}.bin", std::process::id()));
    std::fs::write(&rom, b"").unwrap();

    let mut core = DesktopCoreLoader::new(tmp.clone(), tmp.clone(), tmp)
        .with_vulkan_shared_device(shared)
        .open_core(
            &CoreId(core_path.to_string_lossy().into_owned()),
            rom.to_str().unwrap(),
        )
        .expect("carregar o core de teste Vulkan adotando o device do wgpu");

    // Alguns frames: o 1º prima o readback com pipeline (None), depois vem.
    let mut got_color = false;
    for i in 0..8 {
        let Some(frame) = core.next_frame() else {
            continue;
        };
        if let Some((w, h, rgba)) = fp.process(&frame) {
            assert_eq!(rgba.len(), (w * h * 4) as usize);
            // fundo do core = RGB (0.8, 0.6, 0.2) ≈ (204, 153, 51).
            let bright = rgba.chunks(4).any(|p| p[0] > 20 && p[1] > 20);
            if bright {
                got_color = true;
                eprintln!("frame {i}: {w}x{h}, 1º pixel = {:?}", &rgba[..4]);
                break;
            }
        }
    }
    let _ = std::fs::remove_file(&rom);
    assert!(
        got_color,
        "a chain nunca recebeu um frame colorido do core Vulkan"
    );
}

/// Etapa 12: os handles Vulkan que exportamos pro core de HW render têm
/// que ser REAIS — reconstrói `ash::Instance`/`Device` a partir deles
/// (exatamente o que `VkContext::adopt` faz no `core-loader-desktop`) e
/// chama a API pra provar que respondem.
#[test]
fn vulkan_shared_device_handles_are_usable() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let Some(shared) = fp.vulkan_shared_device() else {
        eprintln!("backend wgpu não-Vulkan (ou queue sem GRAPHICS+COMPUTE) — pulando");
        return;
    };

    assert_ne!(shared.get_instance_proc_addr, 0);
    assert_ne!(shared.instance, 0);
    assert_ne!(shared.physical_device, 0);
    assert_ne!(shared.device, 0);
    assert_ne!(shared.queue, 0);

    // SAFETY: mesma reconstrução do `VkContext::adopt`; só lê propriedades.
    let name = unsafe {
        let gipa = std::mem::transmute::<usize, ash::vk::PFN_vkGetInstanceProcAddr>(
            shared.get_instance_proc_addr,
        );
        let static_fn = ash::StaticFn {
            get_instance_proc_addr: gipa,
        };
        let inst_handle = std::mem::transmute::<usize, ash::vk::Instance>(shared.instance);
        let instance = ash::Instance::load(&static_fn, inst_handle);
        let phd = std::mem::transmute::<usize, ash::vk::PhysicalDevice>(shared.physical_device);
        let props = instance.get_physical_device_properties(phd);
        // Provar que o VkDevice também responde: carregar a tabela dele e
        // pedir a queue declarada tem que devolver o MESMO handle.
        let dev_handle = std::mem::transmute::<usize, ash::vk::Device>(shared.device);
        let device = ash::Device::load(instance.fp_v1_0(), dev_handle);
        let q = device.get_device_queue(shared.queue_family_index, 0);
        assert_eq!(
            std::mem::transmute::<ash::vk::Queue, usize>(q),
            shared.queue,
            "a queue exportada tem que ser a (family, 0) do device"
        );
        std::ffi::CStr::from_ptr(props.device_name.as_ptr())
            .to_string_lossy()
            .into_owned()
    };
    assert!(
        !name.is_empty(),
        "vkGetPhysicalDeviceProperties devolveu nome vazio"
    );
    eprintln!(
        "device compartilhável: {name} (queue family {})",
        shared.queue_family_index
    );
}

/// Handle de teste pro lado consumidor do interop GL (`import_dmabuf`/
/// `bind_interop_input`) — embrulha um `DmabufPlaneInfo` já pronto (vindo
/// de `render_solid_rgba_to_dmabuf`, o lado produtor GL do
/// `core-loader-desktop`) na mesma interface que um `GlInteropHandle` de
/// verdade usaria.
#[cfg(target_os = "linux")]
struct TestDmabufHandle {
    slot: u32,
    plane: std::sync::Mutex<Option<domain::frame_source::DmabufPlaneInfo>>,
    sync_fd: std::sync::Mutex<Option<i32>>,
    flip: bool,
}

#[cfg(target_os = "linux")]
impl domain::frame_source::GpuTextureHandle for TestDmabufHandle {
    fn flip_y(&self) -> bool {
        self.flip
    }
    fn slot(&self) -> u32 {
        self.slot
    }
    fn take_plane(&self) -> Option<domain::frame_source::DmabufPlaneInfo> {
        self.plane.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
    fn take_sync_fd(&self) -> Option<i32> {
        self.sync_fd
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

/// Etapa 02 (backlog): valida o lado CONSUMIDOR do interop dma_buf —
/// `import_dmabuf`/`bind_interop_input` (`gpu.rs`) — contra um `dma_buf`
/// produzido de verdade pelo lado GL do `core-loader-desktop`
/// (`render_solid_rgba_to_dmabuf`, `EGL_EXT_image_dma_buf_import` + GBM),
/// não um mock. Fecha a ponta que faltava: o produtor (GL) já tinha teste
/// próprio (`core-loader-desktop::gl_context::tests::
/// interop_ring_renders_into_dmabuf_backed_texture`); este prova que o
/// wgpu do lado do compositor importa e amostra esse MESMO `dma_buf`
/// corretamente, ponta a ponta entre os dois crates.
///
/// Com a fence nativa (padrão), o produtor NÃO faz `glFinish`: manda o
/// `sync_file` e é o semáforo importado no wgpu que segura o submit — este
/// teste cobre esse caminho (e acusa se a fence não vier nesta máquina).
#[cfg(target_os = "linux")]
#[test]
#[ignore = "precisa de EGL+GBM+Vulkan em hardware real (render node DRM)"]
fn dmabuf_from_gl_producer_imports_correctly_into_wgpu() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    // negociação: o produtor aloca só com o que este device importa
    core_loader_desktop::set_dmabuf_import_modifiers(fp.dmabuf_import_modifiers());
    let (plane, sync_fd) =
        core_loader_desktop::render_solid_rgba_to_dmabuf([220, 40, 10, 255], 64, 64)
            .expect("renderizar dma_buf de teste via GL (core-loader-desktop)");
    eprintln!("fence sync_file do produtor: {sync_fd:?}");
    assert!(
        fp.interop_ok,
        "device wgpu sem VULKAN_EXTERNAL_MEMORY_DMA_BUF — não dá pra validar interop aqui"
    );
    eprintln!("semáforo sync_fd no device: {}", fp.sync_fd.is_some());

    let frame = Frame {
        origin: FrameOrigin::HardwareTexture(Box::new(TestDmabufHandle {
            slot: 0,
            plane: std::sync::Mutex::new(Some(plane)),
            sync_fd: std::sync::Mutex::new(sync_fd),
            flip: false,
        })),
        metadata: FrameMetadata {
            native_width: 64,
            native_height: 64,
            aspect_ratio: 1.0,
            rotation_degrees: 0,
        },
    };

    assert!(
        fp.process(&frame).is_none(),
        "1º process deve primar o pipeline (None)"
    );
    let (w, h, data) = fp.process(&frame).expect("2º process entrega");
    assert_eq!((w, h), (64, 64));
    assert_eq!(data.len(), 64 * 64 * 4);
    // `plain` (default) é passthrough — a cor tem que sair igual à que
    // foi renderizada no dma_buf do lado GL (R,G,B,A na mesma ordem de
    // memória do fourcc ABGR8888 usado pelo produtor).
    assert!(
        data[0].abs_diff(220) <= 2 && data[1].abs_diff(40) <= 2 && data[2].abs_diff(10) <= 2,
        "esperava ~[220,40,10,..], veio {:?}",
        &data[0..4]
    );
}

/// O readback com pipeline prima 1 frame e depois entrega o frame ANTERIOR
/// (atraso de exatamente 1 frame). `plain` = passthrough, então a cor sai
/// igual à que entrou 1 frame antes.
/// `process_packed` (o que o canvas usa) = cabeçalho `[w][h]` + os MESMOS
/// pixels do `process`. Largura 50 → 200 bytes/linha contra 256 com padding:
/// um erro na retirada do padding apareceria aqui.
#[test]
fn process_packed_matches_process() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let (Some(mut a), Some(mut b)) = (FrameProcessor::new(), FrameProcessor::new()) else {
        eprintln!("sem adapter wgpu — pulando teste de readback");
        return;
    };
    let frame = || grey_frame(50, 30, 0x7A);
    assert!(a.process(&frame()).is_none());
    assert!(b.process_packed(&frame()).is_none());
    let (w, h, plain) = a.process(&frame()).expect("2º process entrega");
    let packed = b
        .process_packed(&frame())
        .expect("2º process_packed entrega");
    assert_eq!((w, h), (50, 30));
    assert_eq!(&packed[0..4], &50u32.to_le_bytes());
    assert_eq!(&packed[4..8], &30u32.to_le_bytes());
    assert_eq!(&packed[8..], plain);
}

#[test]
fn pipelined_readback_has_one_frame_delay() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando teste de readback");
        return;
    };

    assert!(
        fp.process(&grey_frame(64, 48, 0x40)).is_none(),
        "1º process deve primar o pipeline (None)"
    );

    let (w, h, d2) = fp
        .process(&grey_frame(64, 48, 0xC0))
        .expect("2º process entrega");
    assert_eq!((w, h), (64, 48));
    assert_eq!(d2.len(), 64 * 48 * 4);
    assert!(
        d2[0].abs_diff(0x40) <= 2,
        "esperava ~0x40, veio {:#x}",
        d2[0]
    );

    let (_, _, d3) = fp
        .process(&grey_frame(64, 48, 0x10))
        .expect("3º process entrega");
    assert!(
        d3[0].abs_diff(0xC0) <= 2,
        "esperava ~0xC0, veio {:#x}",
        d3[0]
    );
}

/// Preset slang de 2 passes: o 2º amostra `Source` + `Original` +
/// `OriginalHistory1` + o feedback do 1º (via alias). Exercita a BGL
/// dinâmica, `resolve_tex_view`, o ring de history e a cópia de feedback.
#[test]
fn multipass_slang_with_history_and_feedback_runs() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let dir = std::env::temp_dir().join("reemu_gpu_phase2");
    std::fs::create_dir_all(&dir).unwrap();
    let pass0 = dir.join("p0.slang");
    let pass1 = dir.join("p1.slang");
    let slangp = dir.join("chain.slangp");
    let vs = concat!(
        "#pragma stage vertex\n",
        "layout(location=0) in vec4 Position; layout(location=1) in vec2 TexCoord;\n",
        "layout(location=0) out vec2 vUV;\n",
        "layout(std140, set=0, binding=0) uniform UBO { mat4 MVP; } g;\n",
        "void main(){ gl_Position = g.MVP * Position; vUV = TexCoord; }\n",
    );
    std::fs::write(
        &pass0,
        [
            "#version 450\n#pragma name First\n",
            vs,
            "#pragma stage fragment\nlayout(location=0) in vec2 vUV;\n",
            "layout(location=0) out vec4 c;\n",
            "layout(set=0,binding=2) uniform sampler2D Source;\n",
            "void main(){ c = texture(Source, vUV); }\n",
        ]
        .concat(),
    )
    .unwrap();
    std::fs::write(
        &pass1,
        [
            "#version 450\n",
            vs,
            "#pragma stage fragment\nlayout(location=0) in vec2 vUV;\n",
            "layout(location=0) out vec4 c;\n",
            "layout(set=0,binding=2) uniform sampler2D Source;\n",
            "layout(set=0,binding=3) uniform sampler2D Original;\n",
            "layout(set=0,binding=4) uniform sampler2D OriginalHistory1;\n",
            "layout(set=0,binding=5) uniform sampler2D FirstFeedback;\n",
            "void main(){ c = 0.25*(texture(Source,vUV)+texture(Original,vUV)",
            "+texture(OriginalHistory1,vUV)+texture(FirstFeedback,vUV)); }\n",
        ]
        .concat(),
    )
    .unwrap();
    std::fs::write(
        &slangp,
        "shaders = 2\nshader0 = p0.slang\nalias0 = First\nfeedback_pass0 = true\nshader1 = p1.slang\n",
    )
    .unwrap();

    fp.set_preset(slangp.to_str().unwrap())
        .expect("preset multi-passe deve montar");
    assert_eq!(fp.passes.len(), 2);
    assert_eq!(fp.history_depth, 2, "OriginalHistory1 ⇒ 2 slots");
    assert!(fp.passes[0].feedback, "feedback_pass0 + FirstFeedback");
    assert!(
        fp.passes[0].feedback_target.is_none(),
        "alocado sob demanda"
    );

    fp.process(&grey_frame(32, 32, 0x20));
    let (w, h, d) = fp
        .process(&grey_frame(32, 32, 0x80))
        .expect("2º process entrega um frame");
    assert_eq!((w, h), (32, 32));
    assert_eq!(d.len(), 32 * 32 * 4);
    assert!(
        fp.passes[0].feedback_target.is_some(),
        "feedback alocado ao rodar"
    );
    let _ = fp.process(&grey_frame(32, 32, 0x40));

    std::fs::remove_dir_all(&dir).ok();
}

/// `mipmap_input1 = true` ⇒ o passe 0 precisa gerar a cadeia de mip da
/// própria saída (fase 2, `generate_mips`/`ensure_target`). Prova de
/// ponta a ponta, não só "não crasha": o passe 0 desenha um xadrez 8×8
/// (64×64px, célula=8px) e o passe 1 lê `textureLod(Source, vUV, 6.0)`
/// — o nível 6 de uma cadeia de 64px é 1×1, então TODO pixel da saída
/// tem que ler essa média única. Matemago: 8px de célula ÷ 2³ = célula
/// de 1px no nível 3 (ainda xadrez exato), e um box de 2×2 nesse nível
/// cobre sempre 2 pretos + 2 brancos ⇒ níveis 4..6 já saem uniformes em
/// 127 (exato, sem viés de arredondamento). Se a cadeia NÃO foi gerada,
/// o clamp de LOD cai no nível 0 e a saída reproduz o xadrez cru — alto
/// contraste, nada uniforme.
#[test]
fn mipmap_input_generates_full_mip_chain_for_next_pass() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let dir = std::env::temp_dir().join("reemu_gpu_mipmap_test");
    std::fs::create_dir_all(&dir).unwrap();
    let checker = dir.join("checker.slang");
    let sample_mip = dir.join("sample_mip.slang");
    let slangp = dir.join("mip.slangp");
    let vs = concat!(
        "#pragma stage vertex\n",
        "layout(location=0) in vec4 Position; layout(location=1) in vec2 TexCoord;\n",
        "layout(location=0) out vec2 vUV;\n",
        "layout(std140, set=0, binding=0) uniform UBO { mat4 MVP; } g;\n",
        "void main(){ gl_Position = g.MVP * Position; vUV = TexCoord; }\n",
    );
    std::fs::write(
        &checker,
        [
            "#version 450\n#pragma name Checker\n",
            vs,
            "#pragma stage fragment\nlayout(location=0) in vec2 vUV;\n",
            "layout(location=0) out vec4 c;\n",
            "layout(set=0,binding=2) uniform sampler2D Source;\n",
            "void main(){ float cb = mod(floor(vUV.x*8.0)+floor(vUV.y*8.0), 2.0); c = vec4(cb,cb,cb,1.0); }\n",
        ]
        .concat(),
    )
    .unwrap();
    std::fs::write(
        &sample_mip,
        [
            "#version 450\n",
            vs,
            "#pragma stage fragment\nlayout(location=0) in vec2 vUV;\n",
            "layout(location=0) out vec4 c;\n",
            "layout(set=0,binding=2) uniform sampler2D Source;\n",
            "void main(){ c = textureLod(Source, vUV, 6.0); }\n",
        ]
        .concat(),
    )
    .unwrap();
    std::fs::write(
        &slangp,
        "shaders = 2\nshader0 = checker.slang\nshader1 = sample_mip.slang\nmipmap_input1 = true\n",
    )
    .unwrap();

    fp.set_preset(slangp.to_str().unwrap())
        .expect("preset com mipmap_input deve montar");
    assert!(fp.passes[1].mipmap_input, "mipmap_input1 parseado");

    // readback com pipeline tem 1 frame de atraso (ver
    // `pipelined_readback_has_one_frame_delay`) — o 1º `process` prima.
    fp.process(&grey_frame(64, 64, 0x80));
    let (w, h, out) = fp
        .process(&grey_frame(64, 64, 0x80))
        .expect("frame processado");
    assert_eq!((w, h), (64, 64));

    // R=G=B por construção do shader — qualquer um dos 3 primeiros
    // bytes do pixel serve, sem depender da ordem exata de canal.
    let vals: Vec<u8> = out.chunks(4).map(|px| px[0]).collect();
    let (min, max) = (*vals.iter().min().unwrap(), *vals.iter().max().unwrap());
    assert!(
        max - min <= 10,
        "saída devia ser uniforme (mip 1×1 amostrado em toda parte) — \
         min={min} max={max}: cadeia de mip não foi gerada?"
    );
    assert!(
        (100..=155).contains(&min),
        "valor uniforme longe da média esperada (~127) do xadrez 50/50 — min={min}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

fn collect_slangp(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_slangp(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("slangp") {
            out.push(p);
        }
    }
}

/// Roda `build_specs` (parse + glslang→SPIR-V→naga por passe) em cada
/// `.slangp` de uma pasta e tabula. Ignorado por padrão — é validação de
/// campo, não CI. Rode com:
///   REEMU_SHADER_DIR=~/.local/share/com.reemu.desktop/shaders/slang-shaders \
///   cargo test -p reemu-desktop --lib field_validate_real_presets -- --ignored --nocapture
#[test]
#[ignore]
fn field_validate_real_presets() {
    let dir = std::env::var("REEMU_SHADER_DIR").unwrap_or_else(|_| {
        format!(
            "{}/.local/share/com.reemu.desktop/shaders/slang-shaders",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let root = std::path::Path::new(&dir);
    let mut presets = Vec::new();
    collect_slangp(root, &mut presets);
    presets.sort();
    assert!(!presets.is_empty(), "nenhum .slangp em {dir}");
    eprintln!("validando {} presets em {dir}\n", presets.len());

    let limit: usize = std::env::var("REEMU_SHADER_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);

    let (mut ok, mut err) = (0usize, 0usize);
    // tally por categoria (1º componente do caminho; `bezel/X` usa 2 —
    // os mega-pacotes são projetos distintos)
    let mut by_dir: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    let cat = |rel: &str| -> String {
        let parts: Vec<&str> = rel.split('/').collect();
        match parts.as_slice() {
            ["bezel", sub, ..] => format!("bezel/{sub}"),
            [top, _, ..] => top.to_string(),
            _ => "(raiz)".to_string(),
        }
    };
    let mut by_reason: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    // erro completo do 1º caso de cada grupo (a chave é truncada)
    let mut full: std::collections::BTreeMap<String, String> = Default::default();
    // `REEMU_SHADER_OK_LIST=/caminho` → grava os `.slangp` que compilam
    // (lista curada pro pacote de shaders).
    let mut ok_list: Vec<String> = Vec::new();
    for p in presets.iter().take(limit) {
        let rel = p.strip_prefix(root).unwrap_or(p).display().to_string();
        let entry = by_dir.entry(cat(&rel)).or_default();
        match build_specs(p.to_str().unwrap()) {
            Ok(b) => {
                ok += 1;
                entry.0 += 1;
                ok_list.push(rel.clone());
                let _ = b;
            }
            Err(e) => {
                err += 1;
                entry.1 += 1;
                // agrupa pela causa: tira os caminhos absolutos (ruído) e
                // corta na 1ª linha, senão cada erro do glslang vira um grupo.
                let cleaned = e.replace(&dir, "…").replace('\n', " ");
                let key: String = cleaned.chars().take(140).collect();
                full.entry(key.clone()).or_insert(cleaned);
                by_reason.entry(key).or_default().push(rel);
            }
        }
    }
    if let Ok(path) = std::env::var("REEMU_SHADER_OK_LIST") {
        ok_list.sort();
        let _ = std::fs::write(&path, ok_list.join("\n") + "\n");
        eprintln!("lista de {} presets ok gravada em {path}", ok_list.len());
    }
    eprintln!("\n=== {ok} ok · {err} falharam ===\n");
    let mut reasons: Vec<_> = by_reason.into_iter().collect();
    reasons.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for (reason, files) in &reasons {
        eprintln!("[{}×] {}", files.len(), reason);
        if std::env::var_os("REEMU_SHADER_FULL_ERR").is_some() {
            if let Some(f) = full.get(reason) {
                eprintln!("  ↳ {f}");
            }
        }
        for f in files.iter().take(4) {
            eprintln!("      {f}");
        }
        if files.len() > 4 {
            eprintln!("      … +{}", files.len() - 4);
        }
    }
    eprintln!("\n=== por categoria (ok/total) ===");
    let mut cats: Vec<_> = by_dir.into_iter().collect();
    cats.sort_by_key(|(_, (o, e))| std::cmp::Reverse(o + e));
    for (c, (o, e)) in &cats {
        let t = o + e;
        eprintln!(
            "  {:>4}/{:<4} {:>5.1}%  {c}",
            o,
            t,
            100.0 * *o as f64 / t as f64
        );
    }
    eprintln!(
        "\ntaxa de sucesso: {:.1}%",
        100.0 * ok as f64 / (ok + err).max(1) as f64
    );
}

/// Regressão: shaders slang saíam de cabeça pra baixo depois que o frontend
/// virou glslang→SPIR-V→naga (o naga negava o Y de `gl_Position` por
/// padrão). Frame com a metade de CIMA branca, metade de baixo preta →
/// depois de um passthrough slang, o topo da saída tem que continuar branco.
#[test]
fn slang_passthrough_keeps_orientation() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let dir = std::env::temp_dir().join("reemu_gpu_orient");
    std::fs::create_dir_all(&dir).unwrap();
    let sp = dir.join("pass.slangp");
    let sl = dir.join("pass.slang");
    std::fs::write(
        &sl,
        concat!(
            "#version 450\n",
            "#pragma stage vertex\n",
            "layout(location=0) in vec4 Position; layout(location=1) in vec2 TexCoord;\n",
            "layout(location=0) out vec2 vUV;\n",
            "layout(std140, set=0, binding=0) uniform UBO { mat4 MVP; } g;\n",
            "void main(){ gl_Position = g.MVP * Position; vUV = TexCoord; }\n",
            "#pragma stage fragment\n",
            "layout(location=0) in vec2 vUV;\n",
            "layout(location=0) out vec4 c;\n",
            "layout(set=0,binding=2) uniform sampler2D Source;\n",
            "void main(){ c = texture(Source, vUV); }\n",
        ),
    )
    .unwrap();
    std::fs::write(
        &sp,
        "shaders = 1\nshader0 = pass.slang\nscale_type0 = source\nscale0 = 1.0\n",
    )
    .unwrap();
    fp.set_preset(sp.to_str().unwrap())
        .expect("preset passthrough");

    let (w, h) = (32u32, 32u32);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let v = if y < h / 2 { 0xFF } else { 0x00 };
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&[v, v, v, 0xFF]);
        }
    }
    let mk = || Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data: data.clone(),
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: 1.0,
            rotation_degrees: 0,
        },
    };
    fp.process(&mk());
    let (ow, oh, out) = fp.process(&mk()).expect("2º process entrega");
    let px = |x: u32, y: u32| out[((y * ow + x) * 4) as usize];
    assert!(
        px(ow / 2, 1) > 0xC0,
        "topo devia estar branco, veio {:#x}",
        px(ow / 2, 1)
    );
    assert!(
        px(ow / 2, oh - 2) < 0x40,
        "base devia estar preta, veio {:#x}",
        px(ow / 2, oh - 2)
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Etapa 12 B3b ponta-a-ponta: com `REEMU_HW=vulkan` + o device do
/// compositor publicado, `EmuSession` roda o core de teste `vk_rendering`
/// **in-process** (nunca sobe o `reemu-core-host`), e o `Frame` que sai
/// pela API de sempre (`take_latest_frame`) é uma `HardwareVulkanImage`
/// que a chain amostra e devolve pixel colorido.
///
/// `#[ignore]`: precisa de GPU Vulkan + `scripts/build-vk-test-core.sh`.
/// Rode isolado: `-- --ignored --test-threads=1` (mexe no env `REEMU_HW`).
#[test]
#[ignore = "precisa de ICD Vulkan + scripts/build-vk-test-core.sh"]
fn emu_session_routes_vulkan_core_in_process() {
    use emu_session::{EmuSession, SessionConfig};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let core_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target/vk-test-core/testvulkan_libretro.so");
    if !core_path.is_file() {
        eprintln!("core de teste ausente — rode scripts/build-vk-test-core.sh");
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let Some(shared) = fp.vulkan_shared_device() else {
        eprintln!("backend wgpu não-Vulkan — pulando");
        return;
    };

    // Opt-in: sem isso o `GET_PREFERRED_HW_RENDER` não devolve Vulkan e o
    // roteamento local nem é tentado.
    std::env::set_var("REEMU_HW", "vulkan");

    let tmp = std::env::temp_dir();
    let rom = tmp.join(format!("reemu-b3b-{}.bin", std::process::id()));
    std::fs::write(&rom, b"").unwrap();

    let session = EmuSession::spawn(SessionConfig::new(tmp.clone(), tmp.clone(), tmp.clone()));
    session.attach_vulkan_device(shared);
    session
        .load(
            core_path.to_str().unwrap(),
            rom.to_str().unwrap(),
            HashMap::new(),
        )
        .expect("carregar o core Vulkan pela sessão");

    assert!(
        session.debug_child_pid().is_none(),
        "core Vulkan foi pro processo filho — devia rodar in-process"
    );

    // B3b/D4: o core Vulkan local é dirigido por ESTA thread (o papel do
    // video pump), a mesma que roda a chain — `step_vk_local` faz o
    // `retro_run` e devolve o `Frame`.
    let mut got_color = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !got_color {
        let Some(frame) = session.step_vk_local() else {
            std::thread::sleep(Duration::from_millis(15));
            continue;
        };
        assert!(
            matches!(frame.origin, FrameOrigin::HardwareVulkanImage(_)),
            "esperava HardwareVulkanImage da sessão"
        );
        if let Some((w, h, rgba)) = fp.process(&frame) {
            assert_eq!(rgba.len(), (w * h * 4) as usize);
            if rgba.chunks(4).any(|p| p[0] > 20 && p[1] > 20) {
                got_color = true;
                eprintln!("B3b frame {w}x{h}, 1º pixel = {:?}", &rgba[..4]);
            }
        }
    }

    session.unload().ok();
    let _ = std::fs::remove_file(&rom);
    std::env::remove_var("REEMU_HW");
    assert!(
        got_color,
        "a chain nunca recebeu frame colorido pela sessão"
    );
}

/// Modo canvas: com moldura, o readback sai só com o jogo (tamanho da saída
/// da chain, não da moldura) e o cabeçalho leva a geração da moldura e o
/// retângulo do jogo — a moldura em si vai uma vez pelo `decoration_image`.
#[test]
fn split_decoration_sends_game_only() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando teste de readback");
        return;
    };
    let (dw, dh) = (400u32, 300u32);
    fp.set_decoration(Some((
        vec![0u8; (dw * dh * 4) as usize],
        dw,
        dh,
        Some(DecoViewport {
            x: 100.0,
            y: 50.0,
            w: 200.0,
            h: 200.0,
        }),
    )));
    let frame = || grey_frame(50, 30, 0x7A);
    assert!(
        fp.process_packed_split(&frame()).is_none(),
        "1º quadro: pipeline"
    );
    let out = fp
        .process_packed_split(&frame())
        .expect("2º quadro entrega");
    let u32_at = |i: usize| u32::from_le_bytes(out[i..i + 4].try_into().unwrap());
    let f32_at = |i: usize| f32::from_le_bytes(out[i..i + 4].try_into().unwrap());
    assert_eq!(
        (u32_at(0), u32_at(4)),
        (50, 30),
        "tamanho do jogo, não da moldura"
    );
    assert_eq!(out.len(), 36 + 50 * 30 * 4);
    assert_eq!(
        f32_at(32),
        1.0,
        "sem integer scaling a moldura não tem zoom"
    );
    let (gen, img, w, h) = fp.decoration_image().expect("moldura guardada");
    assert!(gen > 0);
    assert_eq!(u32_at(8), gen);
    assert_eq!((w, h, img.len()), (dw, dh, (dw * dh * 4) as usize));
    // janela 200×200 centrada em (200,150) de 400×300 → NDC (0, 0, 0.5, 2/3)
    assert!(f32_at(12).abs() < 1e-4 && f32_at(16).abs() < 1e-4);
    assert!((f32_at(20) - 0.5).abs() < 1e-4);
    assert!((f32_at(24) - 200.0 / 300.0).abs() < 1e-4);
    assert_eq!(
        f32_at(28),
        frame().metadata.aspect_ratio,
        "proporção do quadro"
    );
}

/// Tabuleiro 64×48 (casas de 8 px) — bordas nítidas pros filtros de
/// ampliação/nitidez terem o que processar.
fn checker_frame(w: u32, h: u32) -> Frame {
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = if ((x / 8) + (y / 8)) % 2 == 0 {
                0xE0
            } else {
                0x20
            };
            let i = ((y * w + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&[v, v, v, 0xFF]);
        }
    }
    Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data,
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: w as f32 / h as f32,
            rotation_degrees: 0,
        },
    }
}

/// Roda de verdade (GPU) os presets de ampliação/nitidez do pacote: FSR 1
/// (EASU + RCAS, AMD), só EASU, RCAS e NIS (NVIDIA). Confere o tamanho da
/// saída — passe `viewport` sem superfície amplia pra 3× o nativo — e que a
/// imagem não sai preta nem lisa. Ignorado por padrão (precisa do pacote):
///   cargo test -p reemu-desktop --lib field_render_upscalers -- --ignored --nocapture
#[test]
#[ignore]
fn field_render_upscalers() {
    let root = std::path::PathBuf::from(std::env::var("REEMU_SHADER_DIR").unwrap_or_else(|_| {
        format!(
            "{}/.local/share/com.reemu.desktop/shaders/slang-shaders",
            std::env::var("HOME").unwrap_or_default()
        )
    }));
    let cases = [
        ("edge-smoothing/fsr/fsr.slangp", 3),
        ("edge-smoothing/fsr/fsr-easu.slangp", 3),
        ("sharpen/rca_sharpen.slangp", 1),
        ("edge-smoothing/nis/nis.slangp", 3),
    ];
    for (rel, scale) in cases {
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let path = root.join(rel);
        fp.set_preset(path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let _ = fp.process(&checker_frame(64, 48));
        let (w, h, d) = fp
            .process(&checker_frame(64, 48))
            .unwrap_or_else(|| panic!("{rel}: sem quadro"));
        assert_eq!((w, h), (64 * scale, 48 * scale), "{rel}: tamanho da saída");
        let luma: Vec<u8> = d.chunks(4).map(|p| p[1]).collect();
        let (lo, hi) = (*luma.iter().min().unwrap(), *luma.iter().max().unwrap());
        eprintln!("{rel}: {w}x{h} luma {lo}..{hi}");
        assert!(
            hi > 0x80 && lo < 0x60,
            "{rel}: saída sem contraste ({lo}..{hi})"
        );
    }
}

/// Core que desenha MENOR que o máximo (flycast: buffer 853x853, quadro
/// 640x480 no canto de origem do GL): a saída tem que ser só o quadro,
/// inteiro colorido — com e sem inversão de Y. Antes o passe de flip copiava
/// o buffer inteiro e o jogo aparecia cortado num canto (2026-09-25).
#[cfg(target_os = "linux")]
#[test]
#[ignore = "precisa de EGL+GBM+Vulkan em hardware real (render node DRM)"]
fn interop_crops_frame_smaller_than_the_buffer() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    for flip in [true, false] {
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        core_loader_desktop::set_dmabuf_import_modifiers(fp.dmabuf_import_modifiers());
        let (plane, sync_fd) =
            core_loader_desktop::render_rect_rgba_to_dmabuf([30, 200, 60, 255], 64, 64, 40, 24)
                .expect("renderizar dma_buf de teste via GL");
        let handle = TestDmabufHandle {
            slot: 0,
            plane: std::sync::Mutex::new(Some(plane)),
            sync_fd: std::sync::Mutex::new(sync_fd),
            flip,
        };
        let frame = Frame {
            origin: FrameOrigin::HardwareTexture(Box::new(handle)),
            metadata: FrameMetadata {
                native_width: 40,
                native_height: 24,
                aspect_ratio: 40.0 / 24.0,
                rotation_degrees: 0,
            },
        };
        let _ = fp.process(&frame);
        let (w, h, data) = fp.process(&frame).expect("2º process entrega");
        assert_eq!((w, h), (40, 24), "saída do tamanho do quadro (flip={flip})");
        let dark = data
            .chunks(4)
            .filter(|p| p[0].abs_diff(30) > 3 || p[1].abs_diff(200) > 3)
            .count();
        assert_eq!(
            dark, 0,
            "{dark} pixels fora da cor — recorte errado (flip={flip})"
        );
    }
}

/// Core GL de verdade + ROM de verdade, pela sessão (processo filho), com o
/// quadro passando pela chain — pra reproduzir problema de interop que só
/// aparece com jogo rodando (ex.: a tela preta do parallel_n64 que deixou
/// `REEMU_GL_INTEROP` opt-in até 2026-09-25). Imprime por segundo: quadros, quantos de HW
/// (dma_buf) e quantos não-pretos.
///
/// ```text
/// cargo build -p core-host-desktop
/// REEMU_TEST_GL_CORE=parallel_n64_libretro REEMU_TEST_ROM=/caminho/jogo.zip \
///   cargo test -p reemu-desktop --lib gl_core_real_rom \
///   -- --ignored --nocapture
/// ```
#[cfg(target_os = "linux")]
#[test]
#[ignore = "core GL e ROM reais (REEMU_TEST_GL_CORE / REEMU_TEST_ROM)"]
fn gl_core_real_rom() {
    use emu_session::{EmuSession, SessionConfig};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_TEST_GL_CORE"),
        std::env::var("REEMU_TEST_ROM"),
    ) else {
        eprintln!("defina REEMU_TEST_GL_CORE e REEMU_TEST_ROM");
        return;
    };
    // Pasta de dados do app (cores, system/BIOS) em cada sistema.
    let data = if cfg!(windows) {
        std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
    } else {
        std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".local/share")
    }
    .join("com.reemu.desktop");
    let secs: u64 = std::env::var("REEMU_TEST_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("reemu-glrom-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut cfg = SessionConfig::new(data.join("cores"), data.join("system"), tmp.clone());
    // `REEMU_TEST_AUDIO=1`: som de verdade no dispositivo padrão (com
    // `REEMU_AUDIO_DEBUG=1` o sink loga fill/descartes/underrun por segundo).
    let _ = env_logger::builder().is_test(true).try_init();
    if std::env::var_os("REEMU_TEST_AUDIO").is_some() {
        cfg.audio_sink = Some(Box::new(|| {
            audio_desktop::CpalAudioSink::new(&Default::default())
                .ok()
                .map(|s| Box::new(s) as _)
        }));
    }
    let session = EmuSession::spawn(cfg);
    session.set_dmabuf_modifiers(fp.dmabuf_import_modifiers());
    // `REEMU_TEST_OPTS="chave=valor,chave=valor"`: opções de core no load.
    let opts: HashMap<String, String> = std::env::var("REEMU_TEST_OPTS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    session
        .load(&core, &rom, opts)
        .expect("carregar core GL + ROM pela sessão");
    if let Ok(keys) = std::env::var("REEMU_TEST_SHOW_OPTS") {
        let (_, values) = session.core_options();
        for k in keys.split(',') {
            eprintln!("opção {k} = {:?}", values.get(k));
        }
    }

    let (mut frames, mut hw, mut lit, mut out) = (0u32, 0u32, 0u32, 0u32);
    let mut last_dims = (0, 0);
    let start = Instant::now();
    let mut next_report = 1;
    while start.elapsed() < Duration::from_secs(secs) {
        session.wait_for_frame(Duration::from_millis(100));
        if let Some(frame) = session.take_latest_frame() {
            frames += 1;
            if matches!(frame.origin, FrameOrigin::HardwareTexture(_)) {
                hw += 1;
            }
            last_dims = (frame.metadata.native_width, frame.metadata.native_height);
            if let Some((_, _, rgba)) = fp.process(&frame) {
                out += 1;
                if rgba.chunks(4).any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16) {
                    lit += 1;
                }
            }
            session.recycle_frame(frame);
        }
        if start.elapsed().as_secs() >= next_report {
            eprintln!(
                "{next_report:>3}s: quadros {frames}, HW {hw}, saída {out}, não-pretos {lit}, último {}x{}",
                last_dims.0, last_dims.1
            );
            next_report += 1;
        }
    }
    session.unload().ok();
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(frames > 0, "nenhum quadro chegou");
    assert!(lit > 0, "todos os quadros saíram pretos");
}

/// Diagnóstico manual: compõe um quadro com a moldura de verdade e grava
/// PNG em `$REEMU_DIAG_OUT` (interop 853x853 → 640x480 e software 640x480),
/// pra comparar o alinhamento jogo × moldura. Não afirma nada.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "diagnóstico manual (REEMU_DIAG_BEZEL, REEMU_DIAG_OUT)"]
fn diag_bezel_composition() {
    let (Ok(bezel), Ok(out)) = (
        std::env::var("REEMU_DIAG_BEZEL"),
        std::env::var("REEMU_DIAG_OUT"),
    ) else {
        return;
    };
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&bezel).unwrap(),
    ));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    let (bw, bh) = (info.width, info.height);
    let vp = || {
        Some(DecoViewport {
            x: 263.0,
            y: 22.0,
            w: 1396.0,
            h: 1037.0,
        })
    };
    let preset = std::env::var("REEMU_DIAG_PRESET").ok();
    let integer = std::env::var_os("REEMU_DIAG_INTEGER").is_some();
    for (name, hw) in [("interop", true), ("software", false)] {
        let mut fp = FrameProcessor::new().expect("adapter");
        if let Some(p) = &preset {
            set_shader_root(
                std::path::PathBuf::from(std::env::var("HOME").unwrap())
                    .join(".local/share/com.reemu.desktop/shaders/slang-shaders"),
            );
            fp.set_preset(p).expect("preset");
        }
        fp.set_integer_scaling(integer);
        fp.set_decoration(Some((buf.clone(), bw, bh, vp())));
        let frame = if hw {
            core_loader_desktop::set_dmabuf_import_modifiers(fp.dmabuf_import_modifiers());
            let (plane, sync_fd) = core_loader_desktop::render_rect_rgba_to_dmabuf(
                [30, 200, 60, 255],
                853,
                853,
                640,
                480,
            )
            .unwrap();
            Frame {
                origin: FrameOrigin::HardwareTexture(Box::new(TestDmabufHandle {
                    slot: 0,
                    plane: std::sync::Mutex::new(Some(plane)),
                    sync_fd: std::sync::Mutex::new(sync_fd),
                    flip: true,
                })),
                metadata: FrameMetadata {
                    native_width: 640,
                    native_height: 480,
                    aspect_ratio: 4.0 / 3.0,
                    rotation_degrees: 0,
                },
            }
        } else {
            let mut f = grey_frame(640, 480, 0x7A);
            f.metadata.aspect_ratio = 4.0 / 3.0;
            f
        };
        let _ = fp.process(&frame);
        let (w, h, data) = fp.process(&frame).expect("saída");
        let path = format!("{out}/{name}.png");
        let mut enc = png::Encoder::new(std::fs::File::create(&path).unwrap(), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.write_header().unwrap().write_image_data(data).unwrap();
        eprintln!("{name}: {w}x{h} → {path}");
    }
}

/// Etapa 12 com core e ROM REAIS: core Vulkan in-process (device do
/// compositor, `REEMU_HW=vulkan`) dirigido por esta thread via
/// `step_vk_local`, quadro pela chain. Imprime por segundo quadros,
/// não-pretos e o tempo do passo (retro_run + submit). Rode isolado
/// (`--test-threads=1`, mexe no env).
///
/// ```text
/// REEMU_TEST_VK_CORE=flycast_libretro REEMU_TEST_ROM=/caminho/jogo.cue \
///   cargo test -p reemu-desktop --lib vk_core_real_rom -- --ignored --nocapture --test-threads=1
/// ```
#[test]
#[ignore = "core Vulkan e ROM reais (REEMU_TEST_VK_CORE / REEMU_TEST_ROM)"]
fn vk_core_real_rom() {
    use emu_session::{EmuSession, SessionConfig};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    // Rota in-process (o padrão no Windows virou o processo filho, C3c).
    if std::env::var_os("REEMU_VK_CHILD").is_none() {
        std::env::set_var("REEMU_VK_CHILD", "0");
    }
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_TEST_VK_CORE"),
        std::env::var("REEMU_TEST_ROM"),
    ) else {
        eprintln!("defina REEMU_TEST_VK_CORE e REEMU_TEST_ROM");
        return;
    };
    let _ = env_logger::builder().is_test(true).try_init();
    // Pasta de dados do app (cores, system/BIOS) em cada sistema.
    let data = if cfg!(windows) {
        std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
    } else {
        std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".local/share")
    }
    .join("com.reemu.desktop");
    let secs: u64 = std::env::var("REEMU_TEST_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);
    let mut fp = FrameProcessor::new().expect("adapter wgpu");
    let shared = fp.vulkan_shared_device().expect("backend Vulkan");
    // `REEMU_TEST_NO_FORCE=1`: como o app por padrão (sem `REEMU_HW`).
    if std::env::var_os("REEMU_TEST_NO_FORCE").is_none() {
        std::env::set_var("REEMU_HW", "vulkan");
    }
    let tmp = std::env::temp_dir().join(format!("reemu-vkrom-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let session = EmuSession::spawn(SessionConfig::new(
        data.join("cores"),
        data.join("system"),
        tmp.clone(),
    ));
    session.attach_vulkan_device(shared);
    // Negociador como no app (lib.rs): core com `create_device` (Beetle,
    // flycast) cria o device e o FrameProcessor é refeito em cima dele.
    let rebuilt: std::sync::Arc<std::sync::Mutex<Option<FrameProcessor>>> = Default::default();
    {
        let rebuilt = std::sync::Arc::clone(&rebuilt);
        session.attach_vulkan_negotiator(std::sync::Arc::new(move |neg| {
            let (fp, shared) = unsafe { FrameProcessor::from_core_negotiation(neg) }?;
            *rebuilt.lock().unwrap() = Some(fp);
            Ok(shared)
        }));
    }
    let opts: HashMap<String, String> = std::env::var("REEMU_TEST_OPTS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    // Como o video pump do app: esta thread dirige o core, então o load
    // (feito pela thread da sessão) vem rodar aqui (`on_vk_thread`) enquanto
    // ela segue chamando `step_vk_local`.
    let _ = session.step_vk_local();
    // Chamada bloqueante da sessão (load, save state, unload) numa thread à
    // parte, enquanto esta segue dirigindo o core.
    fn driving<R: Send>(session: &EmuSession, call: impl FnOnce() -> R + Send) -> R {
        std::thread::scope(|sc| {
            let h = sc.spawn(call);
            while !h.is_finished() {
                let gate = session.lock_vk_queue();
                let _ = session.step_vk_local();
                drop(gate);
                std::thread::sleep(Duration::from_millis(2));
            }
            h.join().unwrap()
        })
    }
    driving(&session, || session.load(&core, &rom, opts)).expect("carregar core Vulkan + ROM");
    if let Some(new_fp) = rebuilt.lock().unwrap().take() {
        eprintln!("device criado pelo core (negociação) — FrameProcessor refeito");
        fp = new_fp;
    }
    let in_process = session.debug_child_pid().is_none();
    eprintln!("in-process: {in_process}");

    let (mut frames, mut vk, mut lit) = (0u32, 0u32, 0u32);
    let (mut step_sum, mut step_max) = (Duration::ZERO, Duration::ZERO);
    let start = Instant::now();
    let mut next = 1;
    let mut last_dump = u64::MAX;
    let (mut saved, mut restored): (Option<Vec<u8>>, bool) = (None, false);
    let mut tick = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let t = Instant::now();
        let gate = session.lock_vk_queue();
        let frame = session.step_vk_local();
        drop(gate);
        let dt = t.elapsed();
        step_sum += dt;
        step_max = step_max.max(dt);
        if let Some(frame) = frame {
            frames += 1;
            if matches!(frame.origin, FrameOrigin::HardwareVulkanImage(_)) {
                vk += 1;
            }
            if let Some((w, h, rgba)) = fp.process(&frame) {
                if rgba.chunks(4).any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16) {
                    lit += 1;
                }
                // `REEMU_TEST_DUMP=/pasta`: grava um PNG por segundo (1º quadro)
                if let Ok(dir) = std::env::var("REEMU_TEST_DUMP") {
                    let sec = start.elapsed().as_secs();
                    if sec != last_dump {
                        last_dump = sec;
                        let path = format!("{dir}/q{sec:02}.png");
                        let mut enc =
                            png::Encoder::new(std::fs::File::create(&path).unwrap(), w, h);
                        enc.set_color(png::ColorType::Rgba);
                        enc.write_header().unwrap().write_image_data(rgba).unwrap();
                    }
                }
            }
        }
        // ~60 Hz, como o video pump
        let spent = tick.elapsed();
        if spent < Duration::from_micros(16_683) {
            std::thread::sleep(Duration::from_micros(16_683) - spent);
        }
        tick = Instant::now();
        // `REEMU_TEST_SAVESTATE=1`: salva no 6º s e restaura no 9º (carga
        // na fila compartilhada: serialize/unserialize podem submeter).
        if std::env::var_os("REEMU_TEST_SAVESTATE").is_some() {
            let el = start.elapsed().as_secs();
            if el >= 6 && saved.is_none() && !restored {
                saved = driving(&session, || session.save_state()).expect("save_state");
                eprintln!("save state: {:?} bytes", saved.as_ref().map(|b| b.len()));
            }
            if el >= 9 && !restored {
                if let Some(b) = saved.take() {
                    let ok = driving(&session, || session.restore_state(b)).expect("restore_state");
                    eprintln!("restore state: {ok}");
                }
                restored = true;
            }
        }
        if start.elapsed().as_secs() >= next {
            eprintln!(
                "{next:>3}s: quadros {frames}, Vulkan {vk}, não-pretos {lit}, passo méd {:.2} máx {:.2} ms",
                step_sum.as_secs_f64() * 1000.0 / frames.max(1) as f64,
                step_max.as_secs_f64() * 1000.0
            );
            step_max = Duration::ZERO;
            next += 1;
        }
    }
    driving(&session, || session.unload()).ok();
    std::env::remove_var("REEMU_HW");
    let _ = std::fs::remove_dir_all(&tmp);
    // `REEMU_TEST_EXPECT_CHILD=1`: o core TEM que ir pro filho (ex.: mupen
    // com GLideN64). Senão, forçando Vulkan, tem que ficar in-process.
    if std::env::var_os("REEMU_TEST_EXPECT_CHILD").is_some() {
        assert!(!in_process, "core ficou in-process — devia ir pro filho");
    } else if std::env::var_os("REEMU_TEST_NO_FORCE").is_none() {
        assert!(in_process, "core foi pro processo filho");
    }
}

/// Lista os modificadores DRM com que o device importa dma_buf RGBA8 pra
/// amostrar (negociação do `VK_EXT_image_drm_format_modifier`).
#[cfg(target_os = "linux")]
#[test]
#[ignore = "hardware real"]
fn print_dmabuf_import_modifiers() {
    let Some(fp) = FrameProcessor::new() else {
        return;
    };
    let mods = fp.dmabuf_import_modifiers();
    eprintln!("modificadores aceitos ({}):", mods.len());
    for m in &mods {
        eprintln!("  {m:#018x}");
    }
}

/// Modo canvas (Windows): a passada `scale_type = viewport` sai no tamanho
/// que a webview informou (`set_canvas_viewport`), não em 3× a nativa. Era
/// isso que fazia a máscara de CRT virar moiré (resultado reduzido no
/// canvas) e o quadro de ~11 MB pelo IPC derrubar o desempenho.
#[test]
fn canvas_viewport_sizes_viewport_passes() {
    if std::env::var_os("REEMU_NO_GPU").is_some() {
        return;
    }
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let dir = std::env::temp_dir().join("reemu_gpu_canvas_vp");
    std::fs::create_dir_all(&dir).unwrap();
    let sl = dir.join("pass.slang");
    let sp = dir.join("vp.slangp");
    std::fs::write(
        &sl,
        concat!(
            "#version 450\n",
            "#pragma stage vertex\n",
            "layout(location=0) in vec4 Position; layout(location=1) in vec2 TexCoord;\n",
            "layout(location=0) out vec2 vUV;\n",
            "layout(std140, set=0, binding=0) uniform UBO { mat4 MVP; } g;\n",
            "void main(){ gl_Position = g.MVP * Position; vUV = TexCoord; }\n",
            "#pragma stage fragment\n",
            "layout(location=0) in vec2 vUV;\n",
            "layout(location=0) out vec4 c;\n",
            "layout(set=0,binding=2) uniform sampler2D Source;\n",
            "void main(){ c = texture(Source, vUV); }\n",
        ),
    )
    .unwrap();
    std::fs::write(
        &sp,
        "shaders = 1\nshader0 = pass.slang\nscale_type0 = viewport\nscale0 = 1.0\n",
    )
    .unwrap();
    fp.set_preset(sp.to_str().unwrap())
        .expect("preset viewport");

    let frame = grey_frame(64, 48, 0x80);
    let size = |fp: &mut FrameProcessor| {
        let _ = fp.process(&frame);
        let (w, h, _) = fp.process(&frame).expect("saída");
        (w, h)
    };
    // sem viewport informado: o antigo fallback de 3× a nativa
    assert_eq!(size(&mut fp), (192, 144));
    // com o tamanho real da área do jogo na tela
    fp.set_canvas_viewport(990, 735);
    assert_eq!(size(&mut fp), (990, 735));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Tarefa A4: o período do monitor tem que sair bem mais preciso que os
/// ~0,3% de diferença entre core e monitor que o `RateMatch` casa. Amostras
/// de um monitor de 59,8 Hz com jitter de ±0,8 ms, um quadro duplo de vez
/// em quando e acquires que não bloquearam no meio.
#[test]
fn vblank_estimator_is_precise_despite_jitter_and_gaps() {
    use std::time::{Duration, Instant};
    let period = 1.0 / 59.8;
    let t0 = Instant::now();
    let at = |s: f64| t0 + Duration::from_secs_f64(s);
    let mut e = VblankEstimator::default();
    // Pseudoaleatório determinístico pro jitter.
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut jitter = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        ((seed % 1601) as f64 - 800.0) * 1e-6
    };
    let mut k = 1.0;
    for i in 0..1200 {
        // Quadro duplo a cada 97 e acquire sem bloqueio a cada 131.
        k += if i % 97 == 0 { 2.0 } else { 1.0 };
        let vblank = k * period + 0.0005 + jitter();
        if i % 131 == 0 {
            e.observe(at(vblank), at(vblank)); // não bloqueou
        } else {
            e.observe(at(vblank - 0.010), at(vblank));
        }
    }
    let got = e.period().expect("período publicado").as_secs_f64();
    let err = (got - period).abs() / period;
    assert!(
        err < 0.0005,
        "erro de {:.4}% (medido {:.4} ms)",
        err * 100.0,
        got * 1000.0
    );
}

#[test]
fn vblank_estimator_needs_blocked_samples() {
    use std::time::{Duration, Instant};
    let t0 = Instant::now();
    let mut e = VblankEstimator::default();
    // Acquire que nunca bloqueia (VRR, janela escondida): sem período.
    for i in 0..1000 {
        let t = t0 + Duration::from_millis(16 * i);
        e.observe(t, t);
    }
    assert!(e.period().is_none());
}

/// Save state com core e ROM reais, pela sessão completa (rota padrão do
/// app: processo filho pra software/GL). Mede o que importa no uso:
/// 1. pausado, salva → restaura → salva de novo: os dois estados deveriam
///    ser iguais (imprime a % de bytes iguais);
/// 2. despausado, os quadros continuam e não ficam pretos;
/// 3. fecha o jogo, abre de novo e restaura o estado da sessão anterior.
///
/// ```text
/// REEMU_TEST_CORE=snes9x2002_libretro REEMU_TEST_ROM=/caminho/jogo.smc \
///   cargo test -p reemu-desktop --lib savestate_real_rom -- --ignored --nocapture
/// ```
#[test]
#[ignore = "core e ROM reais (REEMU_TEST_CORE / REEMU_TEST_ROM)"]
fn savestate_real_rom() {
    use emu_session::{EmuSession, SessionConfig};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_TEST_CORE"),
        std::env::var("REEMU_TEST_ROM"),
    ) else {
        eprintln!("defina REEMU_TEST_CORE e REEMU_TEST_ROM");
        return;
    };
    let _ = env_logger::builder().is_test(true).try_init();
    let data = if cfg!(windows) {
        std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
    } else {
        std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".local/share")
    }
    .join("com.reemu.desktop");
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let opts: HashMap<String, String> = std::env::var("REEMU_TEST_OPTS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let tmp = std::env::temp_dir().join(format!("reemu-ssrom-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let session = EmuSession::spawn(SessionConfig::new(
        data.join("cores"),
        data.join("system"),
        tmp.clone(),
    ));
    // Roda `secs` segundos; devolve (quadros, não-pretos).
    let mut run = |secs: u64| {
        let (mut frames, mut lit) = (0u32, 0u32);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(secs) {
            session.wait_for_frame(Duration::from_millis(100));
            if let Some(frame) = session.take_latest_frame() {
                frames += 1;
                if let Some((_, _, rgba)) = fp.process(&frame) {
                    if rgba.chunks(4).any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16) {
                        lit += 1;
                    }
                }
                session.recycle_frame(frame);
            }
        }
        (frames, lit)
    };
    let same = |a: &[u8], b: &[u8]| {
        if a.len() != b.len() {
            return format!("tamanhos diferentes ({} × {})", a.len(), b.len());
        }
        let eq = a.iter().zip(b).filter(|(x, y)| x == y).count();
        format!(
            "{:.2}% dos bytes iguais",
            eq as f64 * 100.0 / a.len().max(1) as f64
        )
    };

    session.load(&core, &rom, opts.clone()).expect("carregar");
    let (f, l) = run(4);
    eprintln!("jogo: {f} quadros, {l} não-pretos");
    session.set_paused(true);
    std::thread::sleep(Duration::from_millis(200));
    let s1 = session
        .save_state()
        .expect("save_state")
        .expect("core sem save state");
    let ok = session.restore_state(s1.clone()).expect("restore_state");
    let s2 = session
        .save_state()
        .expect("save_state 2")
        .unwrap_or_default();
    eprintln!(
        "estado: {} bytes | restore {ok} | salvar→restaurar→salvar: {}",
        s1.len(),
        same(&s1, &s2)
    );
    session.set_paused(false);
    let (f, l) = run(2);
    eprintln!("depois de restaurar: {f} quadros, {l} não-pretos");
    session.unload().ok();

    // Outra sessão do mesmo jogo, restaurando o estado da anterior.
    session.load(&core, &rom, opts).expect("carregar de novo");
    // `REEMU_TEST_WAIT2`: segundos rodando antes de restaurar (padrão 1).
    run(std::env::var("REEMU_TEST_WAIT2")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1));
    let ok = session
        .restore_state(s1)
        .expect("restore_state (2ª sessão)");
    let (f, l) = run(2);
    eprintln!("2ª sessão: restore {ok} | {f} quadros, {l} não-pretos");
    session.unload().ok();
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(ok, "restaurar na 2ª sessão falhou");
    assert!(f > 0, "sem quadros depois de restaurar");
}

/// Preview de shader (Configurações › Vídeo): a cena de exemplo passa pela
/// chain no tamanho pedido, e o CRT muda a imagem em relação ao "plain".
#[test]
fn shader_preview_renders_the_sample_scene() {
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando preview de shader");
        return;
    };
    let w = crate::commands::PREVIEW_W;
    let h = crate::commands::PREVIEW_H;
    let frame = || Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data: crate::commands::sample_scene(),
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: w as f32 / h as f32,
            rotation_degrees: 0,
        },
    };
    // "plain" não escala: sai no tamanho nativo (a UI amplia).
    let (pw, ph, _) = fp.render_still(&frame(), 960, 720, 4).expect("plain");
    assert_eq!((pw, ph), (w, h));
    // O CRT desenha no viewport pedido, e a máscara/scanline muda os pixels:
    // compara com a cena ampliada por vizinho mais próximo.
    fp.set_preset("crt").expect("crt");
    let (cw, ch, crt) = fp.render_still(&frame(), 960, 720, 4).expect("crt");
    eprintln!("crt → {cw}x{ch}");
    assert!(cw > w && ch > h, "o CRT não usou o viewport: {cw}x{ch}");
    let src = crate::commands::sample_scene();
    let mut diff = 0usize;
    for y in 0..ch {
        for x in 0..cw {
            let s = (((y * h / ch) * w + x * w / cw) * 4) as usize;
            let d = ((y * cw + x) * 4) as usize;
            // fonte em B,G,R,X; saída em R,G,B,A
            if (src[s + 2] as i32 - crt[d] as i32).abs() > 24 {
                diff += 1;
            }
        }
    }
    assert!(
        diff > (cw * ch / 10) as usize,
        "o CRT quase não mudou a imagem ({diff} px)"
    );

    // A cena 3D também passa pela chain no tamanho pedido.
    let scene3d = crate::commands::sample_scene_3d();
    assert_eq!(scene3d.len(), (w * h * 4) as usize);
    let frame3d = Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data: scene3d,
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: w as f32 / h as f32,
            rotation_degrees: 0,
        },
    };
    let (w3, h3, _) = fp.render_still(&frame3d, 960, 720, 4).expect("crt 3d");
    assert_eq!((w3, h3), (960, 720));
}

/// B1 numa GPU real: identidade do device Vulkan do wgpu e as extensões Win32
/// de memória/semáforo externos. Imprime pra comparar com o
/// `wgl_interop_caps_read_this_gpu` (core-loader-desktop).
#[test]
#[ignore = "precisa de GPU"]
fn vk_interop_id_reads_this_device() {
    let Some(fp) = FrameProcessor::new() else {
        eprintln!("sem GPU");
        return;
    };
    let Some(id) = fp.vk_interop_id() else {
        eprintln!("wgpu fora do Vulkan");
        return;
    };
    eprintln!("Vulkan interop: {id:?}");
    assert_ne!(id.device_uuid, [0; 16]);
    assert_ne!(id.driver_uuid, [0; 16]);
}

/// B2 numa GPU real: com `REEMU_GL_INTEROP=1`, o device abre com as extensões
/// Win32, o anel de imagens exportáveis é alocado, cada imagem tem um handle
/// NT, e o handle se duplica pra um processo (aqui, o próprio).
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU (Vulkan com external_memory_win32)"]
fn win32_interop_ring_exports_handles() {
    // SAFETY: teste de processo único pra esta variável (`--ignored`).
    unsafe { std::env::set_var("REEMU_GL_INTEROP", "1") };
    let Some(fp) = FrameProcessor::new() else {
        eprintln!("sem GPU");
        return;
    };
    let ring = fp.export_interop_ring(3, 640, 480).expect("alocar o anel");
    assert_eq!(ring.len(), 3);
    for img in &ring {
        assert_ne!(img.handle, 0);
        assert!(img.alloc_size >= 640 * 480 * 4);
    }
    let dup = win_interop::duplicate_into(std::process::id(), ring[0].handle).expect("duplicar");
    assert_ne!(dup, 0);
    // SAFETY: handle duplicado agora, no próprio processo.
    unsafe { windows_sys::Win32::Foundation::CloseHandle(dup as _) };
    eprintln!(
        "anel: {} imagens, {} bytes cada",
        ring.len(),
        ring[0].alloc_size
    );
}

/// B3 numa GPU real: o anel Vulkan exportado (com os handles duplicados
/// pro próprio processo, como o filho os recebe) importa no GL
/// (`GL_EXT_memory_object_win32`) e cada textura vira o alvo do FBO.
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU (WGL + Vulkan com external_memory_win32)"]
fn win32_interop_ring_imports_into_gl() {
    let _ = env_logger::builder().is_test(true).try_init();
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem GPU");
        return;
    };
    let reply = fp
        .install_interop_ring(64, 48, std::process::id(), false)
        .expect("alocar e duplicar o anel");
    let px =
        core_loader_desktop::render_solid_into_interop_offer(offer_of(&reply), [255, 0, 0, 255])
            .expect("importar o anel no GL");
    assert_eq!(px.len(), 3);
    assert!(px.iter().all(|p| *p == [255, 0, 0, 255]), "{px:?}");
}

/// A resposta do provedor como o filho a recebe (`InteropOffer`).
#[cfg(windows)]
fn offer_of(reply: &emu_session::InteropReply) -> core_loader_desktop::InteropOffer {
    core_loader_desktop::InteropOffer {
        width: reply.width,
        height: reply.height,
        dedicated: reply.dedicated,
        image_flags: reply.image_flags,
        image_usage: reply.image_usage,
        images: reply
            .images
            .iter()
            .map(|i| core_loader_desktop::OfferedImage {
                memory: i.handle,
                alloc_size: i.alloc_size,
                done_sem: i.done_sem,
                free_sem: i.free_sem,
                memory_type: i.memory_type,
            })
            .collect(),
    }
}

/// B4 numa GPU real, ida e volta: o GL desenha uma cor em cada imagem e
/// sinaliza `done`; o Vulkan espera `done`, copia a imagem pra um buffer
/// (SHADER_READ_ONLY → TRANSFER_SRC → SHADER_READ_ONLY) e sinaliza `free`;
/// o GL redesenha no mesmo slot depois de esperar `free`. Confere as cores
/// lidas do lado Vulkan — prova que a memória é a mesma e que os semáforos
/// ordenam os dois lados. Submit cru na fila do wgpu: nada mais submete
/// neste teste.
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU (WGL + Vulkan com external_memory/semaphore_win32)"]
fn win32_interop_semaphores_round_trip() {
    use ash::vk;
    let _ = env_logger::builder().is_test(true).try_init();
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem GPU");
        return;
    };
    let (w, h) = (64u32, 48u32);
    let reply = fp
        .install_interop_ring(w, h, std::process::id(), false)
        .expect("alocar e duplicar o anel");
    let mut probe =
        core_loader_desktop::InteropGlProbe::new(offer_of(&reply)).expect("contexto GL");

    let hal_dev = unsafe { fp.device.as_hal::<wgpu::hal::api::Vulkan>() }.expect("Vulkan");
    let raw = hal_dev.raw_device().clone();
    let queue = hal_dev.raw_queue();
    let family = hal_dev.queue_family_index();
    let mem_props = unsafe {
        hal_dev
            .shared_instance()
            .raw_instance()
            .get_physical_device_memory_properties(hal_dev.raw_physical_device())
    };
    drop(hal_dev);
    let images: Vec<(vk::Image, vk::Semaphore, vk::Semaphore)> = fp
        .interop_ring
        .iter()
        .map(|img| {
            let t = unsafe { img.texture.as_hal::<wgpu::hal::api::Vulkan>() }.expect("Vulkan");
            (unsafe { t.raw_handle() }, img.done_sem, img.free_sem)
        })
        .collect();

    // Buffer de leitura (HOST_VISIBLE | HOST_COHERENT).
    let size = u64::from(w * h * 4);
    unsafe {
        let buf = raw
            .create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(size)
                    .usage(vk::BufferUsageFlags::TRANSFER_DST),
                None,
            )
            .unwrap();
        let req = raw.get_buffer_memory_requirements(buf);
        let want = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let ty = (0..mem_props.memory_type_count)
            .find(|&t| {
                req.memory_type_bits & (1 << t) != 0
                    && mem_props.memory_types[t as usize]
                        .property_flags
                        .contains(want)
            })
            .expect("memória visível pro host");
        let mem = raw
            .allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(req.size)
                    .memory_type_index(ty),
                None,
            )
            .unwrap();
        raw.bind_buffer_memory(buf, mem, 0).unwrap();
        let pool = raw
            .create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(family)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
            .unwrap();
        let cmd = raw
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(pool)
                    .command_buffer_count(1),
            )
            .unwrap()[0];
        let fence = raw
            .create_fence(&vk::FenceCreateInfo::default(), None)
            .unwrap();
        let range = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .level_count(1)
            .layer_count(1);

        // Lê o slot `i` do lado Vulkan: espera `done`, copia, sinaliza `free`.
        let read_slot = |i: usize| -> [u8; 4] {
            let (image, done, free) = images[i];
            raw.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())
                .unwrap();
            raw.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .unwrap();
            let to_src = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(range);
            raw.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[to_src],
            );
            let region = vk::BufferImageCopy::default()
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width: w,
                    height: h,
                    depth: 1,
                });
            raw.cmd_copy_image_to_buffer(
                cmd,
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buf,
                &[region],
            );
            let back = vk::ImageMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::empty())
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(range);
            let host = vk::BufferMemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .buffer(buf)
                .size(vk::WHOLE_SIZE);
            raw.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE | vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &[],
                &[host],
                &[back],
            );
            raw.end_command_buffer(cmd).unwrap();
            let waits = [done];
            let stages = [vk::PipelineStageFlags::TRANSFER];
            let signals = [free];
            let cmds = [cmd];
            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&waits)
                .wait_dst_stage_mask(&stages)
                .command_buffers(&cmds)
                .signal_semaphores(&signals);
            raw.queue_submit(queue, &[submit], fence).unwrap();
            raw.wait_for_fences(&[fence], true, 5_000_000_000)
                .expect("o Vulkan esperou o done do GL");
            raw.reset_fences(&[fence]).unwrap();
            let p = raw
                .map_memory(mem, 0, size, vk::MemoryMapFlags::empty())
                .unwrap() as *const u8;
            let px = [*p, *p.add(1), *p.add(2), *p.add(3)];
            raw.unmap_memory(mem);
            px
        };

        // 1ª volta: uma cor por slot.
        let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
        for (i, c) in colors.iter().enumerate() {
            assert_eq!(probe.draw(*c).unwrap(), i as u32);
            assert_eq!(read_slot(i), *c, "slot {i}, 1ª volta");
            // B5: o `free` foi submetido — o GL pode reusar o slot.
            core_loader_desktop::interop_slot_freed(i as u32);
        }
        // 2ª volta: o GL espera o `free` de cada slot antes de redesenhar.
        for i in 0..3 {
            let c = [255, 255, 0, 255];
            assert_eq!(probe.draw(c).unwrap(), i as u32);
            assert_eq!(read_slot(i), c, "slot {i}, 2ª volta");
            core_loader_desktop::interop_slot_freed(i as u32);
        }

        drop(probe);
        raw.destroy_fence(fence, None);
        raw.destroy_command_pool(pool, None);
        raw.destroy_buffer(buf, None);
        raw.free_memory(mem, None);
    }
}

/// B2 de ponta a ponta numa GPU real: o core-fake com um "jogo" `HW` pede GL
/// 3.3 no processo filho; o filho confere o B1, pede o anel
/// (`InteropRequest`), o app aloca 3 imagens exportáveis, duplica os handles
/// pro filho e manda o `InteropImages`. Confere que o pedido chegou e que o
/// anel foi alocado e duplicado (o filho registra o recebimento no log).
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU (WGL + Vulkan com external_memory_win32)"]
fn win32_interop_ring_reaches_the_core_process() {
    use emu_session::{EmuSession, SessionConfig};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    // SAFETY: teste de processo único pra esta variável (`--ignored`); o
    // filho herda o ambiente e também vê o pedido.
    unsafe { std::env::set_var("REEMU_GL_INTEROP", "1") };
    let _ = env_logger::builder().is_test(true).try_init();
    let Some(fp) = FrameProcessor::new() else {
        eprintln!("sem GPU");
        return;
    };
    let vk = fp.vk_interop_id();
    let fp = Arc::new(Mutex::new(fp));
    let tmp = std::env::temp_dir().join(format!("reemu-interop-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let rom = tmp.join("hw.bin");
    std::fs::write(&rom, b"HW conteudo do jogo").unwrap();

    let session = EmuSession::spawn(SessionConfig::new(tmp.clone(), tmp.clone(), tmp.clone()));
    session.set_vk_interop(vk);
    let rings = Arc::new(AtomicUsize::new(0));
    let (fp2, rings2) = (fp.clone(), rings.clone());
    session.set_interop_provider(Arc::new(move |w, h, pid, vulkan| {
        let reply = fp2
            .lock()
            .unwrap()
            .install_interop_ring(w, h, pid, vulkan)?;
        eprintln!(
            "anel pedido pelo filho (pid {pid}): {} imagens {w}x{h}, handles {:?}",
            reply.images.len(),
            reply.images.iter().map(|i| i.handle).collect::<Vec<_>>()
        );
        rings2.fetch_add(1, Ordering::SeqCst);
        Ok(reply)
    }));
    session
        .load(
            core_loader_desktop::testcore_path(),
            rom.to_str().unwrap(),
            Default::default(),
        )
        .expect("carregar o core-fake com HW render");
    let start = Instant::now();
    while rings.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(10) {
        session.wait_for_frame(Duration::from_millis(100));
        if let Some(f) = session.take_latest_frame() {
            session.recycle_frame(f);
        }
    }
    let _ = session.unload();
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(
        rings.load(Ordering::SeqCst),
        1,
        "o filho devia pedir o anel uma vez"
    );
}

/// Roda `core` com `rom` pelo `EmuSession` com o interop GL → Vulkan do
/// Windows ligado, como a bomba de vídeo do app: pega o quadro, passa pela
/// chain (`process`), recicla e devolve os slots (`flush_interop` →
/// `release_interop_slot`). Para em `frames` quadros pelo anel ou em
/// `timeout`. `check(quadro_do_anel, rgba_lido)` vê cada quadro lido (o
/// readback do `process` é atrasado em um quadro — o 1º vem `None`).
#[cfg(windows)]
fn drive_interop_session(
    core: &str,
    rom: &std::path::Path,
    frames: u32,
    timeout: std::time::Duration,
    mut check: impl FnMut(bool, Option<(u32, u32, &[u8])>),
) -> (u32, u32) {
    use domain::frame_source::FrameOrigin;
    use emu_session::{EmuSession, SessionConfig};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    // Ligado por padrão; um `REEMU_GL_INTEROP=0` de fora mede o readback
    // (B6). O filho herda o ambiente.
    let _ = env_logger::builder().is_test(true).try_init();
    let fp = FrameProcessor::new().expect("GPU");
    let vk = fp.vk_interop_id();
    let fp = Arc::new(Mutex::new(fp));
    let tmp = std::env::temp_dir().join(format!("reemu-b5-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let session = EmuSession::spawn(SessionConfig::new(tmp.clone(), tmp.clone(), tmp.clone()));
    session.set_vk_interop(vk);
    let fp2 = fp.clone();
    session.set_interop_provider(Arc::new(move |w, h, pid, vulkan| {
        fp2.lock().unwrap().install_interop_ring(w, h, pid, vulkan)
    }));
    // `REEMU_TEST_GL_OPTS="chave=valor;..."`: opções do core (ex.: a
    // resolução interna, pra medir o readback em 1920×1440 como na fase A).
    let opts: std::collections::HashMap<String, String> = std::env::var("REEMU_TEST_GL_OPTS")
        .unwrap_or_default()
        .split(';')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    session
        .load(core, rom.to_str().unwrap(), opts)
        .expect("carregar o core");

    let (mut ring_frames, mut readback_frames) = (0u32, 0u32);
    // Tempo do app por quadro: entrada da chain + passes + leitura do teste.
    let mut app_time = Duration::ZERO;
    let start = Instant::now();
    while ring_frames < frames && start.elapsed() < timeout {
        session.wait_for_frame(Duration::from_millis(100));
        if let Some(frame) = session.take_latest_frame() {
            let is_ring = matches!(frame.origin, FrameOrigin::HardwareTexture(_));
            if is_ring {
                ring_frames += 1;
            } else {
                readback_frames += 1;
            }
            {
                let mut g = fp.lock().unwrap();
                let t = Instant::now();
                let out = g.process(&frame);
                app_time += t.elapsed();
                check(is_ring, out);
            }
            session.recycle_frame(frame);
        }
        let dropped = session.take_dropped_interop_slots();
        let freed = fp.lock().unwrap().flush_interop(&dropped);
        for slot in freed {
            session.release_interop_slot(slot);
        }
    }
    let _ = session.unload();
    let _ = std::fs::remove_dir_all(&tmp);
    eprintln!(
        "quadros: {ring_frames} pelo anel, {readback_frames} pelo readback em {:.1} s;          app {:.3} ms/quadro",
        start.elapsed().as_secs_f32(),
        app_time.as_secs_f64() * 1000.0 / f64::from((ring_frames + readback_frames).max(1))
    );
    (ring_frames, readback_frames)
}

/// B5 de ponta a ponta na GPU real: o core-fake (`HWD`) desenha magenta via
/// GL no FBO do frontend; o quadro chega ao app só como índice do anel
/// (`HardwareTexture`), a chain amostra a imagem compartilhada (esperando o
/// `done`), o `flush_interop` submete o `free` e o filho é avisado. Confere
/// a cor e que os quadros continuam vindo pelo anel (nada trava).
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU (WGL + Vulkan com external_memory/semaphore_win32)"]
fn win32_interop_frames_flow_through_the_ring() {
    // SAFETY: teste `--ignored` de processo único pra esta variável.
    unsafe { std::env::set_var("REEMU_GL_INTEROP", "1") };
    let tmp = std::env::temp_dir().join(format!("reemu-b5-rom-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let rom = tmp.join("hwd.bin");
    std::fs::write(&rom, b"HWD desenha via GL").unwrap();
    let mut checked = 0u32;
    let (ring, _) = drive_interop_session(
        core_loader_desktop::testcore_path(),
        &rom,
        120,
        std::time::Duration::from_secs(20),
        |_, px| {
            // O core-fake pinta todo quadro de magenta, pelo anel ou não.
            if let Some((_, _, px)) = px {
                checked += 1;
                assert_eq!(&px[..4], &[255, 0, 255, 255], "quadro {checked}");
            }
        },
    );
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(ring >= 120, "o anel parou de entregar quadros");
    assert!(checked >= 100, "a chain devia ter lido os quadros");
}

/// B5 com um core GL de verdade: `REEMU_TEST_GL_CORE` (DLL do core) e
/// `REEMU_TEST_GL_ROM`. Roda ~15 s e confere que os quadros vêm pelo anel e
/// que a imagem não é toda preta.
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU e de REEMU_TEST_GL_CORE/REEMU_TEST_GL_ROM"]
fn win32_interop_real_gl_core() {
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_TEST_GL_CORE"),
        std::env::var("REEMU_TEST_GL_ROM"),
    ) else {
        eprintln!("sem REEMU_TEST_GL_CORE/REEMU_TEST_GL_ROM");
        return;
    };
    let mut lit = 0u32;
    // `REEMU_TEST_GL_DUMP=arquivo.png` salva o último quadro do anel com
    // imagem — pra conferir a orientação a olho.
    let dump = std::env::var("REEMU_TEST_GL_DUMP").ok();
    let mut last: Option<(u32, u32, Vec<u8>)> = None;
    let (ring, readback) = drive_interop_session(
        &core,
        std::path::Path::new(&rom),
        900,
        std::time::Duration::from_secs(15),
        |is_ring, px| {
            if let (true, Some((w, h, px))) = (is_ring, px) {
                if px
                    .chunks_exact(4)
                    .any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16)
                {
                    lit += 1;
                    if dump.is_some() {
                        last = Some((w, h, px.to_vec()));
                    }
                }
            }
        },
    );
    eprintln!("quadros do anel com imagem: {lit}");
    if let (Some(path), Some((w, h, px))) = (dump, last) {
        let file = std::fs::File::create(&path).unwrap();
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
        eprintln!("quadro salvo em {path}");
    }
    if emu_session::gl_interop_wanted() {
        assert!(
            ring > 0,
            "nenhum quadro veio pelo anel ({readback} pelo readback)"
        );
        assert!(lit > 0, "todos os quadros do anel vieram pretos");
    } else {
        assert_eq!(ring, 0, "REEMU_GL_INTEROP=0 devia ficar no readback");
        assert!(readback > 0);
    }
}

/// Índices livres de TLS (`TlsAlloc`) e de FLS (`FlsAlloc`) neste processo:
/// aloca até a API devolver `TLS_OUT_OF_INDEXES`/`FLS_OUT_OF_INDEXES`
/// (learn.microsoft.com, `TlsAlloc`/`FlsAlloc`) e devolve tudo. A página do
/// `TlsAlloc` não dá o limite por processo — por isso medir. Por um instante
/// não sobra índice: só com o processo quieto (entre um jogo e outro).
#[cfg(windows)]
fn free_tls_fls_indexes() -> (usize, usize) {
    use windows_sys::Win32::System::Threading::{
        FlsAlloc, FlsFree, TlsAlloc, TlsFree, FLS_OUT_OF_INDEXES, TLS_OUT_OF_INDEXES,
    };
    // SAFETY: só índices alocados aqui, devolvidos logo em seguida.
    unsafe {
        let mut tls = Vec::new();
        loop {
            let i = TlsAlloc();
            if i == TLS_OUT_OF_INDEXES {
                break;
            }
            tls.push(i);
        }
        let mut fls = Vec::new();
        loop {
            let i = FlsAlloc(None);
            if i == FLS_OUT_OF_INDEXES {
                break;
            }
            fls.push(i);
        }
        for i in &tls {
            TlsFree(*i);
        }
        for i in &fls {
            FlsFree(*i);
        }
        (tls.len(), fls.len())
    }
}

/// Vazamento de índices de TLS/FLS da rota Vulkan in-process (Windows): o
/// core é carregado e descarregado `REEMU_TEST_CYCLES` vezes (padrão 8) no
/// processo do app, como ao trocar de jogo, e os índices livres são medidos
/// a cada volta. `REEMU_TEST_VK_CORE`, `REEMU_TEST_ROM` e `REEMU_TEST_OPTS`
/// como no `vk_core_real_rom`. Só mede e imprime — o C3 tira o core do
/// processo do app.
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU Vulkan, de um core Vulkan e de uma ROM"]
fn vk_core_tls_leak() {
    use emu_session::{EmuSession, SessionConfig};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    // Rota in-process (o padrão no Windows virou o processo filho, C3c).
    if std::env::var_os("REEMU_VK_CHILD").is_none() {
        std::env::set_var("REEMU_VK_CHILD", "0");
    }
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_TEST_VK_CORE"),
        std::env::var("REEMU_TEST_ROM"),
    ) else {
        eprintln!("defina REEMU_TEST_VK_CORE e REEMU_TEST_ROM");
        return;
    };
    let cycles: u32 = std::env::var("REEMU_TEST_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let data =
        std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("com.reemu.desktop");
    let fp = FrameProcessor::new().expect("adapter wgpu");
    let shared = fp.vulkan_shared_device().expect("backend Vulkan");
    std::env::set_var("REEMU_HW", "vulkan");
    let tmp = std::env::temp_dir().join(format!("reemu-tls-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let session = EmuSession::spawn(SessionConfig::new(
        data.join("cores"),
        data.join("system"),
        tmp.clone(),
    ));
    session.attach_vulkan_device(shared);
    session.attach_vulkan_negotiator(std::sync::Arc::new(|neg| {
        let (_fp, shared) = unsafe { FrameProcessor::from_core_negotiation(neg) }?;
        Ok(shared)
    }));
    let opts: HashMap<String, String> = std::env::var("REEMU_TEST_OPTS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let _ = session.step_vk_local();
    // Chamada bloqueante da sessão enquanto esta thread dirige o core.
    fn driving<R: Send>(session: &EmuSession, call: impl FnOnce() -> R + Send) -> R {
        std::thread::scope(|sc| {
            let h = sc.spawn(call);
            while !h.is_finished() {
                let gate = session.lock_vk_queue();
                let _ = session.step_vk_local();
                drop(gate);
                std::thread::sleep(Duration::from_millis(2));
            }
            h.join().unwrap()
        })
    }

    let (tls0, fls0) = free_tls_fls_indexes();
    eprintln!("antes: {tls0} índices TLS livres, {fls0} FLS");
    let mut last = (tls0, fls0);
    for n in 1..=cycles {
        driving(&session, || session.load(&core, &rom, opts.clone())).expect("carregar");
        assert!(
            session.debug_child_pid().is_none(),
            "core foi pro processo filho"
        );
        let start = Instant::now();
        let mut frames = 0u32;
        while start.elapsed() < Duration::from_secs(2) {
            let gate = session.lock_vk_queue();
            if session.step_vk_local().is_some() {
                frames += 1;
            }
            drop(gate);
            std::thread::sleep(Duration::from_millis(16));
        }
        driving(&session, || session.unload()).ok();
        let now = free_tls_fls_indexes();
        eprintln!(
            "volta {n}: {frames} quadros | TLS livres {} ({:+}) | FLS livres {} ({:+})",
            now.0,
            now.0 as i64 - last.0 as i64,
            now.1,
            now.1 as i64 - last.1 as i64
        );
        last = now;
    }
    std::env::remove_var("REEMU_HW");
    let _ = std::fs::remove_dir_all(&tmp);
    eprintln!(
        "total em {cycles} voltas: TLS {:+}, FLS {:+} (por jogo: TLS {:.1}, FLS {:.1})",
        last.0 as i64 - tls0 as i64,
        last.1 as i64 - fls0 as i64,
        (tls0 as f64 - last.0 as f64) / f64::from(cycles),
        (fls0 as f64 - last.1 as f64) / f64::from(cycles)
    );
}

/// C3a: o core Vulkan da lista roda no processo filho (`REEMU_VK_CHILD=1`),
/// com device próprio, e o quadro chega ao app como RGBA. Troca de jogo
/// `REEMU_TEST_CYCLES` vezes (padrão 3, ~4 s cada) conferindo que os quadros
/// vêm do filho e com imagem, e mede os índices de TLS do app — o core não
/// é mais carregado aqui, então não pode vazar. `REEMU_TEST_VK_CORE`,
/// `REEMU_TEST_ROM` e `REEMU_TEST_OPTS` como no `vk_core_real_rom`;
/// `REEMU_TEST_DUMP=arquivo.png` salva o último quadro.
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU Vulkan, de um core Vulkan e de uma ROM"]
fn vk_core_in_child() {
    use emu_session::{EmuSession, SessionConfig};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_TEST_VK_CORE"),
        std::env::var("REEMU_TEST_ROM"),
    ) else {
        eprintln!("defina REEMU_TEST_VK_CORE e REEMU_TEST_ROM");
        return;
    };
    // SAFETY: teste `--ignored` de processo único pra esta variável; o
    // filho herda o ambiente.
    unsafe { std::env::set_var("REEMU_VK_CHILD", "1") };
    let _ = env_logger::builder().is_test(true).try_init();
    let cycles: u32 = std::env::var("REEMU_TEST_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let secs: u64 = std::env::var("REEMU_TEST_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let data =
        std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("com.reemu.desktop");
    let tmp = std::env::temp_dir().join(format!("reemu-vkchild-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let session = EmuSession::spawn(SessionConfig::new(
        data.join("cores"),
        data.join("system"),
        tmp.clone(),
    ));
    let opts: HashMap<String, String> = std::env::var("REEMU_TEST_OPTS")
        .unwrap_or_default()
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let (tls0, _) = free_tls_fls_indexes();
    let mut last: Option<(u32, u32, Vec<u8>)> = None;
    for n in 1..=cycles {
        session.load(&core, &rom, opts.clone()).expect("carregar");
        assert!(session.debug_child_pid().is_some(), "core ficou in-process");
        let (mut frames, mut lit) = (0u32, 0u32);
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(secs) {
            session.wait_for_frame(Duration::from_millis(100));
            if let Some(f) = session.take_latest_frame() {
                if let FrameOrigin::SoftwareRawBuffer { data, .. } = &f.origin {
                    frames += 1;
                    if data
                        .chunks_exact(4)
                        .any(|p| p[0] > 16 || p[1] > 16 || p[2] > 16)
                    {
                        lit += 1;
                        last = Some((
                            f.metadata.native_width,
                            f.metadata.native_height,
                            data.clone(),
                        ));
                    }
                }
                session.recycle_frame(f);
            }
        }
        let _ = session.unload();
        let (tls, _) = free_tls_fls_indexes();
        eprintln!(
            "volta {n}: {frames} quadros do filho ({:.0}/s), {lit} com imagem | TLS livres no app {tls} ({:+})",
            f64::from(frames) / secs as f64,
            tls as i64 - tls0 as i64
        );
        assert!(lit > 0, "nenhum quadro com imagem na volta {n}");
    }
    if let (Ok(path), Some((w, h, px))) = (std::env::var("REEMU_TEST_DUMP"), last) {
        let mut enc = png::Encoder::new(std::fs::File::create(&path).unwrap(), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
        eprintln!("quadro salvo em {path}");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// CAS (AMD FidelityFX, `shaders/cas`, embutido): o preset roda na GPU e cada
/// pixel bate com uma referência em CPU das mesmas fórmulas do `ffx_cas.h`
/// (caminho sem escala, 5 amostras, pesos do verde, gama 2.0 na entrada e
/// raiz na saída, `sharpness` = 0, o padrão da AMD). Confere também que a
/// borda de fato ganha nitidez.
#[test]
fn cas_matches_the_reference_math() {
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let dir = std::env::temp_dir().join(format!("reemu-cas-{}", std::process::id()));
    crate::bundled_shaders::install(&dir);
    let preset = crate::bundled_shaders::dir(&dir).join("cas/cas.slangp");
    fp.set_preset(preset.to_str().unwrap())
        .expect("compilar o CAS");

    // Degraus verticais de 8 px (cinzas 0x40/0xA0/0x60) + um ponto claro.
    let (w, h) = (32u32, 16u32);
    let gray = |x: u32, y: u32| -> u8 {
        if (x, y) == (20, 8) {
            return 0xF0;
        }
        [0x40u8, 0xA0, 0x60, 0xA0][(x / 8) as usize]
    };
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = gray(x, y);
            let i = ((y * w + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&[v, v, v, 0xFF]);
        }
    }
    let frame = Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data,
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: w as f32 / h as f32,
            rotation_degrees: 0,
        },
    };
    let _ = fp.process(&frame); // o readback devolve o quadro anterior
    let (ow, oh, out) = fp.process(&frame).expect("quadro");
    assert_eq!((ow, oh), (w, h));

    // Referência (f32): CasLoad + CasInput (quadrado), mín/máx do círculo,
    // amp = sqrt(sat(min(mn, 1 - mx) / mx)), peak = -1/lerp(8, 5, 0).
    let load = |x: i32, y: i32| -> f32 {
        let x = x.clamp(0, w as i32 - 1) as u32;
        let y = y.clamp(0, h as i32 - 1) as u32;
        let v = gray(x, y) as f32 / 255.0;
        v * v
    };
    let mut changed = 0;
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (b, d, e, f, hh) = (
                load(x, y - 1),
                load(x - 1, y),
                load(x, y),
                load(x + 1, y),
                load(x, y + 1),
            );
            let mn = b.min(d).min(e).min(f).min(hh);
            let mx = b.max(d).max(e).max(f).max(hh);
            let amp = if mx > 0.0 {
                ((mn.min(1.0 - mx)) / mx).clamp(0.0, 1.0).sqrt()
            } else {
                0.0
            };
            let peak = -1.0 / 8.0;
            let wgt = amp * peak;
            let pix = ((b + d + f + hh) * wgt + e) / (1.0 + 4.0 * wgt);
            let want = (pix.clamp(0.0, 1.0).sqrt() * 255.0).round() as i32;
            let got = out[((y as u32 * w + x as u32) * 4 + 1) as usize] as i32;
            assert!(
                (got - want).abs() <= 2,
                "pixel ({x},{y}): GPU {got}, referência {want}"
            );
            if got != gray(x as u32, y as u32) as i32 {
                changed += 1;
            }
        }
    }
    eprintln!("CAS: {changed} pixels realçados de {}", w * h);
    assert!(changed > 0, "o CAS não mudou nada");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnóstico de HDR (Windows): numa janela oculta, o que a superfície
/// oferece (formatos × espaços de cor, `format_capabilities`) e o que o
/// Windows informa do monitor (`display_hdr_info`: branco SDR e pico, via
/// `DISPLAYCONFIG_SDR_WHITE_LEVEL` e DXGI). Só imprime.
#[cfg(windows)]
#[test]
#[ignore = "precisa de GPU e de um monitor"]
fn hdr_surface_diagnostics() {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, ShowWindow, SW_SHOWNA,
        WNDCLASSW, WS_POPUP,
    };
    unsafe extern "system" fn wndproc(
        h: windows_sys::Win32::Foundation::HWND,
        m: u32,
        w: usize,
        l: isize,
    ) -> isize {
        unsafe { DefWindowProcW(h, m, w, l) }
    }
    let Some(fp) = FrameProcessor::new() else {
        eprintln!("sem GPU");
        return;
    };
    let class: Vec<u16> = "ReEmuHdrDiag\0".encode_utf16().collect();
    // SAFETY: janela oculta só pra criar a surface; destruída no fim.
    let hwnd = unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            WS_POPUP,
            0,
            0,
            64,
            64,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        )
    };
    assert!(!hwnd.is_null());
    // Visível (sem ativar): o Windows associa a janela a um monitor.
    unsafe { ShowWindow(hwnd, SW_SHOWNA) };
    let mut win = raw_window_handle::Win32WindowHandle::new(
        std::num::NonZeroIsize::new(hwnd as isize).unwrap(),
    );
    // `VkWin32SurfaceCreateInfoKHR` precisa do `hinstance` da janela.
    win.hinstance =
        std::num::NonZeroIsize::new(unsafe { GetModuleHandleW(std::ptr::null()) } as isize);
    let surface = unsafe {
        fp.instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Windows(
                    raw_window_handle::WindowsDisplayHandle::new(),
                )),
                raw_window_handle: raw_window_handle::RawWindowHandle::Win32(win),
            })
    }
    .expect("surface");
    let caps = surface.get_capabilities(&fp.adapter);
    eprintln!("formatos: {:?}", caps.formats);
    for fc in &caps.format_capabilities {
        eprintln!("formato {:?}: {:?}", fc.format, fc.color_spaces);
    }
    eprintln!("monitor: {:?}", surface.display_hdr_info(&fp.adapter));
    drop(surface);

    // O caminho do app: `attach_surface` + `set_hdr` + quadros apresentados.
    let mut fp = fp;
    let disp = raw_window_handle::RawDisplayHandle::Windows(
        raw_window_handle::WindowsDisplayHandle::new(),
    );
    assert!(unsafe {
        fp.attach_surface(disp, raw_window_handle::RawWindowHandle::Win32(win), 64, 64)
    });
    fp.set_hdr(HdrSettings {
        enabled: true,
        paper_white_nits: 0,
        highlights: true,
        peak_nits: 0,
    });
    let st = fp.hdr_status();
    eprintln!("HDR ligado: {st:?}");
    for _ in 0..5 {
        fp.render_to_surface(Some(&checker_frame(32, 32)));
    }
    if st.capable {
        assert!(st.active, "a surface aceita scRGB e o HDR não ligou");
    }
    fp.set_hdr(HdrSettings::default());
    fp.render_to_surface(Some(&checker_frame(32, 32)));
    assert!(
        !fp.hdr_status().active,
        "desligar o HDR devia voltar pro SDR"
    );
    drop(fp);
    unsafe { DestroyWindow(hwnd) };
}

/// Passe final em HDR (`HDR_BLIT_WGSL`) numa textura `Rgba16Float` fora da
/// tela: sRGB → linear (IEC 61966-2-1), realce pela inversa do Reinhard
/// estendido e escala branco SDR/80 (scRGB, 1.0 = 80 nits). Com branco SDR
/// 240 e pico 799 nits: branco vai ao pico (799/80), cinza 128 fica perto
/// do SDR escalado e o preto continua preto.
#[test]
fn hdr_blit_maps_sdr_to_scrgb() {
    let Some(fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
    let d = &fp.device;
    let (paper, peak) = (240.0f32, 799.0f32);
    let k = 1.0 - paper / peak;
    // Fonte 3×1 RGBA8 (valores sRGB codificados, como a chain entrega).
    let src = d.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr src"),
        size: wgpu::Extent3d {
            width: 3,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let px: [u8; 12] = [0, 0, 0, 255, 128, 128, 128, 255, 255, 255, 255, 255];
    fp.queue.write_texture(
        src.as_image_copy(),
        &px,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(12),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 3,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let dst = d.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr dst"),
        size: wgpu::Extent3d {
            width: 3,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let params = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hdr params"),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // Retângulo cheio (centro 0,0; meia-extensão 1,1) + [branco/80, k].
    fp.queue.write_buffer(
        &params,
        0,
        f32s_bytes(&[0.0, 0.0, 1.0, 1.0, paper / 80.0, k, 0.0, 0.0]),
    );
    let pipeline = blit_pipeline_for(d, &fp.comp.bgl, wgpu::TextureFormat::Rgba16Float, true);
    let src_view = src.create_view(&Default::default());
    let bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("hdr bg"),
        layout: &fp.comp.bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&src_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&fp.sampler_nearest),
            },
        ],
    });
    let readback = d.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hdr readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let dst_view = dst.create_view(&Default::default());
    let mut enc = d.create_command_encoder(&Default::default());
    {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hdr test"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dst_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        rp.set_pipeline(&pipeline);
        rp.set_bind_group(0, &bg, &[]);
        rp.set_vertex_buffer(0, fp.quad.slice(..));
        rp.draw(0..4, 0..1);
    }
    enc.copy_texture_to_buffer(
        dst.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 3,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    fp.queue.submit([enc.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = d.poll(wgpu::PollType::wait_indefinitely());
    let data = readback.slice(..).get_mapped_range().unwrap();
    // f16 → f32 (IEEE 754 meia precisão).
    let half = |i: usize| -> f32 {
        let h = u16::from_le_bytes([data[i], data[i + 1]]);
        let (s, e, m) = (
            (h >> 15) as i32,
            ((h >> 10) & 0x1f) as i32,
            (h & 0x3ff) as f32,
        );
        let v = if e == 0 {
            m / 1024.0 * 2f32.powi(-14)
        } else {
            (1.0 + m / 1024.0) * 2f32.powi(e - 15)
        };
        if s == 1 {
            -v
        } else {
            v
        }
    };
    // Referência: sRGB → linear, inversa do Reinhard estendido, × branco/80.
    let expect = |c: f32| -> f32 {
        let lin = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
        let x = if lin > 1e-4 {
            lin / (1.0 - lin * k)
        } else {
            lin
        };
        x * paper / 80.0
    };
    for (i, c) in [0.0f32, 128.0 / 255.0, 1.0].into_iter().enumerate() {
        let got = half(i * 8);
        let want = expect(c);
        eprintln!(
            "entrada {:.3} → scRGB {got:.4} (referência {want:.4}, {:.0} nits)",
            c,
            got * 80.0
        );
        assert!(
            (got - want).abs() <= want.max(0.01) * 0.01,
            "pixel {i}: {got} vs {want}"
        );
    }
    assert!(
        (half(16) * 80.0 - peak).abs() < peak * 0.01,
        "branco SDR devia ir ao pico"
    );
}

/// Prévia de verdade (o que a tela de Shaders faz) em cada `.slangp` do
/// pacote: monta os pipelines no wgpu (`set_preset`) e roda a chain na cena
/// 2D (`render_still`). Pega o que o `field_validate_real_presets` não vê —
/// erros de validação do WebGPU ao criar pipeline/shader module e chain que
/// não gera imagem. Um `FrameProcessor` novo por preset (reaproveitar um só
/// esgotava a memória da GPU no Mega Bezel). Ignorado (precisa do pacote):
///   REEMU_SHADER_DIR=…/slang-shaders [REEMU_SHADER_FILTER=downsample/] \
///   cargo test -p reemu-desktop --lib field_render_real_presets -- --ignored --nocapture
#[test]
#[ignore]
fn field_render_real_presets() {
    let dir = std::env::var("REEMU_SHADER_DIR").unwrap_or_else(|_| {
        format!(
            "{}/.local/share/com.reemu.desktop/shaders/slang-shaders",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let filter = std::env::var("REEMU_SHADER_FILTER").unwrap_or_default();
    let _ = env_logger::builder().is_test(true).try_init();
    let root = std::path::Path::new(&dir);
    let mut presets = Vec::new();
    collect_slangp(root, &mut presets);
    presets.sort();
    let w = crate::commands::PREVIEW_W;
    let h = crate::commands::PREVIEW_H;
    let frame = Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data: crate::commands::sample_scene(),
            pitch: w * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: w,
            native_height: h,
            aspect_ratio: w as f32 / h as f32,
            rotation_degrees: 0,
        },
    };
    let mut by_dir: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for p in &presets {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        if !rel.contains(&filter) {
            continue;
        }
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
        let res = fp.set_preset(p.to_str().unwrap()).and_then(|_| {
            fp.render_still(&frame, 960, 720, 2)
                .ok_or_else(|| "sem imagem".to_string())
        });
        let top = rel.split('/').next().unwrap_or("").to_string();
        let e = by_dir.entry(top).or_default();
        e.1 += 1;
        match res {
            Ok(_) => e.0 += 1,
            Err(err) => {
                let last = err.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("");
                eprintln!("FALHOU {rel}: {}", last.trim());
            }
        }
    }
    let (ok, total) = by_dir.values().fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    eprintln!("\n=== {ok}/{total} renderizaram ===");
    for (d, (o, t)) in &by_dir {
        eprintln!("{o:>5}/{t:<5} {d}");
    }
}
