//! Optional tools run in separate process groups after the uninstrumented baseline.
use crate::{
    runner::{self, RunOptions},
    Cli,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::Read,
    os::unix::fs::PermissionsExt,
    path::Path,
};

const PARSE_LIMIT: u64 = 16 * 1024 * 1024;

pub(crate) fn read_report(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > PARSE_LIMIT {
        return Err(format!(
            "{} exceeds the 16 MiB parsing limit; raw artifact retained",
            path.display()
        ));
    }
    let mut text = String::new();
    file.take(PARSE_LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > PARSE_LIMIT {
        return Err("report grew beyond parsing limit".into());
    }
    Ok(text)
}
pub(crate) fn find_tool(
    name: &str,
    env: &BTreeMap<OsString, OsString>,
) -> Result<OsString, String> {
    if let Some(path) = env.get(OsStr::new("PATH")) {
        for dir in std::env::split_paths(path) {
            let candidate = dir.join(name);
            if let Ok(meta) = candidate.metadata() {
                if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                    return candidate
                        .canonicalize()
                        .map(|p| p.into_os_string())
                        .map_err(|e| e.to_string());
                }
            }
        }
    }
    Err(format!("{name} is not installed or not executable on PATH"))
}
fn issue(status: &str, reason: impl AsRef<str>) -> Value {
    json!({"status": status, "reason": reason.as_ref()})
}
pub(crate) fn command_run(
    command: &[OsString],
    opts: &RunOptions<'_>,
    dir: &Path,
    label: &str,
) -> Result<Value, String> {
    let stdout = dir.join(format!("{label}.stdout"));
    let stderr = dir.join(format!("{label}.stderr"));
    let mut row = runner::run_logged(command, opts, &[0], Some((&stdout, &stderr)))?;
    row["command"] = json!(crate::display_command(command));
    row["stdout_file"] = json!(stdout);
    row["stderr_file"] = json!(stderr);
    Ok(row)
}
pub(crate) fn invocation_failed(row: &Value) -> Option<Value> {
    if row["status"] == "ok" {
        return None;
    }
    let status = if row["status"] == "interrupted" {
        "interrupted"
    } else {
        "failed"
    };
    let mut value = issue(
        status,
        format!(
            "diagnostic invocation: {} (exit {}); see retained stderr or --advanced",
            row["status"].as_str().unwrap_or("failed"),
            row["exit_code"]
        ),
    );
    value["invocation"] = row.clone();
    value["interrupt_signal"] = row["interrupt_signal"].clone();
    Some(value)
}

pub fn execute(
    args: &Cli,
    target: &[OsString],
    base: &RunOptions<'_>,
    allowed: bool,
) -> Result<Value, String> {
    let requested: Vec<&str> = [
        ("perf", args.perf),
        ("gpu", args.gpu),
        ("heaptrack", args.heaptrack),
        ("valgrind", args.valgrind),
        ("hotspots", args.hotspots),
    ]
    .into_iter()
    .filter(|(_, enabled)| *enabled || args.all_tools)
    .map(|(name, _)| name)
    .collect();
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
        eprintln!("cppbench: running {name} diagnostic (separate from baseline)");
        let result = match *name {
            "perf" => run_perf(args, target, &opts, &dir),
            "gpu" => run_gpu(args, target, &opts, &dir),
            "heaptrack" => run_heaptrack(target, &opts, &dir),
            "valgrind" => run_valgrind(target, &opts, &dir),
            "hotspots" => crate::hotspots::run(target, &opts, &dir),
            _ => unreachable!(),
        };
        let value = result.unwrap_or_else(|e| issue("unavailable", e));
        signal = value["interrupt_signal"].as_i64().unwrap_or(0);
        results.insert((*name).into(), value);
    }
    let failed = results.values().any(|v| v["status"] != "ok");
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

