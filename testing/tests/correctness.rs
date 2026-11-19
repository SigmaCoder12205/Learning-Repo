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
fn data(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}

#[test]
fn exact_stdout_checks_literal_bytes_and_reports_first_difference() {
    for (expected, status, offset) in [
        ("hello\n", "ok", Value::Null),
        ("hello", "output_mismatch", json!(5)),
        ("help\n", "output_mismatch", json!(3)),
        ("", "output_mismatch", json!(0)),
    ] {
        let output = bench(&[
            "/bin/cat",
            "--input-line=hello",
            "--expect-text",
            expected,
            "--runs=1",
            "--warmup=0",
            "--format=json",
        ]);
        assert_eq!(
            output.status.code(),
            Some(if status == "ok" { 0 } else { 1 }),
            "{output:?}"
        );
        let row = &data(&output)["runs"][0];
        assert_eq!(row["status"], status);
        assert_eq!(row["output_check"]["first_mismatch_byte"], offset);
    }
    let empty = bench(&[
        "/bin/true",
        "--expect-text=",
        "--runs=1",
        "--warmup=0",
        "--format=json",
    ]);
    assert!(empty.status.success(), "{empty:?}");
    let bad_warmup = bench(&[
        "/bin/true",
        "--expect-text=hello",
        "--runs=1",
        "--format=json",
    ]);
    assert_eq!(data(&bad_warmup)["warmups"][0]["status"], "output_mismatch");
    assert!(data(&bad_warmup)["runs"].as_array().unwrap().is_empty());
}

#[test]
fn assertions_scan_full_binary_output_and_cross_buffer_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    let expected = dir.path().join("expected");
    let mut bytes = vec![b'a'; 65535];
    bytes.extend_from_slice("ab€needle\0end\n".as_bytes());
    fs::write(&input, &bytes).unwrap();
    fs::write(&expected, &bytes).unwrap();
    let run = |needle: &str| {
        bench(&[
            "/bin/cat",
            "--input",
            path(&input),
            "--expect-output",
            path(&expected),
            "--expect-contains=aaaab",
            "--expect-contains",
            needle,
            "--runs=1",
            "--warmup=0",
            "--format=json",
        ])
    };
    let output = run("€needle");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["runs"][0]["stdout_preview_truncated"], true);
    let output = run("NEEDLE");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        data(&output)["runs"][0]["output_check"]["missing_contains"],
        json!([2])
    );
    bytes[65537] = b'!';
    fs::write(&expected, bytes).unwrap();
    let output = run("€needle");
    assert_eq!(
        data(&output)["runs"][0]["output_check"]["first_mismatch_byte"],
        65537
    );
}

