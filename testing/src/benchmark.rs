//! Compile a target if needed, then coordinate baseline runs and diagnostic replays.
use crate::runner::display_command;
use crate::{
    cli::Cli, diagnostics, experiment, metrics::METRICS, profiles, runner, stats::summarize_basic,
    storage,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};
fn resolve(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))
}
fn compiler_path(path: &Path, env: &BTreeMap<OsString, OsString>) -> Result<PathBuf, String> {
    if path.components().count() > 1 || path.is_absolute() {
        return resolve(path);
    }
    if let Some(search) = env.get(std::ffi::OsStr::new("PATH")) {
        for dir in std::env::split_paths(search) {
            let candidate = dir.join(path);
            if candidate.is_file() {
                return resolve(&candidate);
            }
        }
    }
    Err(format!("compiler not found: {}", path.display()))
}
pub(crate) enum Outcome {
    Report(Value),
    Interrupted(i32),
}
pub(crate) fn execute(
    args: &Cli,
    selected: &[usize],
    journal: &mut storage::Journal,
) -> Result<Outcome, String> {
    experiment::validate(args)?;
    let cwd = resolve(&args.cwd)?;
    if !cwd.is_dir() {
        return Err("--cwd must be a directory".into());
    }
    if !(args.output_limit * 1048576.0).is_finite() {
        return Err("--output-limit is too large".into());
    }
    let input = args.input.as_deref().map(resolve).transpose()?;
    let expected = args.expect_output.as_deref().map(resolve).transpose()?;
    for path in [&input, &expected].into_iter().flatten() {
        if !path.is_file() {
            return Err(format!("{} must be a file", path.display()));
        }
    }
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    let mut env_keys = Vec::new();
    for assignment in &args.env {
        let (key, value) = assignment
            .split_once('=')
            .filter(|(k, _)| !k.is_empty())
            .ok_or("--env requires KEY=VALUE")?;
        env.insert(key.into(), value.into());
        env_keys.push(key);
    }
    let affinity = experiment::affinity(args.cpu_affinity.as_deref())?;
    let build_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut compilation = Value::Null;
    let binary = if !args.source.is_empty() {
        let binary = build_dir.path().join("program");
        let mut command = vec![
            compiler_path(&args.compiler, &env)?.into_os_string(),
            format!("-std={}", args.standard).into(),
            format!("-O{}", args.opt).into(),
        ];
        command.extend(args.cxxflag.clone());
        for source in &args.source {
            command.push(resolve(source)?.into_os_string());
        }
        command.push("-o".into());
        command.push(binary.clone().into_os_string());
        command.extend(args.ldflag.clone());
        let build_cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let row = runner::run(
            &command,
            &runner::RunOptions {
                cwd: &build_cwd,
                env: &env,
                input: None,
                expected: None,
                expected_exit: 0,
                timeout: args.compile_timeout,
                output_limit: 16.0 * 1048576.0,
                units: 1.0,
                affinity: &[],
            },
            &[0],
        )?;
        if row["status"] == "interrupted" {
            return Ok(Outcome::Interrupted(
                row["interrupt_signal"].as_i64().unwrap_or(2) as i32,
            ));
        }
        if row["status"] != "ok" {
            return Err(format!(
                "compilation failed: {} (exit {})\n{}",
                row["status"],
                row["exit_code"],
                row["stderr_preview"].as_str().unwrap_or("")
            ));
        }
        compilation = json!({"command":display_command(&command),"wall_seconds":row["metrics"]["wall"],"diagnostics":row["stderr_preview"]});
        binary
    } else {
        resolve(
            args.program
                .as_deref()
                .or(args.executable.as_deref())
                .ok_or("provide an executable path: cppbench ./program (or use --source)")?,
        )?
    };
    let provenance = experiment::provenance(&binary, input.as_deref(), &env)?;
    let name = args.name.clone().unwrap_or_else(|| {
        binary
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    });
    let mut command = vec![binary.into_os_string()];
    command.extend(args.args.clone());
    let opts = runner::RunOptions {
        cwd: &cwd,
        env: &env,
        input: input.as_deref(),
        expected: expected.as_deref(),
        expected_exit: args.expect_exit,
        timeout: args.timeout,
        output_limit: args.output_limit * 1048576.0,
        units: args.units,
        affinity: &affinity,
    };
    let mut report = json!({"schema_version":3,"tool_version":env!("CARGO_PKG_VERSION"),"name":name,
        "implementation":"Rust CLI with isolated C process measurement", "measurement_backend":runner::BACKEND,
        "memory_accounting":{"source":"Linux wait4 ru_maxrss","unit":"KiB","scope":"Peak resident memory over the target child lifetime, including C launch setup, runtime mappings and waited-for descendants; not simultaneous process-tree RSS, heap allocations or executable section size"},
        "command":display_command(&command),"provenance":provenance,
        "host":{"system":std::env::consts::OS,"machine":std::env::consts::ARCH,"logical_cpus":std::thread::available_parallelism().map(|n|n.get()).ok()},
        "settings":{"runs":args.runs,"warmup":args.warmup,"timeout_seconds":args.timeout,"cwd":cwd,"work_units":args.units,
            "output_limit_mib":args.output_limit,"expected_exit":args.expect_exit,"input":input,"expected_output":expected,
            "environment_override_keys":env_keys,"cpu_affinity":affinity,"adaptive":args.adaptive,"max_runs":args.max_runs,
            "precision_percent":args.precision,"min_time_seconds":args.min_time},
        "compilation":compilation,"custom_metrics":args.metrics.is_some(),"selected_metrics":selected.iter().map(|&i|METRICS[i].0).collect::<Vec<_>>(),
        "metric_units":selected.iter().map(|&i|(METRICS[i].0,METRICS[i].1)).collect::<BTreeMap<_,_>>(),
        "warmups":[],"runs":[],"summary":{},"successful_runs":0,"warnings":[],"interrupt_signal":0,
        "completed":false,"measurement_passed":false,
        "diagnostics":{"results":{},"failed":false},
        "gpu":{"status":"unavailable","usage_percent":null,"reason":"Not collected; pass --gpu to monitor Intel GPU activity"}});
    journal.emit(json!({"event":"start","report":report}))?;
    let mut collect = selected.to_vec();
    if !collect.contains(&0) {
        collect.push(0);
    }
    let extended = args.adaptive || args.min_time.is_some();
    let limit = if extended { args.max_runs } else { args.runs };
    let mut walls = Vec::new();
    let started = std::time::Instant::now();
    let mut cancelled = 0;
    let mut stop_reason = "fixed_runs";
    for (phase, count) in [("warmups", args.warmup), ("runs", limit)] {
        if args.progress {
            eprintln!("cppbench [{name}]: starting {phase}, limit {count}");
        }
        for index in 0..count {
            let mut row = runner::run(&command, &opts, &collect)?;
            let wall = row["metrics"]["wall"]
                .as_f64()
                .ok_or("invalid wall measurement")?;
            row["elapsed_seconds"] = json!(wall);
            row["run"] = json!(index + 1);
            if !selected.contains(&0) {
                row["metrics"].as_object_mut().unwrap().remove("wall");
            }
            let failed = row["status"] != "ok";
            if row["status"] == "interrupted" {
                cancelled = row["interrupt_signal"].as_i64().unwrap_or(2);
            }
            if phase == "runs" && !failed {
                walls.push(wall);
            }
            journal.emit(json!({"event":"sample","name":name,"phase":phase,"row":row}))?;
            report[phase].as_array_mut().unwrap().push(row);
            if args.progress {
                let eta = report[phase]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|r| r["elapsed_seconds"].as_f64())
                    .sum::<f64>()
                    / (index + 1) as f64
                    * (count - index - 1) as f64;
                eprintln!(
                    "cppbench [{name}]: {phase} {}/{}; elapsed {:.1}s; ETA to run limit {:.1}s",
                    index + 1,
                    count,
                    started.elapsed().as_secs_f64(),
                    eta
                );
            }
            if cancelled != 0 || (failed && (phase == "warmups" || !args.keep_going)) {
                stop_reason = if cancelled != 0 {
                    "interrupted"
                } else {
                    "failure"
                };
                break;
            }
            if phase == "runs" && extended && index + 1 >= args.runs {
                let time_ok = walls.iter().sum::<f64>() >= args.min_time.unwrap_or(0.);
                let precision_ok =
                    !args.adaptive || crate::stats::precision_met(&walls, args.precision);
                if time_ok && precision_ok {
                    stop_reason = "targets_reached";
                    break;
                }
                stop_reason = "run_limit_reached";
            }
        }
        if cancelled != 0
            || (phase == "warmups"
                && report[phase]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["status"] != "ok"))
        {
            break;
        }
    }
    experiment::refresh(&mut report);
    let failed = ["warmups", "runs"].iter().any(|phase| {
        report[*phase]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["status"] != "ok")
    });
    let passed = cancelled == 0 && !failed && walls.len() >= args.runs as usize;
    let warnings = report["warnings"].as_array_mut().unwrap();
    if failed {
        warnings.push(json!(
            "Failures are excluded from summary statistics; inspect individual run statuses."
        ));
    }
    if !walls.is_empty() && walls.len() < 5 {
        warnings.push(json!(
            "Fewer than five successful measurements; distribution statistics have limited value."
        ));
    }
    if selected.contains(&0)
        && !walls.is_empty()
        && summarize_basic(walls.clone())["median"].as_f64().unwrap() < 0.01
    {
        warnings.push(json!("Runs under 10 ms are sensitive to launch and scheduling overhead; increase work per invocation."));
    }
    if stop_reason == "run_limit_reached" {
        warnings.push(json!("Run limit reached before the requested precision or total measurement duration; measurements are retained."));
    }
    report["stop_reason"] = json!(stop_reason);
    report["measurement_passed"] = json!(passed);
    report["interrupt_signal"] = json!(cancelled);
    journal.emit(json!({"event":"report","report":report}))?; // Preserve baseline before expensive diagnostics.
    let diagnostics = diagnostics::execute(args, &command, &opts, passed)?;
    if let Some(signal) = diagnostics["interrupt_signal"].as_i64().filter(|s| *s != 0) {
        cancelled = signal;
    }
    if diagnostics["results"]["gpu"].is_object() {
        report["gpu"] = diagnostics["results"]["gpu"].clone();
    }
    report["diagnostics"] = diagnostics;
    if !args.profile_flags.is_empty() {
        report["profiles"] = profiles::build(args, &report);
        if let Some(dir) = report["diagnostics"]["artifact_directory"].as_str() {
            storage::write_json(&Path::new(dir).join("profiles.json"), &report["profiles"])?;
        }
    }
    report["interrupt_signal"] = json!(cancelled);
    report["completed"] = json!(cancelled == 0);
    journal.emit(json!({"event":"report","report":report}))?;
    Ok(Outcome::Report(report))
}
