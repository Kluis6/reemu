//! `DesktopCoreLoader`: implementa `domain::core_loader::CoreLoader` via
//! `libloading`. Caminho software-only completo; cores que exigem HW render GL
//! negociam um contexto EGL offscreen (`setup_gl_context`); HW render Vulkan
//! monta um `VkContext`/`VkFrameBridge` (`setup_vk_context`, etapa 12).

use crate::core::DesktopCore;
use crate::ffi_state::{self, HwRenderRequest};
use crate::raw::RawCore;
use crate::sys;
use crate::vk_context::{VkConfig, VkContext};
use crate::vk_frame::VkFrameBridge;
use async_trait::async_trait;
use domain::core_loader::{
    CoreId, CoreLoadError, CoreLoader, CoreRenderRequirements, InstalledCoreRepository, LoadedCore,
    RenderBackend, SystemAvInfo, SystemGeometry, SystemTiming,
};
use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct DesktopCoreLoader {
    cores_dir: PathBuf,
    system_dir: PathBuf,
    save_dir: PathBuf,
    installed: Option<Arc<dyn InstalledCoreRepository>>,
    /// Cache em memória dos requisitos já descobertos nesta sessão.
    known: Mutex<HashMap<String, CoreRenderRequirements>>,
    /// `Some` = um core de HW render Vulkan (etapa 12) deve ADOTAR este
    /// `VkDevice` (o do compositor wgpu) em vez de criar um novo — é o que
    /// torna o frame zero-cópia. Só faz sentido quando o loader roda no mesmo
    /// processo do compositor (não no `reemu-core-host`).
    vk_shared_device: Option<domain::core_loader::VulkanSharedDevice>,
}

impl DesktopCoreLoader {
    pub fn new(
        cores_dir: impl Into<PathBuf>,
        system_dir: impl Into<PathBuf>,
        save_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            cores_dir: cores_dir.into(),
            system_dir: system_dir.into(),
            save_dir: save_dir.into(),
            installed: None,
            known: Mutex::new(HashMap::new()),
            vk_shared_device: None,
        }
    }

    /// Faz os cores de HW render Vulkan adotarem o `VkDevice` do compositor
    /// (`FrameProcessor::vulkan_shared_device()`) — zero-cópia. Ver
    /// `docs/ai-context/12-vulkan-hw-render-fase2.md` (fase B).
    pub fn with_vulkan_shared_device(
        mut self,
        shared: domain::core_loader::VulkanSharedDevice,
    ) -> Self {
        self.vk_shared_device = Some(shared);
        self
    }

    /// Liga o repositório de `installed_cores` (etapa 01) — os requisitos de
    /// render detectados no primeiro load são persistidos ali.
    pub fn with_installed_repo(mut self, repo: Arc<dyn InstalledCoreRepository>) -> Self {
        self.installed = Some(repo);
        self
    }

    fn resolve_path(&self, core_id: &CoreId) -> Result<PathBuf, CoreLoadError> {
        let raw = Path::new(&core_id.0);
        let candidates = if raw.is_absolute() || raw.components().count() > 1 {
            vec![raw.to_path_buf()]
        } else {
            vec![
                self.cores_dir.join(&core_id.0),
                self.cores_dir
                    .join(format!("{}{}", core_id.0, dylib_suffix())),
            ]
        };
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .ok_or_else(|| CoreLoadError::NotFound(core_id.0.clone()))
    }
}

fn dylib_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        ".dll"
    } else if cfg!(target_os = "macos") {
        ".dylib"
    } else {
        ".so"
    }
}

fn map_backend(req: &HwRenderRequest) -> Result<CoreRenderRequirements, CoreLoadError> {
    let (backend, profile) = match req.context_type {
        sys::RETRO_HW_CONTEXT_NONE => (RenderBackend::Software, None),
        sys::RETRO_HW_CONTEXT_OPENGL => (RenderBackend::OpenGl, Some("compat".to_string())),
        sys::RETRO_HW_CONTEXT_OPENGL_CORE => (RenderBackend::OpenGl, Some("core".to_string())),
        sys::RETRO_HW_CONTEXT_OPENGLES2
        | sys::RETRO_HW_CONTEXT_OPENGLES3
        | sys::RETRO_HW_CONTEXT_OPENGLES_VERSION => (RenderBackend::OpenGl, Some("es".to_string())),
        sys::RETRO_HW_CONTEXT_VULKAN => (RenderBackend::Vulkan, None),
        other => {
            return Err(CoreLoadError::IncompatiblePlatform(format!(
                "retro_hw_context_type {other} não suportado no desktop"
            )))
        }
    };
    let gl_version_min = if matches!(backend, RenderBackend::OpenGl)
        && (req.version_major, req.version_minor) != (0, 0)
    {
        Some(format!("{}.{}", req.version_major, req.version_minor))
    } else {
        None
    };
    Ok(CoreRenderRequirements {
        render_backend: backend,
        gl_version_min,
        gl_profile: profile,
        needs_depth_stencil: req.depth || req.stencil,
    })
}

