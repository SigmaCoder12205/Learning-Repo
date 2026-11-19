//! Bounded, non-executing input hints. These are heuristics, never a full input schema.
use crate::cli::Cli;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap, fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::PathBuf,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Hint {
    pub channel: String,
    pub position: Option<usize>,
    pub label: String,
    pub kind: String,
    pub suggestions: Vec<String>,
    pub evidence: String,
}

pub(crate) fn samples(kind: &str) -> Vec<String> {
    let values: &[&str] = match kind {
        "integer" => &["0", "1", "-1", "10"],
        "number" => &["0", "1.5", "-1.5", "1000"],
        "text" => &["hello", "", "two words"],
        "character" => &["a", "0", "?"],
        "boolean" => &["y", "n"],
        _ => &[],
    };
    values.iter().map(|s| (*s).into()).collect()
}

// Match identifier boundaries: print(...) is not int(...), message is not age.
fn token(text: &str, needle: &str) -> bool {
    let identifier = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(needle).any(|(index, _)| {
        !text[..index].chars().next_back().is_some_and(identifier)
            && (needle.ends_with('(')
                || !text[index + needle.len()..]
                    .chars()
                    .next()
                    .is_some_and(identifier))
    })
}

pub(crate) fn prompt(text: &str, position: usize) -> Hint {
    let lower = text.to_lowercase();
    let mut choices = Vec::new();
    for (start, end) in [('(', ')'), ('[', ']')] {
        for part in text.split(start).skip(1) {
            if let Some((inside, _)) = part.split_once(end) {
                let values: Vec<_> = inside.split([',', '/', '|']).map(str::trim).collect();
                if (2..=12).contains(&values.len())
                    && values
                        .iter()
                        .all(|s| !s.is_empty() && s.len() <= 16 && !s.contains(' '))
                {
                    choices.extend(values.into_iter().map(str::to_owned));
                }
            }
        }
    }
    // Common single-character operator menus include '/' as one of the choices.
    if lower.contains("operator") && text.contains('+') && text.contains('*') {
        choices = ["+", "-", "*", "/"]
            .iter()
            .filter(|s| text.contains(**s))
            .map(|s| (*s).into())
            .collect();
    }
    let kind = if !choices.is_empty() {
        "choice"
    } else if lower.contains("password") || lower.contains("secret") {
        "unknown"
    } else if lower.contains("y/n") || lower.contains("yes/no") {
        "boolean"
    } else if ["integer", "age", "count"].iter().any(|s| token(&lower, s)) {
        "integer"
    } else if ["number", "decimal", "float", "amount"]
        .iter()
        .any(|s| token(&lower, s))
    {
        "number"
    } else if ["name", "text", "word", "string"]
        .iter()
        .any(|s| token(&lower, s))
    {
        "text"
    } else {
        "unknown"
    };
    if choices.is_empty() {
        choices = if kind == "boolean" && lower.contains("yes/no") {
            vec!["yes".into(), "no".into()]
        } else {
            samples(kind)
        };
    }
    choices.dedup();
    Hint {
        channel: "stdin".into(),
        position: Some(position),
        label: text
            .chars()
            .rev()
            .take(200)
            .collect::<String>()
            .chars()
            .rev()
            .collect(),
        kind: kind.into(),
        suggestions: choices,
        evidence: "prompt wording (inferred; valid ranges are unknown)".into(),
    }
}

fn quoted(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut value = String::new();
    for c in text.chars() {
        if let Some(q) = quote {
            if escaped {
                value.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                result.push(std::mem::take(&mut value));
                quote = None;
            } else {
                value.push(c);
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        }
    }
    result
}

fn conversion(text: &str) -> &'static str {
    if ["float(", "double", "atof(", "strtod(", "stod("]
        .iter()
        .any(|s| token(text, s))
    {
        "number"
    } else if ["int(", "atoi(", "strtol(", "stoi("]
        .iter()
        .any(|s| token(text, s))
    {
        "integer"
    } else {
        "text"
    }
}

