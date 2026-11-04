use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
    time::Instant,
};

fn script(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args(args)
        .args([
            "--warmup",
            "0",
            "--runs",
            "2",
            "--format",
            "json",
            "--artifacts-dir",
        ])
        .arg(dir.join("artifacts with spaces"))
        .env("PATH", format!("{}:/usr/bin:/bin", dir.display()))
        .output()
        .unwrap()
}
fn report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}
const PERF: &str = r#"
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) output=$2; shift 2;;
        --) shift; break;;
        *) shift;;
    esac
done
printf '100;;cycles:u;5;100.00;;\n200;;instructions:u;5;100.00;;\n' > "$output"
exec "$@"
"#;
#[test]
fn perf_replays_target_and_preserves_baseline_and_correctness() {
    let dir = tempfile::tempdir().unwrap();
    script(dir.path(), "perf", PERF);
    let target = script(
        dir.path(),
        "target with spaces",
        "echo run >> \"$COUNTER\"; /bin/cat",
    );
    let input = dir.path().join("input");
    fs::write(&input, b"binary\0input\n").unwrap();
    let counter = dir.path().join("counter");
    let out = Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .arg(&target)
        .args([
            "--perf", "--warmup", "1", "--runs", "2", "--format", "json", "--input",
        ])
        .arg(&input)
        .arg("--expect-output")
        .arg(&input)
        .arg("--artifacts-dir")
        .arg(dir.path().join("artifacts"))
        .env("COUNTER", &counter)
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let r = report(&out);
    assert_eq!(r["successful_runs"], 2);
    assert_eq!(r["summary"]["wall"]["count"], 2);
    assert_eq!(
        r["diagnostics"]["results"]["perf"]["instructions_per_cycle"],
        2.
    );
    assert_eq!(fs::read_to_string(counter).unwrap().lines().count(), 4);
    let artifacts = Path::new(r["diagnostics"]["artifact_directory"].as_str().unwrap());
    assert!(artifacts.join("diagnostics.json").is_file());
    assert_eq!(
        fs::read(artifacts.join("perf.stdout")).unwrap(),
        b"binary\0input\n"
    );
}
#[test]
fn missing_tool_does_not_erase_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args([
            "/bin/true",
            "--perf",
            "--runs",
            "1",
            "--warmup",
            "0",
            "--format",
            "json",
            "--artifacts-dir",
        ])
        .arg(dir.path().join("artifacts"))
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let r = report(&out);
    assert_eq!(r["successful_runs"], 1);
    assert_eq!(r["diagnostics"]["results"]["perf"]["status"], "unavailable");
}
#[test]
fn timeout_of_profiler_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    script(dir.path(), "perf", "exec /bin/sleep 30");
    let start = Instant::now();
    let out = run(
        dir.path(),
        &["/bin/true", "--perf", "--diagnostic-timeout", "0.1"],
    );
    assert!(start.elapsed().as_secs_f64() < 3.);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        report(&out)["diagnostics"]["results"]["perf"]["invocation"]["status"],
        "timeout"
    );
}
#[test]
fn valgrind_findings_change_exit_status_and_keep_xml() {
    let dir = tempfile::tempdir().unwrap();
    script(
        dir.path(),
        "valgrind",
        r#"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --xml-file=*) output=${1#*=}; shift;;
        --) shift; break;;
        *) shift;;
    esac
done
cat > "$output" <<'XML'
<valgrindoutput><status><state>FINISHED</state></status><error><unique>0</unique><kind>Leak_DefinitelyLost</kind><xwhat><text>64 bytes lost</text><leakedbytes>64</leakedbytes></xwhat></error><errorcounts/></valgrindoutput>
XML
exec "$@"
"#,
    );
    let out = run(dir.path(), &["/bin/true", "--valgrind"]);
    assert_eq!(out.status.code(), Some(1));
    let r = report(&out);
    let v = &r["diagnostics"]["results"]["valgrind"];
    assert_eq!(v["status"], "findings");
    assert_eq!(v["leaked_bytes_by_kind"]["Leak_DefinitelyLost"], 64);
    assert!(Path::new(v["raw_file"].as_str().unwrap()).is_file());
    assert_eq!(r["summary"]["wall"]["count"], 2);
}
#[test]
fn heaptrack_retains_trace_and_analysis() {
    let dir = tempfile::tempdir().unwrap();
    script(
        dir.path(),
        "heaptrack",
        r#"
shift
output=$2
shift 2
printf trace > "$output.zst"
exec "$@"
"#,
    );
    script(dir.path(), "heaptrack_print", "printf 'calls to allocation functions: 42 (10/s)\ntemporary memory allocations: 3 (1/s)\npeak heap memory consumption: 1.25M\ntotal memory leaked: 0B\n'");
    let out = run(dir.path(), &["/bin/true", "--heaptrack"]);
    assert!(out.status.success(), "{out:?}");
    let r = report(&out);
    let h = &r["diagnostics"]["results"]["heaptrack"];
    assert_eq!(h["allocation_calls"], 42);
    assert_eq!(h["peak_heap_bytes_approx"], 1_250_000.);
    assert!(Path::new(h["raw_file"].as_str().unwrap()).is_file());
    assert!(Path::new(h["analysis_file"].as_str().unwrap()).is_file());
}
#[test]
fn gpu_permission_denial_is_reported_and_other_tools_continue() {
    let dir = tempfile::tempdir().unwrap();
    script(
        dir.path(),
        "intel_gpu_top",
        "echo 'Permission denied: CAP_PERFMON' >&2; exit 1",
    );
    script(dir.path(), "perf", PERF);
    let out = run(dir.path(), &["/bin/true", "--perf", "--gpu"]);
    assert_eq!(out.status.code(), Some(1));
    let r = report(&out);
    assert_eq!(r["gpu"]["status"], "unavailable");
    assert!(r["gpu"]["reason"].as_str().unwrap().contains("CAP_PERFMON"));
    assert_eq!(r["diagnostics"]["results"]["perf"]["status"], "ok");
}
#[test]
fn failed_baseline_skips_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), &["/bin/false", "--all-tools"]);
    let r = report(&out);
    assert_eq!(out.status.code(), Some(1));
    for name in ["perf", "gpu", "heaptrack", "valgrind"] {
        assert_eq!(r["diagnostics"]["results"][name]["status"], "skipped");
    }
    assert!(!dir.path().join("artifacts with spaces").exists());
}

