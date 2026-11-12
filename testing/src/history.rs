//! Opt-in local report history and immutable named baseline snapshots.
use clap::{Args, Subcommand};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Args, Debug, Clone)]
pub(crate) struct Options {
    /// Override the local history directory
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    /// Emit machine-readable list/pin results (show always emits full JSON)
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand, Debug, Clone)]
enum Action {
    /// List newest saved runs (or named pins)
    List {
        #[arg(long, default_value = "20", value_parser = clap::value_parser!(u32).range(1..=1000))]
        limit: u32,
        #[arg(long)]
        pins: bool,
    },
    /// Print a complete saved report as JSON
    Show {
        id: String,
        #[arg(long)]
        pin: bool,
    },
    /// Copy a successful saved run into a new named baseline; existing pins are never replaced
    Pin { id: String, name: String },
}

pub(crate) fn location(path: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = path {
        return std::path::absolute(path).map_err(|e| e.to_string());
    }
    if let Some(root) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Ok(root.join("bench/history"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|p| p.join(".local/share/bench/history"))
        .ok_or_else(|| "set HOME/XDG_DATA_HOME or pass a history directory".into())
}

fn private(metadata: &fs::Metadata, directory: bool) -> Result<(), String> {
    // SAFETY: geteuid has no arguments or memory safety preconditions.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || (directory && !metadata.is_dir())
        || (!directory && (!metadata.is_file() || metadata.nlink() != 1))
    {
        return Err("history requires private, user-owned directories and regular files; symlinks/hard links are not accepted".into());
    }
    Ok(())
}

fn directory(path: &Path, create: bool) -> Result<(), String> {
    if create {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(|e| format!("history {}: {e}", path.display()))?;
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => private(&metadata, true),
        Err(error) if !create && error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn area(root: &Path, pins: bool, create: bool) -> Result<PathBuf, String> {
    directory(root, create)?;
    let path = root.join(if pins { "pins" } else { "runs" });
    directory(&path, create)?;
    Ok(path)
}

pub(crate) fn prepare(root: &Path) -> Result<(), String> {
    area(root, false, true).map(|_| ())
}

fn valid_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 100
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(
            "history IDs and pin names need 1..100 ASCII letters, digits, '-' or '_'".into(),
        );
    }
    Ok(())
}

pub(crate) fn read(root: &Path, name: &str, pin: bool) -> Result<Value, String> {
    valid_name(name)?;
    let path = area(root, pin, false)?.join(format!("{name}.json"));
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|e| format!("history {}: {e}", path.display()))?;
    private(&file.metadata().map_err(|e| e.to_string())?, false)?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("history report exceeds 64 MiB".into());
    }
    let report: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid history report: {e}"))?;
    if !report["summary"].is_object() && !report["cases"].is_array() {
        return Err("history entry is not a bench report".into());
    }
    Ok(report)
}

fn write_new(path: &Path, report: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("history report exceeds 64 MiB".into());
    }
    let parent = path.parent().ok_or("history path lacks parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist_noclobber(path)
        .map_err(|e| format!("cannot create history entry (existing pins are immutable): {e}"))?;
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}

pub(crate) fn record(root: &Path, report: &mut Value) -> Result<String, String> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    use rand_core::RngCore;
    let mut random = [0u8; 8];
    rand_core::OsRng
        .try_fill_bytes(&mut random)
        .map_err(|e| e.to_string())?;
    let id = format!(
        "run-{:020}-{:016x}",
        time.as_nanos(),
        u64::from_le_bytes(random)
    );
    report["history"] = json!({"id": id, "recorded_unix_seconds": time.as_secs()});
    let path = area(root, false, true)?.join(format!("{id}.json"));
    write_new(&path, report)?;
    Ok(id)
}

fn eligible(report: &Value) -> bool {
    report["completed"] == true
        && if let Some(cases) = report["cases"].as_array() {
            !cases.is_empty() && cases.iter().all(eligible)
        } else {
            report["measurement_passed"] == true
                && report["successful_runs"].as_u64().unwrap_or(0) > 0
                && report["diagnostics"]["failed"] != true
                && report["profiles"]["failed"] != true
                && report["python_overhead"]["failed"] != true
        }
}

pub(crate) fn run(options: Options) -> Result<i32, String> {
    let root = location(options.dir.as_deref())?;
    match options.action {
        Action::Show { id, pin } => println!(
            "{}",
            serde_json::to_string_pretty(&read(&root, &id, pin)?).unwrap()
        ),
        Action::Pin { id, name } => {
            valid_name(&name)?;
            let mut report = read(&root, &id, false)?;
            if !eligible(&report) {
                return Err(
                    "only completed, successful measurements and diagnostics can be pinned".into(),
                );
            }
            report["baseline_pin"] = json!({"name":name,"source_run":id});
            write_new(
                &area(&root, true, true)?.join(format!("{name}.json")),
                &report,
            )?;
            if options.json {
                println!("{}", json!({"name":name,"source_run":id}));
            } else {
                println!("Pinned {id} as {name}. Compare with --compare-pin {name}.");
            }
        }
        Action::List { limit, pins } => {
            let directory = area(&root, pins, false)?;
            let mut names = Vec::new();
            match fs::read_dir(&directory) {
                Ok(entries) => {
                    for entry in entries {
                        let path = entry.map_err(|e| e.to_string())?.path();
                        if path.extension().is_some_and(|e| e == "json") {
                            names.push(
                                path.file_stem()
                                    .and_then(|s| s.to_str())
                                    .ok_or("invalid history filename")?
                                    .to_string(),
                            );
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
            names.sort();
            if !pins {
                names.reverse();
            }
            let entries = names.into_iter().take(limit as usize).map(|id| {
                let report = read(&root, &id, pins)?;
                Ok(json!({"id":id,"name":report["name"],"recorded_unix_seconds":report["history"]["recorded_unix_seconds"],
                    "completed":report["completed"],"exit_code":crate::comparison::exit_code(&report),
                    "successful_runs":report["successful_runs"],"wall_median_seconds":report["summary"]["wall"]["median"],
                    "case_count":report["cases"].as_array().map(Vec::len)}))
            }).collect::<Result<Vec<Value>, String>>()?;
            if options.json {
                println!("{}", serde_json::to_string_pretty(&entries).unwrap());
            } else if entries.is_empty() {
                println!("No saved {}.", if pins { "pins" } else { "runs" });
            } else {
                for entry in entries {
                    println!(
                        "{} | {} | exit {} | median wall {} s",
                        entry["id"].as_str().unwrap(),
                        entry["name"],
                        entry["exit_code"],
                        entry["wall_median_seconds"]
                    );
                }
            }
        }
    }
    Ok(0)
}
