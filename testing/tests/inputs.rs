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
fn terminal(workload: &Path, mode: &str) -> Value {
    let out = Command::new("python3")
        .args([
            "tests/fixtures/input_terminal.py",
            env!("CARGO_BIN_EXE_bench"),
            path(workload),
            mode,
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    data(&out)
}

#[test]
fn inline_answers_replay_and_keep_arguments_separate() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(
        &script,
        "import sys\na=input()\nb=input()\nprint(a,b,sys.argv[1])\n",
    )
    .unwrap();
    let out = bench(&[
        path(&script),
        "--input-line=hello world",
        "--input-line=-2",
        "--runs=3",
        "--warmup=1",
        "--format=json",
        "--",
        "argument",
    ]);
    assert!(out.status.success(), "{out:?}");
    let report = data(&out);
    assert_eq!(report["successful_runs"], 3);
    assert_eq!(report["warmups"].as_array().unwrap().len(), 1);
    for row in report["runs"].as_array().unwrap() {
        assert_eq!(row["stdout_preview"], "hello world -2 argument\n");
    }
    assert!(report["settings"]["input"].is_null());
    assert_eq!(report["input_replay"]["source"], "inline");
    assert_eq!(
        report["provenance"]["input_sha256"].as_str().unwrap().len(),
        64
    );
    let exact = dir.path().join("expected");
    fs::write(&exact, "no trailing newline").unwrap();
    let out = bench(&[
        "/bin/cat",
        "--input-text=no trailing newline",
        "--expect-output",
        path(&exact),
        "--runs=1",
        "--warmup=0",
        "--format=json",
    ]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        data(&out)["runs"][0]["stdout_preview"],
        "no trailing newline"
    );
}

#[test]
fn discovery_does_not_execute_and_reports_source_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(&script,"raise RuntimeError('must not run')\nimport argparse\np=argparse.ArgumentParser()\np.add_argument('--count', type=int, choices=[1,2,3])\na=int(input('Age: '))\nb=float(input('Amount: '))\nc=input('Name: ')\n").unwrap();
    let out = bench(&[
        path(&script),
        "--discover-inputs",
        "--format=json",
        "--",
        "--count",
        "2",
    ]);
    assert!(out.status.success(), "{out:?}");
    let report = data(&out);
    let hints = report["hints"].as_array().unwrap();
    assert!(hints.iter().any(|h| h["channel"] == "argument"
        && h["position"] == 2
        && h["suggestions"] == json!(["1", "2", "3"])));
    for kind in ["integer", "number", "text"] {
        assert!(hints
            .iter()
            .any(|h| h["kind"] == kind && h["channel"] == "stdin"));
    }
    let source = dir.path().join("native.cpp");
    fs::write(&source,"int age;\ndouble amount;\nstd::cin >> age;\nscanf(\"%lf\", &amount);\nchar op;\nscanf(\" %c\", &op);\nswitch(op) {\ncase '+': break;\ncase '-': break;\n}\n").unwrap();
    let out = bench(&[
        "--input-source",
        path(&source),
        "--discover-inputs",
        "--format=json",
    ]);
    let hints = data(&out)["hints"].as_array().unwrap().clone();
    assert_eq!(hints[0]["kind"], "integer");
    assert_eq!(hints[1]["kind"], "number");
    assert_eq!(hints[2]["suggestions"], json!(["+", "-"]));
    let unknown = bench(&["/bin/true", "--discover-inputs", "--format=json"]);
    assert_eq!(data(&unknown)["status"], "unknown");
}