fn scan(source: &str, python: bool, args: &Cli) -> Vec<Hint> {
    let mut hints = Vec::new();
    let mut stdin_position = 0;
    let mut declarations = BTreeMap::new();
    let compact_source: String = source.chars().filter(|c| !c.is_whitespace()).collect();
    let switch_choices: Vec<String> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("case "))
        .flat_map(quoted)
        .filter(|s| s.chars().count() == 1)
        .collect();
    if !python {
        for line in source.lines() {
            let words: Vec<_> = line
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|s| !s.is_empty())
                .collect();
            for pair in words.windows(2) {
                let kind = match pair[0] {
                    "int" | "long" | "short" => "integer",
                    "float" | "double" => "number",
                    "char" => "character",
                    "string" => "text",
                    _ => continue,
                };
                declarations.insert(pair[1].to_owned(), kind);
            }
        }
    }
    for (line_index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        let evidence = format!(
            "source line {} (heuristic; branches/loops may change order)",
            line_index + 1
        );
        if python && token(&compact, "input(") {
            stdin_position += 1;
            let label = quoted(line)
                .first()
                .cloned()
                .unwrap_or_else(|| format!("answer {stdin_position}"));
            let mut hint = prompt(&label, stdin_position);
            let mut kind = conversion(&compact);
            if let Some((variable, _)) = compact.split_once('=') {
                if variable.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    if token(&compact_source, &format!("int({variable})")) {
                        kind = "integer";
                    } else if token(&compact_source, &format!("float({variable})")) {
                        kind = "number";
                    }
                }
            }
            if kind != "text" || hint.kind == "unknown" {
                hint.kind = kind.into();
                hint.suggestions = samples(kind);
            }
            hint.evidence = evidence.clone();
            hints.push(hint);
        }
        if !python {
            if let Some((_, tail)) = compact.split_once("cin>>") {
                for token in tail.split(">>") {
                    let name: String = token
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if name.is_empty() {
                        break;
                    }
                    stdin_position += 1;
                    let kind = declarations.get(&name).copied().unwrap_or("unknown");
                    hints.push(Hint {
                        channel: "stdin".into(),
                        position: Some(stdin_position),
                        label: name,
                        kind: kind.into(),
                        suggestions: samples(kind),
                        evidence: evidence.clone(),
                    });
                    if token.contains(';') {
                        break;
                    }
                }
            }
            if let Some((_, call)) = line
                .split_once("scanf")
                .filter(|_| token(&compact, "scanf("))
            {
                if let Some(format) = quoted(call).first() {
                    let mut chars = format.chars();
                    while let Some(c) = chars.next() {
                        if c != '%' {
                            continue;
                        }
                        let mut spec = chars.next().unwrap_or('%');
                        if spec == '%' {
                            continue;
                        }
                        let skip = spec == '*';
                        while spec.is_ascii_digit() || ['*', 'l', 'h', 'L', 'z'].contains(&spec) {
                            spec = chars.next().unwrap_or('%');
                        }
                        if skip {
                            continue;
                        }
                        let kind = match spec {
                            'd' | 'i' | 'u' | 'x' | 'o' => "integer",
                            'f' | 'g' | 'e' => "number",
                            's' => "text",
                            'c' => "character",
                            _ => continue,
                        };
                        stdin_position += 1;
                        hints.push(Hint {
                            channel: "stdin".into(),
                            position: Some(stdin_position),
                            label: format!("scanf %{spec}"),
                            kind: kind.into(),
                            suggestions: samples(kind),
                            evidence: evidence.clone(),
                        });
                    }
                }
            }
            if compact.contains("getline(") {
                stdin_position += 1;
                hints.push(Hint {
                    channel: "stdin".into(),
                    position: Some(stdin_position),
                    label: "text line".into(),
                    kind: "text".into(),
                    suggestions: samples("text"),
                    evidence: evidence.clone(),
                });
            }
        }
        // Direct argv indexing in Python/C/C++; this does not inspect dynamic indices.
        for tail in compact.split("argv[").skip(1) {
            if let Some((index, _)) = tail.split_once(']') {
                if let Ok(index) = index.parse::<usize>() {
                    if index > 0 {
                        let kind = conversion(&compact);
                        hints.push(Hint {
                            channel: "argument".into(),
                            position: Some(index),
                            label: format!("argv[{index}]"),
                            kind: kind.into(),
                            suggestions: samples(kind),
                            evidence: evidence.clone(),
                        });
                    }
                }
            }
        }
        if python && compact.contains(".add_argument(") {
            let strings = quoted(line);
            if let Some(flag) = strings.first() {
                let position = args
                    .args
                    .iter()
                    .position(|v| v == flag.as_str())
                    .filter(|i| i + 1 < args.args.len())
                    .map(|i| i + 2);
                let kind = if compact.contains("type=int") {
                    "integer"
                } else if compact.contains("type=float") {
                    "number"
                } else if compact.contains("store_true") || compact.contains("store_false") {
                    "flag"
                } else {
                    "text"
                };
                let mut choices = Vec::new();
                if let Some((_, tail)) = compact.split_once("choices=[") {
                    if let Some((list, _)) = tail.split_once(']') {
                        choices = list
                            .split(',')
                            .map(|s| s.trim_matches(['\'', '"']).to_owned())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                }
                hints.push(Hint {
                    channel: "argument".into(),
                    position,
                    label: flag.clone(),
                    kind: if choices.is_empty() {
                        kind.into()
                    } else {
                        "choice".into()
                    },
                    suggestions: if choices.is_empty() {
                        samples(kind)
                    } else {
                        choices
                    },
                    evidence: evidence.clone(),
                });
            }
        }
        if hints.len() >= 128 {
            hints.truncate(128);
            break;
        }
    }
    let mut unique = Vec::<Hint>::new();
    for hint in hints {
        if !unique.iter().any(|h| {
            h.channel == hint.channel && h.position == hint.position && h.label == hint.label
        }) {
            unique.push(hint);
        }
    }
    if !switch_choices.is_empty() {
        for hint in &mut unique {
            if hint.kind == "character" {
                hint.suggestions = switch_choices.clone();
                hint.evidence
                    .push_str("; possible choices from switch cases");
            }
        }
    }
    unique
}

