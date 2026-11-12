use crate::metrics::METRICS;
use clap::ValueEnum;
use serde_json::Value;
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum View {
    Simple,
    Intermediate,
    Advanced,
}

const SIMPLE: &[usize] = &[0, 5, 4, 14, 12, 13];
const INTERMEDIATE: &[usize] = &[0, 5, 4, 1, 2, 3, 14, 8, 9, 12, 13];
const LABELS: [&str; 15] = [
    "Time",
    "User CPU time",
    "System CPU time",
    "Total CPU time",
    "CPU usage",
    "Peak memory (RSS)",
    "Minor page faults",
    "Major page faults",
    "Filesystem reads",
    "Filesystem writes",
    "Voluntary switches",
    "Involuntary switches",
    "Standard output",
    "Error output",
    "Throughput",
];

fn duration(seconds: f64) -> String {
    if seconds >= 60.0 {
        format!("{:.2} min", seconds / 60.0)
    } else if seconds >= 1.0 {
        format!("{seconds:.3} s")
    } else if seconds >= 0.001 {
        format!("{:.3} ms", seconds * 1000.0)
    } else {
        format!("{:.2} µs", seconds * 1_000_000.0)
    }
}
fn bytes(value: f64) -> String {
    if value >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GiB", value / (1024.0 * 1024.0 * 1024.0))
    } else if value >= 1024.0 * 1024.0 {
        format!("{:.2} MiB", value / (1024.0 * 1024.0))
    } else if value >= 1024.0 {
        format!("{:.2} KiB", value / 1024.0)
    } else {
        format!("{value:.0} B")
    }
}
fn readable(index: usize, value: f64) -> String {
    match index {
        0..=3 => duration(value),
        4 => format!("{value:.1}%"),
        5 => bytes(value * 1024.0),
        12 | 13 => bytes(value),
        14 => format!("{value:.2} units/s"),
        _ => format!("{value:.2}"),
    }
}

