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
    let (plane, sync_fd) =
        core_loader_desktop::render_solid_rgba_to_dmabuf([220, 40, 10, 255], 64, 64)
            .expect("renderizar dma_buf de teste via GL (core-loader-desktop)");
    eprintln!("fence sync_file do produtor: {sync_fd:?}");
    let Some(mut fp) = FrameProcessor::new() else {
        eprintln!("sem adapter wgpu — pulando");
        return;
    };
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
    assert_eq!(out.len(), 32 + 50 * 30 * 4);
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
        let (plane, sync_fd) =
            core_loader_desktop::render_rect_rgba_to_dmabuf([30, 200, 60, 255], 64, 64, 40, 24)
                .expect("renderizar dma_buf de teste via GL");
        let Some(mut fp) = FrameProcessor::new() else {
            eprintln!("sem adapter wgpu — pulando");
            return;
        };
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
    let data = std::path::PathBuf::from(std::env::var("HOME").unwrap())
        .join(".local/share/com.reemu.desktop");
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
    let session = EmuSession::spawn(SessionConfig::new(
        data.join("cores"),
        data.join("system"),
        tmp.clone(),
    ));
    session
        .load(&core, &rom, HashMap::new())
        .expect("carregar core GL + ROM pela sessão");

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
