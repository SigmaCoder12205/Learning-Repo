use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn bench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args(args)
        .output()
        .unwrap()
}
fn data(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn regression_gates_and_missing_metrics_cannot_be_waived() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline.json");
    let first = bench(&[
        "/bin/true",
        "--runs=2",
        "--warmup=0",
        "--save-baseline",
        path(&baseline),
        "--format=json",
    ]);
    assert!(first.status.success(), "{first:?}");
    let mut saved = data(&first);
    saved["summary"]["wall"]["median"] = json!(0);
    fs::write(&baseline, serde_json::to_vec(&saved).unwrap()).unwrap();
    let next = bench(&[
        "/bin/true",
        "--runs=2",
        "--warmup=0",
        "--compare",
        path(&baseline),
        "--regression=wall=5",
        "--format=json",
    ]);
    assert_eq!(next.status.code(), Some(3), "{next:?}");
    assert_eq!(data(&next)["comparison"]["status"], "regression");
    let missing = bench(&[
        "/bin/true",
        "--runs=1",
        "--warmup=0",
        "--compare",
        path(&baseline),
        "--regression=allocations=5",
        "--allow-mismatch",
        "--format=json",
    ]);
    assert_eq!(missing.status.code(), Some(2), "{missing:?}");
    assert_eq!(data(&missing)["comparison"]["status"], "incompatible");
}

#[test]
fn suites_resolve_relative_paths_merge_environment_and_preflight_all_cases() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("work")).unwrap();
    fs::write(dir.path().join("input"), "hello\n").unwrap();
    fs::write(dir.path().join("expected"), "hello\nbase case\n").unwrap();
    std::os::unix::fs::symlink("/bin/sh", dir.path().join("shell")).unwrap();
    let suite = dir.path().join("suite.json");
    let mut config = json!({"version":1,"defaults":{"runs":1,"warmup":0,"cwd":"work","env":{"FIRST":"base","SECOND":"default"}},"benchmarks":[{"name":"input","executable":"shell","input":"input","expect_output":"expected","env":{"SECOND":"case"},"args":["-c","cat; printf '%s %s\\n' \"$FIRST\" \"$SECOND\""]}]});
    fs::write(&suite, config.to_string()).unwrap();
    let output = bench(&["--suite", path(&suite), "--format=json"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["cases"][0]["successful_runs"], 1);
    assert_eq!(
        data(&output)["cases"][0]["settings"]["cwd"],
        json!(dir.path().join("work"))
    );
    // A later invalid entry must be detected before any earlier target executes.
    let marker = dir.path().join("marker");
    config["benchmarks"][0]["args"] = json!(["-c", format!("touch {}", path(&marker))]);
    config["benchmarks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"bad","executable":"absent"}));
    fs::write(&suite, config.to_string()).unwrap();
    assert_eq!(bench(&["--suite", path(&suite)]).status.code(), Some(2));
    assert!(!marker.exists());
}

#[test]
fn durable_recovery_handles_torn_tail_and_rejects_invalid_structure() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("runs.jsonl");
    let first = bench(&[
        "/bin/true",
        "--runs=2",
        "--warmup=0",
        "--checkpoint",
        path(&checkpoint),
        "--format=json",
    ]);
    assert!(first.status.success(), "{first:?}");
    let recovered = bench(&["--recover", path(&checkpoint), "--format=json"]);
    assert!(recovered.status.success(), "{recovered:?}");
    assert_eq!(data(&first), data(&recovered));
    let mut partial = String::new();
    for line in fs::read_to_string(&checkpoint).unwrap().lines() {
        let event: Value = serde_json::from_str(line).unwrap();
        if event["event"] == "start" || event["event"] == "sample" {
            partial.push_str(line);
            partial.push('\n');
        }
    }
    partial.push_str("{\"event\":");
    fs::write(&checkpoint, partial).unwrap();
    let recovered = bench(&["--recover", path(&checkpoint), "--format=json"]);
    assert_eq!(recovered.status.code(), Some(1), "{recovered:?}");
    assert_eq!(data(&recovered)["successful_runs"], 2);
    assert_eq!(data(&recovered)["stop_reason"], "recovered_partial");
    fs::write(
        &checkpoint,
        "{\"event\":\"start\",\"report\":{\"name\":\"bad\"}}\n",
    )
    .unwrap();
    let invalid = bench(&["--recover", path(&checkpoint)]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("panicked"));
}

#[test]
fn adaptive_run_cap_metric_filtering_and_html_escaping() {
    let output = bench(&[
        "/bin/true",
        "--runs=2",
        "--warmup=0",
        "--adaptive",
        "--min-time=100",
        "--max-runs=4",
        "--metrics=rss",
        "--progress",
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let r = data(&output);
    assert_eq!(r["runs"].as_array().unwrap().len(), 4);
    assert!(r["summary"]["wall"].is_null());
    assert!(r["runs"][0]["metrics"]["wall"].is_null());
    assert!(r["summary"]["rss"]["mean_ci95"].is_object());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ETA"));
    let output = bench(&[
        "/bin/true",
        "--runs=1",
        "--warmup=0",
        "--name=</script><script>alert(1)</script>",
        "--format=html",
    ]);
    assert!(output.status.success(), "{output:?}");
    let html = String::from_utf8(output.stdout).unwrap();
    assert!(html.contains("&lt;/script&gt;"));
    assert!(html.contains("\\u003c/script\\u003e"));
    assert!(!html.contains("<script>alert(1)</script>"));
}

#[test]
fn cpu_affinity_reaches_target_and_path_collisions_are_rejected() {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let allowed = status
        .lines()
        .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
        .unwrap()
        .trim();
    let cpu = allowed.split([',', '-']).next().unwrap();
    let script = format!("while IFS= read -r line; do case \"$line\" in Cpus_allowed_list:*) set -- $line; test \"$2\" = {cpu}; exit $?;; esac; done < /proc/self/status; exit 1");
    let output = bench(&[
        "/bin/sh",
        "--runs=1",
        "--warmup=0",
        "--cpu-affinity",
        cpu,
        "--format=json",
        "--",
        "-c",
        &script,
    ]);
    assert!(output.status.success(), "{output:?}");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("same");
    assert_eq!(
        bench(&[
            "/bin/true",
            "--checkpoint",
            path(&file),
            "--output",
            path(&file)
        ])
        .status
        .code(),
        Some(2)
    );
    assert!(!file.exists());
}
