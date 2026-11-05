use crate::Cli;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
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
pub fn precision_met(values: &[f64], target: f64) -> bool {
    if values.len() < 5 {
        return false;
    }
    let s = crate::summarize_basic(values.to_vec());
    let mean = s["mean"].as_f64().unwrap_or(0.);
    mean > 0.
        && 196. * s["stdev"].as_f64().unwrap_or(f64::INFINITY) / (values.len() as f64).sqrt() / mean
            <= target
}
pub fn intervals(values: &[f64]) -> Value {
    if values.len() < 2 {
        return json!({"mean_ci95": null, "method": "insufficient samples", "outlier_count": 0});
    }
    let mut random = 0x4d595df4d0f33173_u64;
    let mut means = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let mut mean = 0.;
        for _ in values {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            mean += values[(random as usize) % values.len()] / values.len() as f64;
        }
        means.push(mean);
    }
    means.sort_by(f64::total_cmp);
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let q1 = crate::percentile(&sorted, 0.25);
    let q3 = crate::percentile(&sorted, 0.75);
    let iqr = q3 - q1;
    json!({"mean_ci95": {"low": crate::percentile(&means, 0.025), "high": crate::percentile(&means, 0.975)},
        "method": "deterministic percentile bootstrap, 1000 resamples; descriptive, especially with adaptive stopping",
        "outlier_count": values.iter().filter(|&&v| v < q1-1.5*iqr || v > q3+1.5*iqr).count()})
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
            summary.insert(name.into(), crate::summarize(values));
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
pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, value).map_err(|e| e.to_string())?;
    temp.write_all(b"\n").map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub struct Journal(Option<File>);
impl Journal {
    pub fn open(path: Option<&Path>) -> Result<Self, String> {
        Ok(Self(
            path.map(|p| {
                File::options()
                    .write(true)
                    .create_new(true)
                    .open(p)
                    .map_err(|e| format!("checkpoint {}: {e}", p.display()))
            })
            .transpose()?,
        ))
    }
    pub fn emit(&mut self, value: Value) -> Result<(), String> {
        if let Some(file) = &mut self.0 {
            serde_json::to_writer(&mut *file, &value).map_err(|e| e.to_string())?;
            file.write_all(b"\n").map_err(|e| e.to_string())?;
            file.sync_data().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
pub fn recover(path: &Path) -> Result<Value, String> {
    fn validate_report(report: &Value) -> Result<(), String> {
        if let Some(cases) = report["cases"].as_array() {
            for case in cases {
                validate_report(case)?;
            }
            return Ok(());
        }
        if !report.is_object()
            || !report["runs"].is_array()
            || !report["warmups"].is_array()
            || !report["selected_metrics"]
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string))
        {
            return Err("invalid checkpoint report structure".into());
        }
        if report["error"].is_string() {
            return Ok(());
        }
        if !report["warnings"]
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_string))
            || !report["summary"].is_object()
        {
            return Err("checkpoint report lacks warnings or summary".into());
        }
        for row in report["runs"]
            .as_array()
            .unwrap()
            .iter()
            .chain(report["warmups"].as_array().unwrap())
        {
            if !row["status"].is_string() || !row["metrics"].is_object() {
                return Err("invalid checkpoint sample".into());
            }
        }
        for stats in report["summary"].as_object().unwrap().values() {
            for key in [
                "mean",
                "median",
                "min",
                "max",
                "stdev",
                "p90",
                "p95",
                "cv_percent",
            ] {
                if stats[key].as_f64().is_none() {
                    return Err("invalid checkpoint statistics".into());
                }
            }
        }
        Ok(())
    }
    let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut reports = BTreeMap::<String, Value>::new();
    let mut order = Vec::new();
    let mut suite = None;
    let mut final_report = None;
    let mut line = String::new();
    let mut number = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        number += 1;
        if !line.ends_with('\n') {
            break;
        } // Torn final record after termination.
        let event: Value = serde_json::from_str(&line)
            .map_err(|e| format!("invalid checkpoint line {number}: {e}"))?;
        if matches!(
            event["event"].as_str(),
            Some("suite_start" | "start" | "report" | "final")
        ) {
            validate_report(&event["report"])?;
        }
        match event["event"].as_str() {
            Some("suite_start") => suite = Some(event["report"].clone()),
            Some("start") => {
                let r = event["report"].clone();
                let name = r["name"]
                    .as_str()
                    .ok_or("checkpoint start lacks name")?
                    .to_string();
                order.push(name.clone());
                reports.insert(name, r);
            }
            Some("sample") => {
                if !event["row"]["status"].is_string() || !event["row"]["metrics"].is_object() {
                    return Err("invalid checkpoint sample".into());
                }
                let name = event["name"]
                    .as_str()
                    .ok_or("checkpoint sample lacks name")?;
                let r = reports.get_mut(name).ok_or("sample before start")?;
                let phase = event["phase"]
                    .as_str()
                    .filter(|p| *p == "runs" || *p == "warmups")
                    .ok_or("invalid phase")?;
                r[phase]
                    .as_array_mut()
                    .ok_or("invalid start record")?
                    .push(event["row"].clone());
            }
            Some("report") => {
                let r = event["report"].clone();
                let name = r["name"].as_str().ok_or("report lacks name")?.to_string();
                if !order.contains(&name) {
                    order.push(name.clone());
                }
                reports.insert(name, r);
            }
            Some("final") => final_report = Some(event["report"].clone()),
            _ => return Err(format!("unknown checkpoint event on line {number}")),
        }
    }
    if let Some(report) = final_report {
        return Ok(report);
    }
    for report in reports.values_mut() {
        refresh(report);
        if report["completed"] != true {
            report["stop_reason"] = json!("recovered_partial");
            report["measurement_passed"] = json!(false);
        }
    }
    if let Some(mut suite) = suite {
        suite["cases"] = json!(order
            .into_iter()
            .filter_map(|n| reports.remove(&n))
            .collect::<Vec<_>>());
        suite["completed"] = json!(false);
        suite["recovered"] = json!(true);
        return Ok(suite);
    }
    let mut report = reports
        .into_values()
        .next()
        .ok_or("checkpoint has no measurements")?;
    report["recovered"] = json!(true);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uncertainty_and_adaptive_stopping_keep_outliers() {
        assert!(intervals(&[1.])["mean_ci95"].is_null());
        assert_eq!(
            intervals(&[2.; 5])["mean_ci95"],
            json!({"low":2.,"high":2.})
        );
        let values = [1., 1., 1., 1., 1., 20.];
        assert_eq!(intervals(&values)["outlier_count"], 1);
        assert!(!precision_met(&values, 5.));
        assert!(!precision_met(&[2.; 4], 5.));
        assert!(precision_met(&[2.; 5], 5.));
        assert_eq!(affinity(Some("2,0-2")).unwrap(), vec![0, 1, 2]);
        assert!(affinity(Some("4-2")).is_err());
    }
}
