//! Output assertions and repeatability checks, always outside measured execution.
use crate::cli::Cli;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Default)]
pub(crate) struct Checks<'a> {
    pub exact: Option<&'a str>,
    pub contains: &'a [String],
    pub fingerprint: bool,
    pub validator: Option<&'a Validator>,
}
impl<'a> Checks<'a> {
    pub fn from_args(args: &'a Cli) -> Self {
        Self {
            exact: args.expect_text.as_deref(),
            contains: &args.expect_contains,
            fingerprint: args.check_output_stability,
            validator: None,
        }
    }
}

/// A user-written check program in any language, run after each successful invocation.
#[derive(Debug, Clone)]
pub(crate) struct Validator {
    pub command: Vec<OsString>,
    pub timeout: f64,
}

const VALIDATOR_OUTPUT_LIMIT: u64 = 64 * 1024;
const MAX_VALIDATOR_METRICS: usize = 32;

/// Resolve `--validate` once, before any target runs, so a typo fails early.
pub(crate) fn validator(
    args: &Cli,
    env: &BTreeMap<OsString, OsString>,
) -> Result<Option<Validator>, String> {
    let Some(program) = &args.validate else {
        return Ok(None);
    };
    // Absolute but not canonical: a virtual environment's `bin/python` symlink must
    // keep its own path, or the interpreter loses the environment's packages.
    let resolved: PathBuf = if program.components().count() > 1 || program.is_absolute() {
        std::path::absolute(program)
            .map_err(|e| format!("--validate {}: {e}", program.display()))?
    } else {
        env.get(std::ffi::OsStr::new("PATH"))
            .into_iter()
            .flat_map(std::env::split_paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| format!("--validate: {} is not on PATH", program.display()))?
    };
    use std::os::unix::fs::PermissionsExt;
    let metadata = resolved
        .metadata()
        .map_err(|e| format!("--validate {}: {e}", resolved.display()))?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!(
            "--validate {} must be an executable file; for scripts use a shebang or e.g. --validate python3 --validate-arg check.py",
            resolved.display()
        ));
    }
    let mut command = vec![resolved.into_os_string()];
    command.extend(args.validate_arg.iter().cloned());
    Ok(Some(Validator {
        command,
        timeout: args.validate_timeout,
    }))
}

fn valid_metric_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}

/// Parse validator stdout: `bench-metric NAME VALUE [UNIT]` lines become custom
/// metrics; every other line is part of the human-readable message.
fn parse_validator_output(text: &str) -> Result<(Value, Value, String), String> {
    let mut metrics = serde_json::Map::new();
    let mut units = serde_json::Map::new();
    let mut message = String::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if fields.next() != Some("bench-metric") {
            if !line.trim().is_empty() {
                message.push_str(line.trim_end());
                message.push('\n');
            }
            continue;
        }
        let (Some(name), Some(value)) = (fields.next(), fields.next()) else {
            return Err(format!(
                "malformed metric line {line:?}; expected `bench-metric NAME VALUE [UNIT]`"
            ));
        };
        let unit = fields.next().unwrap_or("");
        if fields.next().is_some() || !valid_metric_name(name) || unit.len() > 32 {
            return Err(format!("malformed metric line {line:?}; names use 1..64 letters, digits, '_', '-' or '.', and units are one word"));
        }
        let value: f64 = value
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
            .ok_or_else(|| format!("metric {name} needs a finite number, got {value:?}"))?;
        if metrics.insert(name.into(), json!(value)).is_some() {
            return Err(format!("metric {name} was reported more than once"));
        }
        if metrics.len() > MAX_VALIDATOR_METRICS {
            return Err(format!(
                "validators may report at most {MAX_VALIDATOR_METRICS} metrics"
            ));
        }
        units.insert(name.into(), json!(unit));
    }
    Ok((Value::Object(metrics), Value::Object(units), message))
}

