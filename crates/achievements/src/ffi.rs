//! Declarações FFI mínimas conferidas contra os cabeçalhos oficiais do
//! `RetroAchievements/rcheevos` `v12.5.0` (`include/rc_client.h`,
//! `include/rc_api_request.h`, `include/rc_consoles.h`, `include/rc_error.h`,
//! `include/rc_hash.h` e `src/rc_libretro.h`) e `libretro.h`.
#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_int, c_uint, c_void};

#[cfg(windows)]
pub type c_time_t = i64;
#[cfg(not(windows))]
pub type c_time_t = std::os::raw::c_long;

#[repr(C)]
pub struct rc_client_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct rc_client_async_handle_t {
    _private: [u8; 0],
}

pub const RC_OK: c_int = 0;
pub const RC_NO_GAME_LOADED: c_int = -29;
pub const RC_INVALID_CREDENTIALS: c_int = -34;
pub const RC_EXPIRED_TOKEN: c_int = -35;

pub const RC_CLIENT_LOG_LEVEL_NONE: c_int = 0;
pub const RC_CLIENT_LOG_LEVEL_ERROR: c_int = 1;
pub const RC_CLIENT_LOG_LEVEL_WARN: c_int = 2;
pub const RC_CLIENT_LOG_LEVEL_INFO: c_int = 3;

pub const RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE: c_int = 1 << 0;
pub const RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE_AND_UNOFFICIAL: c_int = (1 << 0) | (1 << 1);
pub const RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_LOCK_STATE: c_int = 0;
pub const RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_PROGRESS: c_int = 1;

pub const RC_CLIENT_ACHIEVEMENT_STATE_ACTIVE: u8 = 1;
pub const RC_CLIENT_ACHIEVEMENT_STATE_UNLOCKED: u8 = 2;
pub const RC_CLIENT_ACHIEVEMENT_STATE_DISABLED: u8 = 3;

pub const RC_CLIENT_ACHIEVEMENT_UNLOCKED_NONE: u8 = 0;
pub const RC_CLIENT_ACHIEVEMENT_UNLOCKED_SOFTCORE: u8 = 1 << 0;
pub const RC_CLIENT_ACHIEVEMENT_UNLOCKED_HARDCORE: u8 = 1 << 1;

pub const RC_CLIENT_ACHIEVEMENT_TYPE_STANDARD: u8 = 0;
pub const RC_CLIENT_ACHIEVEMENT_TYPE_MISSABLE: u8 = 1;
pub const RC_CLIENT_ACHIEVEMENT_TYPE_PROGRESSION: u8 = 2;
pub const RC_CLIENT_ACHIEVEMENT_TYPE_WIN: u8 = 3;

pub const RC_CLIENT_LEADERBOARD_STATE_INACTIVE: u8 = 0;
pub const RC_CLIENT_LEADERBOARD_STATE_ACTIVE: u8 = 1;
pub const RC_CLIENT_LEADERBOARD_STATE_TRACKING: u8 = 2;
pub const RC_CLIENT_LEADERBOARD_STATE_DISABLED: u8 = 3;

pub const RC_CLIENT_LEADERBOARD_FORMAT_TIME: u8 = 0;
pub const RC_CLIENT_LEADERBOARD_FORMAT_SCORE: u8 = 1;
pub const RC_CLIENT_LEADERBOARD_FORMAT_VALUE: u8 = 2;

pub const RC_CLIENT_LEADERBOARD_LIST_GROUPING_NONE: c_int = 0;
pub const RC_CLIENT_LEADERBOARD_LIST_GROUPING_TRACKING: c_int = 1;

pub const RC_CLIENT_LEADERBOARD_DISPLAY_SIZE: usize = 24;

