//! Domain reports keep process accounting, traced syscalls and application telemetry separate.
pub mod catalog;
mod network;
use crate::{diagnostics, runner::RunOptions, Cli};
use catalog::{Metric, Profile, Topic, PROFILES};
use clap::{Arg, ArgAction, ArgMatches, Command, CommandFactory};
use serde_json::{json, Value};
use std::{collections::BTreeSet, ffi::OsString, fmt::Write, path::Path};

pub fn command() -> Command {
    let mut command = Cli::command();
    for profile in PROFILES {
        command = command.arg(
            Arg::new(profile.flag)
                .long(profile.flag)
                .help(profile.title)
                .help_heading("Domain profiles")
                .action(ArgAction::SetTrue),
        );
        for topic in profile.topics {
            command = command.arg(
                Arg::new(topic.flag)
                    .long(topic.flag)
                    .help(topic.title)
                    .help_heading(profile.title)
                    .requires(profile.flag)
                    .action(ArgAction::SetTrue),
            );
        }
    }
    command
}
pub fn flag_names() -> impl Iterator<Item = &'static str> {
    PROFILES
        .iter()
        .flat_map(|p| std::iter::once(p.flag).chain(p.topics.iter().map(|t| t.flag)))
}
pub fn selection(matches: &ArgMatches) -> Vec<String> {
    flag_names()
        .filter(|name| matches.get_flag(name))
        .map(str::to_string)
        .collect()
}
pub fn selected(flags: &[String]) -> Vec<(&'static Profile, Vec<&'static Topic>)> {
    PROFILES
        .iter()
        .filter(|p| flags.iter().any(|f| f == p.flag))
        .map(|p| {
            let focused: Vec<_> = p
                .topics
                .iter()
                .filter(|t| flags.iter().any(|f| f == t.flag))
                .collect();
            (
                p,
                if focused.is_empty() {
                    p.topics.iter().collect()
                } else {
                    focused
                },
            )
        })
        .collect()
}
pub fn includes(args: &Cli, topic: &str) -> bool {
    selected(&args.profile_flags)
        .iter()
        .any(|(_, topics)| topics.iter().any(|t| t.flag == topic))
}
pub fn metric(key: &str) -> Option<&'static Metric> {
    PROFILES
        .iter()
        .flat_map(|p| p.topics)
        .flat_map(|t| t.metrics)
        .find(|m| m.key == key)
}
pub fn catalog_json() -> Value {
    json!(PROFILES.iter().map(|p| json!({"flag":p.flag,"title":p.title,"topics":p.topics.iter().map(|t|
        json!({"flag":t.flag,"title":t.title,"guidance":t.guidance,"metrics":t.metrics.iter().map(|m|
            json!({"key":m.key,"label":m.label,"unit":m.unit,"kind":m.kind})).collect::<Vec<_>>()})).collect::<Vec<_>>()})).collect::<Vec<_>>())
}
pub fn catalog_text() -> String {
    let mut out = String::from(
        "Domain profiles: a main flag includes all three topics; subflags focus its report.\n",
    );
    for p in PROFILES {
        let _ = writeln!(out, "--{}: {}", p.flag, p.title);
        for t in p.topics {
            let _ = writeln!(out, "  --{}: {}", t.flag, t.title);
            for m in t.metrics {
                let _ = writeln!(out, "    {} [{}; {}]", m.key, m.unit, m.kind);
            }
        }
    }
    out.push_str("Application-specific metrics require CPPBENCH_TELEMETRY_FILE output; see docs/profiles.md.\n");
    out
}

