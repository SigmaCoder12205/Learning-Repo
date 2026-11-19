//! Output assertions and repeatability checks, always outside measured execution.
use crate::cli::Cli;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct Checks<'a> {
    pub exact: Option<&'a str>,
    pub contains: &'a [String],
    pub fingerprint: bool,
}
impl<'a> Checks<'a> {
    pub fn from_args(args: &'a Cli) -> Self {
        Self {
            exact: args.expect_text.as_deref(),
            contains: &args.expect_contains,
            fingerprint: args.check_output_stability,
        }
    }
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