pub const RC_CLIENT_EVENT_TYPE_NONE: u32 = 0;
pub const RC_CLIENT_EVENT_ACHIEVEMENT_TRIGGERED: u32 = 1;
pub const RC_CLIENT_EVENT_LEADERBOARD_STARTED: u32 = 2;
pub const RC_CLIENT_EVENT_LEADERBOARD_FAILED: u32 = 3;
pub const RC_CLIENT_EVENT_LEADERBOARD_SUBMITTED: u32 = 4;
pub const RC_CLIENT_EVENT_ACHIEVEMENT_CHALLENGE_INDICATOR_SHOW: u32 = 5;
pub const RC_CLIENT_EVENT_ACHIEVEMENT_CHALLENGE_INDICATOR_HIDE: u32 = 6;
pub const RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_SHOW: u32 = 7;
pub const RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_HIDE: u32 = 8;
pub const RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_UPDATE: u32 = 9;
pub const RC_CLIENT_EVENT_LEADERBOARD_TRACKER_SHOW: u32 = 10;
pub const RC_CLIENT_EVENT_LEADERBOARD_TRACKER_HIDE: u32 = 11;
pub const RC_CLIENT_EVENT_LEADERBOARD_TRACKER_UPDATE: u32 = 12;
pub const RC_CLIENT_EVENT_LEADERBOARD_SCOREBOARD: u32 = 13;
pub const RC_CLIENT_EVENT_RESET: u32 = 14;
pub const RC_CLIENT_EVENT_GAME_COMPLETED: u32 = 15;
pub const RC_CLIENT_EVENT_SERVER_ERROR: u32 = 16;
pub const RC_CLIENT_EVENT_DISCONNECTED: u32 = 17;
pub const RC_CLIENT_EVENT_RECONNECTED: u32 = 18;

pub const RC_API_SERVER_RESPONSE_CLIENT_ERROR: c_int = -1;
pub const RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR: c_int = -2;

/// Prefixo de `rc_api_request_t` (`include/rc_api_request.h`): lido apenas por
/// ponteiro em `rc_client_server_call_t`.
#[repr(C)]
pub struct rc_api_request_t {
    pub url: *const c_char,
    pub post_data: *const c_char,
    pub content_type: *const c_char,
}

#[repr(C)]
pub struct rc_api_server_response_t {
    pub body: *const c_char,
    pub body_length: usize,
    pub http_status_code: c_int,
}

#[repr(C)]
pub struct rc_client_user_t {
    pub display_name: *const c_char,
    pub username: *const c_char,
    pub token: *const c_char,
    pub score: u32,
    pub score_softcore: u32,
    pub num_unread_messages: u32,
    pub avatar_url: *const c_char,
    pub avatar_last_updated: c_time_t,
}

#[repr(C)]
#[derive(Default)]
pub struct rc_client_user_game_summary_t {
    pub num_core_achievements: u32,
    pub num_unofficial_achievements: u32,
    pub num_unlocked_achievements: u32,
    pub num_unsupported_achievements: u32,
    pub points_core: u32,
    pub points_unlocked: u32,
    pub beaten_time: c_time_t,
    pub completed_time: c_time_t,
}

#[repr(C)]
pub struct rc_client_game_t {
    pub id: u32,
    pub console_id: u32,
    pub title: *const c_char,
    pub hash: *const c_char,
    pub badge_name: *const c_char,
    pub badge_url: *const c_char,
}

#[repr(C)]
pub struct rc_client_achievement_t {
    pub title: *const c_char,
    pub description: *const c_char,
    pub badge_name: [c_char; 8],
    pub measured_progress: [c_char; 24],
    pub measured_percent: f32,
    pub id: u32,
    pub points: u32,
    pub unlock_time: c_time_t,
    pub state: u8,
    pub category: u8,
    pub bucket: u8,
    pub unlocked: u8,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    pub r#type: u8,
    pub badge_url: *const c_char,
    pub badge_locked_url: *const c_char,
}

#[repr(C)]
pub struct rc_client_achievement_bucket_t {
    pub achievements: *const *const rc_client_achievement_t,
    pub num_achievements: u32,
    pub label: *const c_char,
    pub subset_id: u32,
    pub bucket_type: u8,
}

#[repr(C)]
pub struct rc_client_achievement_list_t {
    pub buckets: *const rc_client_achievement_bucket_t,
    pub num_buckets: u32,
}

