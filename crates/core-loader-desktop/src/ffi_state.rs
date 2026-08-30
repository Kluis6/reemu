//! Estado global do frontend + callbacks `extern "C"` que o core chama.
//!
//! Os callbacks libretro (`retro_video_refresh_t`, `retro_environment_t`,
//! ...) NÃO recebem ponteiro de contexto do usuário — a única forma de
//! rotear os dados é estado global. Por isso o loader impõe **um core por
//! processo** (`acquire`/`CoreGuard`).
//!
//! Acesso serializado: os callbacks só disparam de dentro de `retro_run`
//! (ou `retro_load_game`), sempre na thread que dirige o core, e o
//! consumidor (`DesktopCore::next_frame`) nunca segura o lock enquanto
//! chama `retro_run`. O `Mutex` é basicamente livre de contenção.

use crate::coreopts::{self, CoreOption};
use crate::sys;
use domain::core_loader::CoreLoadError;
use domain::frame_source::SoftwarePixelFormat;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::{c_char, c_uint, c_void};
use std::path::Path;
use std::sync::Mutex;

pub(crate) struct RawFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// bytes por linha, já sem padding (repackado no callback).
    pub pitch: u32,
    pub format: SoftwarePixelFormat,
}

#[derive(Clone, Copy)]
pub(crate) struct HwRenderRequest {
    pub context_type: c_uint,
    pub version_major: u32,
    pub version_minor: u32,
    pub depth: bool,
    pub stencil: bool,
}

pub(crate) struct FrontendState {
    pub pixel_format: SoftwarePixelFormat,
    pub system_dir: CString,
    pub save_dir: CString,
    pub hw_render: Option<HwRenderRequest>,
    pub last_frame: Option<RawFrame>,
    pub had_new_frame: bool,
    /// PCM interleaved estéreo i16 acumulado desde o último drain.
    pub audio: Vec<i16>,
    pub save_pending: bool,
    /// Schema de core options declarado pelo core (`SET_VARIABLES`/`SET_CORE_OPTIONS*`).
    pub core_options: Vec<CoreOption>,
    /// Valor atual de cada opção (`key -> value`). Semeado dos valores
    /// pendentes (DB) e depois de cada `install_core_options`.
    pub option_values: HashMap<String, String>,
    /// O core deve reler as opções no próximo `GET_VARIABLE_UPDATE`.
    pub options_dirty: bool,
    /// `CString` viva por chave, pro ponteiro que devolvemos em `GET_VARIABLE`
    /// continuar válido até o valor mudar.
    option_value_cache: HashMap<String, CString>,
}

impl FrontendState {
    fn new(system_dir: &Path, save_dir: &Path) -> Self {
        let to_c = |p: &Path| {
            CString::new(p.to_string_lossy().into_owned().into_bytes())
                .unwrap_or_else(|_| CString::new("").unwrap())
        };
        FrontendState {
            pixel_format: SoftwarePixelFormat::Rgb1555, // default libretro
            system_dir: to_c(system_dir),
            save_dir: to_c(save_dir),
            hw_render: None,
            last_frame: None,
            had_new_frame: false,
            audio: Vec::new(),
            save_pending: false,
            core_options: Vec::new(),
            option_values: coreopts::take_pending_core_option_values(),
            options_dirty: false,
            option_value_cache: HashMap::new(),
        }
    }

    /// Instala o schema declarado pelo core. Mantém valores já escolhidos que
    /// ainda são válidos; o resto cai no default.
    pub(crate) fn install_core_options(&mut self, opts: Vec<CoreOption>) {
        for o in &opts {
            let cur = self
                .option_values
                .entry(o.key.clone())
                .or_insert_with(|| o.default.clone());
            if !o.values.contains(cur) {
                *cur = o.default.clone();
            }
        }
        self.core_options = opts;
        self.options_dirty = true;
        self.option_value_cache.clear();
    }

