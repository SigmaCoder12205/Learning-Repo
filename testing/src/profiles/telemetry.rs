//! Bounded application-telemetry validation and its isolated target replay.
use super::{catalog::MetricKind, metric};
use crate::{cli::Cli, diagnostics, runner::RunOptions};
use serde_json::{json, Value};
use std::{ffi::OsString, path::Path};

fn parse_telemetry(text: &str, frame_budget: f64, deadline: Option<f64>) -> Result<Value, String> {
    let doc: Value =
        serde_json::from_str(text).map_err(|e| format!("invalid profile telemetry JSON: {e}"))?;
    if doc["version"] != 1 {
        return Err("profile telemetry version must be 1".into());
    }
    if doc.as_object().is_none_or(|m| {
        m.keys()
            .any(|k| !["version", "metrics"].contains(&k.as_str()))
    }) {
        return Err("unknown telemetry document field".into());
    }
    let mut output = serde_json::Map::new();
    let mut total_samples = 0;
    for (key, entry) in doc["metrics"]
        .as_object()
        .ok_or("telemetry metrics must be an object")?
    {
        let spec = metric(key).ok_or_else(|| format!("unknown profile metric {key}"))?;
        let fields = entry.as_object().ok_or("metric must be an object")?;
        if fields
            .keys()
            .any(|k| !["unit", "value", "samples"].contains(&k.as_str()))
        {
            return Err(format!("unknown field in {key}"));
        }
        if entry["unit"] != spec.unit {
            return Err(format!("{key} requires unit {}", spec.unit));
        }
        let mut values = if spec.kind == MetricKind::Samples {
            if fields.contains_key("value") {
                return Err(format!("{key} requires samples, not value"));
            }
            entry["samples"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 100_000)
                .ok_or_else(|| format!("{key} needs 1..100000 samples"))?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| format!("{key} samples must be numbers"))
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            if fields.contains_key("samples") {
                return Err(format!("{key} requires a scalar value"));
            }
            vec![entry["value"]
                .as_f64()
                .ok_or_else(|| format!("{key} needs a numeric value"))?]
        };
        total_samples += values.len();
        if total_samples > 200_000 {
            return Err("telemetry exceeds 200000 total samples".into());
        }
        if values.iter().any(|v| {
            !v.is_finite()
                || *v < 0.
                || ((spec.unit == "count" || spec.unit == "B")
                    && (v.fract() != 0. || *v > 9_007_199_254_740_991.))
        }) {
            return Err(format!(
                "{key} needs finite nonnegative values; counts/bytes must be exact integers"
            ));
        }
        let sum: f64 = values.iter().sum();
        if !sum.is_finite() {
            return Err(format!("{key} total overflows"));
        }
        values.sort_by(f64::total_cmp);
        let mut summary = crate::stats::summarize_basic(values.clone());
        if summary["stdev"].as_f64().is_none() {
            return Err(format!("{key} values overflow variance calculation"));
        }
        summary["p99"] = json!(crate::stats::percentile(&values, 0.99));
        let mut result = json!({"status":"ok","source":"application telemetry; separate replay","unit":spec.unit,"kind":spec.kind.as_str(),"summary":summary});
        if spec.kind != MetricKind::Samples {
            result["value"] = json!(values[0]);
        }
        if key == "gamedev.frame_ms" || key == "mobile.frame_ms" {
            result["fps_from_mean_frame_interval"] = if sum > 0. {
                json!(1000. * values.len() as f64 / sum)
            } else {
                Value::Null
            };
            result["budget_ms"] = json!(frame_budget);
            result["over_budget_count"] =
                json!(values.iter().filter(|&&v| v > frame_budget).count());
        }
        if let Some(budget) =
            deadline.filter(|_| key == "embedded.cycle_ms" || key == "embedded.control_ms")
        {
            result["budget_ms"] = json!(budget);
            result["over_budget_count"] = json!(values.iter().filter(|&&v| v > budget).count());
        }
        output.insert(key.clone(), result);
    }
    Ok(Value::Object(output))
}

pub(crate) fn run(
    args: &Cli,
    target: &[OsString],
    opts: &RunOptions<'_>,
    dir: &Path,
) -> Result<Value, String> {
    let path = dir.join("profile-telemetry.json");
    let mut env = opts.env.clone();
    env.insert(
        "CPPBENCH_TELEMETRY_FILE".into(),
        path.clone().into_os_string(),
    );
    env.insert(
        "CPPBENCH_PROFILE_FLAGS".into(),
        args.profile_flags.join(",").into(),
    );
    let opts = RunOptions { env: &env, ..*opts };
    let row = diagnostics::command_run(target, &opts, dir, "profile-telemetry")?;
    if let Some(failure) = diagnostics::invocation_failed(&row) {
        return Ok(failure);
    }
    if !path.exists() {
        return Ok(
            json!({"status":"unavailable","reason":"target did not write CPPBENCH_TELEMETRY_FILE; application metrics need instrumentation or a test driver","invocation":row}),
        );
    }
    if !std::fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Ok(
            json!({"status":"failed","reason":"telemetry must be a regular file, not a link or special device","invocation":row}),
        );
    }
    let parsed = diagnostics::read_report(&path)
        .and_then(|s| parse_telemetry(&s, args.frame_budget_ms, args.deadline_ms));
    Ok(match parsed {
        Ok(metrics) => json!({"status":"ok","metrics":metrics,"raw_file":path,"invocation":row}),
        Err(reason) => json!({"status":"failed","reason":reason,"raw_file":path,"invocation":row}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_and_deadlines_use_application_samples() {
        let text = r#"{"version":1,"metrics":{"gamedev.frame_ms":{"unit":"ms","samples":[10,10,20,40]},"embedded.cycle_ms":{"unit":"ms","samples":[1,2,3]}}}"#;
        let r = parse_telemetry(text, 16., Some(2.)).unwrap();
        assert_eq!(r["gamedev.frame_ms"]["fps_from_mean_frame_interval"], 50.);
        assert_eq!(r["gamedev.frame_ms"]["over_budget_count"], 2);
        assert_eq!(r["embedded.cycle_ms"]["over_budget_count"], 1);
        assert_eq!(r["gamedev.frame_ms"]["summary"]["count"], 4);
        assert!(r["gamedev.frame_ms"]["summary"]["p99"].as_f64().unwrap() > 39.);
    }
    #[test]
    fn bad_units_unknown_keys_and_malformed_values_are_errors() {
        for entry in [
            json!({"unit":"seconds","samples":[1]}),
            json!({"unit":"ms","samples":[]}),
            json!({"unit":"ms","samples":[-1]}),
            json!({"unit":"ms","samples":["1"]}),
            json!({"unit":"ms","samples":[1],"value":1}),
            json!({"unit":"ms","samples":[1e300,0.]}),
        ] {
            let doc = json!({"version":1,"metrics":{"backend.request_ms":entry}});
            assert!(
                parse_telemetry(&doc.to_string(), 16., None).is_err(),
                "{doc}"
            );
        }
        for key in ["backend.typo", "backend.requests"] {
            assert!(parse_telemetry(
                &json!({"version":1,"metrics":{key:{"unit":"count","value":1.5}}}).to_string(),
                16.,
                None
            )
            .is_err());
        }
    }
}
