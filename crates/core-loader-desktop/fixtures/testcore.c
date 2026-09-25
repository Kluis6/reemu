/* Core libretro fake, só pra testes do loader. Software-only, RGB565,
 * 64x48, 60fps. Se o conteúdo do "jogo" começar com "HW", declara
 * SET_HW_RENDER (context type OpenGL core 3.3 + depth) pra exercitar a
 * detecção de requisitos e a negociação de contexto GL. */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef bool (*retro_environment_t)(unsigned, void *);
typedef void (*retro_video_refresh_t)(const void *, unsigned, unsigned, size_t);
typedef void (*retro_audio_sample_t)(int16_t, int16_t);
typedef size_t (*retro_audio_sample_batch_t)(const int16_t *, size_t);
typedef void (*retro_input_poll_t)(void);
typedef int16_t (*retro_input_state_t)(unsigned, unsigned, unsigned, unsigned);

struct retro_game_geometry {
   unsigned base_width, base_height, max_width, max_height;
   float aspect_ratio;
};
struct retro_system_timing {
   double fps, sample_rate;
};
struct retro_system_av_info {
   struct retro_game_geometry geometry;
   struct retro_system_timing timing;
};
struct retro_system_info {
   const char *library_name, *library_version, *valid_extensions;
   bool need_fullpath, block_extract;
};
struct retro_game_info {
   const char *path;
   const void *data;
   size_t size;
   const char *meta;
};

typedef void (*retro_hw_context_reset_t)(void);
typedef uintptr_t (*retro_hw_get_current_framebuffer_t)(void);
typedef void (*retro_proc_address_t)(void);
typedef retro_proc_address_t (*retro_hw_get_proc_address_t)(const char *);
struct retro_hw_render_callback {
   unsigned context_type;
   retro_hw_context_reset_t context_reset;
   retro_hw_get_current_framebuffer_t get_current_framebuffer;
   retro_hw_get_proc_address_t get_proc_address;
   bool depth, stencil, bottom_left_origin;
   unsigned version_major, version_minor;
   bool cache_context;
   retro_hw_context_reset_t context_destroy;
   bool debug_context;
};

#define RETRO_ENVIRONMENT_SET_HW_RENDER 14
#define RETRO_ENVIRONMENT_SET_PIXEL_FORMAT 10
#define RETRO_ENVIRONMENT_GET_VARIABLE 15
#define RETRO_ENVIRONMENT_SET_VARIABLES 16
#define RETRO_ENVIRONMENT_SET_GEOMETRY 37 /* libretro.h */
#define RETRO_ENVIRONMENT_GET_INPUT_BITMASKS (51 | 0x10000) /* libretro.h */
#define RETRO_DEVICE_JOYPAD 1
#define RETRO_DEVICE_ID_JOYPAD_MASK 256
#define RETRO_PIXEL_FORMAT_RGB565 2
#define RETRO_HW_CONTEXT_OPENGL_CORE 3

struct retro_variable {
   const char *key;
   const char *value;
};

#define FB_W 64
#define FB_H 48

static retro_environment_t env_cb;
static retro_video_refresh_t video_cb;
static retro_audio_sample_batch_t audio_batch_cb;
static retro_audio_sample_t audio_sample_cb;
static retro_input_poll_t input_poll_cb;
static retro_input_state_t input_state_cb;

static uint16_t fb[FB_W * FB_H];
static unsigned frame_n;

/* SAVE_RAM de 64 bytes, pré-carregada com um padrão reconhecível — deixa o
   `emu-session` exercitar o fluxo de battery save de verdade. SRAM[2] recebe
   o valor atual da core option `testcore_mark` a cada `retro_run`. */
static unsigned char testcore_sram[64] = {0xA5, 0x5A};

