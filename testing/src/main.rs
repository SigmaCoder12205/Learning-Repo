mod comparison;
mod diagnostics;
mod experiment;
mod hotspots;
mod html;
mod report;
mod runner;
mod suite;
use clap::Parser;
use report::View;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::Write as _,
    path::{Path, PathBuf},
};

pub const METRICS: [(&str, &str, &str); 15] = [
    (
        "wall",
        "s",
        "Elapsed process time including launch and exit",
    ),
    ("user", "s", "User-mode CPU time"),
    ("system", "s", "Kernel-mode CPU time"),
    ("cpu", "s", "Total user + system CPU time"),
    ("cpu_percent", "%", "CPU / wall time; can exceed 100%"),
    ("rss", "KiB", "Linux peak resident memory"),
    ("minor_faults", "count", "Page faults without disk I/O"),
    ("major_faults", "count", "Page faults requiring disk I/O"),
    (
        "input_blocks",
        "count",
        "Filesystem input operations, not bytes",
    ),
    (
        "output_blocks",
        "count",
        "Filesystem output operations, not bytes",
    ),
    ("voluntary_switches", "count", "Voluntary context switches"),
    (
        "involuntary_switches",
        "count",
        "Involuntary context switches",
    ),
    ("stdout_bytes", "bytes", "Captured stdout size"),
    ("stderr_bytes", "bytes", "Captured stderr size"),
    (
        "throughput",
        "units/s",
        "Declared work units / elapsed time",
    ),
];
fn positive(s: &str) -> Result<f64, String> {
    let n: f64 = s.parse().map_err(|_| "expected a number")?;
    if n.is_finite() && n > 0.0 {
        Ok(n)
    } else {
        Err("must be finite and greater than zero".into())
    }
}
#[derive(Parser, Debug, Clone)]
#[command(
    version,
    about = "Native Rust/C process benchmarks for Linux",
    after_help = "Examples:\n  cppbench ./build/program\n  cppbench ./program --runs 30 --metrics timing,memory -- --size 1000\n  cppbench --source main.cpp --cxxflag=-march=native"
)]
struct Cli {
    /// Executable path to benchmark
    #[arg(conflicts_with_all = ["executable", "source"])]
    program: Option<PathBuf>,
    /// Existing executable (alternative to positional path)
    #[arg(long, conflicts_with = "source")]
    executable: Option<PathBuf>,
    /// C++ translation units to compile once
    #[arg(long, num_args = 1..)]
    source: Vec<PathBuf>,
    #[arg(long, default_value = "g++")]
    compiler: PathBuf,
    #[arg(long = "std", default_value = "c++17")]
    standard: String,
    #[arg(long, default_value = "2", value_parser = ["0", "1", "2", "3", "s", "g", "fast"])]
    opt: String,
    #[arg(long, allow_hyphen_values = true)]
    cxxflag: Vec<OsString>,
    #[arg(long, allow_hyphen_values = true)]
    ldflag: Vec<OsString>,
    #[arg(long, default_value = "120", value_parser = positive)]
    compile_timeout: f64,
    #[arg(long, default_value = "10", value_parser = clap::value_parser!(u32).range(1..))]
    runs: u32,
    #[arg(long, default_value = "1")]
    warmup: u32,
    /// Deadline in seconds per invocation
    #[arg(long, default_value = "10", value_parser = positive)]
    timeout: f64,
    /// Combined stdout/stderr limit in MiB
    #[arg(long, default_value = "64", value_parser = positive)]
    output_limit: f64,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long)]
    expect_output: Option<PathBuf>,
    #[arg(long, default_value = "0", allow_negative_numbers = true)]
    expect_exit: i32,
    #[arg(long, default_value = ".")]
    cwd: PathBuf,
    /// Repeatable environment override
    #[arg(long, value_name = "KEY=VALUE")]
    env: Vec<String>,
    #[arg(long, default_value = "1", value_parser = positive)]
    units: f64,
    /// Comma-separated metric names or groups
    #[arg(long)]
    metrics: Option<String>,
    /// Text report detail (exports retain all selected data)
    #[arg(long, value_enum, default_value = "simple")]
    view: View,
    /// Show resource ranges and CPU breakdown
    #[arg(long, conflicts_with_all = ["advanced", "view"])]
    intermediate: bool,
    /// Show all statistics, configuration, and individual runs
    #[arg(long, conflicts_with_all = ["intermediate", "view"])]
    advanced: bool,
    #[arg(long)]
    list_metrics: bool,
    #[arg(long, default_value = "text", value_parser = ["text", "json", "csv", "html"])]
    format: String,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    keep_going: bool,
    /// Extra replay with CPU hardware counters (user-space events)
    #[arg(long)]
    perf: bool,
    /// Extra replay monitored by intel_gpu_top (device-wide activity)
    #[arg(long)]
    gpu: bool,
    /// Extra replay with heap allocation profiling
    #[arg(long)]
    heaptrack: bool,
    /// Extra replay with Valgrind Memcheck
    #[arg(long)]
    valgrind: bool,
    /// Run all five optional diagnostic passes after the baseline
    #[arg(long)]
    all_tools: bool,
    /// Seconds allowed per diagnostic or analysis command
    #[arg(long, default_value = "120", value_parser = positive)]
    diagnostic_timeout: f64,
    /// Parent directory for retained diagnostic artifacts
    #[arg(long, default_value = "cppbench-artifacts")]
    artifacts_dir: PathBuf,
    /// Comma-separated perf stat events
    #[arg(
        long,
        default_value = "cycles:u,instructions:u,cache-references:u,cache-misses:u,branches:u,branch-misses:u"
    )]
    perf_events: String,
    /// Intel GPU sampling interval in milliseconds
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(10..=60000))]
    gpu_interval_ms: u32,
    /// intel_gpu_top device selector, e.g. drm:/dev/dri/card1
    #[arg(long)]
    gpu_device: Option<String>,
    /// Run named benchmarks from a JSON suite
    #[arg(long, conflicts_with_all = ["program", "executable", "source", "recover", "check_tools"])]
    suite: Option<PathBuf>,
    /// Name stored with this experiment
    #[arg(long)]
    name: Option<String>,
    /// Save a successful experiment as a JSON comparison baseline
    #[arg(long)]
    save_baseline: Option<PathBuf>,
    /// Compare against a saved JSON report
    #[arg(long)]
    compare: Option<PathBuf>,
    /// Allowed worsening, e.g. --regression wall=5 --regression rss=10
    #[arg(long, requires = "compare")]
    regression: Vec<String>,
    /// Compare despite different workloads/hosts (differences remain visible)
    #[arg(long, requires = "compare")]
    allow_mismatch: bool,
    /// Extend measurements until the relative precision target is reached
    #[arg(long)]
    adaptive: bool,
    /// Maximum measured invocations when extending the experiment
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    max_runs: u32,
    /// Desired approximate 95% mean interval half-width as percent of mean
    #[arg(long, default_value = "5", value_parser = positive)]
    precision: f64,
    /// Minimum total measured wall time in seconds (across invocations)
    #[arg(long, value_parser = positive)]
    min_time: Option<f64>,
    /// Pin target processes to a CPU list, e.g. 0,2-3
    #[arg(long)]
    cpu_affinity: Option<String>,
    /// Show run progress, elapsed time, and estimated remaining work on stderr
    #[arg(long)]
    progress: bool,
    /// Append durable completed-run records to a new JSONL journal
    #[arg(long, conflicts_with = "recover")]
    checkpoint: Option<PathBuf>,
    /// Recover a report from a JSONL journal without executing the target
    #[arg(long, conflicts_with_all = ["program", "executable", "source", "suite", "check_tools"])]
    recover: Option<PathBuf>,
    /// Check installed optional tools and counter access without a target
    #[arg(long, conflicts_with_all = ["program", "executable", "source", "suite", "recover"])]
    check_tools: bool,
    /// Extra CPU sampling pass with function hotspots and a flamegraph
    #[arg(long)]
    hotspots: bool,
    /// Arguments passed verbatim to the target after --
    #[arg(last = true)]
    args: Vec<OsString>,
}
fn select_metrics(value: &str) -> Result<Vec<usize>, String> {
    let mut result = Vec::new();
    for token in value.split(',').map(str::trim) {
        let group = match token {
            "all" => (0..15).collect(),
            "timing" => vec![0, 1, 2, 3, 4, 14],
            "memory" => vec![5, 6, 7],
            "io" => vec![8, 9, 12, 13],
            "scheduling" => vec![10, 11],
            name => vec![METRICS
                .iter()
                .position(|m| m.0 == name)
                .ok_or_else(|| format!("unknown metric/group: {name}; use --list-metrics"))?],
        };
        for index in group {
            if !result.contains(&index) {
                result.push(index);
            }
        }
    }
    Ok(result)
}
fn percentile(values: &[f64], fraction: f64) -> f64 {
    let index = (values.len() - 1) as f64 * fraction;
    let low = index.floor() as usize;
    let high = (low + 1).min(values.len() - 1);
    values[low] + (values[high] - values[low]) * (index - low as f64)
}
fn summarize(values: Vec<f64>) -> Value {
    let mut result = summarize_basic(values.clone());
    let extra = experiment::intervals(&values);
    result
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    result
}
fn summarize_basic(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    let count = values.len();
    let mean = values.iter().sum::<f64>() / count as f64;
    let stdev = if count > 1 {
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (count - 1) as f64).sqrt()
    } else {
        0.0
    };
    json!({"count": count, "min": values[0], "max": values[count-1], "mean": mean,
        "median": percentile(&values, 0.5), "stdev": stdev, "p90": percentile(&values, 0.9),
        "p95": percentile(&values, 0.95), "cv_percent": if mean != 0.0 { stdev / mean * 100.0 } else { 0.0 }})
}
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
enum Outcome {
    Report(Value),
    Interrupted(i32),
}
fn execute(
    args: &Cli,
    selected: &[usize],
    journal: &mut experiment::Journal,
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
        "implementation":"Rust CLI with C process measurement","command":display_command(&command),"provenance":provenance,
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
                    !args.adaptive || experiment::precision_met(&walls, args.precision);
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
    if walls.len() < 5 {
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
    report["interrupt_signal"] = json!(cancelled);
    report["completed"] = json!(cancelled == 0);
    journal.emit(json!({"event":"report","report":report}))?;
    Ok(Outcome::Report(report))
}

fn display_command(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}
fn app() -> Result<i32, String> {
    use clap::{CommandFactory, FromArgMatches};
    let command = Cli::command();
    let matches = command.clone().get_matches();
    let args = Cli::from_arg_matches(&matches).map_err(|e| e.to_string())?;
    if args.list_metrics {
        for (name, unit, description) in METRICS {
            println!("{name:24} {unit:10} {description}");
        }
        println!("Groups: timing, memory, io, scheduling, all");
        return Ok(0);
    }
    if args.suite.is_some() {
        let allowed = [
            "suite",
            "format",
            "output",
            "progress",
            "checkpoint",
            "compare",
            "regression",
            "allow_mismatch",
            "save_baseline",
            "advanced",
            "intermediate",
            "view",
            "help",
            "version",
        ];
        for id in matches
            .ids()
            .filter(|id| command.get_arguments().any(|arg| arg.get_id() == *id))
        {
            if matches.value_source(id.as_str()) == Some(clap::parser::ValueSource::CommandLine)
                && !allowed.contains(&id.as_str())
            {
                return Err(format!(
                    "put --{} inside the suite's defaults or benchmark entry",
                    id.as_str().replace('_', "-")
                ));
            }
        }
    }
    if args.recover.is_some() || args.check_tools {
        let allowed = if args.check_tools {
            vec!["check_tools", "format", "output"]
        } else {
            vec![
                "recover",
                "format",
                "output",
                "compare",
                "regression",
                "allow_mismatch",
                "save_baseline",
                "advanced",
                "intermediate",
                "view",
            ]
        };
        for id in matches
            .ids()
            .filter(|id| command.get_arguments().any(|arg| arg.get_id() == *id))
        {
            if matches.value_source(id.as_str()) == Some(clap::parser::ValueSource::CommandLine)
                && !allowed.contains(&id.as_str())
            {
                return Err(format!(
                    "--{} does not apply to this mode",
                    id.as_str().replace('_', "-")
                ));
            }
        }
        if args.check_tools && !["text", "json"].contains(&args.format.as_str()) {
            return Err("--check-tools supports text or json output".into());
        }
    }
    // Output destinations must not clobber a journal or another requested report.
    let paths = [
        args.output.as_deref(),
        args.save_baseline.as_deref(),
        args.checkpoint.as_deref(),
    ];
    let canonical = |p: &Path| -> Result<PathBuf, String> {
        if p.exists() {
            return p.canonicalize().map_err(|e| e.to_string());
        }
        let parent = p
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok(parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(p.file_name().ok_or("output path needs a file name")?))
    };
    let mut destinations = Vec::new();
    for path in paths.into_iter().flatten() {
        let path = canonical(path)?;
        if destinations.contains(&path) {
            return Err("output, baseline, and checkpoint paths must differ".into());
        }
        if args
            .recover
            .as_deref()
            .is_some_and(|p| canonical(p).ok().as_ref() == Some(&path))
        {
            return Err("output must not overwrite the recovery journal".into());
        }
        destinations.push(path);
    }
    let baseline = args.compare.as_deref().map(comparison::load).transpose()?;
    let limits = comparison::thresholds(&args.regression)?;
    let view = if args.advanced {
        View::Advanced
    } else if args.intermediate {
        View::Intermediate
    } else {
        args.view
    };
    let mut journal = experiment::Journal::open(args.checkpoint.as_deref())?;
    let mut report = if let Some(path) = &args.recover {
        experiment::recover(path)?
    } else if args.check_tools {
        diagnostics::check_tools()?
    } else if args.suite.is_some() {
        suite::execute(&args, &mut journal)?
    } else {
        let selected = select_metrics(args.metrics.as_deref().unwrap_or("all"))?;
        match execute(&args, &selected, &mut journal)? {
            Outcome::Report(r) => r,
            Outcome::Interrupted(signal) => {
                eprintln!("cppbench: compilation interrupted");
                return Ok(128 + signal);
            }
        }
    };
    if let Some(baseline) = baseline {
        comparison::apply(&mut report, &baseline, &limits, args.allow_mismatch);
    }
    journal.emit(json!({"event":"final","report":report}))?;
    if let Some(path) = &args.save_baseline {
        if report["kind"] == "tool_check"
            || report["completed"] != true
            || report["cases"]
                .as_array()
                .map(|a| a.iter().any(|r| r["measurement_passed"] != true))
                .unwrap_or(report["measurement_passed"] != true)
        {
            return Err("cannot save an incomplete or failed measurement baseline".into());
        }
        experiment::write_json(path, &report)?;
    }
    let output = if report["kind"] == "tool_check" {
        if args.format == "json" {
            serde_json::to_string_pretty(&report).unwrap() + "\n"
        } else {
            let mut out = String::from("cppbench tool compatibility check\n");
            for (name, value) in report["tools"].as_object().unwrap() {
                out.push_str(&format!(
                    "{name}: {} | {}\n",
                    value["status"].as_str().unwrap_or("unavailable"),
                    value["identity"]
                        .as_str()
                        .unwrap_or(value["reason"].as_str().unwrap_or(""))
                        .lines()
                        .next()
                        .unwrap_or("")
                ));
            }
            for (name, probe) in report["counter_probes"].as_object().unwrap() {
                let status = probe["status"].as_str().unwrap_or("unavailable");
                out.push_str(&format!("{name} counter access: {status}\n"));
                if status != "ok" {
                    out.push_str(probe["stderr"].as_str().unwrap_or(""));
                    out.push('\n');
                }
            }
            out
        }
    } else if report["cases"].is_array() {
        suite::render(&report, &args.format, view)
    } else if args.format == "html" {
        html::render(&report)
    } else {
        let selected = report["selected_metrics"]
            .as_array()
            .ok_or("report lacks metric selection")?
            .iter()
            .filter_map(|n| METRICS.iter().position(|m| Some(m.0) == n.as_str()))
            .collect::<Vec<_>>();
        report::render(
            &report,
            &args.format,
            &selected,
            view,
            report["custom_metrics"] == true,
        )
    };
    if let Some(path) = &args.output {
        std::fs::write(path, output).map_err(|e| e.to_string())?;
    } else {
        std::io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    if report["kind"] == "tool_check" {
        let signal = report["interrupt_signal"].as_i64().unwrap_or(0);
        if signal != 0 {
            return Ok(128 + signal as i32);
        }
        return Ok(
            if report["tools"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v["status"] != "ok")
                || report["counter_probes"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|v| v["status"] != "ok")
            {
                1
            } else {
                0
            },
        );
    }
    Ok(comparison::exit_code(&report))
}

fn main() {
    let code = match app() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("cppbench: {error}");
            2
        }
    };
    std::process::exit(code);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics() {
        let s = summarize(vec![5., 1., 4., 2., 3.]);
        assert_eq!(s["count"], 5);
        assert_eq!(s["median"], 3.);
        assert!((s["p95"].as_f64().unwrap() - 4.8).abs() < 1e-10);
        assert!((s["stdev"].as_f64().unwrap() - 1.5811388300841898).abs() < 1e-10);
        assert_eq!(summarize(vec![0.])["cv_percent"], 0.);
    }
    #[test]
    fn selection() {
        assert_eq!(
            select_metrics("wall,memory,wall").unwrap(),
            vec![0, 5, 6, 7]
        );
        assert!(select_metrics("unknown").is_err());
        assert_eq!(select_metrics("all").unwrap().len(), 15);
    }
}
