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
    /// Also save the measurement table; format from the extension: .csv, .jsonl or .parquet
    #[arg(long, value_name = "FILE")]
    save_data: Option<PathBuf>,
    /// Write a Jupyter notebook with the data embedded instead of running the analysis
    #[arg(long, value_name = "FILE.ipynb", conflicts_with_all = ["plot", "output", "format", "metric"])]
    notebook: Option<PathBuf>,
}

/// Parquet is written by pandas (pyarrow or fastparquet), keeping Arrow out of the binary.
const PARQUET: &str = r#"
import sys
try:
    import pandas as pd
except ImportError:
    sys.stderr.write(f"parquet export needs pandas and pyarrow in {sys.executable}:\n"
                     "  python3 -m venv .venv && .venv/bin/pip install pandas pyarrow\n")
    sys.exit(3)
frame = pd.read_csv(sys.argv[1], keep_default_na=False, na_values={"recorded_unix_seconds": [""]},
                    dtype={"source": str, "case": str, "status": str, "kind": str, "metric": str, "unit": str})
frame["recorded_unix_seconds"] = frame["recorded_unix_seconds"].astype("Int64")
frame["run"] = frame["run"].astype("Int64")
frame["value"] = frame["value"].astype("float64")
try:
    frame.to_parquet(sys.argv[2], index=False)
except ImportError as error:
    sys.stderr.write(f"parquet export needs pyarrow (or fastparquet): {error}\n"
                     f"  {sys.executable} -m pip install pyarrow\n")
    sys.exit(3)
"#;

/// Write tidy rows as csv, jsonl or parquet (parquet runs `interpreter`).
pub(crate) fn write_data(
    rows: &[Value],
    path: &Path,
    format: &str,
    interpreter: &Path,
) -> Result<(), String> {
    if format != "parquet" {
        return std::fs::write(path, render(rows, format))
            .map_err(|e| format!("{}: {e}", path.display()));
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let data = dir.path().join("measurements.csv");
    std::fs::write(&data, render(rows, "csv")).map_err(|e| e.to_string())?;
    let target = std::path::absolute(path).map_err(|e| e.to_string())?;
    let status = Command::new(interpreter)
        .arg("-c")
        .arg(PARQUET)
        .arg(&data)
        .arg(&target)
        .status()
        .map_err(|e| {
            format!(
                "cannot start {}: {e}; pass --interpreter PATH to a Python with pandas and pyarrow",
                interpreter.display()
            )
        })?;
    if !status.success() {
        return Err(format!(
            "parquet export failed with exit {} (interpreter {})",
            status.code().map_or("signal".into(), |c| c.to_string()),
            interpreter.display()
        ));
    }
    Ok(())
}

/// Data format from a file extension, for `--save-data`.
pub(crate) fn format_for(path: &Path) -> Result<&'static str, String> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => Ok("csv"),
        Some("jsonl") => Ok("jsonl"),
        Some("parquet") => Ok("parquet"),
        _ => Err(format!(
            "{}: use a .csv, .jsonl or .parquet extension",
            path.display()
        )),
    }
}

const NOTEBOOK_LIMIT: usize = 16 * 1024 * 1024;