fn run_perf(
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

fn run_valgrind(target: &[OsString], opts: &RunOptions<'_>, dir: &Path) -> Result<Value, String> {
    let tool = find_tool("valgrind", opts.env)?;
    let path = dir.join("valgrind.xml");
    let mut command = vec![
        tool,
        "--tool=memcheck".into(),
        "--xml=yes".into(),
        format!(
            "--xml-file={}",
            path.display().to_string().replace('%', "%%")
        )
        .into(),
        "--leak-check=full".into(),
        "--show-leak-kinds=all".into(),
        "--track-origins=yes".into(),
        "--trace-children=no".into(),
        "--child-silent-after-fork=yes".into(),
        "--".into(),
    ];
    command.extend_from_slice(target);
    let row = command_run(&command, opts, dir, "valgrind")?;
    if let Some(value) = invocation_failed(&row) {
        return Ok(value);
    }
    let mut result = parse_valgrind(&read_report(&path)?)?;
    result["invocation"] = row;
    result["raw_file"] = json!(path);
    result["scope"] = json!("Memcheck replay; exec children are not traced; still-reachable memory is reported but not treated as an error");
    Ok(result)
}
fn parse_valgrind(text: &str) -> Result<Value, String> {
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| format!("invalid/incomplete Valgrind XML: {e}"))?;
    if doc.root_element().tag_name().name() != "valgrindoutput"
        || !doc
            .descendants()
            .any(|n| n.has_tag_name("state") && n.text() == Some("FINISHED"))
    {
        return Err("Valgrind did not finish a valid Memcheck report".into());
    }
    let child_text = |node: roxmltree::Node<'_, '_>, tag: &str| -> String {
        node.children()
            .find(|n| n.has_tag_name(tag))
            .and_then(|n| n.text())
            .unwrap_or("")
            .into()
    };
    let mut counts = BTreeMap::new();
    for pair in doc.descendants().filter(|n| {
        n.has_tag_name("pair") && n.parent().is_some_and(|p| p.has_tag_name("errorcounts"))
    }) {
        counts.insert(
            child_text(pair, "unique"),
            child_text(pair, "count").parse::<u64>().unwrap_or(1),
        );
    }
    let mut kinds = BTreeMap::<String, u64>::new();
    let mut leaks = BTreeMap::<String, u64>::new();
    let mut errors = Vec::new();
    let mut actionable = 0_u64;
    let mut contexts = 0_u64;
    for node in doc.descendants().filter(|n| n.has_tag_name("error")) {
        let kind = child_text(node, "kind");
        let count = *counts.get(&child_text(node, "unique")).unwrap_or(&1);
        *kinds.entry(kind.clone()).or_default() += count;
        contexts += 1;
        if kind != "Leak_StillReachable" {
            actionable += 1;
        }
        if let Some(bytes) = node
            .descendants()
            .find(|n| n.has_tag_name("leakedbytes"))
            .and_then(|n| n.text())
            .and_then(|s| s.parse::<u64>().ok())
        {
            *leaks.entry(kind.clone()).or_default() += bytes;
        }
        if errors.len() < 20 {
            let description = node
                .children()
                .find(|n| n.has_tag_name("what"))
                .and_then(|n| n.text())
                .or_else(|| {
                    node.descendants()
                        .find(|n| n.has_tag_name("text"))
                        .and_then(|n| n.text())
                })
                .unwrap_or("");
            let frames: Vec<_> = node
                .descendants()
                .filter(|n| n.has_tag_name("frame"))
                .take(12)
                .map(|n| {
                    json!({"function": child_text(n, "fn"), "file": child_text(n, "file"),
                    "directory": child_text(n, "dir"), "line": child_text(n, "line")})
                })
                .collect();
            errors.push(json!({"kind": kind, "occurrences": count, "description": description, "frames": frames}));
        }
    }
    Ok(
        json!({"status": if actionable == 0 { "ok" } else { "findings" },
        "error_contexts": contexts, "actionable_contexts": actionable, "occurrences_by_kind": kinds,
        "leaked_bytes_by_kind": leaks, "errors": errors, "errors_truncated": contexts > 20}),
    )
}

