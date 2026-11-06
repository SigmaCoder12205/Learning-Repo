//! Shared diagnostic process invocation, discovery and bounded artifact reads.
use crate::runner::{self, RunOptions};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::File,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::Path,
};
const PARSE_LIMIT: u64 = 16 * 1024 * 1024;

pub(crate) fn read_report(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > PARSE_LIMIT {
        return Err(format!(
            "{} exceeds the 16 MiB parsing limit; raw artifact retained",
            path.display()
        ));
    }
    let mut text = String::new();
    file.take(PARSE_LIMIT + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > PARSE_LIMIT {
        return Err("report grew beyond parsing limit".into());
    }
    Ok(text)
}
pub(crate) fn find_tool(
    name: &str,
    env: &BTreeMap<OsString, OsString>,
) -> Result<OsString, String> {
    if let Some(path) = env.get(OsStr::new("PATH")) {
        for dir in std::env::split_paths(path) {
            let candidate = dir.join(name);
            if let Ok(meta) = candidate.metadata() {
                if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                    return candidate
                        .canonicalize()
                        .map(|p| p.into_os_string())
                        .map_err(|e| e.to_string());
                }
            }
        }
    }
    Err(format!("{name} is not installed or not executable on PATH"))
}
pub(super) fn issue(status: &str, reason: impl AsRef<str>) -> Value {
    json!({"status": status, "reason": reason.as_ref()})
}
pub(crate) fn command_run(
    command: &[OsString],
    opts: &RunOptions<'_>,
    dir: &Path,
    label: &str,
) -> Result<Value, String> {
    let stdout = dir.join(format!("{label}.stdout"));
    let stderr = dir.join(format!("{label}.stderr"));
    let mut row = runner::run_logged(command, opts, &[0], Some((&stdout, &stderr)))?;
    row["command"] = json!(crate::runner::display_command(command));
    row["stdout_file"] = json!(stdout);
    row["stderr_file"] = json!(stderr);
    Ok(row)
}
pub(crate) fn invocation_failed(row: &Value) -> Option<Value> {
    if row["status"] == "ok" {
        return None;
    }
    let status = if row["status"] == "interrupted" {
        "interrupted"
    } else {
        "failed"
    };
    let mut value = issue(
        status,
        format!(
            "diagnostic invocation: {} (exit {}); see retained stderr or --advanced",
            row["status"].as_str().unwrap_or("failed"),
            row["exit_code"]
        ),
    );
    value["invocation"] = row.clone();
    value["interrupt_signal"] = row["interrupt_signal"].clone();
    Some(value)
}
