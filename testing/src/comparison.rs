use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

pub fn load(path: &Path) -> Result<Value, String> {
    let data = fs::read(path).map_err(|e| format!("baseline {}: {e}", path.display()))?;
    if data.len() > 64 * 1024 * 1024 {
        return Err("baseline exceeds 64 MiB".into());
    }
    let value: Value =
        serde_json::from_slice(&data).map_err(|e| format!("invalid baseline: {e}"))?;
    if !value["summary"].is_object() && !value["cases"].is_array() {
        return Err("baseline is not a cppbench report".into());
    }
    Ok(value)
}
pub fn thresholds(values: &[String]) -> Result<BTreeMap<String, f64>, String> {
    let mut result = BTreeMap::new();
    for arg in values {
        let (name, value) = arg
            .split_once('=')
            .ok_or("regression must be METRIC=PERCENT")?;
        if !crate::METRICS.iter().any(|m| m.0 == name)
            && !["allocations", "heap_peak", "leaked_bytes"].contains(&name)
        {
            return Err(format!("unknown regression metric: {name}"));
        }
        if ["cpu_percent", "stdout_bytes", "stderr_bytes"].contains(&name) {
            return Err(format!("{name} has no automatic better/worse direction"));
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
fn metric(report: &Value, name: &str) -> Option<f64> {
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
    for name in limits.keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let mut changes = Vec::new();
    let mut missing = Vec::new();
    for name in names {
        let (Some(before), Some(after)) = (metric(baseline, &name), metric(current, &name)) else {
            if limits.contains_key(&name) {
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
            Some((after - before) / before * 100.)
        };
        let higher_better = name == "throughput";
        let worsening = change.map(|p| if higher_better { -p } else { p });
        let regression = limits.get(&name).is_some_and(|limit| {
            worsening
                .map(|p| p > *limit)
                .unwrap_or(!higher_better && after > 0.)
        });
        changes.push(json!({"metric":name,"baseline":before,"current":after,"change_percent":change,
            "worsening_percent":worsening,"higher_is_better":higher_better,"threshold_percent":limits.get(&name),"regression":regression,
            "statistic":if name=="rss" {"maximum peak RSS"}else if ["allocations","heap_peak","leaked_bytes"].contains(&name.as_str()) {"single diagnostic replay"}else{"median"}}));
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
            || cases
                .iter()
                .any(|r| r["measurement_passed"] != true || r["diagnostics"]["failed"] == true)
        {
            return 1;
        }
    } else if report["completed"] != true
        || report["measurement_passed"] != true
        || report["diagnostics"]["failed"] == true
    {
        return 1;
    }
    match report["comparison"]["status"].as_str() {
        Some("incompatible") => 2,
        Some("regression") => 3,
        _ => 0,
    }
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