fn run_heaptrack(target: &[OsString], opts: &RunOptions<'_>, dir: &Path) -> Result<Value, String> {
    let tool = find_tool("heaptrack", opts.env)?;
    let printer = find_tool("heaptrack_print", opts.env)?;
    // heaptrack's shell launcher in some distributions cannot handle output paths
    // containing spaces. Record in a private temp path, then copy the finished trace.
    let scratch = tempfile::Builder::new()
        .prefix("cppbench-heap-")
        .tempdir_in("/tmp")
        .map_err(|e| e.to_string())?;
    let mut command = vec![
        tool,
        "--record-only".into(),
        "-o".into(),
        scratch.path().join("trace").into_os_string(),
    ];
    command.extend_from_slice(target);
    let recording_options = RunOptions {
        expected: None,
        ..*opts
    };
    let row = command_run(&command, &recording_options, dir, "heaptrack")?;
    let trace = fs::read_dir(scratch.path())
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|ext| ext == "zst" || ext == "gz"));
    let saved = if let Some(trace) = trace {
        let path = dir.join(trace.file_name().unwrap());
        fs::copy(trace, &path).map_err(|e| e.to_string())?;
        Some(path)
    } else {
        None
    };
    if let Some(mut value) = invocation_failed(&row) {
        value["raw_file"] = json!(saved);
        return Ok(value);
    }
    let trace = saved.ok_or("heaptrack did not produce a compressed trace")?;
    let command = vec![
        printer,
        "-f".into(),
        trace.clone().into_os_string(),
        "-n".into(),
        "5".into(),
        "-s".into(),
        "3".into(),
        "-l".into(),
        "1".into(),
    ];
    let analysis_options = RunOptions {
        input: None,
        expected: None,
        expected_exit: 0,
        ..*opts
    };
    let analysis = command_run(&command, &analysis_options, dir, "heaptrack-analysis")?;
    if let Some(mut value) = invocation_failed(&analysis) {
        value["raw_file"] = json!(trace);
        return Ok(value);
    }
    let mut result = parse_heaptrack(&read_report(&dir.join("heaptrack-analysis.stdout"))?)?;
    result["invocation"] = row;
    result["analysis_invocation"] = analysis;
    result["raw_file"] = json!(trace);
    result["analysis_file"] = json!(dir.join("heaptrack-analysis.stdout"));
    result["output_validation"] = json!("not checked: heaptrack mixes launcher messages with target stdout; baseline output is checked normally");
    result["scope"] = json!("heaptrack replay; heap sizes are approximate because heaptrack_print rounds them; retained allocations are not proof of a leak");
    Ok(result)
}
fn parse_heaptrack(text: &str) -> Result<Value, String> {
    let field = |prefix: &str| {
        text.lines()
            .rev()
            .find_map(|l| l.strip_prefix(prefix))
            .map(str::trim)
    };
    let count = |prefix: &str| {
        field(prefix)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse::<u64>().ok())
    };
    let allocations = count("calls to allocation functions:")
        .ok_or("heaptrack_print did not produce an allocation summary")?;
    let peak = field("peak heap memory consumption:")
        .and_then(parse_size)
        .ok_or("heaptrack_print did not report peak heap memory")?;
    Ok(json!({"status": "ok", "allocation_calls": allocations,
        "temporary_allocations": count("temporary memory allocations:"),
        "peak_heap_bytes_approx": peak,
        "retained_bytes_approx": field("total memory leaked:").and_then(parse_size)}))
}
fn parse_size(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let number: f64 = text[..split].parse().ok()?;
    let multiplier = match text[split..].trim().trim_end_matches('.') {
        "B" => 1.,
        "K" | "KB" => 1e3,
        "M" | "MB" => 1e6,
        "G" | "GB" => 1e9,
        "KiB" => 1024.,
        "MiB" => 1024. * 1024.,
        "GiB" => 1024. * 1024. * 1024.,
        _ => return None,
    };
    let result = number * multiplier;
    result.is_finite().then_some(result)
}

// This supervisor and all its descendants remain in the C runner's process group.
// It supplies no interpolated shell code: paths and target arguments use "$@".
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
fn run_gpu(
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
        .map(|(name, values)| (name, crate::summarize(values)))
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
        .map(|(name, values)| (name, crate::summarize(values)))
        .collect();
    Ok(
        json!({"status":if summaries.is_empty()&&resident.is_empty(){"unavailable"}else{"ok"},"pid":pid,"observed_samples":observed,
        "engines_percent":summaries,"peak_drm_resident_bytes":resident.into_iter().reduce(f64::max),
        "scope":"exact target PID only; child processes excluded; sums matching DRM clients and memory regions; shared buffers may be counted more than once; resident bytes are not dedicated VRAM"}),
    )
}