#[repr(C)]
pub struct rc_client_leaderboard_t {
    pub title: *const c_char,
    pub description: *const c_char,
    pub tracker_value: *const c_char,
    pub id: u32,
    pub state: u8,
    pub format: u8,
    pub lower_is_better: u8,
}

#[repr(C)]
pub struct rc_client_leaderboard_tracker_t {
    pub display: [c_char; RC_CLIENT_LEADERBOARD_DISPLAY_SIZE],
    pub id: u32,
}

#[repr(C)]
pub struct rc_client_leaderboard_bucket_t {
    pub leaderboards: *const *const rc_client_leaderboard_t,
    pub num_leaderboards: u32,
    pub label: *const c_char,
    pub subset_id: u32,
    pub bucket_type: u8,
}

#[repr(C)]
pub struct rc_client_leaderboard_list_t {
    pub buckets: *const rc_client_leaderboard_bucket_t,
    pub num_buckets: u32,
}

#[repr(C)]
pub struct rc_client_leaderboard_scoreboard_entry_t {
    pub username: *const c_char,
    pub rank: u32,
    pub score: [c_char; RC_CLIENT_LEADERBOARD_DISPLAY_SIZE],
}

#[repr(C)]
pub struct rc_client_leaderboard_scoreboard_t {
    pub leaderboard_id: u32,
    pub submitted_score: [c_char; RC_CLIENT_LEADERBOARD_DISPLAY_SIZE],
    pub best_score: [c_char; RC_CLIENT_LEADERBOARD_DISPLAY_SIZE],
    pub new_rank: u32,
    pub num_entries: u32,
    pub top_entries: *const rc_client_leaderboard_scoreboard_entry_t,
    pub num_top_entries: u32,
}

#[repr(C)]
pub struct rc_client_server_error_t {
    pub error_message: *const c_char,
    pub api: *const c_char,
    pub result: c_int,
    pub related_id: u32,
}

#[repr(C)]
pub struct rc_client_event_t {
    pub r#type: u32,
    pub achievement: *const rc_client_achievement_t,
    pub leaderboard: *const rc_client_leaderboard_t,
    pub leaderboard_tracker: *const rc_client_leaderboard_tracker_t,
    pub leaderboard_scoreboard: *const rc_client_leaderboard_scoreboard_t,
    pub server_error: *const rc_client_server_error_t,
    pub subset: *const c_void,
}

pub type rc_client_read_memory_func_t = Option<
    unsafe extern "C" fn(
        address: u32,
        buffer: *mut u8,
        num_bytes: u32,
        client: *mut rc_client_t,
    ) -> u32,
>;

pub type rc_client_server_callback_t = Option<
    unsafe extern "C" fn(
        server_response: *const rc_api_server_response_t,
        callback_data: *mut c_void,
    ),
>;

pub type rc_client_server_call_t = Option<
    unsafe extern "C" fn(
        request: *const rc_api_request_t,
        callback: rc_client_server_callback_t,
        callback_data: *mut c_void,
        client: *mut rc_client_t,
    ),
>;

pub type rc_client_callback_t = Option<
    unsafe extern "C" fn(
        result: c_int,
        error_message: *const c_char,
        client: *mut rc_client_t,
        userdata: *mut c_void,
    ),
>;

pub type rc_client_event_handler_t =
    Option<unsafe extern "C" fn(event: *const rc_client_event_t, client: *mut rc_client_t)>;

pub type rc_client_message_callback_t =
    Option<unsafe extern "C" fn(message: *const c_char, client: *const rc_client_t)>;

/// `struct retro_memory_descriptor` (`libretro.h`).
#[repr(C)]
pub struct retro_memory_descriptor {
    pub flags: u64,
    pub ptr: *mut c_void,
    pub offset: usize,
    pub start: usize,
    pub select: usize,
    pub disconnect: usize,
    pub len: usize,
    pub addrspace: *const c_char,
}

/// `struct retro_memory_map` (`libretro.h`).
#[repr(C)]
pub struct retro_memory_map {
    pub descriptors: *const retro_memory_descriptor,
    pub num_descriptors: c_uint,
}

