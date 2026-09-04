//! Declarações FFI da API libretro. Valores/layout conferidos contra
//! `libretro-common/include/libretro.h` (RetroArch, master) — não inventar
//! assinatura por suposição.
//!
//! Alguns itens ainda não são consumidos (RETRO_MEMORY_*, GET_LOG_INTERFACE,
//! ...) — entram nas etapas 05/06/08. Mantidos aqui como binding completo.
#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_uint, c_void};

pub const RETRO_API_VERSION: c_uint = 1;

// --- retro_pixel_format ---
pub const RETRO_PIXEL_FORMAT_0RGB1555: c_uint = 0;
pub const RETRO_PIXEL_FORMAT_XRGB8888: c_uint = 1;
pub const RETRO_PIXEL_FORMAT_RGB565: c_uint = 2;

// --- RETRO_ENVIRONMENT_* (subconjunto tratado nesta etapa) ---
pub const RETRO_ENVIRONMENT_EXPERIMENTAL: c_uint = 0x10000;
pub const RETRO_ENVIRONMENT_SET_ROTATION: c_uint = 1;
pub const RETRO_ENVIRONMENT_GET_CAN_DUPE: c_uint = 3;
pub const RETRO_ENVIRONMENT_SET_MESSAGE: c_uint = 6;
pub const RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL: c_uint = 8;
pub const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
pub const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: c_uint = 11;
pub const RETRO_ENVIRONMENT_SET_HW_RENDER: c_uint = 14;
pub const RETRO_ENVIRONMENT_GET_VARIABLE: c_uint = 15;
pub const RETRO_ENVIRONMENT_SET_VARIABLES: c_uint = 16;
pub const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: c_uint = 17;
pub const RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME: c_uint = 18;
pub const RETRO_ENVIRONMENT_GET_LOG_INTERFACE: c_uint = 27;
pub const RETRO_ENVIRONMENT_GET_CONTENT_DIRECTORY: c_uint = 30;
pub const RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY: c_uint = 31;
pub const RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO: c_uint = 32;
pub const RETRO_ENVIRONMENT_SET_SUBSYSTEM_INFO: c_uint = 34;
pub const RETRO_ENVIRONMENT_SET_CONTROLLER_INFO: c_uint = 35;
pub const RETRO_ENVIRONMENT_SET_GEOMETRY: c_uint = 37;
pub const RETRO_ENVIRONMENT_GET_LANGUAGE: c_uint = 39;
pub const RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER: c_uint = 56;
pub const RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION: c_uint = 52;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS: c_uint = 53;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL: c_uint = 54;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY: c_uint = 55;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2: c_uint = 67;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL: c_uint = 68;

/// `RETRO_HW_FRAME_BUFFER_VALID` = `((void*)-1)` — o `data` que o core passa pro
/// `retro_video_refresh` quando o frame está no FBO de HW render (não é ponteiro
/// válido; nunca deref).
pub const RETRO_HW_FRAME_BUFFER_VALID: *const c_void = usize::MAX as *const c_void;

// --- retro_hw_context_type ---
pub const RETRO_HW_CONTEXT_NONE: c_uint = 0;
pub const RETRO_HW_CONTEXT_OPENGL: c_uint = 1;
pub const RETRO_HW_CONTEXT_OPENGLES2: c_uint = 2;
pub const RETRO_HW_CONTEXT_OPENGL_CORE: c_uint = 3;
pub const RETRO_HW_CONTEXT_OPENGLES3: c_uint = 4;
pub const RETRO_HW_CONTEXT_OPENGLES_VERSION: c_uint = 5;
pub const RETRO_HW_CONTEXT_VULKAN: c_uint = 6;

// --- RETRO_MEMORY_* ---
pub const RETRO_MEMORY_SAVE_RAM: c_uint = 0;
pub const RETRO_MEMORY_RTC: c_uint = 1;
pub const RETRO_MEMORY_SYSTEM_RAM: c_uint = 2;
pub const RETRO_MEMORY_VIDEO_RAM: c_uint = 3;

// --- RETRO_DEVICE_* (base) --- (libretro.h)
pub const RETRO_DEVICE_NONE: c_uint = 0;
pub const RETRO_DEVICE_JOYPAD: c_uint = 1;
pub const RETRO_DEVICE_ANALOG: c_uint = 5;

// `retro_input_state_t(port, RETRO_DEVICE_ANALOG, index, id)`:
//   index = LEFT (0) / RIGHT (1) → id X (0) / Y (1), retorno em [-0x8000, 0x7fff]
//   index = BUTTON (2)           → id = RETRO_DEVICE_ID_JOYPAD_*, retorno [0, 0x7fff]
pub const RETRO_DEVICE_INDEX_ANALOG_LEFT: c_uint = 0;
pub const RETRO_DEVICE_INDEX_ANALOG_RIGHT: c_uint = 1;
pub const RETRO_DEVICE_INDEX_ANALOG_BUTTON: c_uint = 2;
pub const RETRO_DEVICE_ID_ANALOG_X: c_uint = 0;
pub const RETRO_DEVICE_ID_ANALOG_Y: c_uint = 1;

/// `id` especial no `RETRO_DEVICE_JOYPAD`: pede o bitmask de todos os botões
/// (só quando o frontend anuncia `GET_INPUT_BITMASKS` — não anunciamos ainda).
pub const RETRO_DEVICE_ID_JOYPAD_MASK: c_uint = 256;

