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
use crate::vk_sys;
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
    /// O core desenha com origem bottom-left (default GL). Se `false`, ele já
    /// entrega top-left.
    pub bottom_left_origin: bool,
    /// Callbacks do core: chamados por nós após criar/antes de destruir o
    /// contexto GL.
    pub context_reset: sys::retro_hw_context_reset_t,
    pub context_destroy: sys::retro_hw_context_reset_t,
}

pub(crate) struct FrontendState {
    pub pixel_format: SoftwarePixelFormat,
    pub system_dir: CString,
    pub save_dir: CString,
    pub hw_render: Option<HwRenderRequest>,
    /// Id do FBO GL que o core deve renderizar (`get_current_framebuffer`).
    /// `Some` só depois que o `loader` cria o contexto GL.
    pub hw_fbo: Option<u32>,
    /// `retro_hw_render_context_negotiation_interface_vulkan*` do core
    /// (`SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`). `usize` cru pra ser
    /// `Send`; memória é `static` do core (viva pelo processo). Etapa 12.
    pub vk_negotiation: Option<usize>,
    /// `retro_hw_render_interface_vulkan*` que publicamos — o
    /// `loader::setup_vk_context` seta antes do `context_reset`, o core lê em
    /// `GET_HW_RENDER_INTERFACE`. `usize` cru; aponta pro `Box<VkFrameBridge>`
    /// do `DesktopCore` (vivo enquanto o core estiver carregado).
    pub vk_interface_ptr: Option<usize>,
    /// `GET_PREFERRED_HW_RENDER` responde Vulkan: na rota in-process
    /// (`DesktopCoreLoader::vulkan_only`) ou com `REEMU_HW=vulkan`.
    pub prefer_vulkan: bool,
    /// Dimensão (`w`, `h`) do último frame de HW render — o core só passa isso
    /// no `video_refresh` com `data == RETRO_HW_FRAME_BUFFER_VALID`.
    pub hw_frame: Option<(u32, u32)>,
    /// Rotação da tela pedida via `SET_ROTATION` (0/90/180/270°, anti-horário).
    pub rotation_degrees: u16,
    /// `(fps, sample_rate)` novos pedidos pelo core em runtime via
    /// `SET_SYSTEM_AV_INFO` — a thread do core drena e reconfigura o pacing e o
    /// resampler. `parallel_n64` faz isso logo após o load (32040 → ~26807 Hz).
    pub av_update: Option<(f64, f64)>,
    /// Geometria nova pedida em runtime (`SET_GEOMETRY`, ou a `geometry` de
    /// um `SET_SYSTEM_AV_INFO`): `(base_width, base_height, aspect_ratio)`.
    /// O `DesktopCore` aplica antes de montar o próximo quadro — é o que leva
    /// a proporção nova até a tela (libretro.h: `SET_GEOMETRY` é o caminho
    /// indicado pra mudar a proporção sem reiniciar o vídeo).
    pub geometry_update: Option<(u32, u32, f32)>,
    /// Portas declaradas pelo core em `SET_CONTROLLER_INFO` (0 = não
    /// declarou). O loader liga um `RETRO_DEVICE_JOYPAD` em cada uma depois
    /// do `retro_load_game`, como o RetroArch.
    pub controller_ports: u32,
    pub last_frame: Option<RawFrame>,
    /// Buffer de um frame já consumido, devolvido por
    /// `DesktopCore::recycle_frame_buffer` — o próximo `video_refresh_cb`
    /// reusa em vez de alocar um `Vec` novo por quadro.
    pub spare_frame: Vec<u8>,
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
    /// Opções que o PRÓPRIO core trocou (`SET_VARIABLE`), ainda não
    /// entregues ao frontend pra guardar (`take_core_set_options`).
    pub core_set_options: Vec<(String, String)>,
    /// `retro_game_info_ext` do load em andamento (`GET_GAME_INFO_EXT` só
    /// vale dentro do `retro_load_game`); `None` fora dele.
    pub game_info_ext: Option<GameInfoExt>,
    /// `SET_CONTENT_INFO_OVERRIDE`: extensão (minúscula) → (`need_fullpath`,
    /// `persistent_data`). Só a 1ª ocorrência de cada extensão vale.
    pub content_overrides: Vec<(String, bool, bool)>,
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
            hw_fbo: None,
            vk_negotiation: None,
            vk_interface_ptr: None,
            prefer_vulkan: matches!(
                std::env::var("REEMU_HW")
                    .map(|v| v.trim().to_ascii_lowercase())
                    .as_deref(),
                Ok("vulkan") | Ok("vk")
            ),
            hw_frame: None,
            rotation_degrees: 0,
            av_update: None,
            geometry_update: None,
            controller_ports: 0,
            last_frame: None,
            spare_frame: Vec::new(),
            had_new_frame: false,
            audio: Vec::new(),
            save_pending: false,
            core_options: Vec::new(),
            option_values: coreopts::take_pending_core_option_values(),
            options_dirty: false,
            core_set_options: Vec::new(),
            game_info_ext: None,
            content_overrides: Vec::new(),
            option_value_cache: HashMap::new(),
        }
    }

    /// Instala o schema declarado pelo core. Mantém valores já escolhidos que
    /// ainda são válidos; um pré-setado que não bate exato tenta casar
    /// case-insensitive (ex.: "disabled" vs "Disabled" entre versões de core);
    /// o resto cai no default.
    pub(crate) fn install_core_options(&mut self, mut opts: Vec<CoreOption>) {
        coreopts::apply_frontend_defaults(&mut opts);
        for o in &opts {
            let cur = self
                .option_values
                .entry(o.key.clone())
                .or_insert_with(|| o.default.clone());
            if !o.values.contains(cur) {
                if let Some(m) = o.values.iter().find(|v| v.eq_ignore_ascii_case(cur)) {
                    log::info!("core option '{}': '{}' → '{}' (case)", o.key, cur, m);
                    *cur = m.clone();
                } else {
                    log::warn!(
                        "core option '{}': valor '{}' inválido — opções: {:?}; caindo pro default '{}'",
                        o.key,
                        cur,
                        o.values,
                        o.default
                    );
                    *cur = o.default.clone();
                }
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
        // Zera o input global — senão um botão/eixo segurado no fim de um jogo
        // vaza pro próximo core carregado.
        crate::input::retropad().clear();
        crate::input::analog().clear();
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
    crate::input::retropad().clear();
    crate::input::analog().clear();
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
        // Suportado: o `input_state_cb` responde `RETRO_DEVICE_ID_JOYPAD_MASK`
        // com todos os botões de uma vez (uma chamada por porta em vez de 16).
        // A doc diz que o ponteiro é ignorado; alguns cores passam um `bool*`
        // mesmo assim, então marcamos `true` nele quando vem.
        // `struct retro_log_callback { retro_log_printf_t log; }`. Tem que
        // existir: cores como o VBA-M chamam `log_cb` sem checar se é nulo
        // depois de outras perguntas (ex.: GET_INPUT_BITMASKS) — sem isto
        // o `retro_init` dele caía com ponteiro nulo.
        sys::RETRO_ENVIRONMENT_GET_LOG_INTERFACE => {
            if data.is_null() {
                return false;
            }
            *(data as *mut *const c_void) = reemu_log_printf as *const c_void;
            true
        }
        // VFS v3 sobre `std::fs` (ver `vfs.rs`).
        sys::RETRO_ENVIRONMENT_GET_VFS_INTERFACE => crate::vfs::get_interface(data),
        sys::RETRO_ENVIRONMENT_GET_INPUT_BITMASKS => {
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
            let cb = &mut *(data as *mut sys::retro_hw_render_callback);
            st.hw_render = Some(HwRenderRequest {
                context_type: cb.context_type,
                version_major: cb.version_major,
                version_minor: cb.version_minor,
                depth: cb.depth,
                stencil: cb.stencil,
                bottom_left_origin: cb.bottom_left_origin,
                context_reset: cb.context_reset,
                context_destroy: cb.context_destroy,
            });
            let is_gl = matches!(
                cb.context_type,
                sys::RETRO_HW_CONTEXT_OPENGL
                    | sys::RETRO_HW_CONTEXT_OPENGL_CORE
                    | sys::RETRO_HW_CONTEXT_OPENGLES2
                    | sys::RETRO_HW_CONTEXT_OPENGLES3
                    | sys::RETRO_HW_CONTEXT_OPENGLES_VERSION
            );
            if is_gl {
                // O core lê estes ponteiros pra pegar o FBO e resolver símbolos.
                cb.get_current_framebuffer = Some(get_current_framebuffer_cb);
                cb.get_proc_address = Some(get_proc_address_cb);
            }
            // Vulkan: aceita a declaração; `loader::setup_vk_context` cria o
            // contexto depois (etapa 12).
            true
        }
        sys::RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER => {
            if !data.is_null() {
                *(data as *mut c_uint) = if st.prefer_vulkan {
                    sys::RETRO_HW_CONTEXT_VULKAN
                } else {
                    sys::RETRO_HW_CONTEXT_OPENGL_CORE
                };
            }
            true
        }
        vk_sys::RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE => {
            if data.is_null() {
                return false;
            }
            // Só o cabeçalho comum interessa aqui — valida o tipo antes de guardar.
            let hdr = &*(data as *const vk_sys::retro_hw_render_context_negotiation_interface);
            if hdr.interface_type != vk_sys::RETRO_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE_VULKAN {
                return false;
            }
            st.vk_negotiation = Some(data as usize);
            log::info!(
                "negociação Vulkan registrada (interface_version {})",
                hdr.interface_version
            );
            true
        }
        vk_sys::RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE => {
            let Some(ptr) = st.vk_interface_ptr else {
                return false; // contexto ainda não montado
            };
            if data.is_null() {
                return false;
            }
            *(data as *mut *const c_void) = ptr as *const c_void;
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
        // "After changing a core option value with this callback, it will be
        // reflected in the frontend and GET_VARIABLE_UPDATE will return true";
        // `data` nulo = só pergunta se a chamada existe (`libretro.h`). O
        // PPSSPP usa isto pra fixar o MAC sorteado na 1ª vez
        // (`ppsspp_change_mac_address01..12`); sem guardar, cada sessão
        // sorteava outro.
        sys::RETRO_ENVIRONMENT_SET_VARIABLE => {
            if data.is_null() {
                return true;
            }
            let var = &*(data as *const sys::retro_variable);
            let (Some(key), Some(value)) = (coreopts::cstr(var.key), coreopts::cstr(var.value))
            else {
                return false;
            };
            if key.is_empty() || value.is_empty() || !st.set_option_value(&key, &value) {
                return false;
            }
            log::info!("core trocou a opção '{key}' para '{value}'");
            st.core_set_options.push((key, value));
            true
        }
        // `const struct retro_game_info_ext **`: "may only be called inside
        // retro_load_game()" (`libretro.h`). O RustyNES não carrega sem.
        // Array terminado em `{ NULL, false, false }`; `data` nulo só testa se
        // existe. "If an extension is listed multiple times ... only the first
        // instance will be registered" (`libretro.h`). Par do
        // `GET_GAME_INFO_EXT`: o FCEUmm pede o `.nes` na memória por aqui e
        // lê o buffer de lá.
        sys::RETRO_ENVIRONMENT_SET_CONTENT_INFO_OVERRIDE => {
            let mut p = data as *const sys::retro_system_content_info_override;
            while !p.is_null() {
                let o = &*p;
                let Some(exts) = coreopts::cstr(o.extensions) else {
                    break;
                };
                for e in exts.split('|').filter(|e| !e.is_empty()) {
                    let e = e.to_ascii_lowercase();
                    if !st.content_overrides.iter().any(|(x, _, _)| *x == e) {
                        st.content_overrides
                            .push((e, o.need_fullpath, o.persistent_data));
                    }
                }
                p = p.add(1);
            }
            true
        }
        sys::RETRO_ENVIRONMENT_GET_GAME_INFO_EXT => {
            let Some(ext) = st.game_info_ext.as_ref() else {
                return false;
            };
            if !data.is_null() {
                *(data as *mut *const sys::retro_game_info_ext) = &*ext.info;
            }
            true
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
        sys::RETRO_ENVIRONMENT_SET_ROTATION => {
            // `data` = *const c_uint, 0..=3 (× 90° anti-horário).
            if !data.is_null() {
                let turns = (*(data as *const c_uint)) % 4;
                st.rotation_degrees = (turns as u16) * 90;
                log::info!("SET_ROTATION: {}° (anti-horário)", st.rotation_degrees);
            }
            true
        }
        sys::RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO => {
            // Timing novo (fps/sample_rate) → a thread do core drena via
            // `DesktopCore::take_av_update` e reconfigura pacing + resampler.
            // Geometry nova entra pelo `SET_GEOMETRY` / next frame.
            if !data.is_null() {
                let av = &*(data as *const sys::retro_system_av_info);
                if av.timing.fps > 0.0 && av.timing.sample_rate > 0.0 {
                    log::info!(
                        "SET_SYSTEM_AV_INFO em runtime: fps={:.3} sample_rate={:.0} {}x{}",
                        av.timing.fps,
                        av.timing.sample_rate,
                        av.geometry.base_width,
                        av.geometry.base_height,
                    );
                    st.av_update = Some((av.timing.fps, av.timing.sample_rate));
                }
                st.geometry_update = Some((
                    av.geometry.base_width,
                    av.geometry.base_height,
                    av.geometry.aspect_ratio,
                ));
            }
            true
        }
        sys::RETRO_ENVIRONMENT_SET_GEOMETRY => {
            if !data.is_null() {
                let g = &*(data as *const sys::retro_game_geometry);
                log::debug!(
                    "SET_GEOMETRY: {}x{} AR={:.3}",
                    g.base_width,
                    g.base_height,
                    g.aspect_ratio
                );
                // `max_width`/`max_height` são ignorados neste comando
                // (libretro.h) — só base e proporção.
                st.geometry_update = Some((g.base_width, g.base_height, g.aspect_ratio));
            }
            true
        }
        // Array terminado num `retro_controller_info` zerado; cada item é
        // uma porta do console emulado (libretro.h). Teto de 16 pra não
        // correr memória se o core esquecer o terminador.
        sys::RETRO_ENVIRONMENT_SET_CONTROLLER_INFO => {
            if !data.is_null() {
                let arr = data as *const sys::retro_controller_info;
                let mut n = 0u32;
                while n < 16 && !(*arr.add(n as usize)).types.is_null() {
                    n += 1;
                }
                st.controller_ports = n;
            }
            true
        }
        // Reconhecidos, sem efeito ainda.
        sys::RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME
        | sys::RETRO_ENVIRONMENT_SET_MESSAGE
        | sys::RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL
        | sys::RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
        | sys::RETRO_ENVIRONMENT_SET_SUBSYSTEM_INFO
        | sys::RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY => true,
        other => {
            // Uma linha por comando desconhecido (por processo): quando um core
            // desiste do load, mostra o que ele pediu e recebeu `false`.
            static SEEN: std::sync::Mutex<Vec<c_uint>> = std::sync::Mutex::new(Vec::new());
            let mut seen = SEEN.lock().unwrap_or_else(|p| p.into_inner());
            if !seen.contains(&other) {
                seen.push(other);
                log::info!(
                    "environment {} (0x{other:x}) não tratado — respondido false",
                    other & !0x10000
                );
            }
            false
        }
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

    if std::ptr::eq(data, sys::RETRO_HW_FRAME_BUFFER_VALID) {
        // HW render: o frame está no FBO GL, não neste ponteiro. Só registra as
        // dimensões — o `DesktopCore::next_frame` lê o FBO (readback ou interop).
        st.hw_frame = Some((width, height));
        st.had_new_frame = true;
        return;
    }

    if data.is_null() {
        // frame duplicado (GET_CAN_DUPE) — mantém o último, sem conteúdo novo.
        st.had_new_frame = false;
        return;
    }

    let fmt = st.pixel_format;
    let row_bytes = width as usize * fmt.bytes_per_pixel() as usize;
    let src = data as *const u8;
    let mut buf = std::mem::take(&mut st.spare_frame);
    buf.clear();
    buf.reserve(row_bytes * height as usize);
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

/// `retro_hw_get_current_framebuffer_t` — o core chama a cada frame pra saber
/// onde renderizar. `0` = default framebuffer (antes do contexto GL existir).
pub(crate) unsafe extern "C" fn get_current_framebuffer_cb() -> usize {
    lock()
        .as_ref()
        .and_then(|st| st.hw_fbo)
        .map_or(0, |fbo| fbo as usize)
}

/// `retro_hw_get_proc_address_t` — resolve símbolos GL pro core via EGL.
pub(crate) unsafe extern "C" fn get_proc_address_cb(
    sym: *const c_char,
) -> sys::retro_proc_address_t {
    crate::gl_context::resolve_proc(sym)
}

pub(crate) unsafe extern "C" fn input_poll_cb() {}

pub(crate) unsafe extern "C" fn input_state_cb(
    port: c_uint,
    device: c_uint,
    index: c_uint,
    id: c_uint,
) -> i16 {
    let port = port as usize;
    match device {
        sys::RETRO_DEVICE_JOYPAD if id == sys::RETRO_DEVICE_ID_JOYPAD_MASK => {
            // Bit N = botão de id N — é exatamente como o estado já é guardado.
            crate::input::retropad().mask(port) as i16
        }
        sys::RETRO_DEVICE_JOYPAD => i16::from(crate::input::retropad().query_id(port, id)),
        sys::RETRO_DEVICE_ANALOG => {
            let analog = crate::input::analog();
            analog.mark_used();
            match index {
                sys::RETRO_DEVICE_INDEX_ANALOG_LEFT => analog.axis(port, 0, id),
                sys::RETRO_DEVICE_INDEX_ANALOG_RIGHT => analog.axis(port, 1, id),
                sys::RETRO_DEVICE_INDEX_ANALOG_BUTTON => {
                    // Pressão do botão (id = RETRO_DEVICE_ID_JOYPAD_*), em
                    // [0, 0x7fff] (libretro.h). L2/R2 do controle trazem a
                    // pressão real; sem ela (teclado, outros botões) o
                    // digital vale 0 ou 0x7fff.
                    let (l2, r2) = analog.triggers(port);
                    let pressure = match id {
                        sys::RETRO_DEVICE_ID_JOYPAD_L2 => l2,
                        sys::RETRO_DEVICE_ID_JOYPAD_R2 => r2,
                        _ => 0,
                    };
                    if pressure > 0 {
                        pressure as i16
                    } else {
                        i16::from(crate::input::retropad().query_id(port, id)) * 0x7fff
                    }
                }
                _ => 0,
            }
        }
        // Mouse / teclado / lightgun / pointer: etapa 05+.
        _ => 0,
    }
}

extern "C" {
    /// Em src/log_shim.c — o `retro_log_printf_t` entregue aos cores.
    fn reemu_log_printf(level: c_uint, fmt: *const c_char, ...);
}

/// Chamado por `reemu_log_printf` com a mensagem já formatada. Níveis do
/// `enum retro_log_level` (libretro.h): 0 debug, 1 info, 2 warn, 3 error.
#[no_mangle]
extern "C" fn reemu_core_log(level: c_uint, msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    // SAFETY: o shim passa um buffer seu, terminado em NUL (vsnprintf).
    let text = unsafe { std::ffi::CStr::from_ptr(msg) }.to_string_lossy();
    let text = text.trim_end();
    if text.is_empty() {
        return;
    }
    match level {
        0 => log::debug!(target: "core", "{text}"),
        1 => log::info!(target: "core", "{text}"),
        2 => log::warn!(target: "core", "{text}"),
        _ => log::error!(target: "core", "{text}"),
    }
}

/// Dono das strings e da `retro_game_info_ext` entregue no
/// `GET_GAME_INFO_EXT` (os ponteiros apontam pros `CString` daqui).
pub(crate) struct GameInfoExt {
    _strings: Vec<CString>,
    info: Box<sys::retro_game_info_ext>,
}

// SAFETY: só ponteiros pros próprios `CString`/buffer do load, usados na
// thread que chama o `retro_load_game`, sob o lock do estado global.
unsafe impl Send for GameInfoExt {}

impl GameInfoExt {
    /// `original`: o caminho que o usuário abriu (o `.zip`, se for o caso).
    /// `content`: o arquivo que o core recebe (o extraído, se houver).
    /// `entry`: nome dentro do arquivo comprimido. `data`: buffer passado no
    /// `retro_game_info` (vale até o `retro_load_game` voltar).
    pub(crate) fn new(
        original: &std::path::Path,
        content: &std::path::Path,
        entry: Option<&str>,
        data: Option<&[u8]>,
        persistent_data: bool,
    ) -> Self {
        let c = |s: &str| CString::new(s).unwrap_or_default();
        let lossy = |p: &std::path::Path| p.to_string_lossy().into_owned();
        let in_archive = entry.is_some();
        let stem = |p: &std::path::Path| {
            p.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        // Extensão do conteúdo (de dentro do arquivo, se houver), minúscula.
        let ext = std::path::Path::new(entry.unwrap_or(&lossy(content)))
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let strings = vec![
            c(&lossy(content)),
            c(&if in_archive {
                lossy(original)
            } else {
                String::new()
            }),
            c(entry.unwrap_or("")),
            c(&original.parent().map(lossy).unwrap_or_default()),
            // RetroArch: com arquivo comprimido, o nome é o do `.zip`.
            c(&stem(original)),
            c(&ext),
        ];
        let ptr = |i: usize, keep: bool| {
            if keep {
                strings[i].as_ptr()
            } else {
                std::ptr::null()
            }
        };
        let info = Box::new(sys::retro_game_info_ext {
            full_path: ptr(0, true),
            archive_path: ptr(1, in_archive),
            archive_file: ptr(2, in_archive),
            dir: ptr(3, true),
            name: ptr(4, true),
            ext: ptr(5, true),
            meta: std::ptr::null(),
            data: data.map_or(std::ptr::null(), |d| d.as_ptr().cast()),
            size: data.map_or(0, |d| d.len()),
            file_in_archive: in_archive,
            persistent_data: persistent_data && data.is_some(),
        });
        GameInfoExt {
            _strings: strings,
            info,
        }
    }
}

#[cfg(test)]
mod input_state_tests {
    use super::*;
    use domain::input::RetroPadButton;

    #[test]
    fn analog_button_uses_trigger_pressure_and_falls_back_to_digital() {
        // porta 3: não disputa os globais com outros testes
        let q = |id| unsafe {
            input_state_cb(
                3,
                sys::RETRO_DEVICE_ANALOG,
                sys::RETRO_DEVICE_INDEX_ANALOG_BUTTON,
                id,
            )
        };
        crate::input::analog().set_triggers(3, 0, 12_000);
        assert_eq!(q(sys::RETRO_DEVICE_ID_JOYPAD_R2), 12_000);
        assert_eq!(q(sys::RETRO_DEVICE_ID_JOYPAD_L2), 0);
        // sem pressão (teclado): o digital vale 0x7fff
        crate::input::analog().set_triggers(3, 0, 0);
        crate::input::retropad().set(3, RetroPadButton::R2, true);
        assert_eq!(q(sys::RETRO_DEVICE_ID_JOYPAD_R2), 0x7fff);
        crate::input::retropad().set(3, RetroPadButton::R2, false);
        assert_eq!(q(sys::RETRO_DEVICE_ID_JOYPAD_R2), 0);
    }
}