/// Run the user's validator outside measured time. Its stdin is the target's full stdout.
pub(crate) fn run_validator(
    validator: &Validator,
    opts: &crate::runner::RunOptions<'_>,
    stdout: &File,
    stderr: &File,
    exit_code: i32,
    wall: f64,
) -> Result<Value, String> {
    let pid = std::process::id();
    let own = |file: &File| format!("/proc/{pid}/fd/{}", file.as_raw_fd());
    let mut env = opts.env.clone();
    for (key, value) in [
        ("BENCH_STDOUT_FILE", own(stdout)),
        ("BENCH_STDERR_FILE", own(stderr)),
        (
            "BENCH_INPUT_FILE",
            opts.input
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "/dev/null".into()),
        ),
        ("BENCH_TARGET_EXIT_CODE", exit_code.to_string()),
        ("BENCH_TARGET_WALL_SECONDS", format!("{wall:.9}")),
    ] {
        env.insert(key.into(), value.into());
    }
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let (out, err) = (dir.path().join("stdout"), dir.path().join("stderr"));
    let target_stdout = format!("/proc/self/fd/{}", stdout.as_raw_fd());
    let row = crate::runner::run_logged(
        &validator.command,
        &crate::runner::RunOptions {
            cwd: opts.cwd,
            env: &env,
            input: Some(Path::new(&target_stdout)),
            expected: None,
            checks: Default::default(),
            expected_exit: 0,
            timeout: validator.timeout,
            output_limit: VALIDATOR_OUTPUT_LIMIT as f64 * 16.0,
            units: 1.0,
            affinity: &[],
        },
        &[0],
        Some((&out, &err)),
    )?;
    let mut text = String::new();
    File::open(&out)
        .and_then(|f| f.take(VALIDATOR_OUTPUT_LIMIT).read_to_string(&mut text))
        .map_err(|e| format!("validator stdout: {e}"))?;
    let mut result = json!({"exit_code": row["exit_code"], "wall_seconds": row["metrics"]["wall"],
        "metrics": {}, "metric_units": {}, "message": "", "stderr_preview": row["stderr_preview"]});
    let status = match row["status"].as_str() {
        Some("ok") => "passed",
        Some("unexpected_exit") => "failed",
        Some("interrupted") => {
            result["interrupt_signal"] = row["interrupt_signal"].clone();
            "interrupted"
        }
        Some(other) => {
            result["reason"] = json!(format!("validator {other}"));
            "error"
        }
        None => "error",
    };
    result["status"] = json!(status);
    match parse_validator_output(&text) {
        Ok((metrics, units, mut message)) => {
            if message.len() > 1024 {
                // Cut at a UTF-8 boundary (floor_char_boundary needs a newer Rust than 1.89).
                let cut = (0..=1024)
                    .rev()
                    .find(|&i| message.is_char_boundary(i))
                    .unwrap_or(0);
                message.truncate(cut);
            }
            result["metrics"] = metrics;
            result["metric_units"] = units;
            result["message"] = json!(message);
        }
        Err(reason) if status == "passed" => {
            result["status"] = json!("error");
            result["reason"] = json!(reason);
        }
        Err(_) => {}
    }
    Ok(result)
}