pub fn check_tools() -> Result<Value, String> {
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    env.insert("LC_ALL".into(), "C".into());
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let opts = RunOptions {
        cwd: &cwd,
        env: &env,
        input: None,
        expected: None,
        expected_exit: 0,
        timeout: 10.,
        output_limit: 4. * 1048576.,
        units: 1.,
        affinity: &[],
    };
    let mut tools = serde_json::Map::new();
    let mut interrupted = 0;
    for (name, flag) in [
        ("perf", "--version"),
        ("intel_gpu_top", "-h"),
        ("heaptrack", "--version"),
        ("heaptrack_print", "--version"),
        ("valgrind", "--version"),
    ] {
        if interrupted != 0 {
            break;
        }
        let path = match find_tool(name, &env) {
            Ok(p) => p,
            Err(e) => {
                tools.insert(name.into(), issue("unavailable", e));
                continue;
            }
        };
        let row = command_run(&[path.clone(), flag.into()], &opts, dir.path(), name)?;
        if row["status"] == "interrupted" {
            interrupted = row["interrupt_signal"].as_i64().unwrap_or(2);
        }
        let stdout = read_report(&dir.path().join(format!("{name}.stdout"))).unwrap_or_default();
        let preview = if stdout.trim().is_empty() {
            row["stderr_preview"].as_str().unwrap_or("")
        } else {
            stdout.trim()
        };
        tools.insert(name.into(),json!({"status":if row["status"]=="ok"{"ok"}else{"unavailable"},"path":path,
            "identity":preview.lines().filter(|l|!l.trim().is_empty()).take(3).collect::<Vec<_>>().join("\n"),
            "sha256":crate::experiment::hash_file(Path::new(&path)).ok()}));
    }
    let mut probes = serde_json::Map::new();
    if interrupted == 0 {
        for (name, argv) in [
            ("perf", vec!["stat", "-e", "cycles:u", "--", "/bin/true"]),
            (
                "intel_gpu_top",
                vec!["-J", "-s", "10", "-n", "2", "-o", "-"],
            ),
        ] {
            if let Ok(path) = find_tool(name, &env) {
                let mut command = vec![path];
                command.extend(argv.into_iter().map(OsString::from));
                let row = command_run(&command, &opts, dir.path(), &format!("probe-{name}"))?;
                probes.insert(
                    name.into(),
                    json!({"status":row["status"],"stderr":row["stderr_preview"]}),
                );
                if row["status"] == "interrupted" {
                    interrupted = row["interrupt_signal"].as_i64().unwrap_or(2);
                    break;
                }
            }
        }
    }
    Ok(
        json!({"kind":"tool_check","tools":tools,"counter_probes":probes,"interrupt_signal":interrupted,
        "note":"checks tool identity and basic counter access; individual workloads and report formats can still expose compatibility limits"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_client_attribution_excludes_other_pids_and_baseline() {
        let text = include_str!("../tests/fixtures/intel_gpu_clients.json");
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
    #[test]
    fn valgrind_reports_errors_leaks_and_contexts() {
        let xml = r#"<valgrindoutput><status><state>FINISHED</state></status>
        <error><unique>0</unique><kind>InvalidRead</kind><what>bad read</what></error>
        <error><unique>1</unique><kind>Leak_DefinitelyLost</kind><xwhat><text>lost</text><leakedbytes>64</leakedbytes></xwhat></error>
        <error><unique>2</unique><kind>Leak_StillReachable</kind><xwhat><leakedbytes>8</leakedbytes></xwhat></error>
        <errorcounts><pair><unique>0</unique><count>4</count></pair></errorcounts></valgrindoutput>"#;
        let r = parse_valgrind(xml).unwrap();
        assert_eq!(r["status"], "findings");
        assert_eq!(r["actionable_contexts"], 2);
        assert_eq!(r["occurrences_by_kind"]["InvalidRead"], 4);
        assert_eq!(r["leaked_bytes_by_kind"]["Leak_DefinitelyLost"], 64);
        assert!(parse_valgrind("<valgrindoutput>").is_err());
    }
    #[test]
    fn heap_summary_parses_rounded_units() {
        let r = parse_heaptrack("calls to allocation functions: 42 (10/s)\ntemporary memory allocations: 3 (1/s)\npeak heap memory consumption: 73.73K\ntotal memory leaked: 0B\n").unwrap();
        assert_eq!(r["allocation_calls"], 42);
        assert_eq!(r["peak_heap_bytes_approx"], 73730.);
        assert_eq!(r["retained_bytes_approx"], 0.);
        assert!(parse_heaptrack("failed to read").is_err());
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
        let text = include_str!("../tests/fixtures/intel_gpu_top_short.json");
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
