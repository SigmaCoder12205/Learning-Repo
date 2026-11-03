mod runner;
use clap::Parser;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::Write as _,
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
#[derive(Parser, Debug)]
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
    #[arg(long, default_value = "all")]
    metrics: String,
    #[arg(long)]
    list_metrics: bool,
    #[arg(long, default_value = "text", value_parser = ["text", "json", "csv"])]
    format: String,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    keep_going: bool,
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
fn summarize(mut values: Vec<f64>) -> Value {
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
fn execute(args: &Cli, selected: &[usize]) -> Result<Outcome, String> {
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
        compilation = json!({"command": display_command(&command), "wall_seconds": row["metrics"]["wall"], "diagnostics": row["stderr_preview"]});
        binary
    } else {
        resolve(
            args.program
                .as_deref()
                .or(args.executable.as_deref())
                .ok_or("provide an executable path: cppbench ./program (or use --source)")?,
        )?
    };
    let mut command = vec![binary.into_os_string()];
    command.extend(args.args.clone());
    let options = runner::RunOptions {
        cwd: &cwd,
        env: &env,
        input: input.as_deref(),
        expected: expected.as_deref(),
        expected_exit: args.expect_exit,
        timeout: args.timeout,
        output_limit: args.output_limit * 1048576.0,
        units: args.units,
    };
    let mut phases: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
    let mut cancelled = 0;
    for (phase, count) in [args.warmup, args.runs].into_iter().enumerate() {
        for index in 0..count {
            let mut row = runner::run(&command, &options, selected)?;
            row["run"] = json!(index + 1);
            let failed = row["status"] != "ok";
            if row["status"] == "interrupted" {
                cancelled = row["interrupt_signal"].as_i64().unwrap_or(2);
            }
            phases[phase].push(row);
            if cancelled != 0 || (failed && (phase == 0 || !args.keep_going)) {
                break;
            }
        }
        if cancelled != 0 || (phase == 0 && phases[0].iter().any(|r| r["status"] != "ok")) {
            break;
        }
    }
    let good: Vec<_> = phases[1].iter().filter(|r| r["status"] == "ok").collect();
    let mut summary = serde_json::Map::new();
    if !good.is_empty() {
        for &i in selected {
            let name = METRICS[i].0;
            summary.insert(
                name.into(),
                summarize(
                    good.iter()
                        .map(|r| r["metrics"][name].as_f64().unwrap())
                        .collect(),
                ),
            );
        }
    }
    let mut warnings = Vec::new();
    if phases.iter().flatten().any(|r| r["status"] != "ok") {
        warnings.push(
            "Failures are excluded from summary statistics; inspect individual run statuses.",
        );
    }
    if summary
        .get("wall")
        .and_then(|s| s["median"].as_f64())
        .is_some_and(|n| n < 0.01)
    {
        warnings.push("Runs under 10 ms are sensitive to launch and scheduling overhead; increase work per invocation.");
    }
    if good.len() < 5 {
        warnings.push(
            "Fewer than five successful measurements; distribution statistics have limited value.",
        );
    }
    Ok(Outcome::Report(
        json!({"schema_version": 2, "tool_version": env!("CARGO_PKG_VERSION"),
        "implementation": "Rust CLI with C process measurement", "command": display_command(&command),
        "host": {"system": std::env::consts::OS, "machine": std::env::consts::ARCH,
            "logical_cpus": std::thread::available_parallelism().map(|n| n.get()).ok()},
        "settings": {"runs": args.runs, "warmup": args.warmup, "timeout_seconds": args.timeout,
            "cwd": cwd, "work_units": args.units, "output_limit_mib": args.output_limit,
            "expected_exit": args.expect_exit, "input": input, "expected_output": expected,
            "environment_override_keys": env_keys},
        "compilation": compilation, "selected_metrics": selected.iter().map(|&i| METRICS[i].0).collect::<Vec<_>>(),
        "metric_units": selected.iter().map(|&i| (METRICS[i].0, METRICS[i].1)).collect::<BTreeMap<_,_>>(),
        "warmups": phases[0], "runs": phases[1], "summary": summary,
        "successful_runs": good.len(), "warnings": warnings, "interrupt_signal": cancelled}),
    ))
}
fn display_command(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}
fn render(report: &Value, format: &str, selected: &[usize]) -> String {
    if format == "json" {
        return serde_json::to_string_pretty(report).unwrap() + "\n";
    }
    let runs = report["runs"].as_array().unwrap();
    let mut out = String::new();
    if format == "csv" {
        out.push_str("run,status,exit_code");
        for &i in selected {
            let _ = write!(out, ",{} [{}]", METRICS[i].0, METRICS[i].1);
        }
        out.push('\n');
        for row in runs {
            let _ = write!(
                out,
                "{},{},{}",
                row["run"],
                row["status"].as_str().unwrap(),
                row["exit_code"]
            );
            for &i in selected {
                let _ = write!(out, ",{}", row["metrics"][METRICS[i].0]);
            }
            out.push('\n');
        }
        return out;
    }
    let _ = writeln!(
        out,
        "cppbench {} | {}/{} measured runs passed ({} requested)",
        env!("CARGO_PKG_VERSION"),
        report["successful_runs"],
        runs.len(),
        report["settings"]["runs"]
    );
    let _ = writeln!(
        out,
        "Command: {}\nHost: {} {} | logical CPUs available: {}\nWarmups: {}",
        report["command"],
        report["host"]["system"],
        report["host"]["machine"],
        report["host"]["logical_cpus"],
        report["warmups"].as_array().unwrap().len()
    );
    if let Some(wall) = report["compilation"]["wall_seconds"].as_f64() {
        let _ = writeln!(out, "Compilation: {wall:.6} s (excluded from runs)");
    }
    let _ = writeln!(
        out,
        "{:<32} {:>6} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>9}",
        "Metric (unit)", "N", "Mean", "Median", "Min", "Max", "Stddev", "P90", "P95", "CV %"
    );
    for &i in selected {
        let (name, unit, _) = METRICS[i];
        let stats = &report["summary"][name];
        if stats.is_null() {
            continue;
        }
        let _ = write!(
            out,
            "{:<32} {:>6}",
            format!("{name} ({unit})"),
            stats["count"]
        );
        for key in ["mean", "median", "min", "max", "stdev", "p90", "p95"] {
            let _ = write!(out, " {:>13.6}", stats[key].as_f64().unwrap());
        }
        let _ = writeln!(out, " {:>9.3}", stats["cv_percent"].as_f64().unwrap());
    }
    for phase in ["warmups", "runs"] {
        for row in report[phase].as_array().unwrap() {
            if row["status"] != "ok" {
                let _ = writeln!(
                    out,
                    "{} #{}: {} (exit {})\nstderr (escaped): {}",
                    phase, row["run"], row["status"], row["exit_code"], row["stderr_preview"]
                );
            }
        }
    }
    for warning in report["warnings"].as_array().unwrap() {
        let _ = writeln!(out, "Note: {}", warning.as_str().unwrap());
    }
    out
}
fn app() -> Result<i32, String> {
    let args = Cli::parse();
    if args.list_metrics {
        for (name, unit, description) in METRICS {
            println!("{name:24} {unit:10} {description}");
        }
        println!("Groups: timing, memory, io, scheduling, all");
        return Ok(0);
    }
    let selected = select_metrics(&args.metrics)?;
    let report = match execute(&args, &selected)? {
        Outcome::Report(report) => report,
        Outcome::Interrupted(signal) => {
            eprintln!("cppbench: compilation interrupted");
            return Ok(128 + signal);
        }
    };
    let output = render(&report, &args.format, &selected);
    if let Some(path) = &args.output {
        std::fs::write(path, output).map_err(|e| e.to_string())?;
    } else {
        std::io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    let interrupted = report["interrupt_signal"].as_i64().unwrap_or(0);
    if interrupted != 0 {
        return Ok(128 + interrupted as i32);
    }
    Ok(if report["successful_runs"] == args.runs {
        0
    } else {
        1
    })
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