fn software_requirements() -> CoreRenderRequirements {
    CoreRenderRequirements {
        render_backend: RenderBackend::Software,
        gl_version_min: None,
        gl_profile: None,
        needs_depth_stencil: false,
    }
}

impl DesktopCoreLoader {
    /// Como `CoreLoader::load`, mas devolve o tipo concreto (dá acesso a
    /// `drain_audio`, `serialize_state`, ...) e persiste os requisitos de
    /// render no `InstalledCoreRepository`, se ligado.
    pub async fn load_core(
        &self,
        core_id: &CoreId,
        rom_path: &str,
    ) -> Result<DesktopCore, CoreLoadError> {
        let result = self.open_core(core_id, rom_path);
        // Persiste o que foi detectado — mesmo no caminho de rejeição de HW
        // core, o catálogo precisa saber (`known_render_requirements` guarda).
        if let (Some(repo), Some(reqs)) = (&self.installed, self.known_render_requirements(core_id))
        {
            if let Err(e) = repo.set_render_requirements(&core_id.0, &reqs).await {
                log::warn!("não persistiu render requirements de {}: {e}", core_id.0);
            }
        }
        result
    }

    /// Versão 100% síncrona (sem persistência no repo) — pra rodar de dentro
    /// da thread dedicada de emulação, que não tem executor async. Os
    /// requisitos detectados ficam em `known_render_requirements`.
    pub fn open_core(
        &self,
        core_id: &CoreId,
        rom_path: &str,
    ) -> Result<DesktopCore, CoreLoadError> {
        let path = self.resolve_path(core_id)?;

        // ROM em .zip: extrai a entrada interna pra um arquivo temporário. Vive
        // (via `DesktopCore`) até o unload. Sets de arcade (MAME/FBNeo) não
        // têm "uma ROM" reconhecível dentro — só chip dumps avulsos — nesse
        // caso NÃO é erro: o core (`need_fullpath`) espera o caminho do
        // `.zip` inteiro e abre sozinho, então cai pro caminho original.
        // Outros erros de IO (zip corrompido, permissão) continuam
        // propagando — só "não achei ROM reconhecida aí dentro" tem fallback.
        let extracted = if crate::archive::is_zip(Path::new(rom_path)) {
            match crate::archive::extract_rom(Path::new(rom_path), &std::env::temp_dir()) {
                Ok(e) => Some(e),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    log::info!(
                        "{rom_path}: nenhuma ROM de cartucho reconhecida dentro do .zip — \
                         tratando como set de arcade (caminho original pro core)"
                    );
                    None
                }
                Err(e) => {
                    return Err(CoreLoadError::LoadFailed(format!(
                        "extrair {rom_path}: {e}"
                    )));
                }
            }
        } else {
            None
        };
        let rom_path: &str = extracted
            .as_ref()
            .and_then(|e| e.path().to_str())
            .unwrap_or(rom_path);

        // O guard inicializa o estado global e garante um-core-por-processo.
        let guard = ffi_state::acquire(&self.system_dir, &self.save_dir)?;
        let raw = RawCore::open(&path)?;

        let api = unsafe { (raw.api_version)() };
        if api != sys::RETRO_API_VERSION {
            return Err(CoreLoadError::LoadFailed(format!(
                "RETRO_API_VERSION {api} != {} suportado",
                sys::RETRO_API_VERSION
            )));
        }

        unsafe {
            (raw.set_environment)(ffi_state::environment_cb);
            (raw.set_video_refresh)(ffi_state::video_refresh_cb);
            (raw.set_audio_sample)(ffi_state::audio_sample_cb);
            (raw.set_audio_sample_batch)(ffi_state::audio_sample_batch_cb);
            (raw.set_input_poll)(ffi_state::input_poll_cb);
            (raw.set_input_state)(ffi_state::input_state_cb);
            (raw.init)();
        }