#[test]
fn input_and_argument_matrix_keeps_failures_and_outputs_per_case() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(
        &script,
        "import sys\nn=int(input())\nprint(sys.argv[1], n*2)\n",
    )
    .unwrap();
    let out = bench(&[
        path(&script),
        "--input-line=2",
        "--vary-input=1=[\"0\",\"oops\",\"4\"]",
        "--vary-arg=1=[\"a\",\"b\"]",
        "--runs=2",
        "--warmup=0",
        "--format=json",
        "--",
        "a",
    ]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let report = data(&out);
    assert_eq!(report["kind"], "input_matrix");
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    assert_eq!(
        cases
            .iter()
            .filter(|c| c["measurement_passed"] == true)
            .count(),
        4
    );
    assert!(cases
        .iter()
        .any(|c| c["runs"][0]["stdout_preview"] == "b 8\n"));
    assert!(cases.iter().all(|c| c["input_case"].is_object()));
    assert!(cases
        .iter()
        .filter(|c| c["measurement_passed"] == false)
        .all(|c| c["successful_runs"] == 0));
}

#[test]
fn inferred_exploration_is_bounded_and_retains_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(&script, "n=int(input('Count: '))\nprint(n*n)\n").unwrap();
    let out = bench(&[
        path(&script),
        "--input-line=7",
        "--explore-inputs",
        "--max-cases=4",
        "--runs=1",
        "--warmup=0",
        "--format=json",
    ]);
    assert!(out.status.success(), "{out:?}");
    let report = data(&out);
    assert_eq!(report["cases"].as_array().unwrap().len(), 4);
    assert_eq!(report["cases"][0]["input_case"]["answers"], json!(["7"]));
    assert_eq!(
        report["cases"][0]["runs"][0]["stdout_preview"],
        "Count: 49\n"
    );
    assert!(report["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["input_case"]["answers"] == json!(["0"])));
}

#[test]
fn native_source_matrix_reuses_one_compilation() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("calc.cpp");
    fs::write(&source,"#include <cstdio>\nint main(){ int n; if(scanf(\"%d\",&n)!=1)return 1;printf(\"%d\\n\",n+1); }\n").unwrap();
    let out = bench(&[
        "--source",
        path(&source),
        "--input-line=1",
        "--vary-input=1=[\"1\",\"3\"]",
        "--runs=1",
        "--warmup=0",
        "--format=json",
    ]);
    assert!(out.status.success(), "{out:?}");
    let report = data(&out);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0]["command"][0], cases[1]["command"][0]);
    assert_eq!(cases[0]["compilation"], cases[1]["compilation"]);
    assert!(cases[0]["compilation"].is_object());
    assert_eq!(cases[1]["runs"][0]["stdout_preview"], "4\n");
}

#[test]
fn malformed_or_oversized_matrices_fail_before_target_execution() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("marker");
    let script = dir.path().join("work.py");
    fs::write(
        &script,
        format!(
            "from pathlib import Path\nPath({:?}).touch()\n",
            path(&marker)
        ),
    )
    .unwrap();
    for flags in [
        vec!["--input-line=1", "--vary-input=2=[\"a\"]"],
        vec!["--vary-arg=2=[\"a\"]"],
        vec!["--input-line=1", "--vary-input=0=[\"a\"]"],
        vec!["--input-line=1", "--vary-input=1=[]"],
        vec![
            "--input-line=1",
            "--vary-input=1=[\"a\",\"b\"]",
            "--max-cases=1",
        ],
        vec![
            "--input-line=1",
            "--vary-input=1=[\"a\"]",
            "--vary-input=1=[\"b\"]",
        ],
        vec!["--input-line=1", "--input-text=2"],
        vec!["--interactive"],
    ] {
        let mut args = vec![path(&script)];
        args.extend(flags);
        let out = bench(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        assert!(!marker.exists());
    }
    let input = dir.path().join("large-input");
    fs::write(&input, format!("a\n{}", "x".repeat(1024 * 1024 - 2))).unwrap();
    let out = bench(&[
        path(&script),
        "--input",
        path(&input),
        "--vary-input=1=[\"a\",\"longer\"]",
    ]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(
        !marker.exists(),
        "all generated inputs must be validated before running any case"
    );
}

#[test]
fn interactive_auto_and_explicit_capture_replay_without_human_time() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(
        &script,
        "a=int(input('LIVE'+'1>'))\nb=float(input('LIVE'+'2>'))\nprint(a+b)\n",
    )
    .unwrap();
    for mode in ["auto", "explicit", "explore"] {
        let result = terminal(&script, mode);
        assert_eq!(result["code"], 0, "{result}");
        let report = &result["report"];
        assert_eq!(report["input_setup"]["status"], "ok");
        assert_eq!(report["input_setup"]["captured_bytes"], 6);
        if mode == "explore" {
            assert_eq!(report["cases"].as_array().unwrap().len(), 4);
        } else {
            assert_eq!(report["successful_runs"], 2);
            assert_eq!(report["warmups"].as_array().unwrap().len(), 1);
            assert_eq!(report["runs"][0]["stdout_preview"], "LIVE1>LIVE2>9.5\n");
        }
        assert!(result["terminal"]
            .as_str()
            .unwrap()
            .contains("likely integer"));
    }
}

