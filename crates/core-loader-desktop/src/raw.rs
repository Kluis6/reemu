//! `RawCore`: a `libloading::Library` do core + todos os símbolos `retro_*`
//! resolvidos como ponteiros de função. A `Library` fica guardada aqui pra
//! manter os ponteiros válidos enquanto o `RawCore` existir.

use crate::sys;
use domain::core_loader::CoreLoadError;
use std::path::Path;

macro_rules! resolve {
    ($lib:expr, $name:literal, $ty:ty) => {{
        let sym: libloading::Symbol<$ty> = unsafe {
            $lib.get(concat!($name, "\0").as_bytes())
                .map_err(|e| CoreLoadError::LoadFailed(format!("símbolo `{}`: {e}", $name)))?
        };
        *sym
    }};
}

#[allow(dead_code)]
pub struct RawCore {
    // Ordem importa: os ponteiros abaixo apontam pra dentro desta lib.
    // Drop roda em ordem de declaração, então a lib é a última.
    pub set_environment: sys::retro_set_environment_t,
    pub set_video_refresh: sys::retro_set_video_refresh_t,
    pub set_audio_sample: sys::retro_set_audio_sample_t,
    pub set_audio_sample_batch: sys::retro_set_audio_sample_batch_t,
    pub set_input_poll: sys::retro_set_input_poll_t,
    pub set_input_state: sys::retro_set_input_state_t,
    pub init: sys::retro_init_t,
    pub deinit: sys::retro_deinit_t,
    pub api_version: sys::retro_api_version_t,
    pub get_system_info: sys::retro_get_system_info_t,
    pub get_system_av_info: sys::retro_get_system_av_info_t,
    pub set_controller_port_device: sys::retro_set_controller_port_device_t,
    pub reset: sys::retro_reset_t,
    pub run: sys::retro_run_t,
    pub serialize_size: sys::retro_serialize_size_t,
    pub serialize: sys::retro_serialize_t,
    pub unserialize: sys::retro_unserialize_t,
    pub load_game: sys::retro_load_game_t,
    pub unload_game: sys::retro_unload_game_t,
    pub get_region: sys::retro_get_region_t,
    pub get_memory_data: sys::retro_get_memory_data_t,
    pub get_memory_size: sys::retro_get_memory_size_t,
    lib: libloading::Library,
}

impl RawCore {
    pub fn open(path: &Path) -> Result<Self, CoreLoadError> {
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|e| CoreLoadError::LoadFailed(format!("dlopen {path:?}: {e}")))?;

        let core = RawCore {
            set_environment: resolve!(lib, "retro_set_environment", sys::retro_set_environment_t),
            set_video_refresh: resolve!(
                lib,
                "retro_set_video_refresh",
                sys::retro_set_video_refresh_t
            ),
            set_audio_sample: resolve!(
                lib,
                "retro_set_audio_sample",
                sys::retro_set_audio_sample_t
            ),
            set_audio_sample_batch: resolve!(
                lib,
                "retro_set_audio_sample_batch",
                sys::retro_set_audio_sample_batch_t
            ),
            set_input_poll: resolve!(lib, "retro_set_input_poll", sys::retro_set_input_poll_t),
            set_input_state: resolve!(lib, "retro_set_input_state", sys::retro_set_input_state_t),
            init: resolve!(lib, "retro_init", sys::retro_init_t),
            deinit: resolve!(lib, "retro_deinit", sys::retro_deinit_t),
            api_version: resolve!(lib, "retro_api_version", sys::retro_api_version_t),
            get_system_info: resolve!(lib, "retro_get_system_info", sys::retro_get_system_info_t),
            get_system_av_info: resolve!(
                lib,
                "retro_get_system_av_info",
                sys::retro_get_system_av_info_t
            ),
            set_controller_port_device: resolve!(
                lib,
                "retro_set_controller_port_device",
                sys::retro_set_controller_port_device_t
            ),
            reset: resolve!(lib, "retro_reset", sys::retro_reset_t),
            run: resolve!(lib, "retro_run", sys::retro_run_t),
            serialize_size: resolve!(lib, "retro_serialize_size", sys::retro_serialize_size_t),
            serialize: resolve!(lib, "retro_serialize", sys::retro_serialize_t),
            unserialize: resolve!(lib, "retro_unserialize", sys::retro_unserialize_t),
            load_game: resolve!(lib, "retro_load_game", sys::retro_load_game_t),
            unload_game: resolve!(lib, "retro_unload_game", sys::retro_unload_game_t),
            get_region: resolve!(lib, "retro_get_region", sys::retro_get_region_t),
            get_memory_data: resolve!(lib, "retro_get_memory_data", sys::retro_get_memory_data_t),
            get_memory_size: resolve!(lib, "retro_get_memory_size", sys::retro_get_memory_size_t),
            lib,
        };
        Ok(core)
    }
}