        // Info do core (need_fullpath decide se carregamos a ROM em memória).
        // `info` tem ponteiros crus — não deve cruzar um `.await`.
        let need_fullpath = {
            let mut info: sys::retro_system_info = unsafe { std::mem::zeroed() };
            unsafe { (raw.get_system_info)(&mut info) };
            info.need_fullpath
        };

        let load_ok = {
            let c_path = CString::new(rom_path)
                .map_err(|_| CoreLoadError::LoadFailed("rom_path contém NUL".into()))?;
            let rom_bytes =
                if need_fullpath {
                    None
                } else {
                    Some(std::fs::read(rom_path).map_err(|e| {
                        CoreLoadError::LoadFailed(format!("ler ROM {rom_path}: {e}"))
                    })?)
                };
            let game = sys::retro_game_info {
                path: c_path.as_ptr(),
                data: rom_bytes
                    .as_ref()
                    .map_or(std::ptr::null(), |b| b.as_ptr().cast()),
                size: rom_bytes.as_ref().map_or(0, |b| b.len()),
                meta: std::ptr::null(),
            };
            let ok = unsafe { (raw.load_game)(&game) };
            // rom_bytes/c_path vivem até aqui (o core copia o que precisa).
            ok
        };

        if !load_ok {
            unsafe { (raw.deinit)() };
            drop(raw);
            drop(guard);
            return Err(CoreLoadError::LoadFailed(format!(
                "retro_load_game falhou para {rom_path}"
            )));
        }

        let av_info = read_av_info(&raw);

        let hw = ffi_state::lock().as_ref().and_then(|s| s.hw_render);
        let render_reqs = match hw {
            None => software_requirements(),
            Some(req) => map_backend(&req)?,
        };

        self.known
            .lock()
            .unwrap()
            .insert(core_id.0.clone(), render_reqs.clone());

        let teardown = |raw: &RawCore| unsafe {
            (raw.unload_game)();
            (raw.deinit)();
        };
        let (gl, vk) = match render_reqs.render_backend {
            RenderBackend::Software => (None, None),
            RenderBackend::OpenGl => {
                let req = hw.expect("OpenGl backend sem HwRenderRequest");
                let gl =
                    setup_gl_context(&core_id.0, &req, &av_info).inspect_err(|_| teardown(&raw))?;
                (Some(gl), None)
            }
            RenderBackend::Vulkan => {
                let req = hw.expect("Vulkan backend sem HwRenderRequest");
                let bridge = setup_vk_context(&core_id.0, &req, self.vk_shared_device)
                    .inspect_err(|_| teardown(&raw))?;
                (None, Some(bridge))
            }
        };

        Ok(DesktopCore::new(
            raw,
            av_info,
            render_reqs,
            guard,
            gl,
            vk,
            extracted,
        ))
    }
}

/// Cria o contexto GL offscreen, registra o FBO no estado global e roda o
/// `context_reset` do core (que (re)cria os objetos GL dele).
fn setup_gl_context(
    core_id: &str,
    req: &HwRenderRequest,
    av: &SystemAvInfo,
) -> Result<crate::gl_context::GlContext, CoreLoadError> {
    let cfg = crate::gl_context::GlConfig {
        context_type: req.context_type,
        version_major: req.version_major,
        version_minor: req.version_minor,
        depth: req.depth,
        stencil: req.stencil,
        bottom_left_origin: req.bottom_left_origin,
    };
    let max_w = av.geometry.max_width.max(av.geometry.base_width).max(1);
    let max_h = av.geometry.max_height.max(av.geometry.base_height).max(1);
    let ctx = crate::gl_context::GlContext::create(&cfg, max_w, max_h)
        .map_err(|e| CoreLoadError::HwRenderUnsupported(format!("{core_id}: contexto GL: {e}")))?;
    ctx.make_current()
        .map_err(|e| CoreLoadError::HwRenderUnsupported(format!("{core_id}: makeCurrent: {e}")))?;
    if let Some(st) = ffi_state::lock().as_mut() {
        st.hw_fbo = Some(ctx.fbo());
    }
    // O core (re)constrói seus recursos GL agora, com o FBO já publicado.
    if let Some(reset) = req.context_reset {
        unsafe { reset() };
    }
    // Interop zero-cópia (dma_buf) é o padrão; `try_enable_interop` cai pro
    // readback sozinho se GBM/EGL/wgpu não colaborarem. `REEMU_GL_INTEROP=0`
    // força o readback.
    let mut ctx = ctx;
    let want_interop = !matches!(
        std::env::var("REEMU_GL_INTEROP")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref(),
        Ok("0") | Ok("false") | Ok("off") | Ok("no")
    );
    if want_interop {
        ctx.try_enable_interop();
    }
    log::info!(
        "contexto GL pronto pra {core_id} (FBO {}, interop={})",
        ctx.fbo(),
        ctx.interop_active()
    );
    Ok(ctx)
}

