//! Inline/recorded stdin and bounded input matrices, shared by native/Python targets.
use crate::{
    cli::Cli,
    input_hints::{self, Hint},
    interactive,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const LIMIT: usize = 1024 * 1024;
#[derive(Clone)]
struct Axis {
    argument: bool,
    index: usize,
    values: Vec<String>,
}

fn axes(args: &Cli) -> Result<Vec<Axis>, String> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (argument, items) in [(false, &args.vary_input), (true, &args.vary_arg)] {
        for item in items {
            let (index, values) = item
                .split_once('=')
                .ok_or("variation needs INDEX=[\"value\",\"value\"]")?;
            let index = index
                .parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or("variation index must be positive (1-based)")?
                - 1;
            if !seen.insert((argument, index)) {
                return Err("each input/argument position may be varied only once".into());
            }
            let values = if let Some(kind) = values.strip_prefix('@') {
                let values = input_hints::samples(kind);
                if values.is_empty() {
                    return Err(
                        "unknown preset; use @integer, @number, @text, @character or @boolean"
                            .into(),
                    );
                }
                values
            } else {
                serde_json::from_str::<Vec<String>>(values)
                    .map_err(|e| format!("variation values must be a JSON array of strings: {e}"))?
            };
            if values.is_empty() || values.len() > 256 {
                return Err("each variation needs 1..256 values".into());
            }
            let mut unique = Vec::new();
            for value in values {
                if value.len() > LIMIT
                    || (!argument && value.contains(['\n', '\r']))
                    || (argument && value.contains('\0'))
                {
                    return Err("variation values must fit 1 MiB; answers must be single lines and arguments cannot contain NUL".into());
                }
                if !unique.contains(&value) {
                    unique.push(value);
                }
            }
            result.push(Axis {
                argument,
                index,
                values: unique,
            });
        }
    }
    if result.len() > 64 {
        return Err("at most 64 varied positions are supported".into());
    }
    Ok(result)
}

