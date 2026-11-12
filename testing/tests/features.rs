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
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn data(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}
fn script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn history_records_private_reports_and_keeps_pins_immutable() {
    let dir = tempfile::tempdir().unwrap();
    let history = dir.path().join("history");
    let empty = bench(&["history", "--dir", path(&history), "--json", "list"]);
    assert!(empty.status.success(), "{empty:?}");
    assert_eq!(data(&empty), json!([]));
    assert!(!history.exists());
    let output = bench(&[
        "/bin/true",
        "--runs=2",
        "--warmup=0",
        "--record-history",
        "--history-dir",
        path(&history),
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    let id = report["history"]["id"].as_str().unwrap();
    let saved = history.join("runs").join(format!("{id}.json"));
    assert_eq!(
        fs::metadata(&history).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&saved).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let shown = bench(&["history", "--dir", path(&history), "show", id]);
    assert_eq!(data(&shown), report);
    let listed = bench(&[
        "history",
        "--dir",
        path(&history),
        "--json",
        "list",
        "--limit=1",
    ]);
    assert_eq!(data(&listed)[0]["id"], id);
    let pin = bench(&[
        "history",
        "--dir",
        path(&history),
        "--json",
        "pin",
        id,
        "stable",
    ]);
    assert!(pin.status.success(), "{pin:?}");
    let pin_path = history.join("pins/stable.json");
    let before = fs::read(&pin_path).unwrap();
    assert_eq!(
        bench(&["history", "--dir", path(&history), "pin", id, "stable"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(fs::read(&pin_path).unwrap(), before);
    let compared = bench(&[
        "/bin/true",
        "--runs=1",
        "--warmup=0",
        "--compare-pin=stable",
        "--history-dir",
        path(&history),
        "--warn-regression=wall=100000",
        "--format=json",
    ]);
    assert!(compared.status.success(), "{compared:?}");
    assert_eq!(data(&compared)["comparison"]["status"], "ok");
    assert_eq!(
        bench(&["history", "--dir", path(&history), "show", "../escape"])
            .status
            .code(),
        Some(2)
    );
    let failed = bench(&[
        "/bin/false",
        "--runs=1",
        "--warmup=0",
        "--record-history",
        "--history-dir",
        path(&history),
        "--format=json",
    ]);
    assert_eq!(failed.status.code(), Some(1));
    let failed = data(&failed);
    assert_eq!(
        bench(&[
            "history",
            "--dir",
            path(&history),
            "pin",
            failed["history"]["id"].as_str().unwrap(),
            "bad"
        ])
        .status
        .code(),
        Some(2)
    );
    std::os::unix::fs::symlink(&saved, history.join("runs/linked.json")).unwrap();
    assert_eq!(
        bench(&["history", "--dir", path(&history), "show", "linked"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn history_is_opt_in_and_rejects_public_storage_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_bench"))
        .env("XDG_DATA_HOME", dir.path())
        .args(["/bin/true", "--runs=1", "--warmup=0", "--format=json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!dir.path().join("bench/history").exists());
    let public = dir.path().join("public");
    fs::create_dir(&public).unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o755)).unwrap();
    let marker = dir.path().join("marker");
    let output = bench(&[
        "/bin/touch",
        "--record-history",
        "--history-dir",
        path(&public),
        "--",
        path(&marker),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(!marker.exists());
}

#[test]
fn warning_thresholds_keep_hard_gates_and_missing_data_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline.json");
    let first = bench(&["/bin/true", "--runs=1", "--warmup=0", "--format=json"]);
    assert!(first.status.success(), "{first:?}");
    let mut saved = data(&first);
    saved["summary"]["wall"]["median"] = json!(0.);
    fs::write(&baseline, saved.to_string()).unwrap();
    let args = [
        "/bin/true",
        "--runs=1",
        "--warmup=0",
        "--compare",
        path(&baseline),
        "--warn-regression=wall=5",
        "--format=json",
    ];
    let warning = bench(&args);
    assert!(warning.status.success(), "{warning:?}");
    assert_eq!(
        data(&warning)["comparison"]["advisory"]["status"],
        "regression"
    );
    assert_eq!(data(&warning)["comparison"]["status"], "ok");
    let mut hard = args.to_vec();
    hard.push("--regression=wall=10");
    assert_eq!(bench(&hard).status.code(), Some(3));
    let mut missing = args.to_vec();
    missing.extend(["--warn-regression=allocations=5", "--allow-mismatch"]);
    assert_eq!(bench(&missing).status.code(), Some(2));
    let text = bench(&[
        "/bin/true",
        "--runs=1",
        "--warmup=0",
        "--compare",
        path(&baseline),
        "--warn-regression=wall=5",
    ]);
    assert!(String::from_utf8_lossy(&text.stdout).contains("Regression warning"));
}

#[test]
fn runtime_detection_histograms_and_signal_details_preserve_metric_selection() {
    let output = bench(&[
        "/bin/true",
        "--runs=5",
        "--warmup=0",
        "--histogram",
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    assert_eq!(report["runtime"]["kind"], "elf");
    assert_eq!(
        report["runs"][0]["termination"],
        json!({"kind":"exit","code":0})
    );
    assert_eq!(report["wall_analysis"]["sample_count"], 5);
    assert_eq!(
        report["wall_analysis"]["histogram"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["count"].as_u64().unwrap())
            .sum::<u64>(),
        5
    );
    let text = bench(&["/bin/true", "--runs=2", "--warmup=0", "--histogram"]);
    assert!(String::from_utf8_lossy(&text.stdout).contains("Wall-time histogram"));
    let selected = bench(&[
        "/bin/true",
        "--runs=1",
        "--warmup=0",
        "--metrics=rss",
        "--histogram",
        "--format=json",
    ]);
    assert!(data(&selected)["wall_analysis"].is_null());
    assert!(data(&selected)["runs"][0]["metrics"]["wall"].is_null());
    let dir = tempfile::tempdir().unwrap();
    let shell = dir.path().join("target");
    script(&shell, "#!/bin/sh\nkill -TERM $$\n");
    let signal = bench(&[path(&shell), "--runs=1", "--warmup=0", "--format=json"]);
    assert_eq!(signal.status.code(), Some(1));
    assert_eq!(data(&signal)["runtime"]["kind"], "script");
    assert_eq!(data(&signal)["runs"][0]["exit_code"], -15);
    assert_eq!(
        data(&signal)["runs"][0]["termination"],
        json!({"kind":"signal","signal":15,"name":"SIGTERM"})
    );
}

#[test]
fn python_calibration_is_separate_and_does_not_replay_the_workload() {
    let dir = tempfile::tempdir().unwrap();
    let workload = dir.path().join("work.py");
    let marker = dir.path().join("calls");
    fs::write(&workload,"import sys\nassert not __debug__\nwith open(sys.argv[1], 'a') as f: f.write('call\\n')\nprint(input())\n").unwrap();
    let input = dir.path().join("input");
    fs::write(&input, "hello\n").unwrap();
    let output = bench(&[
        path(&workload),
        "--runs=2",
        "--warmup=1",
        "--python-arg=-O",
        "--python-overhead",
        "--overhead-runs=3",
        "--input",
        path(&input),
        "--expect-output",
        path(&input),
        "--format=json",
        "--",
        path(&marker),
    ]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    assert_eq!(report["runtime"]["kind"], "python");
    assert_eq!(report["successful_runs"], 2);
    assert_eq!(report["summary"]["wall"]["count"], 2);
    assert_eq!(report["python_overhead"]["summary"]["wall"]["count"], 3);
    assert_eq!(
        report["python_overhead"]["warmups"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(report["python_overhead"]["status"], "ok");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "call\ncall\ncall\n");
    let full = report["summary"]["wall"]["median"].as_f64().unwrap();
    let empty = report["python_overhead"]["summary"]["wall"]["median"]
        .as_f64()
        .unwrap();
    assert!(
        (report["python_overhead"]["estimated_workload_wall_seconds"]
            .as_f64()
            .unwrap()
            - (full - empty))
            .abs()
            < 1e-9
    );
    for flag in ["-c", "-m", "-i", "--version"] {
        let bad = bench(&[
            path(&workload),
            "--python-overhead",
            &format!("--python-arg={flag}"),
        ]);
        assert_eq!(bad.status.code(), Some(2), "{bad:?}");
    }
    assert_eq!(
        bench(&["/bin/true", "--python-overhead"]).status.code(),
        Some(2)
    );
    assert_eq!(
        bench(&[path(&workload), "--overhead-runs=2"]).status.code(),
        Some(2)
    );
}

#[test]
fn failed_or_timed_out_calibration_keeps_successful_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let workload = dir.path().join("work.py");
    fs::write(&workload, "pass\n").unwrap();
    let wrapper = dir.path().join("python");
    for (action, status) in [("exit 42", "unexpected_exit"), ("sleep 10", "timeout")] {
        script(
            &wrapper,
            &format!(
                "#!/bin/sh\ncase \"$1\" in *bench-empty-*) {action};; esac\nexec python3 \"$@\"\n"
            ),
        );
        let output = bench(&[
            path(&workload),
            "--interpreter",
            path(&wrapper),
            "--runs=1",
            "--warmup=0",
            "--python-overhead",
            "--overhead-runs=1",
            "--timeout=0.5",
            "--format=json",
        ]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let report = data(&output);
        assert_eq!(report["measurement_passed"], true);
        assert_eq!(report["successful_runs"], 1);
        assert_eq!(report["python_overhead"]["failed"], true);
        assert_eq!(report["python_overhead"]["warmups"][0]["status"], status);
        assert!(report["python_overhead"]["estimated_workload_wall_seconds"].is_null());
    }
}

#[test]
fn cooldown_suites_and_recovery_keep_their_scope() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("run.jsonl");
    let started = std::time::Instant::now();
    let output = bench(&[
        "/bin/true",
        "--runs=3",
        "--warmup=1",
        "--cooldown-ms=30",
        "--checkpoint",
        path(&checkpoint),
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(started.elapsed().as_millis() >= 90);
    assert_eq!(data(&output)["settings"]["cooldown_ms"], 30);
    let recovered = bench(&[
        "--recover",
        path(&checkpoint),
        "--histogram",
        "--format=json",
    ]);
    assert!(recovered.status.success(), "{recovered:?}");
    assert_eq!(data(&recovered)["runs"], data(&output)["runs"]);
    assert_eq!(data(&recovered)["presentation"]["histogram"], true);
    let workload = dir.path().join("work.py");
    fs::write(&workload, "pass\n").unwrap();
    let suite = dir.path().join("suite.json");
    fs::write(
        &suite,
        json!({"version":1,"defaults":{"runs":1,"warmup":0,"cooldown_ms":1},"benchmarks":[
        {"name":"native","executable":"/bin/true"},
        {"name":"python","python":"work.py","python_overhead":true,"overhead_runs":1}]})
        .to_string(),
    )
    .unwrap();
    let output = bench(&["--suite", path(&suite), "--histogram", "--format=json"]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    assert_eq!(report["cases"][1]["python_overhead"]["status"], "ok");
    assert_eq!(report["cases"][0]["presentation"]["histogram"], true);
    assert_eq!(
        bench(&["--check-tools", "--histogram"]).status.code(),
        Some(2)
    );
}

#[test]
fn interruption_during_calibration_or_cooldown_preserves_completed_runs() {
    use std::{
        process::Stdio,
        thread,
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let workload = dir.path().join("work.py");
    fs::write(&workload, "pass\n").unwrap();
    let wrapper = dir.path().join("python");
    script(&wrapper,"#!/bin/sh\ncase \"$1\" in *bench-empty-*) echo $$ > \"$CAL_PID\"; exec /bin/sleep 30;; esac\nexec python3 \"$@\"\n");
    for calibration in [true, false] {
        let stderr = dir.path().join(format!("{calibration}.stderr"));
        let pidfile = dir.path().join("pid");
        let mut command = Command::new(env!("CARGO_BIN_EXE_bench"));
        if calibration {
            command.args([
                path(&workload),
                "--interpreter",
                path(&wrapper),
                "--python-overhead",
                "--overhead-runs=1",
                "--runs=1",
            ]);
        } else {
            command.args(["/bin/true", "--runs=3", "--cooldown-ms=30000"]);
        }
        let mut child = command
            .args(["--warmup=0", "--format=json", "--progress"])
            .env("CAL_PID", &pidfile)
            .stdout(Stdio::piped())
            .stderr(fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let start = Instant::now();
        let ready = loop {
            if (calibration && pidfile.exists())
                || (!calibration
                    && fs::read_to_string(&stderr)
                        .unwrap()
                        .contains("cooldown 30000 ms"))
            {
                break true;
            }
            if start.elapsed() > Duration::from_secs(5) {
                break false;
            }
            thread::sleep(Duration::from_millis(5));
        };
        if !ready {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("did not reach interruptible phase");
        }
        thread::sleep(Duration::from_millis(20));
        assert!(Command::new("/bin/kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success());
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(130), "{output:?}");
        let report = data(&output);
        assert_eq!(report["successful_runs"], 1);
        assert_eq!(report["completed"], false);
        assert_eq!(report["interrupt_signal"], 2);
        if calibration {
            assert_eq!(report["python_overhead"]["status"], "interrupted");
            let pid = fs::read_to_string(&pidfile).unwrap();
            assert!(!Path::new(&format!("/proc/{}", pid.trim())).exists());
        } else {
            assert_eq!(report["stop_reason"], "interrupted");
            assert_eq!(report["runs"].as_array().unwrap().len(), 1);
        }
    }
}
