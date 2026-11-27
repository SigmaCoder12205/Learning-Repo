//! User-written validators in any language (`--validate`) and their custom metrics.
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
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
fn script(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_owned()
}
fn example(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples/validators")
        .join(name)
        .to_str()
        .unwrap()
        .to_owned()
}
const SUM: &str = "#!/bin/sh\nread n\necho \"sum $(( n*(n+1)/2 ${OFFSET:-} ))\"\n";

#[test]
fn python_validator_reads_input_and_reports_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let target = script(dir.path(), "sum.sh", SUM);
    let input = dir.path().join("n.txt");
    fs::write(&input, "100\n").unwrap();
    let input = input.to_str().unwrap();
    let common = [
        target.as_str(),
        "--input",
        input,
        "--runs",
        "3",
        "--warmup",
        "1",
        "--validate",
        "python3",
        "--validate-arg",
    ];
    let check = example("check_sum.py");
    let mut args = common.to_vec();
    args.extend([check.as_str(), "--format", "json"].iter());
    let output = bench(&args);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    assert_eq!(report["successful_runs"], 3);
    assert_eq!(report["warmups"][0]["validation"]["status"], "passed");
    let validation = &report["runs"][0]["validation"];
    assert_eq!(validation["status"], "passed");
    assert_eq!(validation["metrics"]["absolute_error"], 0.0);
    assert_eq!(report["validation_metrics"]["absolute_error"]["count"], 3);
    assert_eq!(
        report["settings"]["validator"]["command"][1],
        example("check_sum.py")
    );

    // A wrong answer fails every run with the validator's own message.
    let mut args = common.to_vec();
    args.extend([check.as_str(), "--env", "OFFSET=+1"].iter());
    let output = bench(&args);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    // The failed warmup stops the experiment, as with any other output check.
    assert!(
        text.contains("Validator failed (warmups 1): exit 1; \"expected 5050, got 5051\""),
        "{text}"
    );
}