    pub(crate) fn set_option_value(&mut self, key: &str, value: &str) -> bool {
        let valid = self
            .core_options
            .iter()
            .find(|o| o.key == key)
            .is_some_and(|o| o.values.iter().any(|v| v == value));
        if !valid {
            return false;
        }
        self.option_values
            .insert(key.to_string(), value.to_string());
        self.option_value_cache.remove(key);
        self.options_dirty = true;
        true
    }

    /// Ponteiro estável pro valor atual de `key` (ou nulo se não existe).
    fn option_ptr(&mut self, key: &str) -> *const c_char {
        let Some(val) = self.option_values.get(key).cloned() else {
            return std::ptr::null();
        };
        self.option_value_cache
            .entry(key.to_string())
            .or_insert_with(|| CString::new(val).unwrap_or_default())
            .as_ptr()
    }
}

static STATE: Mutex<Option<FrontendState>> = Mutex::new(None);

pub(crate) fn lock() -> std::sync::MutexGuard<'static, Option<FrontendState>> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Guarda que representa "há um core carregado". Enquanto viver, um novo
/// `acquire` falha. Ao dropar, zera o estado global.
pub(crate) struct CoreGuard {
    _private: (),
}

impl Drop for CoreGuard {
    fn drop(&mut self) {
        *lock() = None;
    }
}

pub(crate) fn acquire(system_dir: &Path, save_dir: &Path) -> Result<CoreGuard, CoreLoadError> {
    let mut g = lock();
    if g.is_some() {
        return Err(CoreLoadError::LoadFailed(
            "já há um core carregado neste processo (libretro é um-por-processo)".into(),
        ));
    }
    *g = Some(FrontendState::new(system_dir, save_dir));
    Ok(CoreGuard { _private: () })
}

// ---------------------------------------------------------------------------
// Callbacks extern "C"
// ---------------------------------------------------------------------------

pub(crate) unsafe extern "C" fn environment_cb(cmd: c_uint, data: *mut c_void) -> bool {
    let mut guard = lock();
    let Some(st) = guard.as_mut() else {
        return false;
    };

    match cmd {
        sys::RETRO_ENVIRONMENT_GET_CAN_DUPE => {
            if !data.is_null() {
                *(data as *mut bool) = true;
            }
            true
        }
        sys::RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
            if data.is_null() {
                return false;
            }
            match *(data as *const c_uint) {
                sys::RETRO_PIXEL_FORMAT_0RGB1555 => st.pixel_format = SoftwarePixelFormat::Rgb1555,
                sys::RETRO_PIXEL_FORMAT_XRGB8888 => st.pixel_format = SoftwarePixelFormat::Xrgb8888,
                sys::RETRO_PIXEL_FORMAT_RGB565 => st.pixel_format = SoftwarePixelFormat::Rgb565,
                _ => return false,
            }
            true
        }
        sys::RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY => {
            if data.is_null() {
                return false;
            }
            *(data as *mut *const c_char) = st.system_dir.as_ptr();
            true
        }
        sys::RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY
        | sys::RETRO_ENVIRONMENT_GET_CONTENT_DIRECTORY => {
            if data.is_null() {
                return false;
            }
            *(data as *mut *const c_char) = st.save_dir.as_ptr();
            true
        }
        sys::RETRO_ENVIRONMENT_SET_HW_RENDER => {
            if data.is_null() {
                return false;
            }
            let cb = &*(data as *const sys::retro_hw_render_callback);
            st.hw_render = Some(HwRenderRequest {
                context_type: cb.context_type,
                version_major: cb.version_major,
                version_minor: cb.version_minor,
                depth: cb.depth,
                stencil: cb.stencil,
            });
            // Aceita a declaração (o load() detecta e recusa depois, com
            // mensagem clara e requisitos já persistidos). Retornar false
            // aqui faria alguns cores abortarem de forma confusa.
            true
        }
        sys::RETRO_ENVIRONMENT_GET_VARIABLE => {
            if data.is_null() {
                return false;
            }
            let var = &mut *(data as *mut sys::retro_variable);
            let Some(key) = coreopts::cstr(var.key) else {
                return false;
            };
            let ptr = st.option_ptr(&key);
            var.value = ptr;
            !ptr.is_null()
        }
        sys::RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
            if !data.is_null() {
                *(data as *mut bool) = st.options_dirty;
            }
            st.options_dirty = false;
            true
        }
        sys::RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION => {
            if !data.is_null() {
                *(data as *mut c_uint) = 2;
            }
            true
        }
        sys::RETRO_ENVIRONMENT_SET_VARIABLES => {
            st.install_core_options(coreopts::parse_variables(
                data as *const sys::retro_variable,
            ));
            true
        }
        sys::RETRO_ENVIRONMENT_SET_CORE_OPTIONS => {
            st.install_core_options(coreopts::parse_v1(
                data as *const sys::retro_core_option_definition,
            ));
            true
        }
        sys::RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL => {
            if !data.is_null() {
                let intl = &*(data as *const sys::retro_core_options_intl);
                st.install_core_options(coreopts::parse_v1(intl.us));
            }
            true
        }
        sys::RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2 => {
            if !data.is_null() {
                let v2 = &*(data as *const sys::retro_core_options_v2);
                st.install_core_options(coreopts::parse_v2(v2.definitions));
            }
            true
        }
        sys::RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL => {
            if !data.is_null() {
                let intl = &*(data as *const sys::retro_core_options_v2_intl);
                if !intl.us.is_null() {
                    st.install_core_options(coreopts::parse_v2((*intl.us).definitions));
                }
            }
            true
        }
        // Reconhecidos, sem efeito ainda.
        sys::RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME
        | sys::RETRO_ENVIRONMENT_SET_ROTATION
        | sys::RETRO_ENVIRONMENT_SET_MESSAGE
        | sys::RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL
        | sys::RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
        | sys::RETRO_ENVIRONMENT_SET_CONTROLLER_INFO
        | sys::RETRO_ENVIRONMENT_SET_SUBSYSTEM_INFO
        | sys::RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY
        | sys::RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO
        | sys::RETRO_ENVIRONMENT_SET_GEOMETRY => true,
        _ => false,
    }
}

