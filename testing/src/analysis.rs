//! Long-format ("tidy") measurement export and the optional pandas/numpy analysis runner.
//!
//! Rust owns parsing and flattening of reports; the embedded Python script only does
//! statistics, so the benchmark engine never depends on a Python runtime.
use clap::Args;
use serde_json::{json, Value};
use std::{
    fmt::Write as _,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

/// Column order of every tidy export; documented in docs/analytics.md.
pub(crate) const COLUMNS: [&str; 9] = [
    "source",
    "recorded_unix_seconds",
    "case",
    "run",
    "status",
    "kind",
    "metric",
    "unit",
    "value",
];
const SCRIPT: &str = include_str!("../python/bench_analysis.py");
const MAX_REPORT_BYTES: u64 = 64 * 1024 * 1024;

/// One row per measured run and metric: built-in metrics have kind `bench`,
/// metrics printed by a `--validate` program have kind `validator`.
pub(crate) fn tidy(report: &Value, source: &str) -> Vec<Value> {
    let recorded = &report["history"]["recorded_unix_seconds"];
    let mut rows = Vec::new();
    let cases: Vec<&Value> = match report["cases"].as_array() {
        Some(cases) => cases.iter().collect(),
        None => vec![report],
    };
    for case in cases {
        let name = case["name"].as_str().unwrap_or("");
        for run in case["runs"].as_array().into_iter().flatten() {
            let row = |kind: &str, metric: &str, unit: &Value, value: &Value| {
                json!({"source": source, "recorded_unix_seconds": recorded, "case": name,
                    "run": run["run"], "status": run["status"], "kind": kind,
                    "metric": metric, "unit": unit.as_str().unwrap_or(""), "value": value})
            };
            for (metric, value) in run["metrics"].as_object().into_iter().flatten() {
                rows.push(row("bench", metric, &case["metric_units"][metric], value));
            }
            let validation = &run["validation"];
            for (metric, value) in validation["metrics"].as_object().into_iter().flatten() {
                rows.push(row(
                    "validator",
                    metric,
                    &validation["metric_units"][metric],
                    value,
                ));
            }
        }
    }
    rows
}

fn csv_field(value: &Value) -> String {
    let text = match value {
        Value::Null => return String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text
    }
}

pub(crate) fn render(rows: &[Value], format: &str) -> String {
    let mut out = String::new();
    if format == "jsonl" {
        for row in rows {
            let _ = writeln!(out, "{row}");
        }
        return out;
    }
    out.push_str(&COLUMNS.join(","));
    out.push('\n');
    for row in rows {
        let fields: Vec<_> = COLUMNS.iter().map(|c| csv_field(&row[*c])).collect();
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    out
}

pub(crate) fn read_report(path: &Path) -> Result<Value, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() as u64 > MAX_REPORT_BYTES {
        return Err(format!("{} exceeds 64 MiB", path.display()));
    }
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("{}: not a JSON bench report: {e}", path.display()))?;
    if !report["runs"].is_array() && !report["cases"].is_array() {
        return Err(format!(
            "{}: not a bench report (save one with --format json --output FILE)",
            path.display()
        ));
    }
    Ok(report)
}

#[derive(Args, Debug, Clone)]
pub(crate) struct Options {
    /// JSON reports to analyze (from --format json --output, --save-baseline, or history)
    reports: Vec<PathBuf>,
    /// Also analyze saved local history runs
    #[arg(long)]
    history: bool,
    /// History location (default: $XDG_DATA_HOME/bench/history or ~/.local/share/bench/history)
    #[arg(long, requires = "history")]
    history_dir: Option<PathBuf>,
    /// Newest history runs to include
    #[arg(long, default_value = "50", requires = "history", value_parser = clap::value_parser!(u32).range(1..=1000))]
    limit: u32,
    /// Python with pandas and numpy (default: python3 from PATH; virtual environments work)
    #[arg(long, value_name = "PATH", default_value = "python3")]
    interpreter: PathBuf,
    /// Metric to analyze (repeatable); `all` keeps every metric. Default: wall, rss, and validator metrics
    #[arg(long)]
    metric: Vec<String>,
    /// Output format of the analysis
    #[arg(long, default_value = "text", value_parser = ["text", "json"])]
    format: String,
    /// Save a box plot of wall time per case/source (needs matplotlib)
    #[arg(long, value_name = "FILE.png")]
    plot: Option<PathBuf>,
    /// Write the analysis to a file instead of stdout
    #[arg(long)]
    output: Option<PathBuf>,
}

pub(crate) fn run(options: Options) -> Result<i32, String> {
    let mut rows = Vec::new();
    for path in &options.reports {
        let report = read_report(path)?;
        let source = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        rows.extend(tidy(&report, &source));
    }
    if options.history {
        let root = crate::history::location(options.history_dir.as_deref())?;
        for (id, report) in crate::history::newest(&root, options.limit as usize)? {
            rows.extend(tidy(&report, &id));
        }
    }
    if options.reports.is_empty() && !options.history {
        return Err("give one or more JSON reports, or pass --history".into());
    }
    if rows.is_empty() {
        return Err("no measured runs found in the selected reports".into());
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let data = dir.path().join("measurements.csv");
    let script = dir.path().join("bench_analysis.py");
    std::fs::write(&data, render(&rows, "csv")).map_err(|e| e.to_string())?;
    std::fs::write(&script, SCRIPT).map_err(|e| e.to_string())?;
    let mut command = Command::new(&options.interpreter);
    command
        .arg(&script)
        .arg(&data)
        .args(["--format", &options.format]);
    for metric in &options.metric {
        command.arg("--metric").arg(metric);
    }
    if let Some(plot) = &options.plot {
        command
            .arg("--plot")
            .arg(std::path::absolute(plot).map_err(|e| e.to_string())?);
    }
    let failure = |code: Option<i32>| {
        format!(
            "analysis failed with {} (interpreter {})",
            code.map_or("a signal".to_string(), |c| format!("exit {c}")),
            options.interpreter.display()
        )
    };
    let launch = |e: std::io::Error| {
        format!(
            "cannot start {}: {e}; pass --interpreter PATH to a Python with pandas and numpy",
            options.interpreter.display()
        )
    };
    if let Some(path) = &options.output {
        let output = command.output().map_err(launch)?;
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            return Err(failure(output.status.code()));
        }
        std::fs::write(path, &output.stdout).map_err(|e| e.to_string())?;
    } else {
        let status = command.status().map_err(launch)?;
        if !status.success() {
            return Err(failure(status.code()));
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tidy_rows_cover_suites_and_validator_metrics() {
        let report = json!({"cases":[{"name":"a,b","metric_units":{"wall":"s"},
            "runs":[{"run":1,"status":"ok","metrics":{"wall":0.5},
                "validation":{"metrics":{"score":0.9},"metric_units":{"score":"pct"}}}]}],
            "history":{"recorded_unix_seconds":7}});
        let rows = tidy(&report, "h1");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["kind"], "validator");
        let csv = render(&rows, "csv");
        assert!(csv
            .starts_with("source,recorded_unix_seconds,case,run,status,kind,metric,unit,value\n"));
        assert!(csv.contains("h1,7,\"a,b\",1,ok,bench,wall,s,0.5\n"));
        assert!(csv.contains("h1,7,\"a,b\",1,ok,validator,score,pct,0.9\n"));
        assert_eq!(render(&rows, "jsonl").lines().count(), 2);
    }
}