pub const RC_LIBRETRO_MAX_MEMORY_REGIONS: usize = 32;

#[repr(C)]
pub struct rc_libretro_memory_regions_t {
    pub data: [*mut u8; RC_LIBRETRO_MAX_MEMORY_REGIONS],
    pub size: [usize; RC_LIBRETRO_MAX_MEMORY_REGIONS],
    pub total_size: usize,
    pub count: u32,
}

impl Default for rc_libretro_memory_regions_t {
    fn default() -> Self {
        Self {
            data: [std::ptr::null_mut(); RC_LIBRETRO_MAX_MEMORY_REGIONS],
            size: [0; RC_LIBRETRO_MAX_MEMORY_REGIONS],
            total_size: 0,
            count: 0,
        }
    }
}

#[repr(C)]
pub struct rc_libretro_core_memory_info_t {
    pub data: *mut u8,
    pub size: usize,
}

pub type rc_libretro_get_core_memory_info_func =
    Option<unsafe extern "C" fn(id: u32, info: *mut rc_libretro_core_memory_info_t)>;

pub const RC_HASH_CDTRACK_FIRST_DATA: u32 = u32::MAX;
pub const RC_HASH_CDTRACK_LAST: u32 = u32::MAX - 1;
pub const RC_HASH_CDTRACK_LARGEST: u32 = u32::MAX - 2;
pub const RC_HASH_CDTRACK_FIRST_OF_SECOND_SESSION: u32 = u32::MAX - 3;

pub type rc_hash_message_callback_func =
    Option<unsafe extern "C" fn(message: *const c_char, iterator: *const rc_hash_iterator_t)>;

pub type rc_hash_filereader_open_file_handler =
    Option<unsafe extern "C" fn(path_utf8: *const c_char) -> *mut c_void>;
pub type rc_hash_filereader_seek_handler =
    Option<unsafe extern "C" fn(file_handle: *mut c_void, offset: i64, origin: c_int)>;
pub type rc_hash_filereader_tell_handler =
    Option<unsafe extern "C" fn(file_handle: *mut c_void) -> i64>;
pub type rc_hash_filereader_read_handler = Option<
    unsafe extern "C" fn(
        file_handle: *mut c_void,
        buffer: *mut c_void,
        requested_bytes: usize,
    ) -> usize,
>;
pub type rc_hash_filereader_close_file_handler =
    Option<unsafe extern "C" fn(file_handle: *mut c_void)>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct rc_hash_filereader_t {
    pub open: rc_hash_filereader_open_file_handler,
    pub seek: rc_hash_filereader_seek_handler,
    pub tell: rc_hash_filereader_tell_handler,
    pub read: rc_hash_filereader_read_handler,
    pub close: rc_hash_filereader_close_file_handler,
}

pub type rc_hash_cdreader_open_track_handler =
    Option<unsafe extern "C" fn(path: *const c_char, track: u32) -> *mut c_void>;
pub type rc_hash_cdreader_open_track_iterator_handler = Option<
    unsafe extern "C" fn(
        path: *const c_char,
        track: u32,
        iterator: *const rc_hash_iterator_t,
    ) -> *mut c_void,
>;
pub type rc_hash_cdreader_read_sector_handler = Option<
    unsafe extern "C" fn(
        track_handle: *mut c_void,
        sector: u32,
        buffer: *mut c_void,
        requested_bytes: usize,
    ) -> usize,
>;
pub type rc_hash_cdreader_close_track_handler =
    Option<unsafe extern "C" fn(track_handle: *mut c_void)>;
pub type rc_hash_cdreader_first_track_sector_handler =
    Option<unsafe extern "C" fn(track_handle: *mut c_void) -> u32>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct rc_hash_cdreader_t {
    pub open_track: rc_hash_cdreader_open_track_handler,
    pub read_sector: rc_hash_cdreader_read_sector_handler,
    pub close_track: rc_hash_cdreader_close_track_handler,
    pub first_track_sector: rc_hash_cdreader_first_track_sector_handler,
    pub open_track_iterator: rc_hash_cdreader_open_track_iterator_handler,
}