#[test]
fn interactive_eof_timeout_and_interrupt_produce_bounded_reports() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    fs::write(&script, "print('LIVE'+'1>',end='',flush=True)\ninput()\n").unwrap();
    for (mode, code, status) in [("timeout", 1, "timeout"), ("cancel", 130, "interrupted")] {
        let result = terminal(&script, mode);
        assert_eq!(result["code"], code, "{result}");
        assert_eq!(result["report"]["input_setup"]["status"], status);
        assert_eq!(result["report"]["successful_runs"], 0);
    }
    fs::write(
        &script,
        "import sys\nprint('EOF>',end='',flush=True)\nprint(len(sys.stdin.read()))\n",
    )
    .unwrap();
    let result = terminal(&script, "eof");
    assert_eq!(result["code"], 0, "{result}");
    assert_eq!(result["report"]["successful_runs"], 2);
    assert_eq!(result["report"]["runs"][0]["stdout_preview"], "EOF>0\n");
}

#[test]
fn input_cases_round_trip_through_history_recovery_and_html() {
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("runs.jsonl");
    let history = dir.path().join("history");
    let out = bench(&[
        "/bin/cat",
        "--input-line=one",
        "--vary-input=1=[\"one\",\"<script>two\"]",
        "--runs=1",
        "--warmup=0",
        "--checkpoint",
        path(&checkpoint),
        "--record-history",
        "--history-dir",
        path(&history),
        "--format=json",
    ]);
    assert!(out.status.success(), "{out:?}");
    let report = data(&out);
    let recovered = bench(&["--recover", path(&checkpoint), "--format=json"]);
    assert_eq!(data(&recovered), report);
    let html = bench(&["--recover", path(&checkpoint), "--format=html"]);
    let text = String::from_utf8_lossy(&html.stdout);
    assert!(text.contains("&lt;script&gt;two"));
    let id = report["history"]["id"].as_str().unwrap();
    assert!(
        bench(&["history", "--dir", path(&history), "pin", id, "inputs"])
            .status
            .success()
    );
    let compare = bench(&[
        "/bin/cat",
        "--input-line=one",
        "--vary-input=1=[\"one\",\"<script>two\"]",
        "--runs=1",
        "--warmup=0",
        "--compare-pin=inputs",
        "--history-dir",
        path(&history),
        "--format=json",
    ]);
    assert!(compare.status.success(), "{compare:?}");
}