void retro_set_environment(retro_environment_t cb) {
   env_cb = cb;
   struct retro_variable vars[] = {
      {"testcore_speed", "Velocidade; normal|turbo|lento"},
      {"testcore_mark", "Marca no SRAM; A|B|C"},
      {0, 0},
   };
   if (cb)
      cb(RETRO_ENVIRONMENT_SET_VARIABLES, vars);
}
void retro_set_video_refresh(retro_video_refresh_t cb) { video_cb = cb; }
void retro_set_audio_sample(retro_audio_sample_t cb) { audio_sample_cb = cb; }
void retro_set_audio_sample_batch(retro_audio_sample_batch_t cb) { audio_batch_cb = cb; }
void retro_set_input_poll(retro_input_poll_t cb) { input_poll_cb = cb; }
void retro_set_input_state(retro_input_state_t cb) { input_state_cb = cb; }

unsigned retro_api_version(void) { return 1; }
#define RETRO_ENVIRONMENT_GET_LOG_INTERFACE 27 /* libretro.h */
typedef void (*log_printf_t)(unsigned level, const char *fmt, ...);
struct log_callback { log_printf_t log; };

/* Como o VBA-M: pede a interface de log e usa SEM checar se veio nula.
 * Se o frontend não entregar o callback, todo teste que carrega este core
 * cai aqui (regressão: vbam_libretro morria no retro_init). */
#define RETRO_ENVIRONMENT_SET_CONTROLLER_INFO 35 /* libretro.h */
struct controller_description { const char *desc; unsigned id; };
struct controller_info { const struct controller_description *types; unsigned num_types; };

void retro_init(void) {
   struct log_callback log = { 0 };
   static const struct controller_description pad[] = { { "RetroPad", 1 } };
   static const struct controller_info ports[] = { { pad, 1 }, { pad, 1 }, { 0, 0 } };
   frame_n = 0;
   testcore_sram[6] = 0;
   env_cb(RETRO_ENVIRONMENT_SET_CONTROLLER_INFO, (void *)ports);
   env_cb(RETRO_ENVIRONMENT_GET_LOG_INTERFACE, &log);
   log.log(1, "testcore: init %d\n", 42);
}
void retro_deinit(void) {}

void retro_get_system_info(struct retro_system_info *info) {
   memset(info, 0, sizeof(*info));
   info->library_name = "reemu-testcore";
   info->library_version = "0.1.0";
   info->valid_extensions = "test|bin";
   info->need_fullpath = false;
   info->block_extract = false;
}

void retro_get_system_av_info(struct retro_system_av_info *info) {
   info->geometry.base_width = FB_W;
   info->geometry.base_height = FB_H;
   info->geometry.max_width = FB_W;
   info->geometry.max_height = FB_H;
   info->geometry.aspect_ratio = (float)FB_W / (float)FB_H;
   info->timing.fps = 60.0;
   info->timing.sample_rate = 32000.0;
}

/* Portas que o frontend configurou como RETRO_DEVICE_JOYPAD (1) — bit N =
   porta N, espelhado em SRAM[6]. Como o flycast, este core declara 2 portas
   (SET_CONTROLLER_INFO no retro_init) e espera o frontend ligá-las. */
void retro_set_controller_port_device(unsigned port, unsigned device) {
   if (device == 1 && port < 8)
      testcore_sram[6] |= (unsigned char)(1u << port);
}
void retro_reset(void) { frame_n = 0; }

/* Como um core real: pergunta se o frontend suporta bitmask e, se sim,
   lê todos os botões da porta 0 de uma vez — espelhados em SRAM[4..6]. */
static int has_bitmasks = 0;

/* ROM começando com "GEOM": a partir do 3º quadro o core pede
   `SET_GEOMETRY` com proporção 2.0 — testa a mudança em runtime. */
static int geometry_test = 0;

