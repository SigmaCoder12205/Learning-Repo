//! gpu replay, parsing and format fixtures.
use super::{command_run, find_tool, invocation_failed, issue, read_report};
use crate::cli::Cli;
use crate::runner::RunOptions;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

const GPU_SUPERVISOR: &str = r#"
monitor=$1; interval=$2; raw=$3; errors=$4; device=$5; shift 5
if [ -n "$device" ]; then
    "$monitor" -J -s "$interval" -d "$device" -o - > "$raw" 2> "$errors" &
else
    "$monitor" -J -s "$interval" -o - > "$raw" 2> "$errors" &
fi
monitor_pid=$!
sleep 0.1
if ! kill -0 "$monitor_pid" 2>/dev/null; then
    wait "$monitor_pid"
    printf '%s\n' "$?" > "$errors.exit"
    exit 125
fi
"$@" <&0 &
target_pid=$!
printf '%s\n' "$target_pid" > "$errors.target-pid"
wait "$target_pid"
result=$?
kill -INT "$monitor_pid" 2>/dev/null
wait "$monitor_pid"
printf '%s\n' "$?" > "$errors.exit"
exit "$result"
"#;
pub(super) fn run(
    args: &Cli,
    target: &[OsString],
    opts: &RunOptions<'_>,
    dir: &Path,
) -> Result<Value, String> {
    let tool = find_tool("intel_gpu_top", opts.env)?;
    let raw = dir.join("gpu.json");
    let errors = dir.join("gpu-monitor.stderr");
    let mut command = vec![
        "/bin/sh".into(),
        "-c".into(),
        GPU_SUPERVISOR.into(),
        "cppbench-gpu".into(),
        tool,
        args.gpu_interval_ms.to_string().into(),
        raw.clone().into_os_string(),
        errors.clone().into_os_string(),
        args.gpu_device.clone().unwrap_or_default().into(),
    ];
    command.extend_from_slice(target);
    let row = command_run(&command, opts, dir, "gpu")?;
    let diagnostic = read_report(&errors).unwrap_or_default();
    if let Some(mut value) = invocation_failed(&row) {
        if row["status"] != "interrupted"
            && (diagnostic.contains("Permission denied")
                || diagnostic.contains("Operation not permitted"))
        {
            value["status"] = json!("unavailable");
            value["reason"] = json!("intel_gpu_top cannot access GPU counters: CAP_PERFMON or appropriate system permissions are required");
        }
        value["monitor_diagnostics"] = json!(diagnostic);
        value["raw_file"] = json!(raw);
        return Ok(value);
    }
    let monitor_exit = read_report(&dir.join("gpu-monitor.stderr.exit"))?
        .trim()
        .parse::<i32>()
        .map_err(|_| "invalid GPU monitor exit status")?;
    if monitor_exit != 0 && monitor_exit != 130 {
        let mut value = issue(
            "unavailable",
            format!("intel_gpu_top exited with {monitor_exit}; see gpu-monitor.stderr"),
        );
        value["invocation"] = row;
        value["monitor_diagnostics"] = json!(diagnostic);
        value["monitor_exit_code"] = json!(monitor_exit);
        return Ok(value);
    }
    // Keep the invocation and artifact paths even when the monitor's JSON is invalid.
    let mut result = read_report(&raw)
        .and_then(|text| parse_gpu(&text))
        .unwrap_or_else(|reason| issue("unavailable", reason));
    if let Ok(pid) = read_report(&dir.join("gpu-monitor.stderr.target-pid"))
        .and_then(|s| s.trim().parse::<u32>().map_err(|e| e.to_string()))
    {
        result["target"] = read_report(&raw)
            .and_then(|s| parse_gpu_target(&s, pid))
            .unwrap_or_else(|reason| issue("unavailable", reason));
    }
    result["invocation"] = row;
    result["raw_file"] = json!(raw);
    result["monitor_diagnostics"] = json!(diagnostic);
    result["monitor_exit_code"] = json!(monitor_exit);
    result["interval_ms"] = json!(args.gpu_interval_ms);
    result["device_selector"] = json!(args.gpu_device);
    result["scope"] = json!("device-wide engine activity during a separate diagnostic session, including monitor startup/shutdown; other GPU clients contribute; not target-only GPU usage");
    Ok(result)
}
fn parse_gpu(text: &str) -> Result<Value, String> {
    let samples = gpu_samples(text)?;
    let mut engines = BTreeMap::<String, Vec<f64>>::new();
    let mut sample_count = 0;
    // The initial sample is a baseline without a complete sampling interval.
    for sample in samples.iter().skip(1) {
        let mut usable = false;
        if let Some(values) = sample["engines"].as_object() {
            for (name, value) in values {
                if let Some(busy) = value["busy"]
                    .as_f64()
                    .filter(|n| n.is_finite() && *n >= 0. && *n <= 100.)
                {
                    engines.entry(name.clone()).or_default().push(busy);
                    usable = true;
                }
            }
        }
        if usable {
            sample_count += 1;
        }
    }
    if engines.is_empty() {
        let reason = if samples.len() <= 1 {
            "GPU sampling window too short: no complete interval after the initial sample; increase target work or try --gpu-interval-ms 10"
        } else {
            "GPU samples contain no supported engine activity values"
        };
        let mut result = issue("unavailable", reason);
        result["reason_code"] = json!(if samples.len() <= 1 {
            "insufficient_samples"
        } else {
            "unsupported_samples"
        });
        result["captured_sample_count"] = json!(samples.len());
        result["sample_count"] = json!(0);
        result["usage_percent"] = Value::Null;
        return Ok(result);
    }
    let summary: BTreeMap<_, _> = engines
        .into_iter()
        .map(|(name, values)| (name, crate::stats::summarize(values)))
        .collect();
    Ok(
        json!({"status": "ok", "sample_count": sample_count, "captured_sample_count": samples.len(),
        "engines_percent": summary, "usage_percent": null,
        "aggregation": "per-engine averages; no misleading sum or single overall GPU percentage"}),
    )
}

