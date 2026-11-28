//! `bench compare` of saved reports: thresholds, validator directions, and rank tests.
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
fn write_script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn compare_saved_reports_with_validator_thresholds_and_rank_tests() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("prog.sh");
    let grader = dir.path().join("grade.sh");
    // Score 1.0 when the output has "good", 0.5 otherwise; higher is better.
    write_script(
        &grader,
        "#!/bin/sh\nif grep -q good; then s=1; else s=0.5; fi\necho \"bench-metric quality $s higher\"\necho 'bench-metric plain 1'\n",
    );
    let measure = |body: &str, out: &str| {
        write_script(&program, body);
        let output = bench(&[
            program.to_str().unwrap(),
            "--runs",
            "6",
            "--warmup",
            "0",
            "--validate",
            grader.to_str().unwrap(),
            "--format",
            "json",
            "--output",
            dir.path().join(out).to_str().unwrap(),
        ]);
        assert!(output.status.success(), "{output:?}");
        dir.path().join(out).to_str().unwrap().to_owned()
    };
    let before = measure("#!/bin/sh\necho good\n", "before.json");
    let after = measure("#!/bin/sh\nsleep 0.03\necho meh\n", "after.json");

    let json = |args: &[&str]| -> (Option<i32>, Value) {
        let mut all = vec!["compare", &before, &after, "--format", "json"];
        all.extend(args);
        let output = bench(&all);
        (
            output.status.code(),
            serde_json::from_slice(&output.stdout).unwrap_or(Value::Null),
        )
    };
    let (code, result) = json(&[]);
    assert_eq!(code, Some(0), "{result}");
    let change = |result: &Value, name: &str| {
        result["comparison"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["metric"] == name)
            .unwrap()
            .clone()
    };
    let wall = change(&result, "wall");
    assert_eq!(wall["rank_test"]["significant_at_0_05"], true, "{wall}");
    assert_eq!(wall["rank_test"]["probability_current_lower"], 0.0);
    assert_eq!(
        change(&result, "validator.quality")["higher_is_better"],
        true
    );
    assert!(change(&result, "validator.plain")["higher_is_better"].is_null());

    let (code, result) = json(&["--regression", "validator.quality=10"]);
    assert_eq!(code, Some(3));
    assert_eq!(result["status"], "regression");
    let (code, _) = json(&["--regression", "validator.quality=60"]);
    assert_eq!(code, Some(0));
    // No declared direction: a gate is refused rather than guessed.
    let (code, result) = json(&["--regression", "validator.plain=1"]);
    assert_eq!(code, Some(2));
    assert!(result["comparison"]["differences"]
        .to_string()
        .contains("no declared direction"));
    let (code, _) = json(&["--regression", "validator.plain:lower=1"]);
    assert_eq!(code, Some(0));
    let (code, _) = json(&["--warn-regression", "validator.quality=10"]);
    assert_eq!(code, Some(0));

    let text = bench(&["compare", &before, &after, "--regression", "wall=50"]);
    assert_eq!(text.status.code(), Some(3));
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("Baseline comparison: regression"), "{text}");
    assert!(text.contains("rank test p="), "{text}");
    assert!(text.contains("(significant); REGRESSION"), "{text}");

    // Live runs get the same rank test against --compare.
    let output = bench(&[
        program.to_str().unwrap(),
        "--runs",
        "3",
        "--warmup",
        "0",
        "--validate",
        grader.to_str().unwrap(),
        "--compare",
        &after,
        "--format",
        "json",
    ]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["comparison"]["changes"][0]["rank_test"]["p_value"].is_number());

    assert_eq!(
        bench(&["compare", &before, "/etc/hostname"]).status.code(),
        Some(2)
    );
    assert_eq!(
        bench(&[
            "compare",
            &before,
            &after,
            "--regression",
            "validator.x:up=1"
        ])
        .status
        .code(),
        Some(2)
    );
}