/// Summarize validator metrics across successful measured runs.
pub(crate) fn summarize_validation(report: &mut Value) {
    if !report["settings"]["validator"].is_object() {
        return;
    }
    let mut values: BTreeMap<String, (String, Vec<f64>)> = BTreeMap::new();
    for row in report["runs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["status"] == "ok")
    {
        let validation = &row["validation"];
        for (name, value) in validation["metrics"].as_object().into_iter().flatten() {
            if let Some(value) = value.as_f64() {
                let entry = values.entry(name.clone()).or_insert_with(|| {
                    (
                        validation["metric_units"][name]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        Vec::new(),
                    )
                });
                entry.1.push(value);
            }
        }
    }
    let summary: serde_json::Map<_, _> = values
        .into_iter()
        .map(|(name, (unit, samples))| {
            let mut stats = crate::stats::summarize_basic(samples);
            stats["unit"] = json!(unit);
            (name, stats)
        })
        .collect();
    report["validation_metrics"] = Value::Object(summary);
}

pub(crate) fn validate(args: &Cli) -> Result<(), String> {
    if args
        .expect_text
        .as_ref()
        .is_some_and(|s| s.len() > 1024 * 1024)
    {
        return Err("--expect-text exceeds 1 MiB; use --expect-output FILE".into());
    }
    if args.expect_contains.len() > 16
        || args
            .expect_contains
            .iter()
            .any(|s| s.is_empty() || s.len() > 4096)
    {
        return Err(
            "--expect-contains accepts at most 16 nonempty literals, each at most 4096 bytes"
                .into(),
        );
    }
    if args.check_output_stability && args.runs < 2 {
        return Err(
            "--check-output-stability requires --runs of at least 2; warmups do not count".into(),
        );
    }
    Ok(())
}

pub(crate) fn provenance(args: &Cli, expected: Option<&Path>) -> Result<Value, String> {
    if expected.is_none() && args.expect_text.is_none() && args.expect_contains.is_empty() {
        return Ok(Value::Null);
    }
    let digest = |text: &str| format!("{:x}", Sha256::digest(text.as_bytes()));
    let exact = if let Some(path) = expected {
        Some(crate::experiment::hash_file(path)?)
    } else {
        args.expect_text.as_deref().map(digest)
    };
    let mut contains: Vec<_> = args.expect_contains.iter().map(|s| digest(s)).collect();
    contains.sort();
    contains.dedup();
    Ok(json!({"exact_sha256": exact, "contains_sha256": contains}))
}

// Streaming KMP keeps matching linear in output size, including chunk boundaries.
struct Matcher<'a> {
    pattern: &'a [u8],
    prefix: Vec<usize>,
    matched: usize,
    found: bool,
}
impl<'a> Matcher<'a> {
    fn new(pattern: &'a str) -> Self {
        let pattern = pattern.as_bytes();
        let mut prefix = vec![0; pattern.len()];
        let mut matched = 0;
        for i in 1..pattern.len() {
            while matched > 0 && pattern[i] != pattern[matched] {
                matched = prefix[matched - 1];
            }
            if pattern[i] == pattern[matched] {
                matched += 1;
            }
            prefix[i] = matched;
        }
        Self {
            pattern,
            prefix,
            matched: 0,
            found: pattern.is_empty(),
        }
    }
    fn feed(&mut self, bytes: &[u8]) {
        if self.found {
            return;
        }
        for &byte in bytes {
            while self.matched > 0 && byte != self.pattern[self.matched] {
                self.matched = self.prefix[self.matched - 1];
            }
            if byte == self.pattern[self.matched] {
                self.matched += 1;
            }
            if self.matched == self.pattern.len() {
                self.found = true;
                return;
            }
        }
    }
}

pub(crate) fn inspect(
    output: &mut File,
    expected: Option<&Path>,
    checks: Checks<'_>,
) -> io::Result<Value> {
    let requested = expected.is_some() || checks.exact.is_some() || !checks.contains.is_empty();
    if !requested && !checks.fingerprint {
        return Ok(Value::Null);
    }
    let mut expected_file = expected.map(File::open).transpose()?;
    let expected_size = if let Some(file) = &expected_file {
        Some(file.metadata()?.len())
    } else {
        checks.exact.map(|s| s.len() as u64)
    };
    let mut literal = checks.exact.unwrap_or("").as_bytes();
    let expected_reader: &mut dyn Read = if let Some(file) = &mut expected_file {
        file
    } else {
        &mut literal
    };
    let actual_size = output.metadata()?.len();
    let mut mismatch = expected_size
        .filter(|n| *n != actual_size)
        .map(|n| n.min(actual_size));
    let mut matchers: Vec<_> = checks.contains.iter().map(|s| Matcher::new(s)).collect();
    let mut hash = Sha256::new();
    output.seek(SeekFrom::Start(0))?;
    let mut captured = output.take(actual_size);
    let mut buffer = [0; 65536];
    let mut expected_buffer = [0; 65536];
    let mut offset = 0u64;
    loop {
        let count = captured.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let chunk = &buffer[..count];
        if checks.fingerprint {
            hash.update(chunk);
        }
        for matcher in &mut matchers {
            matcher.feed(chunk);
        }
        if let Some(size) = expected_size {
            let expected_count = size.saturating_sub(offset).min(count as u64) as usize;
            expected_reader.read_exact(&mut expected_buffer[..expected_count])?;
            if let Some(index) = chunk[..expected_count]
                .iter()
                .zip(&expected_buffer[..expected_count])
                .position(|(a, b)| a != b)
            {
                let position = offset + index as u64;
                mismatch = Some(mismatch.map_or(position, |old| old.min(position)));
            }
        }
        offset += count as u64;
    }
    let missing: Vec<_> = matchers
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.found)
        .map(|(i, _)| i + 1)
        .collect();
    Ok(json!({
        "status": if !requested { "not_requested" } else if mismatch.is_some() || !missing.is_empty() { "failed" } else { "passed" },
        "actual_bytes": actual_size, "expected_bytes": expected_size,
        "first_mismatch_byte": mismatch, "missing_contains": missing,
        "stdout_sha256": if checks.fingerprint { Some(format!("{:x}", hash.finalize())) } else { None },
        "scope": "Full stdout bytes checked after measurement; offsets are zero-based; substring indices are one-based"
    }))
}

