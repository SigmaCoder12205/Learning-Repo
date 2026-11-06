//! Experiment validation, provenance and successful-run aggregation.
use crate::cli::Cli;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::Path,
};
pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn provenance(
    binary: &Path,
    input: Option<&Path>,
    env: &BTreeMap<OsString, OsString>,
) -> Result<Value, String> {
    use std::os::unix::ffi::OsStrExt;
    let mut hash = Sha256::new();
    for (key, value) in env {
        if key == "_" {
            continue;
        }
        hash.update(key.as_bytes());
        hash.update([0]);
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = cpuinfo
        .lines()
        .find_map(|l| {
            l.strip_prefix("model name")
                .and_then(|s| s.split_once(':'))
                .map(|(_, v)| v.trim())
        })
        .unwrap_or("unknown");
    Ok(json!({"executable_sha256": hash_file(binary)?,
        "input_sha256": input.map(hash_file).transpose()?, "environment_sha256": format!("{:x}", hash.finalize()),
        "cpu_model": model, "kernel": fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().trim(),
        "hostname": fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default().trim(),
        "cpu_governor": fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor").ok().map(|s| s.trim().to_string()),
        "recorded_unix_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()}))
}
pub fn affinity(text: Option<&str>) -> Result<Vec<u32>, String> {
    let Some(text) = text else {
        return Ok(vec![]);
    };
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let parse = |s: &str| {
            s.parse::<u32>()
                .ok()
                .filter(|v| *v < 1024)
                .ok_or_else(|| "CPU IDs must be in 0..1024".to_string())
        };
        let (first, last) = if let Some((a, b)) = part.split_once('-') {
            (parse(a)?, parse(b)?)
        } else {
            let n = parse(part)?;
            (n, n)
        };
        if first > last {
            return Err("CPU range must be ascending".into());
        }
        for n in first..=last {
            if !cpus.contains(&n) {
                cpus.push(n);
            }
        }
    }
    cpus.sort();
    Ok(cpus)
}
pub fn refresh(report: &mut Value) {
    let good: Vec<_> = report["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["status"] == "ok")
        .collect();
    let mut summary = serde_json::Map::new();
    for name in report["selected_metrics"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        let values: Vec<_> = good
            .iter()
            .filter_map(|r| r["metrics"][name].as_f64())
            .collect();
        if !values.is_empty() {
            summary.insert(name.into(), crate::stats::summarize(values));
        }
    }
    let count = good.len();
    report["summary"] = Value::Object(summary);
    report["successful_runs"] = json!(count);
}
pub fn validate(args: &Cli) -> Result<(), String> {
    if args.require_profile_data && args.profile_flags.is_empty() {
        return Err("--require-profile-data needs a domain profile; use --list-profiles".into());
    }
    if args.deadline_ms.is_some() && !args.profile_flags.iter().any(|p| p == "embedded") {
        return Err("--deadline-ms requires --embedded".into());
    }
    if (args.adaptive || args.min_time.is_some()) && args.max_runs < args.runs {
        return Err("--max-runs must be at least --runs".into());
    }
    affinity(args.cpu_affinity.as_deref())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn affinity_ranges_are_sorted_and_validated() {
        assert_eq!(affinity(Some("2,0-2")).unwrap(), vec![0, 1, 2]);
        assert!(affinity(Some("4-2")).is_err());
    }
}
