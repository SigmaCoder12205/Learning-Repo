use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Compare two saved reports without running anything (pure Rust, no Python).
#[derive(clap::Args, Debug, Clone)]
pub(crate) struct Options {
    /// The earlier report (e.g. from --save-baseline or --format json --output)
    baseline: PathBuf,
    /// The later report to judge against the baseline
    current: PathBuf,
    /// Allowed worsening, e.g. --regression wall=5 --regression validator.score:higher=2
    #[arg(long)]
    regression: Vec<String>,
    /// Warn on worsening without exit code 3
    #[arg(long)]
    warn_regression: Vec<String>,
    /// Compare despite different workloads/hosts (differences remain visible)
    #[arg(long)]
    allow_mismatch: bool,
    #[arg(long, default_value = "text", value_parser = ["text", "json"])]
    format: String,
}

pub(crate) fn run(options: Options) -> Result<i32, String> {
    let baseline = load(&options.baseline)?;
    let mut current = load(&options.current)?;
    let limits = thresholds(&options.regression)?;
    let warnings = thresholds(&options.warn_regression)?;
    apply(&mut current, &baseline, &limits, options.allow_mismatch);
    if !warnings.is_empty() {
        apply_warnings(&mut current, &baseline, &warnings, options.allow_mismatch);
    }
    let mut cases = vec![("", &current)];
    if let Some(list) = current["cases"].as_array() {
        cases = list
            .iter()
            .map(|c| (c["name"].as_str().unwrap_or(""), c))
            .collect();
    }
    if options.format == "json" {
        let per_case: Vec<_> = cases
            .iter()
            .map(|(name, case)| json!({"name":name,"comparison":case["comparison"]}))
            .collect();
        let out = json!({"kind":"report_comparison","baseline":options.baseline,"current":options.current,
            "status":current["comparison"]["status"],"exit_code":exit_code(&current),
            "comparison":current["comparison"],"cases":if current["cases"].is_array() {json!(per_case)} else {Value::Null}});
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!(
            "bench compare | {} -> {}",
            options.baseline.display(),
            options.current.display()
        );
        if current["cases"].is_array() {
            println!(
                "Suite comparison: {}",
                current["comparison"]["status"]
                    .as_str()
                    .unwrap_or("unknown")
            );
        }
        for (name, case) in cases {
            if !name.is_empty() {
                print!("\nCase {name}:");
            }
            print!("{}", crate::report::render_comparison(case));
        }
    }
    Ok(exit_code(&current))
}