pub(crate) fn refresh(report: &mut Value) {
    if report["settings"]["check_output_stability"] != true {
        return;
    }
    let samples: Vec<_> = report["runs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["status"] == "ok")
        .filter_map(|r| {
            r["output_check"]["stdout_sha256"]
                .as_str()
                .map(|s| (r["run"].clone(), s.to_owned()))
        })
        .collect();
    let first = samples.first();
    let differing: Vec<_> = samples
        .iter()
        .filter(|(_, hash)| first.is_some_and(|(_, h)| hash != h))
        .map(|(run, _)| run.clone())
        .collect();
    let variants = samples
        .iter()
        .map(|(_, hash)| hash)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    report["output_stability"] = json!({
        "status": if !differing.is_empty() { "unstable" } else if samples.len() < 2 { "insufficient_samples" } else { "stable" },
        "checked_runs": samples.len(), "distinct_outputs": variants,
        "reference_run": first.map(|(run,_)| run), "different_runs": differing,
        "scope": "SHA-256 of full stdout from successful measured runs in this case; warmups, stderr and diagnostics excluded. Stability does not prove correctness."
    });
}

pub(crate) fn render(report: &Value) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if let Some(metrics) = report["validation_metrics"]
        .as_object()
        .filter(|m| !m.is_empty())
    {
        out.push_str("Validator metrics (successful measured runs):\n");
        for (name, stats) in metrics {
            let unit = stats["unit"].as_str().unwrap_or("");
            let _ = writeln!(
                out,
                "  {name}: mean {}{} | median {} | min {} | max {} | n={}",
                stats["mean"],
                if unit.is_empty() {
                    String::new()
                } else {
                    format!(" {unit}")
                },
                stats["median"],
                stats["min"],
                stats["max"],
                stats["count"]
            );
        }
    }
    for phase in ["warmups", "runs"] {
        for row in report[phase]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["status"] == "validation_failed")
        {
            let validation = &row["validation"];
            let _ = write!(
                out,
                "Validator {} ({phase} {}): exit {}",
                validation["status"].as_str().unwrap_or("failed"),
                row["run"],
                validation["exit_code"]
            );
            if let Some(reason) = validation["reason"].as_str() {
                let _ = write!(out, "; {reason}");
            }
            let message = validation["message"].as_str().unwrap_or("").trim();
            let stderr = validation["stderr_preview"].as_str().unwrap_or("").trim();
            for text in [message, stderr].into_iter().filter(|t| !t.is_empty()) {
                let first = text.lines().next().unwrap_or("");
                let _ = write!(out, "; {}", json!(first)); // JSON-escaped for terminals
            }
            out.push('\n');
        }
    }
    if report["output_stability"].is_object() {
        let _ = writeln!(
            out,
            "Output stability: {} | {} checked runs | {} distinct outputs | differing runs {}",
            report["output_stability"]["status"]
                .as_str()
                .unwrap_or("unknown"),
            report["output_stability"]["checked_runs"],
            report["output_stability"]["distinct_outputs"],
            report["output_stability"]["different_runs"]
        );
    }
    for phase in ["warmups", "runs"] {
        for row in report[phase]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["output_check"]["status"] == "failed")
        {
            let check = &row["output_check"];
            let _ = write!(
                out,
                "Output check failed ({phase} {}): actual {} bytes",
                row["run"], check["actual_bytes"]
            );
            if !check["expected_bytes"].is_null() {
                let _ = write!(out, "; expected {} bytes", check["expected_bytes"]);
            }
            if !check["first_mismatch_byte"].is_null() {
                let _ = write!(
                    out,
                    "; first differing byte {} (zero-based)",
                    check["first_mismatch_byte"]
                );
            }
            if check["missing_contains"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
            {
                let _ = write!(
                    out,
                    "; missing --expect-contains checks {} (one-based)",
                    check["missing_contains"]
                );
            }
            out.push('\n');
        }
    }
    out
}