pub(crate) fn validate(args: &Cli) -> Result<(), String> {
    if args.input_text.as_ref().is_some_and(|s| s.len() > LIMIT)
        || args.input_line.iter().map(|s| s.len() + 1).sum::<usize>() > LIMIT
    {
        return Err("inline stdin exceeds 1 MiB".into());
    }
    if args.input_line.iter().any(|s| s.contains(['\r', '\n'])) {
        return Err(
            "--input-line is one answer; repeat it for subsequent answers or use --input-text"
                .into(),
        );
    }
    let axes = axes(args)?;
    let mut count = 1usize;
    for axis in &axes {
        if axis.argument && axis.index >= args.args.len() {
            return Err("--vary-arg index exceeds the arguments supplied after --".into());
        }
        count = count
            .checked_mul(axis.values.len())
            .ok_or("input matrix is too large")?;
        if count > args.max_cases as usize {
            return Err(format!(
                "input matrix exceeds --max-cases {}",
                args.max_cases
            ));
        }
    }
    if args.interactive && !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Err(
            "--interactive needs a terminal on stdin; use --input-line or --input-text in scripts"
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn has_matrix(args: &Cli) -> bool {
    args.explore_inputs || !args.vary_input.is_empty() || !args.vary_arg.is_empty()
}

pub(crate) struct Prepared {
    pub file: Option<tempfile::NamedTempFile>,
    pub info: Value,
    pub trial: Value,
    pub plans: Option<Vec<Cli>>,
    pub discovery: Value,
}
impl Prepared {
    pub fn path(&self) -> Option<PathBuf> {
        self.file.as_ref().map(|f| f.path().to_owned())
    }
}

pub(crate) fn prepare(
    args: &Cli,
    command: &[OsString],
    env: &BTreeMap<OsString, OsString>,
    cwd: &Path,
) -> Result<Prepared, String> {
    validate(args)?;
    let discovery = match input_hints::discover(args) {
        Ok(value) => value,
        Err(error) if args.input_source.is_none() => {
            json!({"status":"unavailable","hints":[],"reason":error})
        }
        Err(error) => return Err(error),
    };
    let mut hints = input_hints::hints(&discovery);
    let mut trial = Value::Null;
    let mut source = if args.input.is_some() { "file" } else { "none" };
    let mut bytes = if let Some(text) = &args.input_text {
        source = "inline";
        Some(text.as_bytes().to_vec())
    } else if !args.input_line.is_empty() {
        source = "inline";
        Some((args.input_line.join("\n") + "\n").into_bytes())
    } else {
        None
    };
    let auto =
        !args.non_interactive && args.input.is_none() && bytes.is_none() && interactive::terminal();
    if args.interactive || auto {
        let (captured, result) = interactive::capture(args, command, env, cwd, &discovery)?;
        for hint in input_hints::hints(&result) {
            hints.retain(|old| !(old.channel == hint.channel && old.position == hint.position));
            hints.push(hint);
        }
        bytes = Some(captured);
        source = "interactive";
        trial = result;
    }
    if has_matrix(args) && bytes.is_none() {
        if let Some(path) = &args.input {
            let file = File::open(path).map_err(|e| e.to_string())?;
            let mut buffer = Vec::new();
            file.take((LIMIT + 1) as u64)
                .read_to_end(&mut buffer)
                .map_err(|e| e.to_string())?;
            if buffer.len() > LIMIT {
                return Err("input variation is limited to 1 MiB of stdin".into());
            }
            bytes = Some(buffer);
        }
    }
    let plans = if has_matrix(args) && (trial.is_null() || trial["status"] == "ok") {
        Some(generate(args, bytes.as_deref(), &hints)?)
    } else {
        None
    };
    let file = if args.input.is_none()
        && !(source == "interactive" && bytes.as_ref().is_some_and(Vec::is_empty))
    {
        if let Some(bytes) = &bytes {
            let mut file = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
            file.write_all(bytes).map_err(|e| e.to_string())?;
            Some(file)
        } else {
            None
        }
    } else {
        None
    };
    let info = json!({"source":source,"bytes":bytes.as_ref().map(Vec::len),"hints":hints,
        "scope":"Identical stdin bytes replayed afresh for every warmup, measurement and diagnostic"});
    Ok(Prepared {
        file,
        info,
        trial,
        plans,
        discovery,
    })
}

pub(crate) fn render(report: &Value) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let source = report["input_replay"]["source"].as_str().unwrap_or("none");
    if source != "none" {
        let _ = writeln!(
            out,
            "Input replay: {source}; identical bytes for every invocation."
        );
    }
    if report["input_setup"].is_object() {
        let _ = writeln!(
            out,
            "Unmeasured input trial: {}; captured {} bytes.",
            report["input_setup"]["status"], report["input_setup"]["captured_bytes"]
        );
        out.push_str(&input_hints::render(&report["input_setup"]));
        if report["input_setup"]["status"] != "ok" {
            out.push_str("Measurements did not start because input setup failed. Adjust answers, the quit command, or --interactive-timeout.\n");
        }
    }
    if report["input_case"].is_object() {
        let _ = writeln!(
            out,
            "Input case: {}\nAnswers: {}{}\nArguments: {}",
            report["input_case"]["label"],
            report["input_case"]["answers"],
            if report["input_case"]["answers_truncated"] == true {
                " (preview truncated)"
            } else {
                ""
            },
            report["input_case"]["arguments"]
        );
        if let Some(row) = report["runs"].as_array().and_then(|a| a.first()) {
            let _ = writeln!(
                out,
                "First measured output (escaped): {}{}",
                row["stdout_preview"],
                if row["stdout_preview_truncated"] == true {
                    " (truncated at 512 bytes)"
                } else {
                    ""
                }
            );
        }
    }
    out
}

#[derive(Clone)]
struct Case {
    lines: Vec<String>,
    arguments: Vec<OsString>,
    label: String,
}

fn generate(args: &Cli, bytes: Option<&[u8]>, hints: &[Hint]) -> Result<Vec<Cli>, String> {
    let text = bytes
        .map(|b| std::str::from_utf8(b).map_err(|_| "input variations require UTF-8 stdin"))
        .transpose()?;
    let mut lines = text
        .map(|t| {
            t.split_terminator('\n')
                .map(|s| s.strip_suffix('\r').unwrap_or(s).to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // split_terminator keeps a single empty answer for "\n"; an empty byte stream has none.
    if text == Some("") {
        lines.clear();
    }
    let base = Case {
        lines,
        arguments: args.args.clone(),
        label: "baseline".into(),
    };
    let mut cases = vec![base.clone()];
    if args.explore_inputs {
        if text.is_none() && hints.iter().any(|h| h.channel == "stdin") {
            return Err("stdin exploration needs example answers: run in a terminal or supply --input-line/--input-text/--input".into());
        }
        'outer: for hint in hints {
            let Some(index) = hint.position.and_then(|p| p.checked_sub(1)) else {
                continue;
            };
            let argument = hint.channel == "argument";
            let original = if argument {
                base.arguments.get(index).and_then(|a| a.to_str())
            } else {
                base.lines.get(index).map(String::as_str)
            };
            let Some(original) = original else { continue };
            if ["q", "quit", "exit"].contains(&original.to_ascii_lowercase().as_str()) {
                continue;
            }
            for value in &hint.suggestions {
                if value == original || value.contains(['\r', '\n', '\0']) {
                    continue;
                }
                if cases.len() >= args.max_cases as usize {
                    break 'outer;
                }
                let mut case = base.clone();
                if argument {
                    case.arguments[index] = value.into();
                } else {
                    case.lines[index] = value.clone();
                }
                case.label = format!("{} {} = {:?}", hint.channel, index + 1, value);
                if !cases
                    .iter()
                    .any(|old| old.lines == case.lines && old.arguments == case.arguments)
                {
                    cases.push(case);
                }
            }
        }
        if cases.len() == 1 {
            return Err("no usable input variations inferred; supply --vary-input '1=[\"value\",\"value\"]' or --vary-arg with known choices".into());
        }
    } else {
        for axis in axes(args)? {
            if !axis.argument && axis.index >= base.lines.len() {
                return Err("--vary-input index exceeds the recorded/supplied answer lines".into());
            }
            let mut expanded = Vec::new();
            for case in cases {
                for value in &axis.values {
                    let mut next = case.clone();
                    if axis.argument {
                        next.arguments[axis.index] = value.into();
                    } else {
                        next.lines[axis.index] = value.clone();
                    }
                    next.label = format!(
                        "{}; {} {} = {:?}",
                        case.label,
                        if axis.argument { "arg" } else { "answer" },
                        axis.index + 1,
                        value
                    );
                    expanded.push(next);
                }
            }
            cases = expanded;
        }
    }
    let mut total = 0usize;
    cases.into_iter().enumerate().map(|(index, case)| {
        let mut plan = args.clone();
        let replay = if case.lines != base.lines {
            let ending = if text.is_some_and(|t| t.ends_with('\n')) { "\n" } else { "" };
            Some(case.lines.join("\n") + ending)
        } else {
            text.map(str::to_owned)
        };
        total += replay.as_ref().map(String::len).unwrap_or(0);
        if total > 16 * LIMIT {
            return Err("generated input cases exceed 16 MiB total stdin".into());
        }
        plan.name = Some(format!("{} / input-{:03}", args.name.as_deref().unwrap_or("workload"), index + 1));
        plan.input = None;
        plan.input_line.clear();
        plan.input_text = replay;
        plan.interactive = false;
        plan.non_interactive = true;
        plan.explore_inputs = false;
        plan.vary_input.clear();
        plan.vary_arg.clear();
        plan.args = case.arguments;
        validate(&plan)?;
        plan.input_case = Some(json!({
            "index": index + 1,
            "label": case.label,
            "answers": case.lines.iter().take(64).map(|s| s.chars().take(256).collect::<String>()).collect::<Vec<_>>(),
            "answers_truncated": case.lines.len() > 64 || case.lines.iter().any(|s| s.chars().count() > 256),
            "arguments": crate::runner::display_command(&plan.args)
        }));
        Ok(plan)
    }).collect()
}