pub fn load(path: &Path) -> Result<Value, String> {
    let data = fs::read(path).map_err(|e| format!("baseline {}: {e}", path.display()))?;
    if data.len() > 64 * 1024 * 1024 {
        return Err("baseline exceeds 64 MiB".into());
    }
    let value: Value =
        serde_json::from_slice(&data).map_err(|e| format!("invalid baseline: {e}"))?;
    if !value["summary"].is_object() && !value["cases"].is_array() {
        return Err("baseline is not a bench report".into());
    }
    Ok(value)
}
pub fn thresholds(values: &[String]) -> Result<BTreeMap<String, f64>, String> {
    let mut result = BTreeMap::new();
    for arg in values {
        let (name, value) = arg
            .split_once('=')
            .ok_or("regression must be METRIC=PERCENT")?;
        if let Some(rest) = name.strip_prefix("validator.") {
            let (metric, direction) = rest.split_once(':').unwrap_or((rest, ""));
            if metric.is_empty() || !["", "higher", "lower"].contains(&direction) {
                return Err(format!(
                    "{name}: use validator.NAME, validator.NAME:higher or validator.NAME:lower"
                ));
            }
        } else if !crate::metrics::METRICS.iter().any(|m| m.0 == name)
            && !["allocations", "heap_peak", "leaked_bytes"].contains(&name)
            && crate::profiles::metric(name).is_none()
        {
            return Err(format!("unknown regression metric: {name}"));
        }
        if ["cpu_percent", "stdout_bytes", "stderr_bytes"].contains(&name) {
            return Err(format!("{name} has no automatic better/worse direction"));
        }
        if crate::profiles::metric(name).is_some_and(|m| {
            m.kind != crate::profiles::catalog::MetricKind::Samples || m.unit != "ms"
        }) {
            return Err(format!("{name} has no automatic improvement direction; domain regression gates currently support duration sample metrics in ms"));
        }
        let n = value
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.)
            .ok_or("regression percent must be finite and nonnegative")?;
        result.insert(name.into(), n);
    }
    Ok(result)
}
/// `validator.NAME[:higher|lower]` split into the metric and an explicit direction.
fn validator_key(key: &str) -> Option<(&str, Option<bool>)> {
    let rest = key.strip_prefix("validator.")?;
    Some(match rest.split_once(':') {
        Some((name, direction)) => (name, Some(direction == "higher")),
        None => (rest, None),
    })
}
/// Per-run samples behind a summary, for the rank test; empty when there are none.
fn samples(report: &Value, name: &str) -> Vec<f64> {
    let runs = report["runs"].as_array().into_iter().flatten();
    let ok = runs.filter(|r| r["status"] == "ok");
    if let Some((metric, _)) = validator_key(name) {
        ok.filter_map(|r| r["validation"]["metrics"][metric].as_f64())
            .collect()
    } else if crate::metrics::METRICS.iter().any(|m| m.0 == name) {
        ok.filter_map(|r| r["metrics"][name].as_f64()).collect()
    } else {
        Vec::new()
    }
}
fn metric(report: &Value, name: &str) -> Option<f64> {
    if let Some((metric, _)) = validator_key(name) {
        return report["validation_metrics"][metric]["median"]
            .as_f64()
            .filter(|v| v.is_finite());
    }
    if let Some(spec) = crate::profiles::metric(name) {
        let value = &report["profiles"]["metrics"][name];
        if value["status"] != "ok" {
            return None;
        }
        return if spec.kind == crate::profiles::catalog::MetricKind::Samples {
            value["summary"]["p95"].as_f64()
        } else {
            value["value"].as_f64()
        };
    }
    let number = match name {
        "allocations" => report["diagnostics"]["results"]["heaptrack"]["allocation_calls"].as_f64(),
        "heap_peak" => {
            report["diagnostics"]["results"]["heaptrack"]["peak_heap_bytes_approx"].as_f64()
        }
        "leaked_bytes" => report["diagnostics"]["results"]["valgrind"]["leaked_bytes_by_kind"]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter(|(k, _)| k.as_str() != "Leak_StillReachable")
                    .filter_map(|(_, v)| v.as_f64())
                    .sum()
            }),
        _ => report["summary"][name][if name == "rss" { "max" } else { "median" }].as_f64(),
    };
    number.filter(|v| v.is_finite() && *v >= 0.)
}
fn single(current: &Value, baseline: &Value, limits: &BTreeMap<String, f64>, allow: bool) -> Value {
    let mut differences = Vec::<String>::new();
    let mut warnings = Vec::new();
    if current["python"].is_object() != baseline["python"].is_object() {
        differences.push("target type differs (Python versus native executable)".into());
    } else if current["python"].is_object() {
        if current["python"]["interpreter"] != baseline["python"]["interpreter"]
            || current["provenance"]["executable_sha256"]
                != baseline["provenance"]["executable_sha256"]
        {
            differences.push("Python interpreter differs".into());
        }
        if current["provenance"]["script_sha256"] != baseline["provenance"]["script_sha256"] {
            warnings.push("Python script contents changed; comparison includes that code change");
        }
    }
    if current["measurement_backend"] != baseline["measurement_backend"] {
        differences.push("measurement backend differs; regenerate the baseline for comparable launch and RSS accounting".into());
    }
    if current["profiles"]["flags"] != baseline["profiles"]["flags"] {
        differences.push("domain profile selection differs".into());
    }
    let mut invalid = false;
    for key in ["cpu_model", "kernel", "hostname"] {
        if !baseline["provenance"][key].is_null()
            && current["provenance"][key] != baseline["provenance"][key]
        {
            differences.push(format!("host {key} differs"));
        }
    }
    if baseline["provenance"].is_null() {
        warnings.push("Baseline lacks provenance; full compatibility could not be verified");
    }
    for key in ["input_sha256", "environment_sha256"] {
        if !baseline["provenance"].is_null()
            && current["provenance"][key] != baseline["provenance"][key]
        {
            differences.push(format!("{key} differs"));
        }
    }
    for key in ["work_units", "cwd", "cpu_affinity", "expected_exit"] {
        if current["settings"][key] != baseline["settings"][key] {
            differences.push(format!("setting {key} differs"));
        }
    }
    if current["settings"]["cooldown_ms"].as_u64().unwrap_or(0)
        != baseline["settings"]["cooldown_ms"].as_u64().unwrap_or(0)
    {
        differences.push("setting cooldown_ms differs".into());
    }
    if current["provenance"]["output_checks"] != baseline["provenance"]["output_checks"] {
        differences.push(
            "output expectations differ (older baselines may lack expectation hashes)".into(),
        );
    }
    if current["settings"]["validator"]["command"] != baseline["settings"]["validator"]["command"] {
        differences.push("validator command differs".into());
    }
    if current["settings"]["check_output_stability"]
        .as_bool()
        .unwrap_or(false)
        != baseline["settings"]["check_output_stability"]
            .as_bool()
            .unwrap_or(false)
    {
        differences.push("setting check_output_stability differs".into());
    }
    if current["command"].as_array().map(|a| a.get(1..))
        != baseline["command"].as_array().map(|a| a.get(1..))
    {
        differences.push("target arguments differ".into());
    }
    if baseline["completed"] == false
        || baseline["measurement_passed"] == false
        || baseline["successful_runs"].as_u64().unwrap_or(0) == 0
    {
        invalid = true;
        differences.push("baseline measurements are incomplete or failed".into());
    }
    if current["completed"] != true || current["measurement_passed"] != true {
        invalid = true;
        differences.push("current measurements are incomplete or failed".into());
    }
    let mut names: Vec<String> = [
        "wall",
        "rss",
        "cpu",
        "throughput",
        "allocations",
        "heap_peak",
        "leaked_bytes",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    // Validator metrics are compared when both sides have them; thresholds may add a direction.
    let mut directions = BTreeMap::new();
    for key in limits.keys() {
        let name = match validator_key(key) {
            Some((metric, direction)) => {
                if let Some(higher) = direction {
                    directions.insert(format!("validator.{metric}"), higher);
                }
                format!("validator.{metric}")
            }
            None => key.clone(),
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for metric in current["validation_metrics"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
    {
        let name = format!("validator.{metric}");
        if baseline["validation_metrics"][metric].is_object() && !names.contains(&name) {
            names.push(name);
        }
    }
    let limit_for = |name: &str| {
        limits
            .iter()
            .find(|(key, _)| {
                key.as_str() == name
                    || validator_key(key).is_some_and(|(m, _)| format!("validator.{m}") == name)
            })
            .map(|(_, limit)| *limit)
    };
    let mut changes = Vec::new();
    let mut missing = Vec::new();
    for name in names {
        let limit = limit_for(&name);
        let (Some(before), Some(after)) = (metric(baseline, &name), metric(current, &name)) else {
            if limit.is_some() {
                missing.push(name);
            }
            continue;
        };
        let change = if before == 0. {
            if after == 0. {
                Some(0.)
            } else {
                None
            }
        } else {
            Some((after - before) / before.abs() * 100.)
        };
        // Built-in metrics have fixed directions; validator metrics need a declared one.
        let direction = if let Some((metric, _)) = validator_key(&name) {
            let declared = |r: &Value| match r["validation_metrics"][metric]["better"].as_str() {
                Some("higher") => Some(true),
                Some("lower") => Some(false),
                _ => None,
            };
            if let (Some(a), Some(b)) = (declared(current), declared(baseline)) {
                if a != b {
                    differences.push(format!("{name} declared direction differs"));
                }
            }
            directions
                .get(&name)
                .copied()
                .or_else(|| declared(current))
                .or_else(|| declared(baseline))
        } else {
            Some(name == "throughput")
        };
        if direction.is_none() && limit.is_some() {
            invalid = true;
            differences.push(format!(
                "{name} has no declared direction; use --regression {name}:higher=PCT or :lower=PCT, or declare \"better\" in the validator"
            ));
        }
        let higher_better = direction.unwrap_or(false);
        if crate::profiles::metric(&name).is_some()
            && baseline["profiles"]["metrics"][&name]["source"]
                != current["profiles"]["metrics"][&name]["source"]
        {
            differences.push(format!("{name} measurement source differs"));
        }
        let worsening = direction.and(change.map(|p| if higher_better { -p } else { p }));
        let regression = direction.is_some()
            && limit.is_some_and(|limit| {
                worsening
                    .map(|p| p > limit)
                    .unwrap_or(!higher_better && after > 0.)
            });
        let (now, before_samples) = (samples(current, &name), samples(baseline, &name));
        let rank_test = if now.is_empty() || before_samples.is_empty() {
            Value::Null
        } else {
            crate::stats::mann_whitney(&now, &before_samples)
        };
        changes.push(json!({"metric":name,"baseline":before,"current":after,"change_percent":change,
            "worsening_percent":worsening,"higher_is_better":direction,"threshold_percent":limit,"regression":regression,
            "rank_test":rank_test,
            "statistic":if crate::profiles::metric(&name).is_some() {"p95 samples from a single diagnostic replay"}else if name=="rss" {"maximum peak RSS"}else if ["allocations","heap_peak","leaked_bytes"].contains(&name.as_str()) {"single diagnostic replay"}else{"median"}}));
    }
    for name in missing {
        invalid = true;
        differences.push(format!("requested threshold metric {name} is unavailable"));
    }
    let incompatible = invalid || (!differences.is_empty() && !allow);
    let regressed = changes.iter().any(|c| c["regression"] == true);
    json!({"status":if incompatible {"incompatible"}else if regressed {"regression"}else{"ok"},
        "compatible":differences.is_empty(),"allow_mismatch":allow,"differences":differences,"warnings":warnings,"changes":changes})
}
pub fn apply(current: &mut Value, baseline: &Value, limits: &BTreeMap<String, f64>, allow: bool) {
    if let Some(cases) = current["cases"].as_array_mut() {
        let removed: Vec<_> = baseline["cases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|old| !cases.iter().any(|case| case["name"] == old["name"]))
            .map(|old| old["name"].clone())
            .collect();
        let mut compared = Vec::new();
        for case in cases {
            let old = baseline["cases"]
                .as_array()
                .and_then(|v| v.iter().find(|b| b["name"] == case["name"]));
            case["comparison"] = if let Some(old) = old {
                single(case, old, limits, allow)
            } else {
                json!({"status":"incompatible","differences":["case missing from baseline"],"changes":[]})
            };
            compared.push(case["comparison"]["status"].clone());
        }
        current["comparison"] = json!({"status":if !removed.is_empty() || compared.iter().any(|s|s=="incompatible") {"incompatible"}else if compared.iter().any(|s|s=="regression") {"regression"}else{"ok"}, "missing_current_cases":removed});
    } else {
        current["comparison"] = single(current, baseline, limits, allow);
    }
}
pub fn exit_code(report: &Value) -> i32 {
    let signal = report["interrupt_signal"].as_i64().unwrap_or(0);
    if signal != 0 {
        return 128 + signal as i32;
    }
    if let Some(cases) = report["cases"].as_array() {
        if report["completed"] != true
            || cases.iter().any(|r| {
                r["measurement_passed"] != true
                    || r["diagnostics"]["failed"] == true
                    || r["profiles"]["failed"] == true
                    || r["python_overhead"]["failed"] == true
            })
        {
            return 1;
        }
    } else if report["completed"] != true
        || report["measurement_passed"] != true
        || report["diagnostics"]["failed"] == true
        || report["profiles"]["failed"] == true
        || report["python_overhead"]["failed"] == true
    {
        return 1;
    }
    match report["comparison"]["status"].as_str() {
        Some("incompatible") => 2,
        Some("regression") => 3,
        _ => 0,
    }
}

/// Advisory thresholds share compatibility/missing-data rules but do not cause exit 3.
pub(crate) fn apply_warnings(
    current: &mut Value,
    baseline: &Value,
    limits: &BTreeMap<String, f64>,
    allow: bool,
) {
    let mut advisory = current.clone();
    apply(&mut advisory, baseline, limits, allow);
    fn attach(current: &mut Value, advisory: &Value) {
        current["comparison"]["advisory"] = advisory["comparison"].clone();
        if advisory["comparison"]["status"] == "incompatible" {
            current["comparison"]["status"] = json!("incompatible");
        }
        if let Some(cases) = current["cases"].as_array_mut() {
            for (index, case) in cases.iter_mut().enumerate() {
                attach(case, &advisory["cases"][index]);
            }
        }
    }
    attach(current, &advisory);
}
#[cfg(test)]
mod tests {
    use super::*;
    fn report(wall: f64, rss: f64) -> Value {
        json!({"command":["./prog"],"completed":true,"measurement_passed":true,"successful_runs":10,"summary":{"wall":{"median":wall},"rss":{"max":rss}},"settings":{}})
    }
    #[test]
    fn thresholds_direction_zero_and_missing() {
        let limits = thresholds(&["wall=5".into(), "rss=10".into()]).unwrap();
        let result = single(&report(1.2, 90.), &report(1., 100.), &limits, false);
        assert_eq!(result["status"], "regression");
        assert_eq!(result["changes"][1]["regression"], false);
        assert_eq!(
            single(&report(1., 100.), &report(0., 100.), &limits, false)["status"],
            "regression"
        );
        assert!(thresholds(&["wall=NaN".into()]).is_err());
        let mut current = report(1., 100.);
        current["settings"]["work_units"] = json!(5);
        assert_eq!(
            single(&current, &report(1., 100.), &limits, false)["status"],
            "incompatible"
        );
    }
    #[test]
    fn changed_measurement_backend_requires_a_new_baseline_or_explicit_override() {
        let old = report(1., 100.);
        let mut current = old.clone();
        current["measurement_backend"] = json!(crate::runner::BACKEND);
        assert_eq!(
            single(&current, &old, &BTreeMap::new(), false)["status"],
            "incompatible"
        );
        assert_ne!(
            single(&current, &old, &BTreeMap::new(), true)["status"],
            "incompatible"
        );
        assert_ne!(
            single(&current, &current, &BTreeMap::new(), false)["status"],
            "incompatible"
        );
    }
    #[test]
    fn validator_thresholds_need_a_direction() {
        let with = |median: f64, better: Value| {
            let mut r = report(1., 100.);
            r["validation_metrics"] = json!({"accuracy":{"median":median,"better":better}});
            r
        };
        let declared = thresholds(&["validator.accuracy=1".into()]).unwrap();
        let result = single(
            &with(0.9, json!("higher")),
            &with(0.95, json!("higher")),
            &declared,
            false,
        );
        assert_eq!(result["status"], "regression");
        let change = &result["changes"][2];
        assert_eq!(change["metric"], "validator.accuracy");
        assert_eq!(change["higher_is_better"], true);
        // Undeclared: informational without a threshold, incompatible with one.
        let undeclared = single(
            &with(0.9, Value::Null),
            &with(0.95, Value::Null),
            &BTreeMap::new(),
            false,
        );
        assert_eq!(undeclared["status"], "ok");
        assert!(undeclared["changes"][2]["higher_is_better"].is_null());
        assert_eq!(
            single(
                &with(0.9, Value::Null),
                &with(0.95, Value::Null),
                &declared,
                false
            )["status"],
            "incompatible"
        );
        // The flag's direction wins; lower-is-better makes the drop an improvement.
        let lower = thresholds(&["validator.accuracy:lower=1".into()]).unwrap();
        assert_eq!(
            single(
                &with(0.9, Value::Null),
                &with(0.95, Value::Null),
                &lower,
                false
            )["status"],
            "ok"
        );
        assert!(thresholds(&["validator.x:up=1".into()]).is_err());
        assert!(thresholds(&["validator.=1".into()]).is_err());
    }
    #[test]
    fn changes_include_rank_tests_from_run_samples() {
        let with_runs = |walls: &[f64]| {
            let mut r = report(walls[walls.len() / 2], 100.);
            r["runs"] = walls
                .iter()
                .map(|w| json!({"status":"ok","metrics":{"wall":w}}))
                .collect();
            r
        };
        let result = single(
            &with_runs(&[2., 2.1, 2.2, 2.3, 2.4]),
            &with_runs(&[1., 1.1, 1.2, 1.3, 1.4]),
            &BTreeMap::new(),
            false,
        );
        let test = &result["changes"][0]["rank_test"];
        assert_eq!(test["significant_at_0_05"], true);
        assert_eq!(test["probability_current_lower"], 0.0);
    }
    #[test]
    fn changed_suite_membership_is_incompatible() {
        let mut current = json!({"cases":[{"name":"one"}]});
        let baseline = json!({"cases":[{"name":"one"},{"name":"two"}]});
        apply(&mut current, &baseline, &BTreeMap::new(), true);
        assert_eq!(current["comparison"]["status"], "incompatible");
        assert_eq!(
            current["comparison"]["missing_current_cases"],
            json!(["two"])
        );
    }
}
