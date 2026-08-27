//! Compila o core-fake em C (`fixtures/testcore.c`) para uma biblioteca
//! dinâmica e passa o caminho pros testes via env `REEMU_TESTCORE`.
//! É o que permite testar o loader ponta a ponta sem baixar um core real
//! nem depender de uma ROM.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=fixtures/testcore.c");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let (fname, link_args): (&str, &[&str]) = match target_os.as_str() {
        "macos" => ("libreemu_testcore.dylib", &["-dynamiclib"]),
        "windows" => ("reemu_testcore.dll", &["-shared"]),
        _ => ("libreemu_testcore.so", &["-shared", "-fPIC"]),
    };
    let so = out.join(fname);

    let mut cmd = cc::Build::new().get_compiler().to_command();
    cmd.args(link_args)
        .arg("-O1")
        .arg("fixtures/testcore.c")
        .arg("-o")
        .arg(&so);

    let status = cmd
        .status()
        .expect("executar o compilador C para o testcore");
    assert!(status.success(), "falha ao compilar fixtures/testcore.c");

    println!("cargo:rustc-env=REEMU_TESTCORE={}", so.display());
}