#[test]
fn shell_and_cpp_validators_work_like_any_other_language() {
    let dir = tempfile::tempdir().unwrap();
    let output = bench(&[
        "/bin/echo",
        "--runs",
        "2",
        "--warmup",
        "0",
        "--validate",
        &example("nonempty_lines.sh"),
        "--format",
        "json",
        "--",
        "hello",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        data(&output)["validation_metrics"]["lines"]["unit"],
        "count"
    );

    let checker = dir.path().join("check_sorted");
    let compiled = Command::new("g++")
        .args(["-O1", "-std=c++17", &example("check_sorted.cpp"), "-o"])
        .arg(&checker)
        .status()
        .unwrap();
    assert!(compiled.success());
    for (words, passed) in [("1 2 2 9", true), ("3 1", false)] {
        let output = bench(&[
            "/bin/echo",
            "--runs",
            "1",
            "--warmup",
            "0",
            "--validate",
            checker.to_str().unwrap(),
            "--format",
            "json",
            "--",
            words,
        ]);
        let report = data(&output);
        assert_eq!(output.status.success(), passed, "{output:?}");
        if passed {
            assert_eq!(report["runs"][0]["validation"]["metrics"]["items"], 4.0);
        } else {
            assert_eq!(report["runs"][0]["status"], "validation_failed");
            assert!(report["runs"][0]["validation"]["message"]
                .as_str()
                .unwrap()
                .contains("item 1 (1) is smaller than 3"));
        }
    }
}

#[test]
fn validator_errors_timeouts_and_skips_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let cases = [
        ("slow.sh", "#!/bin/sh\nexec sleep 5\n", "validator timeout"),
        (
            "bad.sh",
            "#!/bin/sh\necho 'bench-metric bad! 1'\n",
            "malformed metric line",
        ),
        (
            "nan.sh",
            "#!/bin/sh\necho 'bench-metric x NaN'\n",
            "needs a finite number",
        ),
        (
            "dup.sh",
            "#!/bin/sh\necho 'bench-metric n 1'\necho 'bench-metric n 2'\n",
            "more than once",
        ),
    ];
    for (name, body, reason) in cases {
        let validator = script(dir.path(), name, body);
        let output = bench(&[
            "/bin/true",
            "--runs",
            "1",
            "--warmup",
            "0",
            "--validate",
            &validator,
            "--validate-timeout",
            "0.3",
            "--format",
            "json",
        ]);
        assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
        let run = &data(&output)["runs"][0];
        assert_eq!(run["status"], "validation_failed", "{name}");
        assert_eq!(run["validation"]["status"], "error", "{name}");
        assert!(
            run["validation"]["reason"]
                .as_str()
                .unwrap()
                .contains(reason),
            "{name}: {run}"
        );
    }

    // A target failure keeps its own status; the validator never runs.
    let marker = dir.path().join("ran");
    let validator = script(
        dir.path(),
        "mark.sh",
        &format!("#!/bin/sh\ntouch {}\n", marker.display()),
    );
    let output = bench(&[
        "/bin/false",
        "--runs",
        "1",
        "--warmup",
        "0",
        "--validate",
        &validator,
        "--format",
        "json",
    ]);
    let run = &data(&output)["runs"][0];
    assert_eq!(run["status"], "unexpected_exit");
    assert!(run["validation"].is_null());
    assert!(!marker.exists());

    // Missing programs fail before any target runs.
    let output = bench(&["/bin/true", "--validate", "./definitely-missing-validator"]);
    assert_eq!(output.status.code(), Some(2));
    let output = bench(&["/bin/true", "--validate", "definitely-missing-validator"]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not on PATH"));
    let output = bench(&["/bin/true", "--validate", "/etc/hostname"]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be an executable file"));
}

#[test]
fn validator_keeps_virtual_environment_interpreters() {
    let dir = tempfile::tempdir().unwrap();
    let venv = dir.path().join("venv");
    assert!(Command::new("python3")
        .args(["-m", "venv", "--without-pip"])
        .arg(&venv)
        .status()
        .unwrap()
        .success());
    let check = script(
        dir.path(),
        "in_venv.py",
        "import sys\nsys.exit(0 if sys.prefix != sys.base_prefix else 1)\n",
    );
    let python = venv.join("bin/python");
    let output = bench(&[
        "/bin/true",
        "--runs",
        "1",
        "--warmup",
        "0",
        "--validate",
        python.to_str().unwrap(),
        "--validate-arg",
        &check,
    ]);
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn suites_accept_validator_commands() {
    let dir = tempfile::tempdir().unwrap();
    script(dir.path(), "sum.sh", SUM);
    fs::write(dir.path().join("n.txt"), "10\n").unwrap();
    fs::create_dir(dir.path().join("checks")).unwrap();
    fs::copy(
        example("check_sum.py"),
        dir.path().join("checks/check_sum.py"),
    )
    .unwrap();
    let suite = dir.path().join("suite.json");
    fs::write(
        &suite,
        r#"{"version":1,"defaults":{"runs":2,"warmup":0},"benchmarks":[
            {"name":"array","executable":"sum.sh","input":"n.txt","validate":["python3","checks/check_sum.py"]},
            {"name":"string","executable":"sum.sh","input":"n.txt","validate":"checks/check_sum.py","validate_timeout":5}]}"#,
    )
    .unwrap();
    let output = bench(&["--suite", suite.to_str().unwrap(), "--format", "json"]);
    assert!(output.status.success(), "{output:?}");
    for case in data(&output)["cases"].as_array().unwrap() {
        assert_eq!(case["validation_metrics"]["absolute_error"]["max"], 0.0);
    }
    assert_eq!(
        data(&output)["cases"][1]["settings"]["validator"]["timeout_seconds"],
        5.0
    );

    fs::write(
        &suite,
        r#"{"version":1,"benchmarks":[{"name":"x","executable":"sum.sh","validate":"checks/missing.py"}]}"#,
    )
    .unwrap();
    let output = bench(&["--suite", suite.to_str().unwrap(), "--list-cases"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("benchmark x: --validate"));
}
