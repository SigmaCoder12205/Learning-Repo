use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn bench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench"))
        .args(args)
        .output()
        .unwrap()
}
fn report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}
fn successful(output: &Output) {
    assert!(output.status.success(), "{output:?}");
}
#[test]
fn positional_path_defaults_to_overview_and_exports_all_metrics() {
    let output = bench(&["/bin/true"]);
    successful(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    for word in [
        "10/10",
        "Overview",
        "Time",
        "Peak memory",
        "CPU usage",
        "GPU usage",
        "Not collected; pass --gpu",
        "Throughput",
        "Time variation",
    ] {
        assert!(text.contains(word), "missing {word}: {text}");
    }
    assert!(!text.contains("Stddev"));
    assert!(!text.contains("minor_faults"));
    let output = bench(&["/bin/true", "--format", "json"]);
    successful(&output);
    let r = report(&output);
    assert_eq!(r["selected_metrics"].as_array().unwrap().len(), 15);
    assert_eq!(r["runs"].as_array().unwrap().len(), 10);
    assert_eq!(r["warmups"].as_array().unwrap().len(), 1);
    assert_eq!(r["summary"]["wall"]["count"], 10);
    assert_eq!(r["gpu"]["status"], "unavailable");
    assert!(r["gpu"]["usage_percent"].is_null());
}

#[test]
fn report_levels_and_metric_overrides() {
    for flag in ["--intermediate", "--view=intermediate"] {
        let output = bench(&["/bin/true", flag, "--runs", "2"]);
        successful(&output);
        let text = String::from_utf8(output.stdout).unwrap();
        for word in ["Resource summary", "User CPU time", "Lowest", "Highest"] {
            assert!(text.contains(word), "{text}");
        }
        assert!(!text.contains("Stddev"));
    }
    for flag in ["--advanced", "--view=advanced"] {
        let output = bench(&["/bin/true", flag, "--runs", "2"]);
        successful(&output);
        let text = String::from_utf8(output.stdout).unwrap();
        for word in [
            "Stddev",
            "P90",
            "P95",
            "minor_faults",
            "Settings:",
            "Individual runs",
            "#2 ok",
        ] {
            assert!(text.contains(word), "{text}");
        }
    }
    for flag in ["--view=simple", "--intermediate", "--advanced"] {
        let output = bench(&["/bin/true", flag, "--metrics", "wall", "--runs", "1"]);
        successful(&output);
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("Peak memory"));
        assert!(!text.contains("GPU usage"));
        assert!(!text.contains("rss="));
        let output = bench(&["/bin/true", flag, "--format", "json", "--runs", "1"]);
        successful(&output);
        assert_eq!(report(&output)["summary"].as_object().unwrap().len(), 15);
    }
    assert_eq!(
        bench(&["/bin/true", "--intermediate", "--advanced"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        bench(&["/bin/true", "--view", "simple", "--advanced"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn overview_does_not_invent_successful_measurements() {
    let output = bench(&["/bin/false"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("No successful measurements"));
    assert!(text.contains("unexpected_exit"));
    assert!(!text.contains("average per run"));
    let output = bench(&["/bin/true", "--runs", "1"]);
    successful(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("only one successful run"));
}
#[test]
fn arguments_environment_cwd_and_filtering() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap();
    let output = bench(&[
        "/bin/sh",
        "--cwd",
        path,
        "--env",
        "BENCH_TEST=yes",
        "--runs",
        "2",
        "--metrics",
        "wall,memory,wall",
        "--format",
        "json",
        "--",
        "-c",
        "test \"$BENCH_TEST\" = yes && test \"$PWD\" = \"$1\" && test \"$2\" = '--target-flag'",
        "test",
        path,
        "--target-flag",
    ]);
    successful(&output);
    let r = report(&output);
    assert_eq!(r["runs"][0]["metrics"].as_object().unwrap().len(), 4);
    assert_eq!(r["successful_runs"], 2);
}
#[test]
fn input_replay_and_exact_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input");
    fs::write(&path, b"hello\x00world\n").unwrap();
    let input = path.to_str().unwrap();
    let output = bench(&[
        "/bin/cat",
        "--input",
        input,
        "--expect-output",
        input,
        "--runs",
        "2",
        "--format",
        "json",
    ]);
    successful(&output);
    assert_eq!(report(&output)["summary"]["stdout_bytes"]["mean"], 12.0);
    let output = bench(&["/bin/true", "--expect-output", input, "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report(&output)["warmups"][0]["status"], "output_mismatch");
    assert!(report(&output)["runs"].as_array().unwrap().is_empty());
}
#[test]
fn timeout_and_output_limit() {
    let output = bench(&[
        "/bin/sleep",
        "--timeout",
        "0.03",
        "--warmup",
        "0",
        "--runs",
        "1",
        "--format",
        "json",
        "--",
        "5",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let r = report(&output);
    assert_eq!(r["runs"][0]["status"], "timeout");
    assert_eq!(r["runs"][0]["exit_code"], -9);
    assert!(r["runs"][0]["metrics"]["wall"].as_f64().unwrap() < 2.0);
    let output = bench(&[
        "/bin/sh",
        "--output-limit",
        "0.000001",
        "--warmup",
        "0",
        "--runs",
        "1",
        "--format",
        "json",
        "--",
        "-c",
        "printf 'too much output'",
    ]);
    assert_eq!(report(&output)["runs"][0]["status"], "output_limit");
}
#[test]
fn expected_exit_signal_and_launch_failure_are_distinct() {
    successful(&bench(&[
        "/bin/sh",
        "--expect-exit",
        "-13",
        "--runs",
        "1",
        "--",
        "-c",
        "kill -PIPE $$",
    ]));
    successful(&bench(&[
        "/bin/sh",
        "--expect-exit",
        "7",
        "--runs",
        "1",
        "--",
        "-c",
        "exit 7",
    ]));
    successful(&bench(&[
        "/bin/sh",
        "--expect-exit",
        "-15",
        "--runs",
        "1",
        "--",
        "-c",
        "kill -TERM $$",
    ]));
    successful(&bench(&[
        "/bin/sh",
        "--expect-exit",
        "127",
        "--runs",
        "1",
        "--",
        "-c",
        "exit 127",
    ]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not executable");
    fs::write(&path, "hello").unwrap();
    let output = bench(&[path.to_str().unwrap(), "--expect-exit", "127"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot launch"));
}
#[test]
fn keep_going_excludes_failures_from_summary() {
    let dir = tempfile::tempdir().unwrap();
    let script =
        "n=0; test ! -f count || n=$(cat count); n=$((n+1)); echo $n > count; test $n -ne 2";
    let output = bench(&[
        "/bin/sh",
        "--cwd",
        dir.path().to_str().unwrap(),
        "--warmup",
        "0",
        "--runs",
        "3",
        "--keep-going",
        "--format",
        "json",
        "--",
        "-c",
        script,
    ]);
    assert_eq!(output.status.code(), Some(1));
    let r = report(&output);
    assert_eq!(r["successful_runs"], 2);
    assert_eq!(r["summary"]["wall"]["count"], 2);
    assert_eq!(r["runs"][1]["status"], "unexpected_exit");
    let output = bench(&[
        "/bin/false",
        "--warmup",
        "0",
        "--runs",
        "3",
        "--format",
        "json",
    ]);
    assert_eq!(report(&output)["runs"].as_array().unwrap().len(), 1);
}
#[test]
fn csv_file_and_executable_option() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.csv");
    let output = bench(&[
        "--executable",
        "/bin/true",
        "--runs",
        "2",
        "--metrics",
        "rss,wall",
        "--format",
        "csv",
        "--output",
        path.to_str().unwrap(),
    ]);
    successful(&output);
    assert!(output.stdout.is_empty());
    let csv = fs::read_to_string(path).unwrap();
    assert_eq!(csv.lines().count(), 3);
    assert!(csv.starts_with("run,status,exit_code,rss [KiB],wall [s]\n"));
}
#[test]
fn invalid_arguments_and_missing_target() {
    for args in [
        vec![],
        vec!["/bin/true", "--runs", "0"],
        vec!["/bin/true", "--timeout", "NaN"],
        vec!["/bin/true", "--metrics", "unknown"],
        vec!["/bin/true", "--env", "bad"],
        vec!["/bin/true", "--executable", "/bin/true"],
        vec!["/does/not/exist"],
    ] {
        assert_eq!(bench(&args).status.code(), Some(2), "{args:?}");
    }
    successful(&bench(&["--list-metrics"]));
}
#[test]
fn cpp_compilation_and_failure() {
    if Command::new("g++").arg("--version").output().is_err() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source file.cpp");
    let expected = dir.path().join("expected");
    fs::write(
        &source,
        "#include <iostream>\nint main(){std::cout << VALUE << '\\n';}\n",
    )
    .unwrap();
    fs::write(&expected, "42\n").unwrap();
    let output = bench(&[
        "--source",
        source.to_str().unwrap(),
        "--cxxflag=-DVALUE=42",
        "--runs",
        "2",
        "--expect-output",
        expected.to_str().unwrap(),
        "--format",
        "json",
    ]);
    successful(&output);
    assert!(
        report(&output)["compilation"]["wall_seconds"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    fs::write(&source, "invalid C++").unwrap();
    let output = bench(&["--source", source.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("compilation failed"));
}
fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(path.exists(), "target did not start");
}
fn assert_dead(pid: &str) {
    let path = format!("/proc/{}/stat", pid.trim());
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match fs::read_to_string(&path) {
            Err(_) => return,
            Ok(s) if s.split(") ").nth(1).unwrap().starts_with('Z') => return,
            _ if Instant::now() >= deadline => panic!("descendant {pid} still alive"),
            _ => thread::sleep(Duration::from_millis(5)),
        }
    }
}
#[test]
fn interruption_cleans_up_descendants() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let child = Command::new(env!("CARGO_BIN_EXE_bench"))
        .args([
            "/bin/sh",
            "--cwd",
            dir.path().to_str().unwrap(),
            "--warmup",
            "0",
            "--format",
            "json",
            "--",
            "-c",
            "sleep 30 & echo $! > pid; wait",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_file(&pidfile);
    successful(
        &Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .output()
            .unwrap(),
    );
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130), "{output:?}");
    assert_eq!(report(&output)["runs"][0]["status"], "interrupted");
    assert_dead(&fs::read_to_string(pidfile).unwrap());
}
#[test]
fn timeout_cleans_up_descendants() {
    let dir = tempfile::tempdir().unwrap();
    let output = bench(&[
        "/bin/sh",
        "--cwd",
        dir.path().to_str().unwrap(),
        "--timeout",
        "0.3",
        "--warmup",
        "0",
        "--format",
        "json",
        "--",
        "-c",
        "sleep 30 & echo $! > pid; wait",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_dead(&fs::read_to_string(dir.path().join("pid")).unwrap());
}

#[test]
fn compiler_interruption_cleans_up_and_returns_signal_exit_code() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let compiler = dir.path().join("compiler");
    let source = dir.path().join("main.cpp");
    let pidfile = dir.path().join("pid");
    fs::write(&source, "int main() {}\n").unwrap();
    fs::write(
        &compiler,
        "#!/bin/sh\nsleep 30 & echo $! > \"$BENCH_PIDFILE\"; wait\n",
    )
    .unwrap();
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_bench"))
        .arg("--source")
        .arg(&source)
        .arg("--compiler")
        .arg(&compiler)
        .env("BENCH_PIDFILE", &pidfile)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_file(&pidfile);
    successful(
        &Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .output()
            .unwrap(),
    );
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("compilation interrupted"));
    assert_dead(&fs::read_to_string(pidfile).unwrap());
}

#[test]
fn eof_prompt_loop_reports_bounded_output_and_input_replay_resolves_it() {
    // Models a prompt loop that keeps running after a failed input read, like the calculator.
    let script = r#"while :; do printf '\033[31mEnter number (q to quit): '; read -r value; [ "$value" = q ] && break; done"#;
    let common = [
        "/bin/sh",
        "--runs",
        "2",
        "--output-limit",
        "0.001",
        "--metrics",
        "wall",
    ];
    let mut args = common.to_vec();
    args.extend(["--format", "json", "--", "-c", script]);
    let output = bench(&args);
    assert_eq!(output.status.code(), Some(1));
    let r = report(&output);
    assert_eq!(r["warmups"][0]["status"], "output_limit");
    assert!(r["runs"].as_array().unwrap().is_empty());
    assert_eq!(r["warmups"][0]["metrics"].as_object().unwrap().len(), 1);
    assert!(
        r["warmups"][0]["captured_output"]["stdout_bytes"]
            .as_f64()
            .unwrap()
            > 1024.0
    );
    assert_eq!(
        r["warmups"][0]["stdout_preview"].as_str().unwrap().len(),
        512
    );
    assert_eq!(r["warmups"][0]["stdout_preview_truncated"], true);
    assert_eq!(r["warmups"][0]["stderr_preview"], "");

    let mut args = common.to_vec();
    args.extend(["--intermediate", "--", "-c", script]);
    let output = bench(&args);
    let text = String::from_utf8(output.stdout).unwrap();
    for message in [
        "Measured runs did not start because a warmup failed",
        "Output limit exceeded:",
        "combined limit 0.001 MiB",
        "stdout (escaped",
        "[truncated]",
        "--input FILE",
        "immediate EOF",
    ] {
        assert!(text.contains(message), "missing {message}: {text}");
    }
    assert!(
        !text.contains('\x1b'),
        "terminal escape sequences must be escaped"
    );
    assert!(!text.contains("stderr (escaped)"));
    assert!(!text.contains("Fewer than five successful measurements"));

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.txt");
    fs::write(&input, "q\n").unwrap();
    let mut args = common.to_vec();
    args.extend([
        "--input",
        input.to_str().unwrap(),
        "--format",
        "json",
        "--",
        "-c",
        script,
    ]);
    let output = bench(&args);
    successful(&output);
    let r = report(&output);
    assert_eq!(r["successful_runs"], 2);
    assert_eq!(r["warmups"][0]["status"], "ok");
    assert!(r["runs"][0].get("stdout_preview").is_none());

    fs::write(&input, "").unwrap();
    let output = bench(&[
        "/bin/sh",
        "--input",
        input.to_str().unwrap(),
        "--output-limit",
        "0.001",
        "--",
        "-c",
        script,
    ]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("replay file eventually reaches EOF"));
    assert!(!text.contains("stdin is /dev/null"));
}

#[test]
fn short_lived_target_peak_matches_its_own_kernel_accounting_after_free() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("rss.c");
    let target = dir.path().join("rss");
    let self_rss = dir.path().join("self-rss");
    fs::write(
        &source,
        r#"
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    FILE *file = fopen(argv[1], "w");
    if (!file) return 3;
    size_t size = 64 * 1024 * 1024;
    volatile unsigned char *memory = malloc(size);
    if (!memory) return 4;
    for (size_t i = 0; i < size; i += 4096) memory[i] = 1;
    free((void *)memory);
    struct rusage usage;
    if (getrusage(RUSAGE_SELF, &usage)) return 5;
    fprintf(file, "%ld\n", usage.ru_maxrss);
    fclose(file);
    return 0;
}
"#,
    )
    .unwrap();
    successful(
        &Command::new("cc")
            .args(["-O2", "-o"])
            .arg(&target)
            .arg(&source)
            .output()
            .unwrap(),
    );
    let output = bench(&[
        target.to_str().unwrap(),
        "--runs",
        "1",
        "--warmup",
        "0",
        "--format",
        "json",
        "--",
        self_rss.to_str().unwrap(),
    ]);
    successful(&output);
    let r = report(&output);
    let measured = r["runs"][0]["metrics"]["rss"].as_f64().unwrap();
    let reported: f64 = fs::read_to_string(self_rss)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        reported >= 64. * 1024.,
        "touched allocation missing from kernel peak: {reported}"
    );
    assert!(
        (measured - reported).abs() < 1024.,
        "bench {measured} vs target self-accounting {reported}"
    );
    assert_eq!(r["measurement_backend"], "isolated-c-wait4-v1");
    assert_eq!(r["memory_accounting"]["unit"], "KiB");
}
