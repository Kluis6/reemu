//! Compila o `rcheevos` v12.5.0 oficial (MIT) embarcado em `vendor/rcheevos`.
//!
//! Fontes oficiais consultadas:
//! - `RetroAchievements/rcheevos` tag `v12.5.0` (`test/Makefile` e `Package.swift`)
//! - `libretro-common/include/libretro.h` (`retro_memory_descriptor`, `retro_memory_map`)

fn main() {
    let sources = [
        "vendor/rcheevos/src/rc_compat.c",
        "vendor/rcheevos/src/rc_client.c",
        "vendor/rcheevos/src/rc_libretro.c",
        "vendor/rcheevos/src/rc_util.c",
        "vendor/rcheevos/src/rc_version.c",
        "vendor/rcheevos/src/rcheevos/alloc.c",
        "vendor/rcheevos/src/rcheevos/condition.c",
        "vendor/rcheevos/src/rcheevos/condset.c",
        "vendor/rcheevos/src/rcheevos/consoleinfo.c",
        "vendor/rcheevos/src/rcheevos/format.c",
        "vendor/rcheevos/src/rcheevos/lboard.c",
        "vendor/rcheevos/src/rcheevos/memref.c",
        "vendor/rcheevos/src/rcheevos/operand.c",
        "vendor/rcheevos/src/rcheevos/rc_validate.c",
        "vendor/rcheevos/src/rcheevos/richpresence.c",
        "vendor/rcheevos/src/rcheevos/runtime.c",
        "vendor/rcheevos/src/rcheevos/runtime_progress.c",
        "vendor/rcheevos/src/rcheevos/trigger.c",
        "vendor/rcheevos/src/rcheevos/value.c",
        "vendor/rcheevos/src/rapi/rc_api_common.c",
        "vendor/rcheevos/src/rapi/rc_api_editor.c",
        "vendor/rcheevos/src/rapi/rc_api_info.c",
        "vendor/rcheevos/src/rapi/rc_api_runtime.c",
        "vendor/rcheevos/src/rapi/rc_api_user.c",
        "vendor/rcheevos/src/rhash/aes.c",
        "vendor/rcheevos/src/rhash/cdreader.c",
        "vendor/rcheevos/src/rhash/hash.c",
        "vendor/rcheevos/src/rhash/hash_disc.c",
        "vendor/rcheevos/src/rhash/hash_encrypted.c",
        "vendor/rcheevos/src/rhash/hash_rom.c",
        "vendor/rcheevos/src/rhash/hash_zip.c",
        "vendor/rcheevos/src/rhash/md5.c",
    ];

    println!("cargo:rerun-if-changed=vendor/libretro_shim/libretro.h");
    println!("cargo:rerun-if-changed=vendor/rcheevos/include");
    println!("cargo:rerun-if-changed=vendor/rcheevos/src");

    let mut build = cc::Build::new();
    build
        .files(sources)
        .include("vendor/rcheevos/include")
        .include("vendor/rcheevos/src")
        .include("vendor/rcheevos/src/rcheevos")
        .include("vendor/libretro_shim")
        .define("RC_CLIENT_SUPPORTS_HASH", None)
        .define("RC_DISABLE_LUA", None)
        .warnings(false);

    if !build.get_compiler().is_like_msvc() {
        build
            .define("_LARGEFILE64_SOURCE", None)
            .define("_FILE_OFFSET_BITS", "64");
    }

    build.compile("rcheevos");
}
