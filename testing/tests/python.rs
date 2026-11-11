use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn bench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench"))
        .args(args)
        .output()
        .unwrap()
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn data(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("{e}: {o:?}"))
}

#[test]
fn script_arguments_input_environment_and_working_directory_are_replayed() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("my script.py");
    fs::write(&script, "import os, sys\nassert os.getcwd() == os.environ['WORK']\nprint(sys.argv[1], input(), os.environ['TEST_VALUE'])\n").unwrap();
    let input = dir.path().join("input");
    let expected = dir.path().join("expected");
    fs::write(&input, "hello\n").unwrap();
    fs::write(&expected, "--script-flag hello world\n").unwrap();
    let output = bench(&[
        path(&script),
        "--runs=2",
        "--warmup=1",
        "--input",
        path(&input),
        "--expect-output",
        path(&expected),
        "--cwd",
        path(dir.path()),
        "--env=TEST_VALUE=world",
        "--env",
        &format!("WORK={}", dir.path().display()),
        "--format=json",
        "--",
        "--script-flag",
    ]);
    assert!(output.status.success(), "{output:?}");
    let r = data(&output);
    assert_eq!(r["successful_runs"], 2);
    assert_eq!(r["warmups"].as_array().unwrap().len(), 1);
    assert_eq!(r["python"]["script"], json!(script));
    assert_eq!(r["name"], "my script.py");
    assert_eq!(r["provenance"]["script_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(r["command"][1], json!(script));
    assert_eq!(r["command"][2], "--script-flag");
    assert!(r["summary"]["rss"]["max"].as_f64().unwrap() > 0.);
}

#[test]
fn explicit_script_interpreter_options_and_virtual_environment_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let venv = dir.path().join("venv");
    let create = Command::new("python3")
        .args(["-m", "venv", "--without-pip"])
        .arg(&venv)
        .output()
        .unwrap();
    assert!(create.status.success(), "{create:?}");
    let script = dir.path().join("workload");
    fs::write(&script, "import sys\nassert not __debug__\nassert sys.prefix != sys.base_prefix\nassert sys.prefix == sys.argv[1]\n").unwrap();
    let interpreter = venv.join("bin/python");
    let args = [
        "--python",
        path(&script),
        "--interpreter",
        path(&interpreter),
        "--python-arg=-O",
        "--python-arg=-B",
        "--runs=1",
        "--warmup=0",
        "--",
        path(&venv),
    ];
    let output = bench(&args);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("(Python)"));
    assert!(text.contains(path(&script)));
    assert!(text.contains(path(&interpreter)));
    assert!(text.contains("interpreter startup"));
}

#[test]
fn invalid_python_invocations_fail_before_running() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("test.py");
    fs::write(&script, "raise AssertionError('must not execute')\n").unwrap();
    for args in [
        vec![path(&script), "--interpreter=/no/such/python"],
        vec!["--python=/no/such/script"],
        vec!["--interpreter=python3"],
        vec!["--executable=/bin/true", "--python", path(&script)],
        vec!["--source=example.cpp", "--python", path(&script)],
        vec![path(&script), "--env=PATH=/nonexistent"],
    ] {
        let output = bench(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("must not execute"));
    }
}

#[test]
fn python_failures_timeouts_and_output_limits_keep_native_status_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("test.py");
    for (body, flag, status) in [
        (
            "raise RuntimeError('intentional failure')\n",
            "--timeout=3",
            "unexpected_exit",
        ),
        ("import time\ntime.sleep(10)\n", "--timeout=0.1", "timeout"),
        (
            "while True: print('x' * 8192)\n",
            "--output-limit=0.001",
            "output_limit",
        ),
    ] {
        fs::write(&script, body).unwrap();
        let output = bench(&[
            path(&script),
            "--runs=1",
            "--warmup=0",
            "--format=json",
            flag,
        ]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let r = data(&output);
        assert_eq!(r["runs"][0]["status"], status);
        assert_eq!(r["successful_runs"], 0);
    }
}

#[test]
fn mixed_suites_resolve_python_paths_and_preflight_all_interpreters() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(
        &script,
        "import pathlib\nassert not __debug__\npathlib.Path('marker').touch()\n",
    )
    .unwrap();
    let python = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    std::os::unix::fs::symlink(
        String::from_utf8(python.stdout).unwrap().trim(),
        dir.path().join("python"),
    )
    .unwrap();
    let suite = dir.path().join("suite.json");
    let mut config = json!({"version":1,"defaults":{"runs":1,"warmup":0},"benchmarks":[
        {"name":"python","python":"work.py","interpreter":"./python","python_args":["-O"]},
        {"name":"native","executable":"/bin/true"}]});
    fs::write(&suite, config.to_string()).unwrap();
    let output = bench(&["--suite", path(&suite), "--format=json"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["cases"][0]["successful_runs"], 1);
    assert!(data(&output)["cases"][1]["python"].is_null());
    fs::remove_file(dir.path().join("marker")).unwrap();
    config["benchmarks"][1] = json!({"name":"bad","python":"work.py","interpreter":"./missing"});
    fs::write(&suite, config.to_string()).unwrap();
    assert_eq!(bench(&["--suite", path(&suite)]).status.code(), Some(2));
    assert!(!dir.path().join("marker").exists());
}

#[test]
fn comparisons_track_script_edits_and_reject_changed_interpreters() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    let baseline = dir.path().join("baseline.json");
    fs::write(&script, "print(1)\n").unwrap();
    let first = bench(&[
        path(&script),
        "--runs=1",
        "--warmup=0",
        "--save-baseline",
        path(&baseline),
        "--format=json",
    ]);
    assert!(first.status.success(), "{first:?}");
    fs::write(&script, "print(2)\n").unwrap();
    let next = bench(&[
        path(&script),
        "--runs=1",
        "--warmup=0",
        "--compare",
        path(&baseline),
        "--format=json",
    ]);
    assert!(next.status.success(), "{next:?}");
    assert!(data(&next)["comparison"]["warnings"]
        .to_string()
        .contains("script contents changed"));
    let mut saved = data(&first);
    saved["provenance"]["executable_sha256"] = json!("different interpreter binary");
    fs::write(&baseline, saved.to_string()).unwrap();
    let next = bench(&[
        path(&script),
        "--runs=1",
        "--warmup=0",
        "--compare",
        path(&baseline),
        "--format=json",
    ]);
    assert_eq!(next.status.code(), Some(2), "{next:?}");
    assert!(data(&next)["comparison"]["differences"]
        .to_string()
        .contains("Python interpreter differs"));
}

#[test]
fn python_application_telemetry_uses_a_separate_replay() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("api.py");
    fs::write(&script, "import json, os\npath = os.environ.get('BENCH_TELEMETRY_FILE')\nif path:\n    with open(path, 'x') as out:\n        json.dump({'version': 1, 'metrics': {'backend.request_ms': {'unit': 'ms', 'samples': [1, 2]}, 'backend.requests': {'unit': 'count', 'value': 2}, 'backend.errors': {'unit': 'count', 'value': 0}}}, out)\nprint('ok')\n").unwrap();
    let output = bench(&[
        path(&script),
        "--backend",
        "--api",
        "--require-profile-data",
        "--runs=2",
        "--warmup=0",
        "--artifacts-dir",
        path(dir.path()),
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let r = data(&output);
    assert_eq!(r["successful_runs"], 2);
    assert_eq!(r["diagnostics"]["requested"], json!(["telemetry"]));
    assert_eq!(r["profiles"]["status"], "ok");
    assert!(r["runs"][0]["metrics"]["backend.requests"].is_null());
}
