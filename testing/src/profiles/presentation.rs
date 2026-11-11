//! Human-readable domain sections used by text and HTML reports.
use serde_json::Value;
use std::fmt::Write;

pub fn render(report: &Value, detailed: bool) -> String {
    let profiles = &report["profiles"];
    let Some(sections) = profiles["sections"].as_array() else {
        return String::new();
    };
    let mut out = format!(
        "\nDomain profiles: {} (separate diagnostic measurements)\n",
        profiles["status"].as_str().unwrap_or("unavailable")
    );
    for section in sections {
        let _ = writeln!(out, "{}", section["title"].as_str().unwrap_or("Profile"));
        if let Some(context) = section["process_context"].as_object() {
            for (key, stats) in context {
                if let Some(mean) = stats["mean"].as_f64() {
                    let unit = crate::metrics::METRICS
                        .iter()
                        .find(|m| m.0 == key)
                        .map(|m| m.1)
                        .unwrap_or("");
                    let _ = writeln!(
                        out,
                        "  Process context {key}: {mean:.3} {unit} mean (baseline)"
                    );
                }
            }
        }
        for topic in section["topics"].as_array().into_iter().flatten() {
            let _ = writeln!(out, "  {}", topic["title"].as_str().unwrap_or("Topic"));
            for key in topic["metric_keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let m = &profiles["metrics"][key];
                let label = m["label"].as_str().unwrap_or(key);
                if m["status"] != "ok" {
                    let _ = writeln!(out, "    {label}: unavailable");
                    continue;
                }
                let unit = m["unit"].as_str().unwrap_or("");
                let source = m["source"].as_str().unwrap_or("unknown source");
                if let Some(value) = m["value"].as_f64() {
                    let digits = if ["count", "B"].contains(&unit) { 0 } else { 3 };
                    let _ = writeln!(out, "    {label}: {value:.digits$} {unit} ({source})");
                } else {
                    let s = &m["summary"];
                    let mean = s["mean"].as_f64().unwrap_or(0.);
                    let (scale, display_unit) = if unit == "ms" && mean < 0.001 {
                        (1_000_000., "ns")
                    } else if unit == "ms" && mean < 1. {
                        (1000., "µs")
                    } else {
                        (1., unit)
                    };
                    let _ = writeln!(out,"    {label}: mean {:.3}, p95 {:.3}, p99 {:.3}, max {:.3} {display_unit} (n={}; {source})",
                        mean*scale,s["p95"].as_f64().unwrap_or(0.)*scale,s["p99"].as_f64().unwrap_or(0.)*scale,s["max"].as_f64().unwrap_or(0.)*scale,s["count"]);
                }
                if let Some(fps) = m["fps_from_mean_frame_interval"].as_f64() {
                    let _ = writeln!(
                        out,
                        "      FPS from mean presented frame interval: {fps:.2}"
                    );
                }
                if m["over_budget_count"].is_number() {
                    let _ = writeln!(
                        out,
                        "      Samples over {} ms budget: {}",
                        m["budget_ms"], m["over_budget_count"]
                    );
                }
                if detailed {
                    let _ = writeln!(out, "      Metric key: {key}");
                    if let Some(rate) = m["rate_per_replay_second"].as_f64() {
                        let _ = writeln!(out,"      {rate:.3} {unit}/s over the entire instrumented replay (includes startup/tracing overhead)");
                    }
                }
            }
            let _ = writeln!(
                out,
                "    Note: {}",
                topic["guidance"].as_str().unwrap_or("")
            );
        }
    }
    if profiles["missing_metrics"]
        .as_array()
        .is_some_and(|m| !m.is_empty())
    {
        out.push_str("Unavailable application metrics require BENCH_TELEMETRY_FILE output. Use --list-profiles and docs/profiles.md for keys and units.\n");
    }
    out
}