#[test]
fn suites_accept_inline_answers_and_still_preflight() {
    let dir = tempfile::tempdir().unwrap();
    let suite = dir.path().join("suite.json");
    let mut config = json!({"version":1,"defaults":{"runs":1,"warmup":0},"benchmarks":[{"name":"lines","executable":"/bin/cat","input_lines":["hello","world"]},{"name":"text","executable":"/bin/cat","input_text":"exact"}]});
    fs::write(&suite, config.to_string()).unwrap();
    let out = bench(&["--suite", path(&suite), "--format=json"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        data(&out)["cases"][0]["runs"][0]["stdout_preview"],
        "hello\nworld\n"
    );
    config["benchmarks"][1]["input_lines"] = json!(["conflict"]);
    fs::write(&suite, config.to_string()).unwrap();
    assert_eq!(bench(&["--suite", path(&suite)]).status.code(), Some(2));
    config["benchmarks"][1]
        .as_object_mut()
        .unwrap()
        .remove("input_lines");
    config["benchmarks"][1]["input_source"] = json!("missing.cpp");
    fs::write(&suite, config.to_string()).unwrap();
    assert_eq!(bench(&["--suite", path(&suite)]).status.code(), Some(2));
}

#[test]
fn native_terminal_capture_and_trial_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("work.cpp");
    let executable = dir.path().join("work");
    fs::write(&source, "#include <iostream>\nint main(){int a; double b; std::cout<<\"LIVE1>\"; std::cin>>a; std::cout<<\"LIVE2>\"; std::cin>>b; std::cout<<a+b<<'\\n';}\n").unwrap();
    assert!(Command::new("g++")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap()
        .success());
    let result = terminal(&executable, "auto");
    assert_eq!(result["code"], 0, "{result}");
    assert_eq!(
        result["report"]["runs"][0]["stdout_preview"],
        "LIVE1>LIVE2>9.5\n"
    );

    let script = dir.path().join("child.py");
    let pidfile = dir.path().join("pid");
    fs::write(&script, format!("import os,time\nfrom pathlib import Path\npid=os.fork()\nif pid==0:\n    time.sleep(30)\nelse:\n    Path({:?}).write_text(str(pid))\n    print('LIVE'+'1>',flush=True)\n    input()\n", path(&pidfile))).unwrap();
    for mode in ["timeout", "cancel"] {
        let result = terminal(&script, mode);
        assert_eq!(result["report"]["successful_runs"], 0, "{result}");
        let pid = fs::read_to_string(&pidfile).unwrap();
        let stat = format!("/proc/{}/stat", pid.trim());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match fs::read_to_string(&stat) {
                Err(_) => break,
                Ok(text) if text.split(") ").nth(1).unwrap().starts_with('Z') => break,
                _ if std::time::Instant::now() >= deadline => panic!("descendant still alive"),
                _ => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        }
    }
    for (body, mode, status) in [
        (
            "while True: print('x'*1000, flush=True)\n",
            "output_limit",
            "output_limit",
        ),
        ("raise SystemExit(3)\n", "explicit", "unexpected_exit"),
    ] {
        fs::write(&script, body).unwrap();
        let result = terminal(&script, mode);
        assert_eq!(result["code"], 1, "{result}");
        assert_eq!(result["report"]["input_setup"]["status"], status);
        assert_eq!(result["report"]["successful_runs"], 0);
    }
}

#[test]
fn inline_answers_are_replayed_for_separate_telemetry() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("work.py");
    let log = dir.path().join("answers");
    fs::write(&script, format!("import json,os\na=input()\nassert a=='hello'\nwith open({:?},'a') as out: out.write(a+'\\n')\np=os.environ.get('BENCH_TELEMETRY_FILE')\nif p:\n    with open(p,'x') as out: json.dump({{'version':1,'metrics':{{'backend.requests':{{'unit':'count','value':1}}}}}},out)\n", path(&log))).unwrap();
    let output = bench(&[
        path(&script),
        "--input-line=hello",
        "--backend",
        "--api",
        "--runs=2",
        "--warmup=1",
        "--artifacts-dir",
        path(dir.path()),
        "--format=json",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(data(&output)["successful_runs"], 2);
    assert_eq!(
        data(&output)["diagnostics"]["requested"],
        json!(["telemetry"])
    );
    assert_eq!(fs::read_to_string(log).unwrap(), "hello\n".repeat(4));
}