pub fn render(
    report: &Value,
    format: &str,
    selected: &[usize],
    view: View,
    custom_metrics: bool,
) -> String {
    // View selection is presentation only: exports keep their existing metric selection.
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
    let visible = if custom_metrics {
        selected
    } else {
        match view {
            View::Simple => SIMPLE,
            View::Intermediate => INTERMEDIATE,
            View::Advanced => selected,
        }
    };
    let _ = writeln!(
        out,
        "bench {} | {}/{} measured runs passed ({} requested)",
        env!("CARGO_PKG_VERSION"),
        report["successful_runs"],
        runs.len(),
        report["settings"]["runs"]
    );
    if report["python"].is_object() {
        let _ = writeln!(
            out,
            "Target: {} (Python)\nInterpreter: {}",
            report["python"]["script"], report["python"]["interpreter"]
        );
        out.push_str("Python measurements include interpreter startup, imports, script execution, and shutdown.\n");
    } else {
        let _ = writeln!(out, "Target: {}", report["command"][0]);
    }
    if report["completed"] == false {
        out.push_str("Partial experiment: only completed invocations are included.\n");
    }
    let good = report["successful_runs"].as_u64().unwrap_or(0);
    if good == 0 {
        out.push_str("No successful measurements. Resource summaries are unavailable.\n");
        if runs.is_empty()
            && report["warmups"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["status"] != "ok")
        {
            out.push_str("Measured runs did not start because a warmup failed.\n");
        }
    } else if view == View::Simple {
        out.push_str("\nOverview (successful runs)\n");
        for &i in visible {
            let stats = &report["summary"][METRICS[i].0];
            if stats.is_null() {
                continue;
            }
            let (key, explanation) = match i {
                0 => ("mean", "average per run"),
                5 => ("max", "highest peak across runs"),
                4 => ("mean", "average; 100% = one CPU core"),
                _ => ("mean", "average per run"),
            };
            let _ = writeln!(
                out,
                "  {:<18} {} ({explanation})",
                LABELS[i],
                readable(i, stats[key].as_f64().unwrap())
            );
        }
    } else if view == View::Intermediate {
        out.push_str("\nResource summary (successful runs)\n");
        let _ = writeln!(
            out,
            "{:<25} {:>17} {:>17} {:>17}",
            "Metric", "Average", "Lowest", "Highest"
        );
        for &i in visible {
            let stats = &report["summary"][METRICS[i].0];
            if stats.is_null() {
                continue;
            }
            let _ = write!(out, "{:<25}", LABELS[i]);
            for key in ["mean", "min", "max"] {
                let _ = write!(out, " {:>17}", readable(i, stats[key].as_f64().unwrap()));
            }
            out.push('\n');
        }
        out.push_str("Memory values are per-run RSS peaks. CPU 100% = one core; filesystem I/O is counted in operations.\n");
    }
    if good > 0 && visible.contains(&5) {
        out.push_str("Memory: peak resident process memory (RSS), including runtime libraries and stack; not heap allocation or executable section size.\n");
    }
    if !custom_metrics {
        if let Some(engines) = report["gpu"]["target"]["engines_percent"]
            .as_object()
            .filter(|e| !e.is_empty())
        {
            let values: Vec<_> = engines
                .iter()
                .map(|(name, stats)| {
                    format!("{name}: {:.1}%", stats["mean"].as_f64().unwrap_or(0.))
                })
                .collect();
            let _ = writeln!(
                out,
                "  Target GPU         {} (PID {}; sampled)",
                values.join(", "),
                report["gpu"]["target"]["pid"]
            );
        }
        if let Some(engines) = report["gpu"]["engines_percent"].as_object() {
            out.push_str("  GPU engines        ");
            let values: Vec<_> = engines
                .iter()
                .map(|(name, stats)| {
                    format!("{name}: {:.1}%", stats["mean"].as_f64().unwrap_or(0.))
                })
                .collect();
            out.push_str(&values.join(", "));
            out.push_str(" (device-wide; separate diagnostic session)\n");
        } else {
            let _ = writeln!(
                out,
                "  GPU usage          Unavailable — {}",
                report["gpu"]["reason"]
                    .as_str()
                    .unwrap_or("not collected; use --gpu")
            );
        }
    }
    if view != View::Advanced && visible.contains(&0) && good > 0 {
        if good > 1 {
            let _ = writeln!(
                out,
                "  Time variation     {:.1}% (coefficient of variation; lower is steadier)",
                report["summary"]["wall"]["cv_percent"].as_f64().unwrap()
            );
        } else {
            out.push_str("  Time variation     Unavailable — only one successful run\n");
        }
    }
    if let Some(wall) = report["compilation"]["wall_seconds"].as_f64() {
        let _ = writeln!(out, "Compilation: {} (excluded from runs)", duration(wall));
    }
    if view != View::Simple && visible.contains(&0) {
        if let (Some(low), Some(high)) = (
            report["summary"]["wall"]["mean_ci95"]["low"].as_f64(),
            report["summary"]["wall"]["mean_ci95"]["high"].as_f64(),
        ) {
            let _ = writeln!(
                out,
                "Mean time bootstrap interval (95%, descriptive): {} to {}; outliers retained: {}",
                duration(low),
                duration(high),
                report["summary"]["wall"]["outlier_count"]
            );
        }
        if let Some(reason) = report["stop_reason"].as_str() {
            let _ = writeln!(out, "Stopping reason: {reason}");
        }
    }
    if view == View::Advanced {
        let _ = writeln!(out, "\nCommand: {}\nHost: {} {} | logical CPUs available: {}\nSettings: {}\nCompleted warmups: {}",
            report["command"], report["host"]["system"], report["host"]["machine"], report["host"]["logical_cpus"],
            report["settings"], report["warmups"].as_array().unwrap().len());
        out.push_str("\nStatistics (successful measured runs; original metric units)\n");
        if report["provenance"].is_object() {
            let _ = writeln!(out, "Provenance: {}", report["provenance"]);
        }
        let _ = writeln!(
            out,
            "{:<32} {:>6} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>9}",
            "Metric (unit)", "N", "Mean", "Median", "Min", "Max", "Stddev", "P90", "P95", "CV %"
        );
        for &i in visible {
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
            let _ = writeln!(
                out,
                "\nIndividual {phase} (failed values may reflect incomplete work)"
            );
            for row in report[phase].as_array().unwrap() {
                let _ = write!(
                    out,
                    "#{} {} (exit {})",
                    row["run"],
                    row["status"].as_str().unwrap(),
                    row["exit_code"]
                );
                for &i in visible {
                    let _ = write!(
                        out,
                        " | {}={} {}",
                        METRICS[i].0, row["metrics"][METRICS[i].0], METRICS[i].1
                    );
                }
                out.push('\n');
            }
        }
    }
    for phase in ["warmups", "runs"] {
        for row in report[phase].as_array().unwrap() {
            if row["status"] != "ok" {
                let _ = writeln!(
                    out,
                    "{} #{}: {} (exit {})",
                    phase, row["run"], row["status"], row["exit_code"]
                );
                if row["termination"]["kind"] == "signal" {
                    let _ = writeln!(
                        out,
                        "Terminated by {} (signal {}).",
                        row["termination"]["name"]
                            .as_str()
                            .unwrap_or("an unnamed signal"),
                        row["termination"]["signal"]
                    );
                }
                if row["status"] == "output_limit" {
                    if let (Some(stdout), Some(stderr), Some(limit)) = (
                        row["captured_output"]["stdout_bytes"].as_f64(),
                        row["captured_output"]["stderr_bytes"].as_f64(),
                        report["settings"]["output_limit_mib"].as_f64(),
                    ) {
                        let _ = writeln!(out, "Output limit exceeded: {stdout:.0} B stdout + {stderr:.0} B stderr (combined limit {limit} MiB).");
                    }
                }
                if row["stdout_preview"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
                {
                    let _ = writeln!(
                        out,
                        "stdout (escaped, up to 512 bytes): {}{}",
                        row["stdout_preview"],
                        if row["stdout_preview_truncated"] == true {
                            " [truncated]"
                        } else {
                            ""
                        }
                    );
                }
                if row["stderr_preview"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty())
                {
                    let _ = writeln!(out, "stderr (escaped): {}", row["stderr_preview"]);
                }
            }
        }
    }
    if ["warmups", "runs"].iter().any(|phase| {
        report[*phase]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["status"] == "output_limit")
    }) {
        if report["settings"]["input"].is_null() {
            out.push_str("Input: stdin is /dev/null (immediate EOF). For a program that needs input, pass --input FILE; the file is replayed for every invocation.\n");
        } else {
            out.push_str("Input: the replay file eventually reaches EOF; include the program's exit command if it needs one.\n");
        }
        out.push_str("Repeated prompts can indicate an input loop that ignores EOF. Check failed input reads in the target program.\n");
    }
    for warning in report["warnings"].as_array().unwrap() {
        let _ = writeln!(out, "Note: {}", warning.as_str().unwrap());
    }
    out.push_str(&crate::insights::render(report));
    render_diagnostics(&mut out, report, view);
    out.push_str(&crate::profiles::render(report, view != View::Simple));
    if report["comparison"].is_object() {
        let _ = writeln!(
            out,
            "\nBaseline comparison: {}",
            report["comparison"]["status"].as_str().unwrap_or("unknown")
        );
        if let Some(changes) = report["comparison"]["changes"].as_array() {
            for change in changes {
                let percent = change["change_percent"]
                    .as_f64()
                    .map(|n| format!("{n:+.2}%"))
                    .unwrap_or_else(|| "undefined (zero baseline)".into());
                let _ = writeln!(
                    out,
                    "  {}: {} -> {} ({percent}); {}{}",
                    change["metric"].as_str().unwrap_or(""),
                    change["baseline"],
                    change["current"],
                    change["statistic"].as_str().unwrap_or(""),
                    if change["regression"] == true {
                        "; REGRESSION"
                    } else {
                        ""
                    }
                );
            }
        }
        for key in ["differences", "warnings"] {
            if let Some(messages) = report["comparison"][key].as_array() {
                for message in messages {
                    let _ = writeln!(out, "  Note: {}", message);
                }
            }
        }
    }
    if view == View::Simple {
        out.push_str("More detail: --intermediate (resource ranges) or --advanced (full statistics and runs).\n");
    }
    out
}

