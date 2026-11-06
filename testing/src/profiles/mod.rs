//! Domain reports keep process accounting, traced syscalls and application telemetry separate.
pub mod catalog;
mod network;
mod presentation;
mod telemetry;
use crate::cli::Cli;
use catalog::{Metric, MetricKind, Profile, Topic, PROFILES};
use clap::{Arg, ArgAction, ArgMatches, Command, CommandFactory};
pub(crate) use presentation::render;
use serde_json::{json, Value};
use std::{collections::BTreeSet, fmt::Write};
pub(crate) use telemetry::run as collect_telemetry;

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
            json!({"key":m.key,"label":m.label,"unit":m.unit,"kind":m.kind.as_str()})).collect::<Vec<_>>()})).collect::<Vec<_>>()})).collect::<Vec<_>>())
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
                let _ = writeln!(out, "    {} [{}; {}]", m.key, m.unit, m.kind.as_str());
            }
        }
    }
    out.push_str("Application-specific metrics require CPPBENCH_TELEMETRY_FILE output; see docs/profiles.md.\n");
    out
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
                    json!({"status":"unavailable","unit":spec.unit,"kind":spec.kind.as_str(),"reason":tools["telemetry"]["reason"].as_str().unwrap_or("not collected; target instrumentation is required")})
                };
                value["label"] = json!(spec.label);
                if spec.kind == MetricKind::Counter && value["status"] == "ok" {
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
}