pub(crate) fn source_paths(args: &Cli) -> Vec<PathBuf> {
    if let Some(path) = &args.input_source {
        return vec![path.clone()];
    }
    if !args.source.is_empty() {
        return args.source.clone();
    }
    let Some(path) = args
        .python
        .as_ref()
        .or(args.program.as_ref())
        .or(args.executable.as_ref())
    else {
        return vec![];
    };
    if args.python.is_some() || path.extension().is_some_and(|e| e == "py") {
        return vec![path.clone()];
    }
    for extension in ["cpp", "cc", "cxx", "c"] {
        let candidate = path.with_extension(extension);
        if candidate.is_file() {
            return vec![candidate];
        }
    }
    vec![]
}

pub(crate) fn discover(args: &Cli) -> Result<Value, String> {
    let paths = source_paths(args);
    let mut hints = Vec::new();
    for path in &paths {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| format!("input source {}: {e}", path.display()))?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("input source must be a regular file".into());
        }
        let mut text = String::new();
        file.take(262145)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 262144 {
            return Err(
                "input source exceeds 256 KiB; choose a smaller file containing the input handling"
                    .into(),
            );
        }
        let python = path.extension() == Some(std::ffi::OsStr::new("py")) || args.python.is_some();
        hints.extend(scan(&text, python, args));
    }
    Ok(
        json!({"sources":paths,"hints":hints,"status":if hints.is_empty(){"unknown"}else{"inferred"},
        "limitations":"Heuristic source/prompt hints, not exhaustive input discovery. Types, ranges, branches and valid combinations may require user confirmation. No target code was executed for discovery."}),
    )
}

pub(crate) fn hints(value: &Value) -> Vec<Hint> {
    serde_json::from_value(value["hints"].clone()).unwrap_or_default()
}

pub(crate) fn render(value: &Value) -> String {
    use std::fmt::Write;
    let mut out = String::from("Input hints (inferred, not a complete list of valid inputs)\n");
    let hints = hints(value);
    if hints.is_empty() {
        out.push_str("  No recognizable input declarations found. Interactive prompts may provide hints; use --input-source FILE for separate source.\n");
    }
    for hint in hints {
        let _ = writeln!(
            out,
            "  {} {} {:?}: {} | examples {} | {}",
            hint.channel,
            hint.position
                .map(|p| p.to_string())
                .unwrap_or_else(|| "option".into()),
            hint.label,
            hint.kind,
            json!(hint.suggestions),
            hint.evidence
        );
    }
    out
}
