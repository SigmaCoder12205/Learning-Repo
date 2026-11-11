use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};
fn bench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench"))
        .args(args)
        .output()
        .unwrap()
}
fn data(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn script(p: &Path, body: &str) {
    fs::write(p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
}
fn payload() -> Value {
    json!({"version":1,"metrics":{"backend.request_ms":{"unit":"ms","samples":[1,2,3,8]},"backend.requests":{"unit":"count","value":4},"backend.errors":{"unit":"count","value":0}}})
}

#[test]
fn telemetry_is_separate_preserves_stdout_and_survives_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("app");
    let journal = dir.path().join("journal");
    script(&target,&format!("if [ -n \"$BENCH_TELEMETRY_FILE\" ]; then printf '%s' '{}' > \"$BENCH_TELEMETRY_FILE\"; fi\nprintf 'expected\\n'",payload()));
    let expected = dir.path().join("expected");
    fs::write(&expected, "expected\n").unwrap();
    let o = bench(&[
        path(&target),
        "--backend",
        "--api",
        "--runs=2",
        "--warmup=1",
        "--require-profile-data",
        "--expect-output",
        path(&expected),
        "--checkpoint",
        path(&journal),
        "--artifacts-dir",
        path(dir.path()),
        "--format=json",
    ]);
    assert!(o.status.success(), "{o:?}");
    let r = data(&o);
    assert_eq!(r["successful_runs"], 2);
    assert_eq!(r["runs"].as_array().unwrap().len(), 2);
    assert_eq!(r["diagnostics"]["requested"], json!(["telemetry"]));
    assert_eq!(r["profiles"]["status"], "ok");
    assert_eq!(
        r["profiles"]["metrics"]["backend.request_ms"]["summary"]["count"],
        4
    );
    assert!(r["runs"][0]["metrics"]["backend.request_ms"].is_null());
    let recovered = bench(&["--recover", path(&journal), "--format=json"]);
    assert!(recovered.status.success());
    assert_eq!(data(&recovered), r);
    let html = bench(&["--recover", path(&journal), "--format=html"]);
    assert!(html.status.success());
    let html = String::from_utf8(html.stdout).unwrap();
    assert!(html.contains("Domain-specific results"));
    assert!(html.contains("Successful API request duration"));
}

#[test]
fn missing_data_is_partial_unless_required_and_bad_telemetry_fails() {
    let dir = tempfile::tempdir().unwrap();
    let basic = [
        "/bin/true",
        "--backend",
        "--api",
        "--runs=1",
        "--warmup=0",
        "--format=json",
        "--artifacts-dir",
        path(dir.path()),
    ];
    let output = bench(&basic);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["profiles"]["status"], "partial");
    let mut strict = basic.to_vec();
    strict.push("--require-profile-data");
    assert_eq!(bench(&strict).status.code(), Some(1));
    let target = dir.path().join("invalid");
    script(
        &target,
        "if [ -n \"$BENCH_TELEMETRY_FILE\" ]; then printf 'broken' > \"$BENCH_TELEMETRY_FILE\"; fi",
    );
    let bad = bench(&[
        path(&target),
        "--backend",
        "--runs=1",
        "--warmup=0",
        "--format=json",
        "--artifacts-dir",
        path(dir.path()),
    ]);
    assert_eq!(bad.status.code(), Some(1));
    assert_eq!(
        data(&bad)["diagnostics"]["results"]["telemetry"]["status"],
        "failed"
    );
    assert_eq!(bench(&["/bin/true", "--graphics"]).status.code(), Some(2));
}

#[test]
fn graphics_focus_has_actual_frame_statistics_without_guessing_gpu() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("frames");
    let frame = json!({"version":1,"metrics":{"gamedev.frame_ms":{"unit":"ms","samples":[10,10,20,40]},"gamedev.gpu_frame_ms":{"unit":"ms","samples":[8,9]},"gamedev.draw_calls":{"unit":"count","samples":[100,120]}}});
    script(&target,&format!("if [ -n \"$BENCH_TELEMETRY_FILE\" ]; then printf '%s' '{frame}' > \"$BENCH_TELEMETRY_FILE\"; fi"));
    let o = bench(&[
        path(&target),
        "--gamedev",
        "--graphics",
        "--frame-budget-ms=16",
        "--require-profile-data",
        "--env=PATH=/nonexistent",
        "--runs=1",
        "--warmup=0",
        "--format=json",
        "--artifacts-dir",
        path(dir.path()),
    ]);
    assert!(o.status.success(), "{o:?}");
    let r = data(&o);
    assert_eq!(
        r["profiles"]["sections"][0]["topics"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        r["profiles"]["metrics"]["gamedev.frame_ms"]["fps_from_mean_frame_interval"],
        50.
    );
    assert_eq!(r["gpu"]["status"], "unavailable");
    assert_eq!(r["profiles"]["metrics"].as_object().unwrap().len(), 3);
}

