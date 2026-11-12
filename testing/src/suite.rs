use crate::{benchmark::Outcome, cli::Cli, experiment, storage::Journal};
use clap::FromArgMatches;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

pub fn load(path: &Path) -> Result<Vec<Cli>, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let root = path.parent().unwrap();
    let doc: Value = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("invalid suite JSON: {e}"))?;
    let object = doc.as_object().ok_or("suite must be an object")?;
    if doc["version"] != 1 {
        return Err("suite version must be 1".into());
    }
    for key in object.keys() {
        if !["version", "defaults", "benchmarks"].contains(&key.as_str()) {
            return Err(format!("unknown suite field {key}"));
        }
    }
    let defaults = if doc["defaults"].is_null() {
        serde_json::Map::new()
    } else {
        doc["defaults"]
            .as_object()
            .ok_or("defaults must be an object")?
            .clone()
    };
    // Validate before merging so case overrides cannot hide malformed defaults.
    if let Some(env) = defaults.get("env") {
        let env = env.as_object().ok_or("defaults.env must be an object")?;
        if env.values().any(|value| !value.is_string()) {
            return Err("defaults.env values must be strings".into());
        }
    }
    let cases = doc["benchmarks"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or("suite needs a nonempty benchmarks array")?;
    let mut names = BTreeSet::new();
    let mut result = Vec::new();
    for case in cases {
        let case = case.as_object().ok_or("benchmark must be an object")?;
        let name = case
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or("every benchmark needs a name")?;
        if !names.insert(name.to_string()) {
            return Err(format!("duplicate benchmark name {name}"));
        }
        let mut values = defaults.clone();
        for (key, value) in case {
            if key == "env" {
                let mut env = values
                    .get("env")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                env.extend(value.as_object().ok_or("env must be an object")?.clone());
                values.insert(key.clone(), Value::Object(env));
            } else {
                values.insert(key.clone(), value.clone());
            }
        }
        values.entry("cwd").or_insert(json!(root));
        values
            .entry("artifacts_dir")
            .or_insert(json!(root.join("bench-artifacts")));
        let mut argv: Vec<OsString> = vec!["bench".into()];
        let mut target_args = Vec::new();
        for (key, value) in values {
            let flag = format!("--{}", key.replace('_', "-"));
            match key.as_str() {
                key if crate::profiles::flag_names().any(|f| f == key.replace('_', "-"))
                    || key == "require_profile_data" =>
                {
                    if value.as_bool().ok_or("profile flags must be booleans")? {
                        argv.push(flag.into());
                    }
                }
                "args" => {
                    target_args = value
                        .as_array()
                        .ok_or("args must be an array")?
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(OsString::from)
                                .ok_or("args must contain strings".to_string())
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "env" => {
                    for (key, value) in value.as_object().ok_or("env must be an object")? {
                        argv.push("--env".into());
                        argv.push(
                            format!(
                                "{key}={}",
                                value.as_str().ok_or("environment values must be strings")?
                            )
                            .into(),
                        );
                    }
                }
                "python_args" => {
                    for value in value.as_array().ok_or("python_args must be an array")? {
                        argv.push(
                            format!(
                                "--python-arg={}",
                                value.as_str().ok_or("python_args must contain strings")?
                            )
                            .into(),
                        );
                    }
                }
                "interpreter" => {
                    let path = PathBuf::from(value.as_str().ok_or("interpreter must be a string")?);
                    argv.push(flag.into());
                    argv.push(
                        if !path.is_absolute() && path.components().count() > 1 {
                            root.join(path)
                        } else {
                            path
                        }
                        .into_os_string(),
                    );
                }
                "executable" | "python" | "cwd" | "input" | "expect_output" | "artifacts_dir" => {
                    let p = PathBuf::from(value.as_str().ok_or("paths must be strings")?);
                    let p = if p.is_absolute() { p } else { root.join(p) };
                    argv.push(flag.into());
                    argv.push(p.into_os_string());
                }
                "python_overhead" | "adaptive" | "perf" | "gpu" | "heaptrack" | "valgrind"
                | "hotspots" | "all_tools" | "keep_going" => {
                    if value
                        .as_bool()
                        .ok_or("tool and mode flags must be booleans")?
                    {
                        argv.push(flag.into());
                    }
                }
                "cooldown_ms" | "overhead_runs" | "name" | "runs" | "warmup" | "timeout"
                | "output_limit" | "expect_exit" | "units" | "metrics" | "max_runs"
                | "precision" | "min_time" | "cpu_affinity" | "diagnostic_timeout"
                | "perf_events" | "gpu_interval_ms" | "gpu_device" => {
                    if !value.is_string() && !value.is_number() {
                        return Err(format!("invalid value for {key}"));
                    }
                    argv.push(flag.into());
                    argv.push(
                        value
                            .as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| value.to_string())
                            .into(),
                    );
                }
                "frame_budget_ms" | "deadline_ms" => {
                    argv.push(flag.into());
                    argv.push(
                        value
                            .as_f64()
                            .ok_or("profile budgets must be numbers")?
                            .to_string()
                            .into(),
                    );
                }
                _ => return Err(format!("unknown benchmark option {key}")),
            }
        }
        argv.push("--".into());
        argv.extend(target_args);
        let matches = crate::profiles::command()
            .try_get_matches_from(argv)
            .map_err(|e| format!("benchmark {name}: {e}"))?;
        let mut args = Cli::from_arg_matches(&matches).map_err(|e| e.to_string())?;
        args.profile_flags = crate::profiles::selection(&matches);
        experiment::validate(&args)?;
        crate::python::prepare(&args, &crate::python::environment(&args)?)?;
        let binary = args
            .executable
            .as_deref()
            .or(args.python.as_deref())
            .ok_or("suite benchmarks require executable or python")?;
        if !binary.is_file() {
            return Err(format!(
                "benchmark {name}: executable missing: {}",
                binary.display()
            ));
        }
        if !args.cwd.is_dir() {
            return Err(format!("benchmark {name}: cwd missing"));
        }
        for p in [&args.input, &args.expect_output].into_iter().flatten() {
            if !p.is_file() {
                return Err(format!(
                    "benchmark {name}: input file missing: {}",
                    p.display()
                ));
            }
        }
        crate::metrics::select_metrics(args.metrics.as_deref().unwrap_or("all"))?;
        result.push(args);
    }
    Ok(result)
}
pub fn execute(args: &Cli, journal: &mut Journal) -> Result<Value, String> {
    let plans = load(args.suite.as_deref().unwrap())?;
    let mut report = json!({"schema_version":3,"kind":"suite","tool_version":env!("CARGO_PKG_VERSION"),
        "suite_file":args.suite,"cases":[],"completed":false,"interrupt_signal":0});
    journal.emit(json!({"event":"suite_start","report":report}))?;
    for (index, mut plan) in plans.into_iter().enumerate() {
        plan.progress = args.progress;
        if args.progress {
            eprintln!(
                "bench: suite case {}: {}",
                index + 1,
                plan.name.as_deref().unwrap()
            );
        }
        let selected = crate::metrics::select_metrics(plan.metrics.as_deref().unwrap_or("all"))?;
        let result = match crate::benchmark::execute(&plan, &selected, journal) {
            Ok(Outcome::Report(value)) => value,
            Ok(Outcome::Interrupted(signal)) => {
                json!({"name":plan.name,"error":"compilation interrupted","interrupt_signal":signal,"completed":false,"measurement_passed":false,"runs":[],"warmups":[],"selected_metrics":[]})
            }
            Err(error) => {
                json!({"name":plan.name,"error":error,"completed":false,"measurement_passed":false,"runs":[],"warmups":[],"selected_metrics":[]})
            }
        };
        journal.emit(json!({"event":"report","report":result}))?;
        let signal = result["interrupt_signal"].as_i64().unwrap_or(0);
        report["cases"].as_array_mut().unwrap().push(result);
        if signal != 0 {
            report["interrupt_signal"] = json!(signal);
            return Ok(report);
        }
    }
    report["completed"] = json!(true);
    Ok(report)
}
pub fn render(report: &Value, format: &str, view: crate::report::View) -> String {
    if format == "json" {
        return serde_json::to_string_pretty(report).unwrap() + "\n";
    }
    if format == "html" {
        return crate::html::render(report);
    }
    let mut out = String::new();
    if format == "csv" {
        out.push_str("case,run,status,exit_code,metric,unit,value\n");
        let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        for case in report["cases"].as_array().unwrap() {
            for row in case["runs"].as_array().unwrap_or(&Vec::new()) {
                if let Some(metrics) = row["metrics"].as_object() {
                    for (name, value) in metrics {
                        let _ = writeln!(
                            out,
                            "{},{},{},{},{},{},{}",
                            quote(case["name"].as_str().unwrap_or("")),
                            row["run"],
                            row["status"].as_str().unwrap_or(""),
                            row["exit_code"],
                            quote(name),
                            quote(case["metric_units"][name].as_str().unwrap_or("")),
                            value
                        );
                    }
                }
            }
        }
        return out;
    }
    let _ = writeln!(
        out,
        "bench suite | {} cases | completed: {}",
        report["cases"].as_array().unwrap().len(),
        report["completed"]
    );
    if report["comparison"].is_object() {
        let _ = writeln!(out, "Comparison: {}", report["comparison"]);
    }
    for case in report["cases"].as_array().unwrap() {
        let _ = writeln!(out, "\nBenchmark: {}", case["name"]);
        if !case["error"].is_null() {
            let _ = writeln!(out, "Error: {}", case["error"]);
            continue;
        }
        let selected: Vec<_> = case["selected_metrics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| {
                crate::metrics::METRICS
                    .iter()
                    .position(|m| Some(m.0) == n.as_str())
            })
            .collect();
        out.push_str(&crate::report::render(
            case,
            "text",
            &selected,
            view,
            case["custom_metrics"] == true,
        ));
    }
    out
}