bool retro_load_game(const struct retro_game_info *game) {
   enum { fmt = RETRO_PIXEL_FORMAT_RGB565 };
   unsigned pf = RETRO_PIXEL_FORMAT_RGB565;
   if (env_cb)
      env_cb(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, &pf);

   has_bitmasks = env_cb && env_cb(RETRO_ENVIRONMENT_GET_INPUT_BITMASKS, NULL);
   geometry_test = game && game->data && game->size >= 4 &&
                   memcmp(game->data, "GEOM", 4) == 0;

   if (game && game->data && game->size >= 2 &&
       memcmp(game->data, "HW", 2) == 0) {
      struct retro_hw_render_callback hw;
      memset(&hw, 0, sizeof(hw));
      hw.context_type = RETRO_HW_CONTEXT_OPENGL_CORE;
      hw.version_major = 3;
      hw.version_minor = 3;
      hw.depth = true;
      if (env_cb)
         env_cb(RETRO_ENVIRONMENT_SET_HW_RENDER, &hw);
   }
   return true;
}

void retro_unload_game(void) {}
unsigned retro_get_region(void) { return 0; }
bool retro_load_game_special(unsigned t, const struct retro_game_info *i, size_t n) {
   (void)t;
   (void)i;
   (void)n;
   return false;
}

void retro_run(void) {
   if (input_poll_cb)
      input_poll_cb();

   /* Espelha o valor atual da opção `testcore_mark` no SRAM[2] — deixa os
      testes verificarem o roundtrip de core options. */
   if (env_cb) {
      struct retro_variable v = {"testcore_mark", 0};
      if (env_cb(RETRO_ENVIRONMENT_GET_VARIABLE, &v) && v.value)
         testcore_sram[2] = (unsigned char)v.value[0];
   }

   if (has_bitmasks && input_state_cb) {
      int16_t mask = input_state_cb(0, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_MASK);
      testcore_sram[4] = (unsigned char)(mask & 0xFF);
      testcore_sram[5] = (unsigned char)((mask >> 8) & 0xFF);
   }

   if (geometry_test && frame_n == 2 && env_cb) {
      struct retro_game_geometry g = {FB_W, FB_H, FB_W, FB_H, 2.0f};
      env_cb(RETRO_ENVIRONMENT_SET_GEOMETRY, &g);
   }

   uint16_t color = (uint16_t)(frame_n * 111u + 1u);
   for (int i = 0; i < FB_W * FB_H; i++)
      fb[i] = color;
   frame_n++;

   if (video_cb)
      video_cb(fb, FB_W, FB_H, FB_W * sizeof(uint16_t));

   int16_t silence[32] = {0};
   if (audio_batch_cb)
      audio_batch_cb(silence, 16); /* 16 frames estéreo */
}

/* Save state "gordo": frame_n no comeco + preenchimento deterministico.
 * >512KB de proposito — exercita o caminho IPC de datagrama grande
 * (SNES real passa de 800KB), que ja truncou em silencio. */
#define TESTCORE_STATE_SIZE (4u * 1024u * 1024u)  /* > INLINE_MAX do core-ipc: exercita o caminho memfd */
size_t retro_serialize_size(void) { return TESTCORE_STATE_SIZE; }
bool retro_serialize(void *data, size_t size) {
   if (size < TESTCORE_STATE_SIZE)
      return false;
   unsigned char *p = (unsigned char *)data;
   memcpy(p, &frame_n, sizeof(frame_n));
   for (size_t i = sizeof(frame_n); i < TESTCORE_STATE_SIZE; i++)
      p[i] = (unsigned char)((i * 31u + frame_n) & 0xFF);
   return true;
}
bool retro_unserialize(const void *data, size_t size) {
   if (size < sizeof(frame_n))
      return false;
   memcpy(&frame_n, data, sizeof(frame_n));
   return true;
}
void retro_cheat_reset(void) {}
void retro_cheat_set(unsigned index, bool enabled, const char *code) {
   (void)index;
   (void)enabled;
   (void)code;
}
void *retro_get_memory_data(unsigned id) {
   if (id == 0 /* RETRO_MEMORY_SAVE_RAM */)
      return testcore_sram;
   return NULL;
}
size_t retro_get_memory_size(unsigned id) {
   if (id == 0 /* RETRO_MEMORY_SAVE_RAM */)
      return sizeof(testcore_sram);
   return 0;
}
