fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        panic!("cppbench currently requires Linux");
    }
    println!("cargo:rerun-if-changed=native/measure.c");
    println!("cargo:rerun-if-changed=native/measure.h");
    println!("cargo:rerun-if-changed=native/helper.c");
    println!("cargo:rerun-if-changed=native/launcher.c");
    let helper =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("cppbench-measure");
    let status = cc::Build::new()
        .get_compiler()
        .to_command()
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
        .flag_if_supported("-std=c11")
        .warnings(true)
        .extra_warnings(true)
        .compile("cppbench_measure");
}
