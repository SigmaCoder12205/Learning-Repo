//! Application modes, validation, report destinations and exit codes.
use crate::{
    benchmark::{self, Outcome},
    cli::Cli,
    comparison, diagnostics, html,
    metrics::{select_metrics, METRICS},
    profiles,
    report::{self, View},
    storage, suite,
};
use serde_json::json;
use std::{
    io::Write as _,
    path::{Path, PathBuf},
};
pub(crate) fn run() -> Result<i32, String> {
    use clap::FromArgMatches;
    let command = profiles::command();
    let matches = command.clone().get_matches();
    let mut args = Cli::from_arg_matches(&matches).map_err(|e| e.to_string())?;
    if let Some(command) = args.command.take() {
        return match command {
            crate::cli::Command::Account(options) => crate::accounts::run(options),
            crate::cli::Command::History(options) => crate::history::run(options),
        };
    }
    args.profile_flags = profiles::selection(&matches);
    if args.list_profiles {
        let output = if args.format == "json" {
            serde_json::to_string_pretty(&profiles::catalog_json()).unwrap() + "\n"
        } else {
            profiles::catalog_text()
        };
        if let Some(path) = &args.output {
            std::fs::write(path, output).map_err(|e| e.to_string())?;
        } else {
            print!("{output}");
        }
        return Ok(0);
    }
    if args.list_metrics {
        for (name, unit, description) in METRICS {
            println!("{name:24} {unit:10} {description}");
        }
        println!("Groups: timing, memory, io, scheduling, all");
        return Ok(0);
    }
    if args.suite.is_some() {
        let allowed = [
            "suite",
            "format",
            "output",
            "progress",
            "checkpoint",
            "compare",
            "regression",
            "allow_mismatch",
            "save_baseline",
            "advanced",
            "intermediate",
            "view",
            "histogram",
            "record_history",
            "history_dir",
            "compare_pin",
            "warn_regression",
            "help",
            "version",
        ];
        for id in matches
            .ids()
            .filter(|id| command.get_arguments().any(|arg| arg.get_id() == *id))
        {
            if matches.value_source(id.as_str()) == Some(clap::parser::ValueSource::CommandLine)
                && !allowed.contains(&id.as_str())
            {
                return Err(format!(
                    "put --{} inside the suite's defaults or benchmark entry",
                    id.as_str().replace('_', "-")
                ));
            }
        }
    }
    if args.recover.is_some() || args.check_tools {
        let allowed = if args.check_tools {
            vec!["check_tools", "format", "output"]
        } else {
            vec![
                "recover",
                "format",
                "output",
                "compare",
                "regression",
                "allow_mismatch",
                "save_baseline",
                "advanced",
                "intermediate",
                "view",
                "histogram",
                "record_history",
                "history_dir",
                "compare_pin",
                "warn_regression",
            ]
        };
        for id in matches
            .ids()
            .filter(|id| command.get_arguments().any(|arg| arg.get_id() == *id))
        {
            if matches.value_source(id.as_str()) == Some(clap::parser::ValueSource::CommandLine)
                && !allowed.contains(&id.as_str())
            {
                return Err(format!(
                    "--{} does not apply to this mode",
                    id.as_str().replace('_', "-")
                ));
            }
        }
        if args.check_tools && !["text", "json"].contains(&args.format.as_str()) {
            return Err("--check-tools supports text or json output".into());
        }
    }
    // Output destinations must not clobber a journal or another requested report.
    let paths = [
        args.output.as_deref(),
        args.save_baseline.as_deref(),
        args.checkpoint.as_deref(),
    ];
    let canonical = |p: &Path| -> Result<PathBuf, String> {
        if p.exists() {
            return p.canonicalize().map_err(|e| e.to_string());
        }
        let parent = p
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok(parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(p.file_name().ok_or("output path needs a file name")?))
    };
    let mut destinations = Vec::new();
    for path in paths.into_iter().flatten() {
        let path = canonical(path)?;
        if destinations.contains(&path) {
            return Err("output, baseline, and checkpoint paths must differ".into());
        }
        if args
            .recover
            .as_deref()
            .is_some_and(|p| canonical(p).ok().as_ref() == Some(&path))
        {
            return Err("output must not overwrite the recovery journal".into());
        }
        destinations.push(path);
    }
    if args.history_dir.is_some() && !args.record_history && args.compare_pin.is_none() {
        return Err("--history-dir requires --record-history or --compare-pin".into());
    }
    let history_root = if args.record_history || args.compare_pin.is_some() {
        Some(crate::history::location(args.history_dir.as_deref())?)
    } else {
        None
    };
    let baseline = if let Some(pin) = &args.compare_pin {
        Some(crate::history::read(
            history_root.as_deref().unwrap(),
            pin,
            true,
        )?)
    } else {
        args.compare.as_deref().map(comparison::load).transpose()?
    };
    let limits = comparison::thresholds(&args.regression)?;
    let warning_limits = comparison::thresholds(&args.warn_regression)?;
    if args.record_history {
        crate::history::prepare(history_root.as_deref().unwrap())?;
    }
    let view = if args.advanced {
        View::Advanced
    } else if args.intermediate {
        View::Intermediate
    } else {
        args.view
    };
    let mut journal = storage::Journal::open(args.checkpoint.as_deref())?;
    let mut report = if let Some(path) = &args.recover {
        storage::recover(path)?
    } else if args.check_tools {
        diagnostics::check_tools()?
    } else if args.suite.is_some() {
        suite::execute(&args, &mut journal)?
    } else {
        let selected = select_metrics(args.metrics.as_deref().unwrap_or("all"))?;
        match benchmark::execute(&args, &selected, &mut journal)? {
            Outcome::Report(r) => r,
            Outcome::Interrupted(signal) => {
                eprintln!("bench: compilation interrupted");
                return Ok(128 + signal);
            }
        }
    };
    if let Some(baseline) = baseline {
        comparison::apply(&mut report, &baseline, &limits, args.allow_mismatch);
        if !warning_limits.is_empty() {
            comparison::apply_warnings(
                &mut report,
                &baseline,
                &warning_limits,
                args.allow_mismatch,
            );
        }
    }
    if args.histogram {
        crate::insights::enable_histogram(&mut report);
    }
    if args.record_history {
        let id = crate::history::record(history_root.as_deref().unwrap(), &mut report)?;
        eprintln!("bench: saved history {id}");
    }
    journal.emit(json!({"event":"final","report":report}))?;
    if let Some(path) = &args.save_baseline {
        if report["kind"] == "tool_check"
            || report["completed"] != true
            || report["cases"]
                .as_array()
                .map(|a| a.iter().any(|r| r["measurement_passed"] != true))
                .unwrap_or(report["measurement_passed"] != true)
        {
            return Err("cannot save an incomplete or failed measurement baseline".into());
        }
        storage::write_json(path, &report)?;
    }
    let output = if report["kind"] == "tool_check" {
        if args.format == "json" {
            serde_json::to_string_pretty(&report).unwrap() + "\n"
        } else {
            let mut out = String::from("bench tool compatibility check\n");
            for (name, value) in report["tools"].as_object().unwrap() {
                out.push_str(&format!(
                    "{name}: {} | {}\n",
                    value["status"].as_str().unwrap_or("unavailable"),
                    value["identity"]
                        .as_str()
                        .unwrap_or(value["reason"].as_str().unwrap_or(""))
                        .lines()
                        .next()
                        .unwrap_or("")
                ));
            }
            for (name, probe) in report["counter_probes"].as_object().unwrap() {
                let status = probe["status"].as_str().unwrap_or("unavailable");
                out.push_str(&format!("{name} counter access: {status}\n"));
                if status != "ok" {
                    out.push_str(probe["stderr"].as_str().unwrap_or(""));
                    out.push('\n');
                }
            }
            out
        }
    } else if report["cases"].is_array() {
        suite::render(&report, &args.format, view)
    } else if args.format == "html" {
        html::render(&report)
    } else {
        let selected = report["selected_metrics"]
            .as_array()
            .ok_or("report lacks metric selection")?
            .iter()
            .filter_map(|n| METRICS.iter().position(|m| Some(m.0) == n.as_str()))
            .collect::<Vec<_>>();
        report::render(
            &report,
            &args.format,
            &selected,
            view,
            report["custom_metrics"] == true,
        )
    };
    if let Some(path) = &args.output {
        std::fs::write(path, output).map_err(|e| e.to_string())?;
    } else {
        std::io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    if report["kind"] == "tool_check" {
        let signal = report["interrupt_signal"].as_i64().unwrap_or(0);
        if signal != 0 {
            return Ok(128 + signal as i32);
        }
        return Ok(
            if report["tools"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v["status"] != "ok")
                || report["counter_probes"]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|v| v["status"] != "ok")
            {
                1
            } else {
                0
            },
        );
    }
    Ok(comparison::exit_code(&report))
}