#[repr(C)]
pub struct retro_system_info {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_game_geometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_system_timing {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_system_av_info {
    pub geometry: retro_game_geometry,
    pub timing: retro_system_timing,
}

#[repr(C)]
pub struct retro_game_info {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

// --- Core options (SET_VARIABLES v0 / SET_CORE_OPTIONS v1 / _V2) ---

/// Par `key`/`value` de `RETRO_ENVIRONMENT_{SET_VARIABLES,GET_VARIABLE}` (v0).
#[repr(C)]
pub struct retro_variable {
    pub key: *const c_char,
    pub value: *const c_char,
}

pub const RETRO_NUM_CORE_OPTION_VALUES_MAX: usize = 128;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct retro_core_option_value {
    pub value: *const c_char,
    pub label: *const c_char,
}

/// Definição v1 (`SET_CORE_OPTIONS`). Array terminado por `key == null`.
#[repr(C)]
pub struct retro_core_option_definition {
    pub key: *const c_char,
    pub desc: *const c_char,
    pub info: *const c_char,
    pub values: [retro_core_option_value; RETRO_NUM_CORE_OPTION_VALUES_MAX],
    pub default_value: *const c_char,
}

/// Definição v2 (`SET_CORE_OPTIONS_V2`). Array terminado por `key == null`.
#[repr(C)]
pub struct retro_core_option_v2_definition {
    pub key: *const c_char,
    pub desc: *const c_char,
    pub desc_categorized: *const c_char,
    pub info: *const c_char,
    pub info_categorized: *const c_char,
    pub category_key: *const c_char,
    pub values: [retro_core_option_value; RETRO_NUM_CORE_OPTION_VALUES_MAX],
    pub default_value: *const c_char,
}

#[repr(C)]
pub struct retro_core_option_v2_category {
    pub key: *const c_char,
    pub desc: *const c_char,
    pub info: *const c_char,
}

#[repr(C)]
pub struct retro_core_options_v2 {
    pub categories: *mut retro_core_option_v2_category,
    pub definitions: *mut retro_core_option_v2_definition,
}

#[repr(C)]
pub struct retro_core_options_intl {
    pub us: *const retro_core_option_definition,
    pub local: *const retro_core_option_definition,
}

#[repr(C)]
pub struct retro_core_options_v2_intl {
    pub us: *mut retro_core_options_v2,
    pub local: *mut retro_core_options_v2,
}

pub type retro_proc_address_t = Option<unsafe extern "C" fn()>;
pub type retro_hw_context_reset_t = Option<unsafe extern "C" fn()>;
pub type retro_hw_get_current_framebuffer_t = Option<unsafe extern "C" fn() -> usize>;
pub type retro_hw_get_proc_address_t =
    Option<unsafe extern "C" fn(*const c_char) -> retro_proc_address_t>;

#[repr(C)]
pub struct retro_hw_render_callback {
    pub context_type: c_uint,
    pub context_reset: retro_hw_context_reset_t,
    pub get_current_framebuffer: retro_hw_get_current_framebuffer_t,
    pub get_proc_address: retro_hw_get_proc_address_t,
    pub depth: bool,
    pub stencil: bool,
    pub bottom_left_origin: bool,
    pub version_major: c_uint,
    pub version_minor: c_uint,
    pub cache_context: bool,
    pub context_destroy: retro_hw_context_reset_t,
    pub debug_context: bool,
}

// --- callback typedefs (frontend -> core) ---
pub type retro_environment_t = unsafe extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;
pub type retro_video_refresh_t =
    unsafe extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize);
pub type retro_audio_sample_t = unsafe extern "C" fn(left: i16, right: i16);
pub type retro_audio_sample_batch_t =
    unsafe extern "C" fn(data: *const i16, frames: usize) -> usize;
pub type retro_input_poll_t = unsafe extern "C" fn();
pub type retro_input_state_t =
    unsafe extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16;

// --- exports do core (core -> frontend) ---
pub type retro_set_environment_t = unsafe extern "C" fn(retro_environment_t);
pub type retro_set_video_refresh_t = unsafe extern "C" fn(retro_video_refresh_t);
pub type retro_set_audio_sample_t = unsafe extern "C" fn(retro_audio_sample_t);
pub type retro_set_audio_sample_batch_t = unsafe extern "C" fn(retro_audio_sample_batch_t);
pub type retro_set_input_poll_t = unsafe extern "C" fn(retro_input_poll_t);
pub type retro_set_input_state_t = unsafe extern "C" fn(retro_input_state_t);
pub type retro_init_t = unsafe extern "C" fn();
pub type retro_deinit_t = unsafe extern "C" fn();
pub type retro_api_version_t = unsafe extern "C" fn() -> c_uint;
pub type retro_get_system_info_t = unsafe extern "C" fn(*mut retro_system_info);
pub type retro_get_system_av_info_t = unsafe extern "C" fn(*mut retro_system_av_info);
pub type retro_set_controller_port_device_t = unsafe extern "C" fn(c_uint, c_uint);
pub type retro_reset_t = unsafe extern "C" fn();
pub type retro_run_t = unsafe extern "C" fn();
pub type retro_serialize_size_t = unsafe extern "C" fn() -> usize;
pub type retro_serialize_t = unsafe extern "C" fn(*mut c_void, usize) -> bool;
pub type retro_unserialize_t = unsafe extern "C" fn(*const c_void, usize) -> bool;
pub type retro_load_game_t = unsafe extern "C" fn(*const retro_game_info) -> bool;
pub type retro_unload_game_t = unsafe extern "C" fn();
pub type retro_get_region_t = unsafe extern "C" fn() -> c_uint;
pub type retro_get_memory_data_t = unsafe extern "C" fn(c_uint) -> *mut c_void;
pub type retro_get_memory_size_t = unsafe extern "C" fn(c_uint) -> usize;