fn gpu_samples(text: &str) -> Result<Vec<Value>, String> {
    let trimmed = text.trim().trim_end_matches(',').trim_end();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let samples = if trimmed.starts_with('[') {
        // A monitor stopped on a sample boundary can omit the closing bracket.
        let parsed: Value = serde_json::from_str(trimmed)
            .or_else(|_| serde_json::from_str(&format!("{trimmed}]")))
            .map_err(|e| format!("invalid/incomplete intel_gpu_top JSON: {e}"))?;
        parsed
            .as_array()
            .ok_or("GPU output is not an array")?
            .clone()
    } else {
        // Some releases send the array delimiters to stdout even with -o FILE.
        // Accept single objects, comma-separated objects, and JSON object streams.
        let mut remaining = trimmed;
        let mut samples = Vec::new();
        while !remaining.is_empty() {
            let mut stream = serde_json::Deserializer::from_str(remaining).into_iter::<Value>();
            let sample = stream
                .next()
                .ok_or("missing GPU sample")?
                .map_err(|e| format!("invalid/incomplete intel_gpu_top JSON: {e}"))?;
            samples.push(sample);
            remaining = remaining[stream.byte_offset()..].trim_start();
            if let Some(rest) = remaining.strip_prefix(',') {
                remaining = rest.trim_start();
            }
        }
        samples
    };
    if samples.iter().any(|v| !v.is_object()) {
        return Err("intel_gpu_top samples must be JSON objects".into());
    }
    Ok(samples)
}

