fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        panic!("bench currently requires Linux");
    }
    println!("cargo:rerun-if-changed=native/measure.c");
    println!("cargo:rerun-if-changed=native/measure.h");
    println!("cargo:rerun-if-changed=native/helper.c");
    println!("cargo:rerun-if-changed=native/launcher.c");
    println!("cargo:rerun-if-changed=native/interactive.c");
    println!("cargo:rerun-if-changed=native/interactive.h");
    println!("cargo:rerun-if-env-changed=CC");
    let helper =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("bench-measure");
    let mut command = cc::Build::new().get_compiler().to_command();
    // The helper runs from a memfd on the user's machine. A musl release must not
    // depend on a musl loader there, so link it statically.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("musl") {
        command.arg("-static");
    }
    let status = command
        .args([
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "native/helper.c",
            "native/measure.c",
            "-o",
        ])
        .arg(&helper)
        .status()
        .expect("compile isolated C measurement helper");
    assert!(
        status.success(),
        "failed to build isolated C measurement helper"
    );
    cc::Build::new()
        .file("native/launcher.c")
        .file("native/interactive.c")
        .flag_if_supported("-std=c11")
        .warnings(true)
        .extra_warnings(true)
        .compile("bench_measure");
    println!("cargo:rustc-link-lib=util");
}
