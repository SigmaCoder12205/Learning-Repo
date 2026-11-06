//! Optional tool identity and counter-access checks.
use super::{command_run, find_tool, issue, read_report};
use crate::runner::RunOptions;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

pub fn check_tools() -> Result<Value, String> {
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    env.insert("LC_ALL".into(), "C".into());
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let opts = RunOptions {
        cwd: &cwd,
        env: &env,
        input: None,
        expected: None,
        expected_exit: 0,
        timeout: 10.,
        output_limit: 4. * 1048576.,
        units: 1.,
        affinity: &[],
    };
    let mut tools = serde_json::Map::new();
    let mut interrupted = 0;
    for (name, flag) in [
        ("perf", "--version"),
        ("intel_gpu_top", "-h"),
        ("heaptrack", "--version"),
        ("heaptrack_print", "--version"),
        ("valgrind", "--version"),
        ("strace", "--version"),
    ] {
        if interrupted != 0 {
            break;
        }
        let path = match find_tool(name, &env) {
            Ok(p) => p,
            Err(e) => {
                tools.insert(name.into(), issue("unavailable", e));
                continue;
            }
        };
        let row = command_run(&[path.clone(), flag.into()], &opts, dir.path(), name)?;
        if row["status"] == "interrupted" {
            interrupted = row["interrupt_signal"].as_i64().unwrap_or(2);
        }
        let stdout = read_report(&dir.path().join(format!("{name}.stdout"))).unwrap_or_default();
        let preview = if stdout.trim().is_empty() {
            row["stderr_preview"].as_str().unwrap_or("")
        } else {
            stdout.trim()
        };
        tools.insert(name.into(),json!({"status":if row["status"]=="ok"{"ok"}else{"unavailable"},"path":path,
            "identity":preview.lines().filter(|l|!l.trim().is_empty()).take(3).collect::<Vec<_>>().join("\n"),
            "sha256":crate::experiment::hash_file(Path::new(&path)).ok()}));
    }
    let mut probes = serde_json::Map::new();
    if interrupted == 0 {
        for (name, argv) in [
            ("perf", vec!["stat", "-e", "cycles:u", "--", "/bin/true"]),
            (
                "intel_gpu_top",
                vec!["-J", "-s", "10", "-n", "2", "-o", "-"],
            ),
        ] {
            if let Ok(path) = find_tool(name, &env) {
                let mut command = vec![path];
                command.extend(argv.into_iter().map(OsString::from));
                let row = command_run(&command, &opts, dir.path(), &format!("probe-{name}"))?;
                probes.insert(
                    name.into(),
                    json!({"status":row["status"],"stderr":row["stderr_preview"]}),
                );
                if row["status"] == "interrupted" {
                    interrupted = row["interrupt_signal"].as_i64().unwrap_or(2);
                    break;
                }
            }
        }
    }
    Ok(
        json!({"kind":"tool_check","tools":tools,"counter_probes":probes,"interrupt_signal":interrupted,
        "note":"checks tool identity and basic counter access; individual workloads and report formats can still expose compatibility limits"}),
    )
}
