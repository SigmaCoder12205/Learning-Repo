//! heaptrack replay, parsing and format fixtures.
use super::{command_run, find_tool, invocation_failed, read_report};
use crate::runner::RunOptions;
use serde_json::{json, Value};
use std::ffi::OsString;
use std::fs;
use std::path::Path;

pub(super) fn run(target: &[OsString], opts: &RunOptions<'_>, dir: &Path) -> Result<Value, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heap_summary_parses_rounded_units() {
        let r = parse_heaptrack("calls to allocation functions: 42 (10/s)\ntemporary memory allocations: 3 (1/s)\npeak heap memory consumption: 73.73K\ntotal memory leaked: 0B\n").unwrap();
        assert_eq!(r["allocation_calls"], 42);
        assert_eq!(r["peak_heap_bytes_approx"], 73730.);
        assert_eq!(r["retained_bytes_approx"], 0.);
        assert!(parse_heaptrack("failed to read").is_err());
    }
}
