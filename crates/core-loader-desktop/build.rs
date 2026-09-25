//! Compila o core-fake em C (`fixtures/testcore.c`) para uma biblioteca
//! dinâmica e passa o caminho pros testes via env `REEMU_TESTCORE`.
//! É o que permite testar o loader ponta a ponta sem baixar um core real
//! nem depender de uma ROM.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=fixtures/testcore.c");
    println!("cargo:rerun-if-changed=src/log_shim.c");

    // `GET_LOG_INTERFACE`: o callback de log do libretro é variádico
    // (printf) e só C define isso em Rust estável — ver src/log_shim.c.
    cc::Build::new()
        .file("src/log_shim.c")
        .compile("reemu_log_shim");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let fname = match target_os.as_str() {
        "macos" => "libreemu_testcore.dylib",
        "windows" => "reemu_testcore.dll",
        _ => "libreemu_testcore.so",
    };
    let so = out.join(fname);

    // As flags de "biblioteca compartilhada" dependem do COMPILADOR de
    // verdade (`cc::Tool`), não só do SO-alvo: no target `*-pc-windows-msvc`
    // (o padrão do rustup ali) o compilador é `cl.exe`/`link.exe`, que não
    // entende `-shared`/`-o` (sintaxe GCC/Clang) — ele repassa flag
    // desconhecida pro linker calado, que cai no padrão de EXE comum sem
    // `/DLL`; como `testcore.c` não tem `main()`, dá `LNK1561: entry point
    // must be defined`. MSVC precisa de `/LD` (compila E linka como DLL) +
    // `/Fe:<saída>` em vez de `-shared -o <saída>`. `-O1` funciona nos dois
    // (`cl.exe` aceita `/`/`-` como sinônimos pra switch).
    let tool = cc::Build::new().get_compiler();
    let mut cmd = tool.to_command();
    if tool.is_like_msvc() {
        cmd.arg("/LD")
            .arg("-O1")
            .arg("fixtures/testcore.c")
            .arg(format!("/Fe:{}", so.display()));
    } else {
        let link_args: &[&str] = match target_os.as_str() {
            "macos" => &["-dynamiclib"],
            "windows" => &["-shared"], // MinGW-GNU, não MSVC
            _ => &["-shared", "-fPIC"],
        };
        cmd.args(link_args)
            .arg("-O1")
            .arg("fixtures/testcore.c")
            .arg("-o")
            .arg(&so);
    }

    let status = cmd
        .status()
        .expect("executar o compilador C para o testcore");
    assert!(status.success(), "falha ao compilar fixtures/testcore.c");

    println!("cargo:rustc-env=REEMU_TESTCORE={}", so.display());
}