/// Cria o contexto Vulkan compartilhado + a ponte de frame, publica a
/// `retro_hw_render_interface_vulkan` no estado global e roda o `context_reset`
/// do core (que aí chama `GET_HW_RENDER_INTERFACE` e monta os recursos dele).
///
/// Fase A: `VkConfig` default (sem extensões extra). A fase B injeta o conjunto
/// que o `wgpu-hal` exige e liga a imagem no compositor.
fn setup_vk_context(
    core_id: &str,
    req: &HwRenderRequest,
    shared: Option<domain::core_loader::VulkanSharedDevice>,
) -> Result<Box<VkFrameBridge>, CoreLoadError> {
    let ctx = match shared {
        // Fase B: adota o device do compositor -> frame zero-copia.
        Some(s) => VkContext::adopt(s).map_err(|e| {
            CoreLoadError::HwRenderUnsupported(format!("{core_id}: adotar device Vulkan: {e}"))
        })?,
        // Bring-up / teste headless: cria um device proprio.
        None => {
            let mut cfg = VkConfig::default();
            if req.version_major >= 0x0040_0000 {
                // O core pediu uma apiVersion concreta (ex.: flycast manda
                // VK_API_VERSION_1_1). `version_minor` fica 0 nesses cores.
                cfg.api_version = req.version_major;
            }
            VkContext::create(&cfg).map_err(|e| {
                CoreLoadError::HwRenderUnsupported(format!("{core_id}: contexto Vulkan: {e}"))
            })?
        }
    };
    log::info!(
        "contexto Vulkan pronto pra {core_id} ({})",
        ctx.device_name()
    );

    let bridge = VkFrameBridge::new(ctx).map_err(|e| {
        CoreLoadError::HwRenderUnsupported(format!("{core_id}: ponte de frame Vulkan: {e}"))
    })?;

    // Publica a interface ANTES do context_reset — o core a lê lá dentro.
    if let Some(st) = ffi_state::lock().as_mut() {
        st.vk_interface_ptr = Some(bridge.interface_ptr() as usize);
    }
    if let Some(reset) = req.context_reset {
        unsafe { reset() };
    }
    Ok(bridge)
}

#[async_trait]
impl CoreLoader for DesktopCoreLoader {
    async fn load(
        &self,
        core_id: &CoreId,
        rom_path: &str,
    ) -> Result<Box<dyn LoadedCore>, CoreLoadError> {
        Ok(Box::new(self.load_core(core_id, rom_path).await?))
    }

    async fn unload(&self, core: Box<dyn LoadedCore>) -> Result<(), CoreLoadError> {
        drop(core); // Drop de DesktopCore faz unload_game + deinit + libera guard
        Ok(())
    }

    fn known_render_requirements(&self, core_id: &CoreId) -> Option<CoreRenderRequirements> {
        self.known.lock().unwrap().get(&core_id.0).cloned()
    }
}

fn read_av_info(raw: &RawCore) -> SystemAvInfo {
    let mut av: sys::retro_system_av_info = unsafe { std::mem::zeroed() };
    unsafe { (raw.get_system_av_info)(&mut av) };
    SystemAvInfo {
        geometry: SystemGeometry {
            base_width: av.geometry.base_width,
            base_height: av.geometry.base_height,
            max_width: av.geometry.max_width,
            max_height: av.geometry.max_height,
            aspect_ratio: av.geometry.aspect_ratio,
        },
        timing: SystemTiming {
            fps: av.timing.fps,
            sample_rate: av.timing.sample_rate,
        },
    }
}