#[test]
fn changing_output_after_preview_fails_stability_and_skips_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("change.py");
    let counter = dir.path().join("counter");
    let checkpoint = dir.path().join("journal");
    fs::write(&script, format!("from pathlib import Path\np=Path({:?})\nn=int(p.read_text())+1 if p.exists() else 1\np.write_text(str(n))\nprint('x'*1024+str(n))\n",path(&counter))).unwrap();
    let output = bench(&[
        path(&script),
        "--check-output-stability",
        "--runs=3",
        "--warmup=1",
        "--perf",
        "--checkpoint",
        path(&checkpoint),
        "--format=json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report = data(&output);
    assert_eq!(report["successful_runs"], 3);
    assert_eq!(report["summary"]["wall"]["count"], 3);
    assert_eq!(report["measurement_passed"], false);
    assert_eq!(report["output_stability"]["status"], "unstable");
    assert_eq!(report["output_stability"]["different_runs"], json!([2, 3]));
    assert_eq!(
        report["runs"][0]["stdout_preview"],
        report["runs"][1]["stdout_preview"]
    );
    assert_eq!(
        report["diagnostics"]["results"]["perf"]["status"],
        "skipped"
    );
    assert_eq!(
        data(&bench(&["--recover", path(&checkpoint), "--format=json"])),
        report
    );
    // Partial recovery recomputes repeatability from completed samples too.
    let lines = fs::read_to_string(&checkpoint).unwrap();
    let partial = lines
        .lines()
        .filter(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            v["event"] == "start" || v["event"] == "sample"
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&checkpoint, partial).unwrap();
    let recovered = data(&bench(&["--recover", path(&checkpoint), "--format=json"]));
    assert_eq!(recovered["output_stability"]["status"], "unstable");
    assert_eq!(recovered["completed"], false);
}

#[test]
fn stability_ignores_warmups_stderr_and_metric_selection() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("warm.py");
    let counter = dir.path().join("counter");
    fs::write(&script, format!("import sys\nfrom pathlib import Path\np=Path({:?})\nn=int(p.read_text())+1 if p.exists() else 1\np.write_text(str(n))\nprint(n if n<=2 else 'ready')\nprint(n,file=sys.stderr)\n",path(&counter))).unwrap();
    let output = bench(&[
        path(&script),
        "--check-output-stability",
        "--runs=3",
        "--warmup=2",
        "--metrics=rss",
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    assert_eq!(report["output_stability"]["status"], "stable");
    assert_eq!(report["output_stability"]["checked_runs"], 3);
    assert!(report["runs"][0]["metrics"].get("wall").is_none());
    assert!(report["warmups"][0]["output_check"]["stdout_sha256"].is_null());
}

#[test]
fn matrix_stability_is_per_case_and_survives_html_history() {
    let dir = tempfile::tempdir().unwrap();
    let history = dir.path().join("history");
    let output = bench(&[
        "/bin/cat",
        "--input-line=<value>1",
        "--vary-input=1=[\"<value>1\",\"<value>2\"]",
        "--expect-contains=<value>",
        "--check-output-stability",
        "--runs=2",
        "--warmup=0",
        "--format=json",
        "--record-history",
        "--history-dir",
        path(&history),
    ]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    for case in report["cases"].as_array().unwrap() {
        assert_eq!(case["output_stability"]["status"], "stable");
    }
    let checkpoint = dir.path().join("journal");
    fs::write(
        &checkpoint,
        json!({"event":"final","report":report}).to_string() + "\n",
    )
    .unwrap();
    let html = bench(&["--recover", path(&checkpoint), "--format=html"]);
    assert!(html.status.success());
    assert!(String::from_utf8_lossy(&html.stdout).contains("Output stability: stable"));
    assert!(String::from_utf8_lossy(&html.stdout).contains("&lt;value&gt;"));
    assert!(history.join("runs").is_dir());
}

#[test]
fn suite_listing_and_selection_do_not_execute_unselected_cases() {
    let dir = tempfile::tempdir().unwrap();
    let suite = dir.path().join("suite.json");
    let script = dir.path().join("mark.py");
    let marker = dir.path().join("marker");
    fs::write(
        &script,
        format!(
            "from pathlib import Path\nPath({:?}).touch()\n",
            path(&marker)
        ),
    )
    .unwrap();
    let mut config = json!({"version":1,"defaults":{"runs":2,"warmup":0,"check_output_stability":true},"benchmarks":[
        {"name":"unselected","python":"mark.py"},
        {"name":"one","executable":"/bin/cat","input_text":"one","expect_text":"one","expect_contains":["on"]},
        {"name":"two","executable":"/bin/cat","input_text":"two","expect_text":"two"}]});
    fs::write(&suite, config.to_string()).unwrap();
    let listing = bench(&["--suite", path(&suite), "--list-cases", "--format=json"]);
    assert!(listing.status.success(), "{listing:?}");
    assert_eq!(data(&listing)["cases"], json!(["unselected", "one", "two"]));
    assert!(!marker.exists());
    let output = bench(&[
        "--suite",
        path(&suite),
        "--case=two",
        "--case=one",
        "--case=one",
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let report = data(&output);
    assert_eq!(
        report["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].clone())
            .collect::<Vec<_>>(),
        vec![json!("one"), json!("two")]
    );
    assert!(!marker.exists());
    let invalid = bench(&["--suite", path(&suite), "--case=missing"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(!marker.exists());
    config["benchmarks"][0]["expect_contains"] = json!([""]);
    fs::write(&suite, config.to_string()).unwrap();
    assert_eq!(
        bench(&["--suite", path(&suite), "--case=one"])
            .status
            .code(),
        Some(2)
    );
    assert!(
        !marker.exists(),
        "even unselected entries must be validated"
    );
    assert_eq!(
        bench(&["--suite", path(&suite), "--list-cases", "--record-history"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(bench(&["/bin/true", "--case=one"]).status.code(), Some(2));
    assert_eq!(bench(&["/bin/true", "--list-cases"]).status.code(), Some(2));
}

#[test]
fn output_expectations_are_part_of_baseline_compatibility() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline");
    let expected = dir.path().join("expected");
    fs::write(&expected, "hello world").unwrap();
    let common = [
        "/bin/cat",
        "--input-text=hello world",
        "--runs=2",
        "--warmup=0",
        "--format=json",
    ];
    let run = |extra: &[&str]| {
        let mut args = common.to_vec();
        args.extend_from_slice(extra);
        bench(&args)
    };
    assert!(run(&[
        "--expect-output",
        path(&expected),
        "--save-baseline",
        path(&baseline)
    ])
    .status
    .success());
    let same = run(&["--expect-text=hello world", "--compare", path(&baseline)]);
    assert!(same.status.success(), "{same:?}");
    let changed = run(&["--expect-contains=world", "--compare", path(&baseline)]);
    assert_eq!(changed.status.code(), Some(2), "{changed:?}");
    let changed = run(&[
        "--expect-text=hello world",
        "--check-output-stability",
        "--compare",
        path(&baseline),
    ]);
    assert_eq!(changed.status.code(), Some(2), "{changed:?}");
}

#[test]
fn assertion_options_preflight_and_auxiliary_python_calibration_is_exempt() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    let marker = dir.path().join("marker");
    fs::write(
        &script,
        format!(
            "from pathlib import Path\nPath({:?}).touch()\nprint('ready')\n",
            path(&marker)
        ),
    )
    .unwrap();
    for flags in [
        vec!["--expect-contains="],
        vec!["--check-output-stability", "--runs=1"],
        vec!["--expect-text=ready", "--expect-output=missing"],
    ] {
        let mut args = vec![path(&script)];
        args.extend(flags);
        assert_eq!(bench(&args).status.code(), Some(2));
        assert!(!marker.exists());
    }
    let output = bench(&[
        path(&script),
        "--expect-text=ready\n",
        "--expect-contains=ready",
        "--check-output-stability",
        "--python-overhead",
        "--overhead-runs=2",
        "--runs=2",
        "--warmup=0",
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["python_overhead"]["status"], "ok");
    let timeout = bench(&[
        "/bin/sleep",
        "--expect-text=wrong",
        "--timeout=0.05",
        "--runs=1",
        "--warmup=0",
        "--format=json",
        "--",
        "2",
    ]);
    assert_eq!(data(&timeout)["runs"][0]["status"], "timeout");
}

#[test]
fn input_hint_tokens_do_not_confuse_print_message_or_scanf_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("hints.py");
    fs::write(&script, "print(input('Message: '))\nraw=input('comment: ')\nprint(raw)\nn=int(input('Count: '))\nx=float(input('Amount: '))\ny=input('Continue yes/no?')\nprint(sys.argv[1])\nmyinput('ignore')\n").unwrap();
    let report = data(&bench(&[
        path(&script),
        "--discover-inputs",
        "--format=json",
    ]));
    let hints = report["hints"].as_array().unwrap();
    assert_eq!(
        hints
            .iter()
            .map(|h| h["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["text", "text", "integer", "number", "boolean", "text"]
    );
    assert_eq!(hints[4]["suggestions"], json!(["yes", "no"]));
    let source = dir.path().join("hints.cpp");
    fs::write(
        &source,
        "printf(\"Enter value: \"); scanf (\"%lf\", &value);\nsscanf(buffer, \"%d\", &ignored);\n",
    )
    .unwrap();
    let report = data(&bench(&[
        "--input-source",
        path(&source),
        "--discover-inputs",
        "--format=json",
    ]));
    assert_eq!(report["hints"].as_array().unwrap().len(), 1);
    assert_eq!(report["hints"][0]["kind"], "number");
}
