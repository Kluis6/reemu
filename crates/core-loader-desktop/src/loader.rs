//! `DesktopCoreLoader`: implementa `domain::core_loader::CoreLoader` via
//! `libloading`. Caminho software-only completo; se um core exige HW render
//! (GL/Vulkan) os requisitos são detectados e persistidos, mas o load é
//! recusado com `HwRenderUnsupported` (contexto GL real = etapa 02 passo 4).

use crate::core::DesktopCore;
use crate::ffi_state::{self, HwRenderRequest};
use crate::raw::RawCore;
use crate::sys;
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
        }
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

        let render_reqs = match ffi_state::lock().as_ref().and_then(|s| s.hw_render) {
            None => software_requirements(),
            Some(req) => map_backend(&req)?,
        };

        self.known
            .lock()
            .unwrap()
            .insert(core_id.0.clone(), render_reqs.clone());

        let core = DesktopCore::new(raw, av_info, render_reqs.clone(), guard);

        if render_reqs.render_backend != RenderBackend::Software {
            // Drop faz o teardown (unload_game + deinit) e libera o guard.
            drop(core);
            return Err(CoreLoadError::HwRenderUnsupported(format!(
                "{} exige {:?}{} — negociação de contexto ainda não implementada",
                core_id.0,
                render_reqs.render_backend,
                render_reqs
                    .gl_version_min
                    .map(|v| format!(" {v}"))
                    .unwrap_or_default(),
            )));
        }

        Ok(core)
    }
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
