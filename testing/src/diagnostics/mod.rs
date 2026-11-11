//! Optional tool replays are kept separate from uninstrumented baseline runs.
mod check;
mod gpu;
mod heaptrack;
mod perf;
mod support;
mod valgrind;

use crate::{cli::Cli, runner::RunOptions};
pub(crate) use check::check_tools;
use serde_json::{json, Value};
use std::{ffi::OsString, fs};
use support::issue;
pub(crate) use support::{command_run, find_tool, invocation_failed, read_report};
pub fn execute(
    args: &Cli,
    target: &[OsString],
    base: &RunOptions<'_>,
    allowed: bool,
) -> Result<Value, String> {
    let mut requested: Vec<&str> = [
        ("perf", args.perf),
        (
            "gpu",
            args.gpu || crate::profiles::includes(args, "graphics"),
        ),
        ("heaptrack", args.heaptrack),
        ("valgrind", args.valgrind),
        ("hotspots", args.hotspots),
    ]
    .into_iter()
    .filter(|(_, enabled)| *enabled || args.all_tools)
    .map(|(name, _)| name)
    .collect();
    if args.profile_flags.iter().any(|p| p == "networking") {
        requested.push("network");
    }
    if !args.profile_flags.is_empty() {
        requested.push("telemetry");
    }
    let mut results = serde_json::Map::new();
    if requested.is_empty() {
        return Ok(json!({"requested": [], "results": {}, "failed": false, "interrupt_signal": 0}));
    }
    if !allowed {
        for name in &requested {
            results.insert(
                (*name).into(),
                issue("skipped", "baseline did not complete successfully"),
            );
        }
        return Ok(
            json!({"requested": requested, "results": results, "failed": true, "interrupt_signal": 0}),
        );
    }
    fs::create_dir_all(&args.artifacts_dir).map_err(|e| e.to_string())?;
    let parent = args
        .artifacts_dir
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let dir = tempfile::Builder::new()
        .prefix("session-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?
        .keep();
    // Stabilize tool parsers without changing the target's requested environment:
    // locale is overridden for these instrumented passes and explicitly recorded.
    let mut env = base.env.clone();
    env.insert("LC_ALL".into(), "C".into());
    let opts = RunOptions {
        cwd: base.cwd,
        env: &env,
        input: base.input,
        expected: base.expected,
        expected_exit: base.expected_exit,
        timeout: args.diagnostic_timeout,
        output_limit: base.output_limit,
        units: base.units,
        affinity: base.affinity,
    };
    let mut signal = 0;
    for name in &requested {
        if signal != 0 {
            results.insert((*name).into(), issue("skipped", "interrupted"));
            continue;
        }
        eprintln!("bench: running {name} diagnostic (separate from baseline)");
        let result = match *name {
            "perf" => perf::run(args, target, &opts, &dir),
            "gpu" => gpu::run(args, target, &opts, &dir),
            "heaptrack" => heaptrack::run(target, &opts, &dir),
            "valgrind" => valgrind::run(target, &opts, &dir),
            "hotspots" => crate::hotspots::run(target, &opts, &dir),
            "network" => crate::profiles::trace_network(target, &opts, &dir),
            "telemetry" => crate::profiles::collect_telemetry(args, target, &opts, &dir),
            _ => unreachable!(),
        };
        let value = result.unwrap_or_else(|e| issue("unavailable", e));
        signal = value["interrupt_signal"].as_i64().unwrap_or(0);
        results.insert((*name).into(), value);
    }
    let failed = results.iter().any(|(name, v)| {
        if v["status"] == "ok" {
            return false;
        }
        if name == "network" || (name == "gpu" && !args.gpu && !args.all_tools) {
            return false;
        }
        if name == "telemetry" && v["status"] == "unavailable" {
            return false;
        }
        true
    });
    let report = json!({"requested": requested, "results": results, "artifact_directory": dir,
        "failed": failed, "interrupt_signal": signal,
        "execution": "one separate target replay per tool; excluded from baseline statistics",
        "environment_overrides": {"LC_ALL": "C"},
        "timeout_seconds_per_command": args.diagnostic_timeout});
    fs::write(
        dir.join("diagnostics.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(report)
}