fn parse_gpu_target(text: &str, pid: u32) -> Result<Value, String> {
    let samples = gpu_samples(text)?;
    let mut engines = BTreeMap::<String, Vec<f64>>::new();
    let mut resident = Vec::new();
    let mut observed = 0;
    let numeric = |v: &Value| {
        v.as_f64()
            .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
            .filter(|v| v.is_finite() && *v >= 0.)
    };
    for sample in samples.iter().skip(1) {
        let Some(clients) = sample["clients"].as_object() else {
            continue;
        };
        let mut totals = BTreeMap::<String, f64>::new();
        let mut memory = 0.;
        let mut memory_seen = false;
        let mut matched = false;
        for client in clients.values() {
            let client_pid = client["pid"]
                .as_u64()
                .or_else(|| client["pid"].as_str().and_then(|s| s.parse().ok()));
            if client_pid != Some(pid as u64) {
                continue;
            }
            matched = true;
            if let Some(classes) = client["engine-classes"].as_object() {
                for (name, value) in classes {
                    if let Some(value) = numeric(&value["busy"]) {
                        *totals.entry(name.clone()).or_default() += value;
                    }
                }
            }
            if let Some(regions) = client["memory"].as_object() {
                for region in regions.values() {
                    if let Some(value) = numeric(&region["resident"]) {
                        memory += value;
                        memory_seen = true;
                    }
                }
            }
        }
        if matched {
            observed += 1;
        }
        if memory_seen {
            resident.push(memory);
        }
        for (name, value) in totals {
            engines.entry(name).or_default().push(value);
        }
    }
    if observed == 0 {
        return Ok(
            json!({"status":"unavailable","pid":pid,"reason":"target DRM client not observed; it may do no GPU work, finish too quickly, or use a child process"}),
        );
    }
    let summaries: BTreeMap<_, _> = engines
        .into_iter()
        .map(|(name, values)| (name, crate::stats::summarize(values)))
        .collect();
    Ok(
        json!({"status":if summaries.is_empty()&&resident.is_empty(){"unavailable"}else{"ok"},"pid":pid,"observed_samples":observed,
        "engines_percent":summaries,"peak_drm_resident_bytes":resident.into_iter().reduce(f64::max),
        "scope":"exact target PID only; child processes excluded; sums matching DRM clients and memory regions; shared buffers may be counted more than once; resident bytes are not dedicated VRAM"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_client_attribution_excludes_other_pids_and_baseline() {
        let text = include_str!("../../tests/fixtures/intel_gpu_clients.json");
        let target = parse_gpu_target(text, 42).unwrap();
        assert_eq!(target["status"], "ok");
        assert_eq!(target["observed_samples"], 1);
        assert_eq!(target["engines_percent"]["Render/3D"]["mean"], 30.0);
        assert_eq!(target["peak_drm_resident_bytes"], 3072.0);
        let absent = parse_gpu_target(text, 100).unwrap();
        assert_eq!(absent["status"], "unavailable");
        assert!(absent["engines_percent"].is_null());
    }
    #[test]
    fn gpu_preserves_engine_scope_and_unavailable_values() {
        let text = r#"[{"engines":{"Render/3D":{"busy":99}}},{"engines":{"Render/3D":{"busy":20},"Video":{"busy":"-"}}},{"engines":{"Render/3D":{"busy":40}}}"#;
        let r = parse_gpu(text).unwrap();
        assert_eq!(r["engines_percent"]["Render/3D"]["mean"], 30.);
        assert!(r["engines_percent"]["Video"].is_null());
        assert!(r["usage_percent"].is_null());
        assert_eq!(parse_gpu("[]").unwrap()["status"], "unavailable");
        assert!(parse_gpu("[{broken").is_err());
    }

    #[test]
    fn gpu_accepts_arrays_file_fragments_and_object_streams() {
        let objects = [
            r#"{"engines":{"Render/3D":{"busy":99}}}"#,
            r#"{"engines":{"Render/3D":{"busy":20}}}"#,
            r#"{"engines":{"Render/3D":{"busy":40}}}"#,
        ];
        for text in [
            format!("[{}]", objects.join(",")),
            format!("[{},", objects.join(",")),
            objects.join(",\n"),
            objects.join("\n"),
            objects.join(""),
        ] {
            let result = parse_gpu(&text).unwrap();
            assert_eq!(result["status"], "ok", "{text}");
            assert_eq!(result["engines_percent"]["Render/3D"]["mean"], 30.);
            assert_eq!(result["sample_count"], 2);
            assert_eq!(result["captured_sample_count"], 3);
        }
    }

    #[test]
    fn gpu_single_sample_is_insufficient_not_a_format_error() {
        let text = include_str!("../../tests/fixtures/intel_gpu_top_short.json");
        let result = parse_gpu(text).unwrap();
        assert_eq!(result["status"], "unavailable");
        assert_eq!(result["reason_code"], "insufficient_samples");
        assert_eq!(result["sample_count"], 0);
        assert_eq!(result["captured_sample_count"], 1);
        assert!(result["usage_percent"].is_null());
        assert!(result["engines_percent"].is_null());
    }

    #[test]
    fn gpu_rejects_malformed_samples_instead_of_reporting_partial_success() {
        for text in [
            "{} , {broken",
            "[{}, {broken",
            "[1,2]",
            "null",
            "\"hello\"",
            "{} , , {}",
        ] {
            assert!(parse_gpu(text).is_err(), "{text}");
        }
    }
}