fn render_diagnostics(out: &mut String, report: &Value, view: View) {
    let Some(results) = report["diagnostics"]["results"]
        .as_object()
        .filter(|r| !r.is_empty())
    else {
        return;
    };
    out.push_str("\nDiagnostics (separate replays; excluded from baseline statistics)\n");
    for (name, result) in results {
        let _ = writeln!(
            out,
            "  {name}: {}",
            result["status"].as_str().unwrap_or("unavailable")
        );
        if let Some(reason) = result["reason"].as_str() {
            let _ = writeln!(out, "    {reason:?}");
        }
        match name.as_str() {
            "hotspots" => {
                if let Some(functions) = result["functions"].as_array() {
                    for function in functions
                        .iter()
                        .take(if view == View::Simple { 3 } else { 10 })
                    {
                        let _ = writeln!(
                            out,
                            "    {:.1}% self samples: {}{}",
                            function["self_percent"].as_f64().unwrap_or(0.),
                            function["function"].as_str().unwrap_or(""),
                            function["source"]
                                .as_str()
                                .map(|s| format!(" — {s}"))
                                .unwrap_or_default()
                        );
                    }
                }
                if let Some(path) = result["flamegraph_file"].as_str() {
                    let _ = writeln!(out, "    Flamegraph: {path:?}");
                }
            }
            "perf" => {
                for (key, label, unit) in [
                    ("instructions_per_cycle", "Instructions/cycle", ""),
                    ("cache_miss_percent", "Cache miss rate", "%"),
                    ("branch_miss_percent", "Branch miss rate", "%"),
                ] {
                    if let Some(value) = result[key].as_f64() {
                        let _ = writeln!(out, "    {label}: {value:.3}{unit}");
                    }
                }
                if view != View::Simple {
                    if let Some(counters) = result["counters"].as_array() {
                        for counter in counters {
                            let _ = writeln!(
                                out,
                                "    {}: {} {} (running: {}%)",
                                counter["event"].as_str().unwrap_or(""),
                                counter["value"],
                                counter["unit"].as_str().unwrap_or(""),
                                counter["running_percent"]
                            );
                        }
                    }
                }
            }
            "heaptrack" => {
                if let Some(calls) = result["allocation_calls"].as_u64() {
                    let _ = writeln!(
                        out,
                        "    Allocation calls: {calls}; temporary: {}",
                        result["temporary_allocations"]
                    );
                }
                for (key, label) in [
                    ("peak_heap_bytes_approx", "Peak heap (approx.)"),
                    ("retained_bytes_approx", "Retained at exit (approx.)"),
                ] {
                    if let Some(value) = result[key].as_f64() {
                        let _ = writeln!(out, "    {label}: {}", bytes(value));
                    }
                }
            }
            "valgrind" => {
                if !result["actionable_contexts"].is_null() {
                    let _ = writeln!(
                        out,
                        "    Actionable error/leak contexts: {}",
                        result["actionable_contexts"]
                    );
                }
                if let Some(kinds) = result["leaked_bytes_by_kind"].as_object() {
                    for (kind, value) in kinds {
                        let _ =
                            writeln!(out, "    {kind}: {}", bytes(value.as_f64().unwrap_or(0.)));
                    }
                }
                if view == View::Advanced {
                    if let Some(errors) = result["errors"].as_array() {
                        for error in errors {
                            let _ = writeln!(out, "    {}", error);
                        }
                    }
                }
            }
            "gpu" => {
                if let Some(target) = result["target"].as_object() {
                    if let Some(reason) = target.get("reason").and_then(Value::as_str) {
                        let _ = writeln!(out, "    Target attribution: {reason}");
                    }
                    if let Some(bytes_value) = target
                        .get("peak_drm_resident_bytes")
                        .and_then(Value::as_f64)
                    {
                        let _=writeln!(out,"    Target DRM resident memory peak: {} (may include shared buffers; not dedicated VRAM)",bytes(bytes_value));
                    }
                }
                if let Some(engines) = result["engines_percent"].as_object() {
                    for (name, stats) in engines {
                        let _ = writeln!(
                            out,
                            "    {name}: {:.1}% mean, {:.1}% peak (device-wide)",
                            stats["mean"].as_f64().unwrap_or(0.),
                            stats["max"].as_f64().unwrap_or(0.)
                        );
                    }
                }
            }
            _ => {}
        }
        if view == View::Advanced {
            if let Some(scope) = result["scope"].as_str() {
                let _ = writeln!(out, "    Scope: {scope}");
            }
            if let Some(path) = result["raw_file"].as_str() {
                let _ = writeln!(out, "    Raw artifact: {path:?}");
            }
            if let Some(reason) = result["invocation"]["stderr_preview"]
                .as_str()
                .filter(|s| !s.is_empty())
            {
                let _ = writeln!(out, "    Tool stderr: {reason:?}");
            }
        }
    }
    if let Some(dir) = report["diagnostics"]["artifact_directory"].as_str() {
        let _ = writeln!(out, "Artifacts: {dir:?} (including diagnostics.json)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn overview_uses_average_time_and_highest_peak_memory() {
        let report = json!({"command": ["./program"], "successful_runs": 2, "settings": {"runs": 2},
            "runs": [{"status": "ok"}, {"status": "ok"}], "warmups": [], "warnings": [],
            "summary": {"wall": crate::stats::summarize(vec![0.1, 0.3]), "rss": crate::stats::summarize(vec![1024., 3072.])}});
        let text = render(&report, "text", &[0, 5], View::Simple, true);
        assert!(text.contains("200.000 ms (average per run)"), "{text}");
        assert!(
            text.contains("3.00 MiB (highest peak across runs)"),
            "{text}"
        );
        assert!(!text.contains("GPU"));
        assert_eq!(duration(0.000025), "25.00 µs");
        assert_eq!(duration(120.), "2.00 min");
        assert_eq!(bytes(2. * 1024. * 1024. * 1024.), "2.00 GiB");
    }
}
