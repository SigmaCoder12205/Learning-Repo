//! perf replay, parsing and format fixtures.
use super::{command_run, find_tool, invocation_failed, read_report};
use crate::cli::Cli;
use crate::runner::RunOptions;
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::Path;

pub(super) fn run(
    args: &Cli,
    target: &[OsString],
    opts: &RunOptions<'_>,
    dir: &Path,
) -> Result<Value, String> {
    let tool = find_tool("perf", opts.env)?;
    let path = dir.join("perf.csv");
    let mut command = vec![
        tool,
        "stat".into(),
        "--no-big-num".into(),
        "-x".into(),
        ";".into(),
        "-o".into(),
        path.clone().into_os_string(),
        "-e".into(),
        args.perf_events.clone().into(),
        "--".into(),
    ];
    command.extend_from_slice(target);
    let row = command_run(&command, opts, dir, "perf")?;
    if let Some(value) = invocation_failed(&row) {
        return Ok(value);
    }
    let mut result = parse_perf(&read_report(&path)?)?;
    result["invocation"] = row;
    result["raw_file"] = json!(path);
    result["events_requested"] = json!(args.perf_events);
    result["scope"] = json!("instrumented target replay; default events count user-space only; counts may be scaled when multiplexed");
    Ok(result)
}
fn parse_perf(text: &str) -> Result<Value, String> {
    let mut counters = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let fields: Vec<_> = line.split(';').map(str::trim).collect();
        if fields.len() < 3 || fields[2].is_empty() {
            continue;
        }
        let value = fields[0]
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.);
        let coverage = fields
            .get(4)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite());
        counters.push(
            json!({"event": fields[2], "value": value, "unit": fields[1],
            "status": if value.is_some() { "ok" } else { "unavailable" },
            "reason": if value.is_none() { Some(fields[0]) } else { None },
            "running_percent": coverage}),
        );
    }
    if counters.is_empty() {
        return Err("perf produced no counter records".into());
    }
    let sum = |name: &str| -> Option<f64> {
        let matching: Vec<_> = counters
            .iter()
            .filter(|c| {
                let event = c["event"].as_str().unwrap();
                event == name
                    || event.starts_with(&format!("{name}:"))
                    || event.contains(&format!("/{name}/"))
            })
            .collect();
        if matching.is_empty() || matching.iter().any(|c| c["value"].is_null()) {
            return None;
        }
        Some(matching.iter().map(|c| c["value"].as_f64().unwrap()).sum())
    };
    let ratio = |num: &str, den: &str, scale: f64| -> Option<f64> {
        let denominator = sum(den)?;
        if denominator <= 0. {
            return None;
        }
        Some(sum(num)? / denominator * scale).filter(|n| n.is_finite())
    };
    let available = counters.iter().filter(|c| c["status"] == "ok").count();
    Ok(
        json!({"status": if available == counters.len() { "ok" } else if available > 0 { "partial" } else { "unavailable" },
        "counters": counters, "instructions_per_cycle": ratio("instructions", "cycles", 1.),
        "cache_miss_percent": ratio("cache-misses", "cache-references", 100.),
        "branch_miss_percent": ratio("branch-misses", "branches", 100.)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn perf_handles_missing_events_and_zero_denominators() {
        let r = parse_perf("100;;cycles:u;12;75.0;;\n200;;instructions:u;12;75.0;;\n<not supported>;;cache-misses:u;0;0;;\n").unwrap();
        assert_eq!(r["status"], "partial");
        assert_eq!(r["instructions_per_cycle"], 2.);
        assert!(r["counters"][2]["value"].is_null());
        assert_eq!(r["counters"][0]["running_percent"], 75.);
        assert!(
            parse_perf("0;;cycles:u;1;100\n1;;instructions:u;1;100").unwrap()
                ["instructions_per_cycle"]
                .is_null()
        );
        assert!(parse_perf("garbage").is_err());
    }
}