pub fn parse_telemetry(
    text: &str,
    frame_budget: f64,
    deadline: Option<f64>,
) -> Result<Value, String> {
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
        let mut values = if spec.kind == "samples" {
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
        let mut summary = crate::summarize_basic(values.clone());
        if summary["stdev"].as_f64().is_none() {
            return Err(format!("{key} values overflow variance calculation"));
        }
        summary["p99"] = json!(crate::percentile(&values, 0.99));
        let mut result = json!({"status":"ok","source":"application telemetry; separate replay","unit":spec.unit,"kind":spec.kind,"summary":summary});
        if spec.kind != "samples" {
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

pub fn telemetry(
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
pub use network::run as trace_network;

pub fn build(args: &Cli, report: &Value) -> Value {
    let tools = &report["diagnostics"]["results"];
    let mut metrics = serde_json::Map::new();
    let mut sections = Vec::new();
    let mut missing = BTreeSet::new();
    for (p, topics) in selected(&args.profile_flags) {
        let mut topic_reports = Vec::new();
        for t in topics {
            let mut keys = Vec::new();
            for spec in t.metrics {
                let supplied = &tools["telemetry"]["metrics"][spec.key];
                let traced = &tools["network"]["metrics"][spec.key];
                let mut value = if supplied.is_object() {
                    supplied.clone()
                } else if traced.is_object() {
                    traced.clone()
                } else {
                    missing.insert(spec.key);
                    json!({"status":"unavailable","unit":spec.unit,"kind":spec.kind,"reason":tools["telemetry"]["reason"].as_str().unwrap_or("not collected; target instrumentation is required")})
                };
                value["label"] = json!(spec.label);
                if spec.kind == "counter" && value["status"] == "ok" {
                    let tool = if supplied.is_object() {
                        "telemetry"
                    } else {
                        "network"
                    };
                    if let (Some(count), Some(seconds)) = (
                        value["value"].as_f64(),
                        tools[tool]["invocation"]["metrics"]["wall"]
                            .as_f64()
                            .filter(|v| *v > 0.),
                    ) {
                        value["rate_per_replay_second"] = json!(count / seconds);
                    }
                }
                metrics.insert(spec.key.into(), value);
                keys.push(spec.key);
            }
            topic_reports.push(
                json!({"flag":t.flag,"title":t.title,"guidance":t.guidance,"metric_keys":keys}),
            );
        }
        let context: serde_json::Map<String, Value> = p
            .context
            .iter()
            .map(|key| ((*key).into(), report["summary"][*key].clone()))
            .collect();
        sections.push(
            json!({"flag":p.flag,"title":p.title,"topics":topic_reports,"process_context":context}),
        );
    }
    let failed = (args.require_profile_data && !missing.is_empty())
        || tools["telemetry"]["status"] == "failed"
        || tools["telemetry"]["status"] == "interrupted";
    json!({"status":if failed {"failed"}else if missing.is_empty(){"ok"}else{"partial"},"failed":failed,
        "flags":args.profile_flags,"sections":sections,"metrics":metrics,"missing_metrics":missing,
        "frame_budget_ms":args.frame_budget_ms,"deadline_ms":args.deadline_ms,
        "scope":"profile application samples and syscall tracing come from separate replays, excluded from baseline; missing metrics are not zero"})
}

pub fn render(report: &Value, detailed: bool) -> String {
    let profiles = &report["profiles"];
    let Some(sections) = profiles["sections"].as_array() else {
        return String::new();
    };
    let mut out = format!(
        "\nDomain profiles: {} (separate diagnostic measurements)\n",
        profiles["status"].as_str().unwrap_or("unavailable")
    );
    for section in sections {
        let _ = writeln!(out, "{}", section["title"].as_str().unwrap_or("Profile"));
        if let Some(context) = section["process_context"].as_object() {
            for (key, stats) in context {
                if let Some(mean) = stats["mean"].as_f64() {
                    let unit = crate::METRICS
                        .iter()
                        .find(|m| m.0 == key)
                        .map(|m| m.1)
                        .unwrap_or("");
                    let _ = writeln!(
                        out,
                        "  Process context {key}: {mean:.3} {unit} mean (baseline)"
                    );
                }
            }
        }
        for topic in section["topics"].as_array().into_iter().flatten() {
            let _ = writeln!(out, "  {}", topic["title"].as_str().unwrap_or("Topic"));
            for key in topic["metric_keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let m = &profiles["metrics"][key];
                let label = m["label"].as_str().unwrap_or(key);
                if m["status"] != "ok" {
                    let _ = writeln!(out, "    {label}: unavailable");
                    continue;
                }
                let unit = m["unit"].as_str().unwrap_or("");
                let source = m["source"].as_str().unwrap_or("unknown source");
                if let Some(value) = m["value"].as_f64() {
                    let digits = if ["count", "B"].contains(&unit) { 0 } else { 3 };
                    let _ = writeln!(out, "    {label}: {value:.digits$} {unit} ({source})");
                } else {
                    let s = &m["summary"];
                    let mean = s["mean"].as_f64().unwrap_or(0.);
                    let (scale, display_unit) = if unit == "ms" && mean < 0.001 {
                        (1_000_000., "ns")
                    } else if unit == "ms" && mean < 1. {
                        (1000., "µs")
                    } else {
                        (1., unit)
                    };
                    let _ = writeln!(out,"    {label}: mean {:.3}, p95 {:.3}, p99 {:.3}, max {:.3} {display_unit} (n={}; {source})",
                        mean*scale,s["p95"].as_f64().unwrap_or(0.)*scale,s["p99"].as_f64().unwrap_or(0.)*scale,s["max"].as_f64().unwrap_or(0.)*scale,s["count"]);
                }
                if let Some(fps) = m["fps_from_mean_frame_interval"].as_f64() {
                    let _ = writeln!(
                        out,
                        "      FPS from mean presented frame interval: {fps:.2}"
                    );
                }
                if m["over_budget_count"].is_number() {
                    let _ = writeln!(
                        out,
                        "      Samples over {} ms budget: {}",
                        m["budget_ms"], m["over_budget_count"]
                    );
                }
                if detailed {
                    let _ = writeln!(out, "      Metric key: {key}");
                    if let Some(rate) = m["rate_per_replay_second"].as_f64() {
                        let _ = writeln!(out,"      {rate:.3} {unit}/s over the entire instrumented replay (includes startup/tracing overhead)");
                    }
                }
            }
            let _ = writeln!(
                out,
                "    Note: {}",
                topic["guidance"].as_str().unwrap_or("")
            );
        }
    }
    if profiles["missing_metrics"]
        .as_array()
        .is_some_and(|m| !m.is_empty())
    {
        out.push_str("Unavailable application metrics require CPPBENCH_TELEMETRY_FILE output. Use --list-profiles and docs/profiles.md for keys and units.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_ten_profiles_have_three_topics_and_distinct_metrics() {
        command().debug_assert();
        assert_eq!(PROFILES.len(), 10);
        let mut flags = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for p in PROFILES {
            assert!(flags.insert(p.flag));
            assert_eq!(p.topics.len(), 3);
            let matches = command()
                .try_get_matches_from(["cppbench", &format!("--{}", p.flag)])
                .unwrap();
            assert_eq!(selected(&selection(&matches))[0].1.len(), 3);
            for topic in p.topics {
                assert!(flags.insert(topic.flag));
                assert_eq!(topic.metrics.len(), 3);
                let matches = command()
                    .try_get_matches_from([
                        "cppbench",
                        &format!("--{}", p.flag),
                        &format!("--{}", topic.flag),
                    ])
                    .unwrap();
                assert_eq!(selected(&selection(&matches))[0].1.len(), 1);
                assert!(command()
                    .try_get_matches_from(["cppbench", &format!("--{}", topic.flag)])
                    .is_err());
                for m in topic.metrics {
                    assert!(keys.insert(m.key), "duplicate {}", m.key);
                }
            }
        }
        assert_eq!(keys.len(), 90);
    }
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