#[test]
fn networking_fixture_collects_counters_without_target_instrumentation() {
    let dir = tempfile::tempdir().unwrap();
    let trace = dir.path().join("trace");
    fs::write(&trace, include_str!("fixtures/network.strace")).unwrap();
    script(&dir.path().join("strace"),"while [ \"$1\" != -- ]; do if [ \"$1\" = -o ]; then shift; output=$1; fi; shift; done\nshift\n/bin/cat \"$TRACE_FIXTURE\" > \"$output\"\nexec \"$@\"");
    let env = format!("PATH={}", dir.path().display());
    let fixture = format!("TRACE_FIXTURE={}", trace.display());
    let o = bench(&[
        "/bin/true",
        "--networking",
        "--sockets",
        "--bandwidth",
        "--require-profile-data",
        "--env",
        &env,
        "--env",
        &fixture,
        "--runs=1",
        "--warmup=0",
        "--format=json",
        "--artifacts-dir",
        path(dir.path()),
    ]);
    assert!(o.status.success(), "{o:?}");
    let r = data(&o);
    assert_eq!(
        r["profiles"]["metrics"]["networking.sent_bytes"]["value"],
        64
    );
    assert_eq!(r["profiles"]["status"], "ok");
}

#[test]
fn suite_profile_options_and_domain_regression_gates() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("app");
    let suite = dir.path().join("suite.json");
    let baseline = dir.path().join("baseline.json");
    script(&target,&format!("if [ -n \"$BENCH_TELEMETRY_FILE\" ]; then printf '%s' '{}' > \"$BENCH_TELEMETRY_FILE\"; fi",payload()));
    fs::write(&suite,json!({"version":1,"defaults":{"runs":1,"warmup":0,"backend":true,"api":true,"require_profile_data":true},"benchmarks":[{"name":"api","executable":"app"}]}).to_string()).unwrap();
    let o = bench(&[
        "--suite",
        path(&suite),
        "--save-baseline",
        path(&baseline),
        "--format=json",
    ]);
    assert!(o.status.success(), "{o:?}");
    let mut saved = data(&o);
    saved["cases"][0]["profiles"]["metrics"]["backend.request_ms"]["summary"]["p95"] = json!(1);
    fs::write(&baseline, saved.to_string()).unwrap();
    let o = bench(&[
        "--suite",
        path(&suite),
        "--compare",
        path(&baseline),
        "--regression=backend.request_ms=5",
        "--format=json",
    ]);
    assert_eq!(o.status.code(), Some(3), "{o:?}");
    assert_eq!(data(&o)["comparison"]["status"], "regression");
}

#[test]
fn c_and_cpp_header_example_compiles_and_emits_valid_telemetry() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("network");
    let compile = Command::new("g++")
        .args([
            "-std=c++17",
            "-Wall",
            "-Wextra",
            "-Werror",
            "examples/network_profile.cpp",
            "-o",
        ])
        .arg(&binary)
        .output()
        .unwrap();
    assert!(compile.status.success(), "{compile:?}");
    // Make strace unavailable here: the application counters still suffice.
    let o = bench(&[
        path(&binary),
        "--networking",
        "--require-profile-data",
        "--env=PATH=/nonexistent",
        "--runs=1",
        "--warmup=0",
        "--format=json",
        "--artifacts-dir",
        path(dir.path()),
    ]);
    assert!(o.status.success(), "{o:?}");
    assert_eq!(
        data(&o)["profiles"]["metrics"]["networking.sent_bytes"]["value"],
        512000.
    );
    let c = dir.path().join("header.c");
    fs::write(&c,"#include \"bench_telemetry.h\"\nint main(void) { bench_telemetry t = bench_telemetry_open(); return bench_telemetry_close(&t); }\n").unwrap();
    let c_compile = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-Iinclude"])
        .arg(&c)
        .arg("-o")
        .arg(dir.path().join("c"))
        .output()
        .unwrap();
    assert!(c_compile.status.success(), "{c_compile:?}");
}

#[test]
fn legacy_header_and_telemetry_environment_remain_compatible() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("old.c");
    let binary = dir.path().join("old");
    fs::write(&source, "#include \"cppbench_telemetry.h\"\nint main(void) { cppbench_telemetry t = cppbench_telemetry_open(); cppbench_telemetry_value(&t, \"backend.requests\", \"count\", 4); return cppbench_telemetry_close(&t); }\n").unwrap();
    let compiled = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-Iinclude"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{compiled:?}");
    // New header aliases still work under an older runner that only supplies the old variable.
    let telemetry = dir.path().join("old.json");
    let output = Command::new(&binary)
        .env_remove("BENCH_TELEMETRY_FILE")
        .env("CPPBENCH_TELEMETRY_FILE", &telemetry)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let saved: Value = serde_json::from_slice(&fs::read(telemetry).unwrap()).unwrap();
    assert_eq!(saved["metrics"]["backend.requests"]["value"], 4.);
    // An existing instrumented program using only legacy variables works under the new runner.
    let target = dir.path().join("legacy");
    script(&target, &format!("if [ -n \"$CPPBENCH_TELEMETRY_FILE\" ]; then test \"$CPPBENCH_PROFILE_FLAGS\" = backend,api || exit 1; printf '%s' '{}' > \"$CPPBENCH_TELEMETRY_FILE\"; fi", payload()));
    let output = bench(&[
        path(&target),
        "--backend",
        "--api",
        "--require-profile-data",
        "--runs=1",
        "--warmup=0",
        "--artifacts-dir",
        path(dir.path()),
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["profiles"]["status"], "ok");
}
