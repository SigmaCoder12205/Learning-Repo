//! `bench history export` and `bench analyze`. Real pandas runs are optional:
//! set BENCH_TEST_PANDAS_PYTHON to an interpreter with pandas and numpy, or the
//! test uses python3 when it already has them, and otherwise skips that part.
use serde_json::Value;
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
fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn record(history: &Path, words: &str) {
    let validator =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/validators/nonempty_lines.sh");
    let output = bench(&[
        "/bin/echo",
        "--runs",
        "3",
        "--warmup",
        "0",
        "--record-history",
        "--history-dir",
        history.to_str().unwrap(),
        "--validate",
        validator.to_str().unwrap(),
        "--format",
        "json",
        "--",
        words,
    ]);
    assert!(output.status.success(), "{output:?}");
}
fn pandas_python() -> Option<String> {
    let candidate = std::env::var("BENCH_TEST_PANDAS_PYTHON").unwrap_or_else(|_| "python3".into());
    Command::new(&candidate)
        .args(["-c", "import pandas, numpy"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| candidate)
}

#[test]
fn history_export_is_one_row_per_run_and_metric() {
    let dir = private_dir();
    let history = dir.path().join("history");
    record(&history, "first");
    record(&history, "second");
    let history = history.to_str().unwrap();

    let output = bench(&["history", "--dir", history, "export"]);
    assert!(output.status.success(), "{output:?}");
    let csv = String::from_utf8(output.stdout).unwrap();
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("source,recorded_unix_seconds,case,run,status,kind,metric,unit,value")
    );
    let rows: Vec<_> = lines.collect();
    assert!(rows.iter().any(|r| r.contains(",ok,bench,wall,s,")));
    assert_eq!(
        rows.iter()
            .filter(|r| r.contains(",ok,validator,lines,count,1"))
            .count(),
        6
    );
    // Oldest saved run first, so rows read as a time series.
    let sources: Vec<_> = rows.iter().map(|r| r.split(',').next().unwrap()).collect();
    assert!(sources.windows(2).all(|w| w[0] <= w[1]));

    let output = bench(&[
        "history", "--dir", history, "export", "--format", "jsonl", "--limit", "1",
    ]);
    let rows: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        rows.iter()
            .map(|r| r["source"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );
    assert!(rows
        .iter()
        .all(|r| r["run"].is_u64() && r["value"].is_number()));
}

#[test]
fn analyze_passes_tidy_data_to_the_chosen_interpreter() {
    let dir = private_dir();
    let report = dir.path().join("report.json");
    let output = bench(&[
        "/bin/true",
        "--runs",
        "2",
        "--warmup",
        "0",
        "--format",
        "json",
        "--output",
        report.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");

    // A fake interpreter proves the contract without needing pandas.
    let seen = dir.path().join("seen.txt");
    let fake = dir.path().join("fake-python");
    fs::write(
        &fake,
        format!(
            "#!/bin/sh\n{{ echo \"$@\"; head -1 \"$2\"; grep -c ',bench,wall,' \"$2\"; head -c 26 \"$1\"; echo; }} > {}\necho analyzed\n",
            seen.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let out_file = dir.path().join("analysis.txt");
    let output = bench(&[
        "analyze",
        report.to_str().unwrap(),
        "--interpreter",
        fake.to_str().unwrap(),
        "--metric",
        "wall",
        "--format",
        "json",
        "--output",
        out_file.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fs::read_to_string(&out_file).unwrap(), "analyzed\n");
    let seen = fs::read_to_string(&seen).unwrap();
    assert!(seen.contains("--format json --metric wall"), "{seen}");
    assert!(
        seen.contains("source,recorded_unix_seconds,case,run,status,kind,metric,unit,value\n2\n"),
        "{seen}"
    );
    assert!(seen.contains("\"\"\"Statistics over bench"), "{seen}");

    // Failures are explained rather than silent.
    let output = bench(&["analyze"]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("give one or more JSON reports"));
    let output = bench(&["analyze", "/etc/hostname"]);
    assert_eq!(output.status.code(), Some(2));
    let output = bench(&[
        "analyze",
        report.to_str().unwrap(),
        "--interpreter",
        "/nonexistent/python",
    ]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot start /nonexistent/python"));

    // Without site-packages pandas is missing: the script says how to install it.
    let bare = dir.path().join("bare-python");
    fs::write(&bare, "#!/bin/sh\nexec python3 -S \"$@\"\n").unwrap();
    fs::set_permissions(&bare, fs::Permissions::from_mode(0o755)).unwrap();
    let output = bench(&[
        "analyze",
        report.to_str().unwrap(),
        "--interpreter",
        bare.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("needs pandas and numpy") && stderr.contains("exit 3"),
        "{stderr}"
    );
}

#[test]
fn analyze_with_real_pandas_when_available() {
    let Some(python) = pandas_python() else {
        eprintln!("skipping: no interpreter with pandas and numpy (set BENCH_TEST_PANDAS_PYTHON)");
        return;
    };
    let dir = private_dir();
    let history = dir.path().join("history");
    record(&history, "a");
    record(&history, "b");
    let output = bench(&[
        "analyze",
        "--history",
        "--history-dir",
        history.to_str().unwrap(),
        "--interpreter",
        &python,
        "--format",
        "json",
    ]);
    assert!(output.status.success(), "{output:?}");
    let analysis: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(analysis["kind"], "bench_analysis");
    let summary = analysis["summary"].as_array().unwrap();
    let lines = summary.iter().find(|r| r["metric"] == "lines").unwrap();
    assert_eq!(lines["kind"], "validator");
    assert_eq!(lines["n"], 6);
    assert_eq!(lines["median"], 1.0);
    assert!(summary
        .iter()
        .any(|r| r["metric"] == "wall" && r["sources"] == 2));
    assert!(analysis["trends"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["metric"] == "wall" && r["sources"] == 2));
}