#[test]
fn gpu_samples_are_device_wide_and_remain_outside_baseline() {
    let dir = tempfile::tempdir().unwrap();
    script(
        dir.path(),
        "intel_gpu_top",
        r#"
while [ "$#" -gt 0 ]; do
    case "$1" in -o) output=$2; shift 2;; *) shift;; esac
done
[ "$output" = "-" ] || exit 2
printf '[{"engines":{"Render/3D":{"busy":99}}},{"engines":{"Render/3D":{"busy":25}}},{"engines":{"Render/3D":{"busy":75}}}]'
sleep 0.2
"#,
    );
    let out = Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args([
            "/bin/sleep",
            "--gpu",
            "--warmup",
            "0",
            "--runs",
            "1",
            "--format",
            "json",
            "--artifacts-dir",
        ])
        .arg(dir.path().join("artifacts"))
        .args(["--", "0.2"])
        .env("PATH", format!("{}:/usr/bin:/bin", dir.path().display()))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let r = report(&out);
    assert_eq!(r["gpu"]["engines_percent"]["Render/3D"]["mean"], 50.);
    assert!(r["gpu"]["usage_percent"].is_null());
    assert!(r["gpu"]["scope"].as_str().unwrap().contains("device-wide"));
    assert_eq!(r["summary"]["wall"]["count"], 1);
}

#[test]
fn gpu_monitor_json_does_not_contaminate_target_output() {
    let dir = tempfile::tempdir().unwrap();
    script(
        dir.path(),
        "intel_gpu_top",
        r#"
while [ "$#" -gt 0 ]; do
    case "$1" in -o) output=$2; shift 2;; *) shift;; esac
done
[ "$output" = "-" ] || exit 2
printf '[{"engines":{"Render/3D":{"busy":0}}},{"engines":{"Render/3D":{"busy":10}}}]'
sleep 0.2
"#,
    );
    let input = dir.path().join("input");
    fs::write(&input, b"calculator output\0\n").unwrap();
    let out = run(
        dir.path(),
        &[
            "/bin/cat",
            "--gpu",
            "--input",
            input.to_str().unwrap(),
            "--expect-output",
            input.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let r = report(&out);
    let gpu = &r["gpu"];
    assert_eq!(gpu["status"], "ok");
    assert_eq!(
        fs::read(gpu["invocation"]["stdout_file"].as_str().unwrap()).unwrap(),
        b"calculator output\0\n"
    );
    let raw: Value =
        serde_json::from_slice(&fs::read(gpu["raw_file"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(raw.as_array().unwrap().len(), 2);
}

#[test]
fn interrupting_diagnostic_stops_tools_and_keeps_baseline_report() {
    use std::{process::Stdio, thread, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    script(
        dir.path(),
        "perf",
        "echo $$ > \"$TOOL_PIDFILE\"; exec /bin/sleep 30",
    );
    let pidfile = dir.path().join("pid");
    let child = Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args([
            "/bin/true",
            "--perf",
            "--heaptrack",
            "--warmup",
            "0",
            "--runs",
            "1",
            "--format",
            "json",
            "--artifacts-dir",
        ])
        .arg(dir.path().join("artifacts"))
        .env("PATH", dir.path())
        .env("TOOL_PIDFILE", &pidfile)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !pidfile.exists() && start.elapsed() < Duration::from_secs(3) {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(pidfile.exists());
    assert!(Command::new("/bin/kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(130), "{out:?}");
    let r = report(&out);
    assert_eq!(r["successful_runs"], 1);
    assert_eq!(
        r["diagnostics"]["results"]["heaptrack"]["status"],
        "skipped"
    );
    let pid = fs::read_to_string(pidfile).unwrap();
    assert!(!Path::new(&format!("/proc/{}", pid.trim())).exists());
}