pub type rc_hash_3ds_get_cia_normal_key_func =
    Option<unsafe extern "C" fn(common_key_index: u8, out_normal_key: *mut u8) -> c_int>;
pub type rc_hash_3ds_get_ncch_normal_keys_func = Option<
    unsafe extern "C" fn(
        primary_key_y: *mut u8,
        secondary_key_x_slot: u8,
        optional_program_id: *mut u8,
        out_primary_key: *mut u8,
        out_secondary_key: *mut u8,
    ) -> c_int,
>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct rc_hash_encryption_callbacks_t {
    pub get_3ds_cia_normal_key: rc_hash_3ds_get_cia_normal_key_func,
    pub get_3ds_ncch_normal_keys: rc_hash_3ds_get_ncch_normal_keys_func,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct rc_hash_callbacks_t {
    pub verbose_message: rc_hash_message_callback_func,
    pub error_message: rc_hash_message_callback_func,
    pub filereader: rc_hash_filereader_t,
    pub cdreader: rc_hash_cdreader_t,
    pub encryption: rc_hash_encryption_callbacks_t,
}

#[repr(C)]
pub struct rc_hash_iterator_t {
    pub buffer: *const u8,
    pub buffer_size: usize,
    pub consoles: [u8; 12],
    pub index: c_int,
    pub path: *const c_char,
    pub userdata: *mut c_void,
    pub callbacks: rc_hash_callbacks_t,
}

extern "C" {
    pub fn rc_error_str(ret: c_int) -> *const c_char;
    pub fn rc_console_name(console_id: u32) -> *const c_char;

    pub fn rc_client_create(
        read_memory_function: rc_client_read_memory_func_t,
        server_call_function: rc_client_server_call_t,
    ) -> *mut rc_client_t;
    pub fn rc_client_destroy(client: *mut rc_client_t);

    pub fn rc_client_set_hardcore_enabled(client: *mut rc_client_t, enabled: c_int);
    pub fn rc_client_get_hardcore_enabled(client: *const rc_client_t) -> c_int;
    pub fn rc_client_set_encore_mode_enabled(client: *mut rc_client_t, enabled: c_int);
    pub fn rc_client_set_unofficial_enabled(client: *mut rc_client_t, enabled: c_int);
    pub fn rc_client_set_spectator_mode_enabled(client: *mut rc_client_t, enabled: c_int);

    pub fn rc_client_set_userdata(client: *mut rc_client_t, userdata: *mut c_void);
    pub fn rc_client_get_userdata(client: *const rc_client_t) -> *mut c_void;
    pub fn rc_client_get_user_agent_clause(
        client: *mut rc_client_t,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;

    pub fn rc_client_enable_logging(
        client: *mut rc_client_t,
        level: c_int,
        callback: rc_client_message_callback_t,
    );
    pub fn rc_client_set_event_handler(
        client: *mut rc_client_t,
        handler: rc_client_event_handler_t,
    );
    pub fn rc_client_set_allow_background_memory_reads(client: *mut rc_client_t, allowed: c_int);

    pub fn rc_client_begin_login_with_password(
        client: *mut rc_client_t,
        username: *const c_char,
        password: *const c_char,
        callback: rc_client_callback_t,
        callback_userdata: *mut c_void,
    ) -> *mut rc_client_async_handle_t;

    pub fn rc_client_begin_login_with_token(
        client: *mut rc_client_t,
        username: *const c_char,
        token: *const c_char,
        callback: rc_client_callback_t,
        callback_userdata: *mut c_void,
    ) -> *mut rc_client_async_handle_t;

    pub fn rc_client_logout(client: *mut rc_client_t);
    pub fn rc_client_get_user_info(client: *const rc_client_t) -> *const rc_client_user_t;
    pub fn rc_client_get_user_game_summary(
        client: *const rc_client_t,
        summary: *mut rc_client_user_game_summary_t,
    );

    pub fn rc_client_begin_identify_and_load_game(
        client: *mut rc_client_t,
        console_id: u32,
        file_path: *const c_char,
        data: *const u8,
        data_size: usize,
        callback: rc_client_callback_t,
        callback_userdata: *mut c_void,
    ) -> *mut rc_client_async_handle_t;

    pub fn rc_client_begin_load_game(
        client: *mut rc_client_t,
        hash: *const c_char,
        callback: rc_client_callback_t,
        callback_userdata: *mut c_void,
    ) -> *mut rc_client_async_handle_t;

    pub fn rc_client_is_game_loaded(client: *const rc_client_t) -> c_int;
    pub fn rc_client_unload_game(client: *mut rc_client_t);
    pub fn rc_client_get_game_info(client: *const rc_client_t) -> *const rc_client_game_t;

    pub fn rc_client_create_achievement_list(
        client: *mut rc_client_t,
        category: c_int,
        grouping: c_int,
    ) -> *mut rc_client_achievement_list_t;
    pub fn rc_client_destroy_achievement_list(list: *mut rc_client_achievement_list_t);
    pub fn rc_client_has_achievements(client: *mut rc_client_t) -> c_int;

    pub fn rc_client_get_leaderboard_info(
        client: *const rc_client_t,
        id: u32,
    ) -> *const rc_client_leaderboard_t;
    pub fn rc_client_create_leaderboard_list(
        client: *mut rc_client_t,
        grouping: c_int,
    ) -> *mut rc_client_leaderboard_list_t;
    pub fn rc_client_destroy_leaderboard_list(list: *mut rc_client_leaderboard_list_t);
    pub fn rc_client_has_leaderboards(client: *mut rc_client_t) -> c_int;

    pub fn rc_client_has_rich_presence(client: *mut rc_client_t) -> c_int;
    pub fn rc_client_get_rich_presence_message(
        client: *mut rc_client_t,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;

    pub fn rc_client_is_processing_required(client: *mut rc_client_t) -> c_int;
    pub fn rc_client_do_frame(client: *mut rc_client_t);
    pub fn rc_client_idle(client: *mut rc_client_t);
    pub fn rc_client_reset(client: *mut rc_client_t);

    pub fn rc_client_progress_size(client: *mut rc_client_t) -> usize;
    pub fn rc_client_serialize_progress_sized(
        client: *mut rc_client_t,
        buffer: *mut u8,
        buffer_size: usize,
    ) -> c_int;
    pub fn rc_client_deserialize_progress_sized(
        client: *mut rc_client_t,
        serialized: *const u8,
        serialized_size: usize,
    ) -> c_int;

    pub fn rc_libretro_memory_init(
        regions: *mut rc_libretro_memory_regions_t,
        mmap: *const retro_memory_map,
        get_core_memory_info: rc_libretro_get_core_memory_info_func,
        console_id: u32,
    ) -> c_int;
    pub fn rc_libretro_memory_destroy(regions: *mut rc_libretro_memory_regions_t);
    pub fn rc_libretro_memory_read(
        regions: *const rc_libretro_memory_regions_t,
        address: u32,
        buffer: *mut u8,
        num_bytes: u32,
    ) -> u32;

    pub fn rc_hash_get_default_cdreader(cdreader: *mut rc_hash_cdreader_t);
    pub fn rc_hash_initialize_iterator(
        iterator: *mut rc_hash_iterator_t,
        path: *const c_char,
        buffer: *const u8,
        buffer_size: usize,
    );
    pub fn rc_hash_destroy_iterator(iterator: *mut rc_hash_iterator_t);
    pub fn rc_hash_generate(
        hash: *mut c_char,
        console_id: u32,
        iterator: *const rc_hash_iterator_t,
    ) -> c_int;
    pub fn rc_hash_generate_from_buffer(
        hash: *mut c_char,
        console_id: u32,
        buffer: *const u8,
        buffer_size: usize,
    ) -> c_int;
    pub fn rc_hash_generate_from_file(
        hash: *mut c_char,
        console_id: u32,
        path: *const c_char,
    ) -> c_int;
}