/// A Jupyter (nbformat 4) notebook with the measurements embedded as a Python string.
pub(crate) fn notebook(rows: &[Value], sources: &[String]) -> Result<Value, String> {
    let csv = render(rows, "csv");
    if csv.len() > NOTEBOOK_LIMIT {
        return Err("measurements exceed 16 MiB; use --limit or fewer reports for a notebook, or --save-data".into());
    }
    // A JSON string literal is also a valid Python string literal (same escapes).
    let literal = serde_json::to_string(&csv).unwrap();
    let markdown =
        |id: &str, text: &str| json!({"cell_type":"markdown","id":id,"metadata":{},"source":text});
    let code = |id: &str, text: &str| json!({"cell_type":"code","id":id,"metadata":{},"execution_count":null,"outputs":[],"source":text});
    let listed: Vec<_> = sources
        .iter()
        .map(|s| format!("- `{}`", s.replace('`', "'")))
        .collect();
    Ok(json!({
        "nbformat": 4, "nbformat_minor": 5,
        "metadata": {"kernelspec": {"name": "python3", "display_name": "Python 3", "language": "python"},
            "language_info": {"name": "python"}, "bench": {"version": env!("CARGO_PKG_VERSION"), "rows": rows.len()}},
        "cells": [
            markdown("intro", &format!("# bench measurements\n\nGenerated by bench {}. One row per measured run and metric (`kind` is `bench` for built-in metrics and `validator` for your checks). The data is embedded below, so this notebook works on its own. Needs pandas and numpy; matplotlib for the plots.\n\nSources:\n{}", env!("CARGO_PKG_VERSION"), listed.join("\n"))),
            code("load", &format!("import io\n\nimport numpy as np\nimport pandas as pd\n\nDATA = {literal}\nruns = pd.read_csv(io.StringIO(DATA), keep_default_na=False, na_values={{\"recorded_unix_seconds\": [\"\"]}},\n                   dtype={{\"source\": str, \"case\": str, \"unit\": str}})\nok = runs[runs[\"status\"] == \"ok\"].copy()\nprint(f\"{{len(runs)}} rows, {{len(ok)}} from successful runs, {{ok['source'].nunique()}} source(s)\")\nruns.head()")),
            markdown("summary-md", "## Summary per case and metric\n\nSuccessful measured runs only. `cv_%` is the coefficient of variation."),
            code("summary", "summary = ok.groupby([\"case\", \"kind\", \"metric\", \"unit\"])[\"value\"].agg([\"count\", \"median\", \"mean\", \"std\", \"min\", \"max\"])\nsummary[\"cv_%\"] = summary[\"std\"] / summary[\"mean\"] * 100\nsummary"),
            markdown("trend-md", "## Median per source\n\nSources are ordered by recording time when known (history runs), otherwise by name."),
            code("trend", "order = ok.groupby(\"source\")[\"recorded_unix_seconds\"].min().sort_values(na_position=\"last\").index\nmedians = ok.pivot_table(index=\"source\", columns=[\"case\", \"metric\"], values=\"value\", aggfunc=\"median\")\nmedians.reindex(order)"),
            markdown("plot-md", "## Wall time per case and source"),
            code("plot", "try:\n    import matplotlib.pyplot as plt\nexcept ImportError:\n    print(\"Install matplotlib for plots: pip install matplotlib\")\nelse:\n    wall = ok[(ok[\"kind\"] == \"bench\") & (ok[\"metric\"] == \"wall\")]\n    groups = [(f\"{c}\\n{s[-12:]}\", g[\"value\"].to_numpy()) for (c, s), g in wall.groupby([\"case\", \"source\"], sort=False)]\n    if groups:\n        fig, ax = plt.subplots(figsize=(max(6, len(groups) * 1.2), 4.5))\n        ax.boxplot([v for _, v in groups])\n        ax.set_xticks(range(1, len(groups) + 1), [label for label, _ in groups], fontsize=7, rotation=30, ha=\"right\")\n        ax.set_ylabel(\"wall time (s)\")\n        fig.tight_layout()\n        plt.show()"),
            markdown("validator-md", "## Validator metrics\n\nNumbers reported by your `--validate` program, per source."),
            code("validator", "checks = ok[ok[\"kind\"] == \"validator\"]\nchecks.pivot_table(index=\"source\", columns=[\"case\", \"metric\"], values=\"value\", aggfunc=[\"median\", \"min\", \"max\"]) if not checks.empty else \"No validator metrics in this data.\""),
        ]
    }))
}

pub(crate) fn run(options: Options) -> Result<i32, String> {
    let save_format = options.save_data.as_deref().map(format_for).transpose()?;
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    for path in &options.reports {
        let report = read_report(path)?;
        let source = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        rows.extend(tidy(&report, &source));
        sources.push(path.display().to_string());
    }
    if options.history {
        let root = crate::history::location(options.history_dir.as_deref())?;
        for (id, report) in crate::history::newest(&root, options.limit as usize)? {
            rows.extend(tidy(&report, &id));
            sources.push(format!("history {id}"));
        }
    }
    if options.reports.is_empty() && !options.history {
        return Err("give one or more JSON reports, or pass --history".into());
    }
    if rows.is_empty() {
        return Err("no measured runs found in the selected reports".into());
    }
    if let (Some(path), Some(format)) = (&options.save_data, save_format) {
        write_data(&rows, path, format, &options.interpreter)?;
    }
    if let Some(path) = &options.notebook {
        let notebook = notebook(&rows, &sources)?;
        crate::storage::write_json(path, &notebook)?;
        eprintln!(
            "bench: wrote {} ({} rows); open it with Jupyter, e.g. `jupyter lab {}`",
            path.display(),
            rows.len(),
            path.display()
        );
        return Ok(0);
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
        let book = notebook(&rows, &["h1".into()]).unwrap();
        assert_eq!(book["nbformat"], 4);
        let load = book["cells"][1]["source"].as_str().unwrap();
        // The embedded literal round-trips the exact CSV, including quotes and newlines.
        let literal = load
            .split("DATA = ")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(literal).unwrap(), csv);
        assert!(
            format_for(Path::new("x.parquet")).is_ok() && format_for(Path::new("x.txt")).is_err()
        );
        assert_eq!(render(&rows, "jsonl").lines().count(), 2);
    }
}
