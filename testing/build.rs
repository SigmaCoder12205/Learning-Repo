fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        panic!("cppbench currently requires Linux");
    }
    println!("cargo:rerun-if-changed=native/measure.c");
    println!("cargo:rerun-if-changed=native/measure.h");
    cc::Build::new()
        .file("native/measure.c")
        .flag_if_supported("-std=c11")
        .warnings(true)
        .extra_warnings(true)
        .compile("cppbench_measure");
}