pub(crate) unsafe extern "C" fn video_refresh_cb(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    let mut guard = lock();
    let Some(st) = guard.as_mut() else {
        return;
    };

    if data.is_null() {
        // frame duplicado (GET_CAN_DUPE) — mantém o último, sem conteúdo novo.
        st.had_new_frame = false;
        return;
    }

    let fmt = st.pixel_format;
    let row_bytes = width as usize * fmt.bytes_per_pixel() as usize;
    let src = data as *const u8;
    let mut buf = Vec::with_capacity(row_bytes * height as usize);
    for y in 0..height as usize {
        let row = std::slice::from_raw_parts(src.add(y * pitch), row_bytes);
        buf.extend_from_slice(row);
    }

    st.last_frame = Some(RawFrame {
        data: buf,
        width,
        height,
        pitch: row_bytes as u32,
        format: fmt,
    });
    st.had_new_frame = true;
}

pub(crate) unsafe extern "C" fn audio_sample_cb(left: i16, right: i16) {
    if let Some(st) = lock().as_mut() {
        st.audio.push(left);
        st.audio.push(right);
    }
}

pub(crate) unsafe extern "C" fn audio_sample_batch_cb(data: *const i16, frames: usize) -> usize {
    if !data.is_null() {
        if let Some(st) = lock().as_mut() {
            st.audio
                .extend_from_slice(std::slice::from_raw_parts(data, frames * 2));
        }
    }
    frames
}

pub(crate) unsafe extern "C" fn input_poll_cb() {}

pub(crate) unsafe extern "C" fn input_state_cb(
    port: c_uint,
    device: c_uint,
    _index: c_uint,
    id: c_uint,
) -> i16 {
    // Só RetroPad (digital) por enquanto — analógico/mouse/etc na etapa 05+.
    if device == sys::RETRO_DEVICE_JOYPAD && crate::input::retropad().query_id(port as usize, id) {
        1
    } else {
        0
    }
}
