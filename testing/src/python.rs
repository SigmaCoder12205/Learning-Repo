//! Python script preparation; execution still uses the same isolated native runner.
use crate::cli::Cli;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub(crate) struct Target {
    pub script: PathBuf,
    pub interpreter: PathBuf,
}
pub(crate) fn prepare(
    args: &Cli,
    env: &BTreeMap<OsString, OsString>,
) -> Result<Option<Target>, String> {
    let explicit_options = args.interpreter.is_some() || !args.python_arg.is_empty();
    let script = args.python.as_deref().or_else(|| {
        args.program
            .as_deref()
            .filter(|p| p.extension() == Some(OsStr::new("py")) || explicit_options)
    });
    let Some(script) = script else {
        if explicit_options {
            return Err("Python options require a script path or --python SCRIPT".into());
        }
        return Ok(None);
    };
    let script = script
        .canonicalize()
        .map_err(|e| format!("Python script {}: {e}", script.display()))?;
    if !script.is_file() {
        return Err(format!("Python script {} must be a file", script.display()));
    }
    let name = args.interpreter.as_deref().unwrap_or(Path::new("python3"));
    let candidates = if name.components().count() > 1 || name.is_absolute() {
        vec![name.to_path_buf()]
    } else {
        env.get(OsStr::new("PATH"))
            .map(|path| std::env::split_paths(path).map(|p| p.join(name)).collect())
            .unwrap_or_default()
    };
    for path in candidates {
        if std::fs::metadata(&path)
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        {
            // Canonicalizing the executable would collapse .venv/bin/python's symlink,
            // causing Python to miss the pyvenv.cfg beside the requested interpreter.
            let interpreter = if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(path)
            };
            return Ok(Some(Target {
                script,
                interpreter,
            }));
        }
    }
    Err(format!("Python interpreter not found or not executable: {}; install Python or pass --interpreter /path/to/python", name.display()))
}

pub(crate) fn environment(args: &Cli) -> Result<BTreeMap<OsString, OsString>, String> {
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    for assignment in &args.env {
        let (key, value) = assignment
            .split_once('=')
            .filter(|(key, _)| !key.is_empty())
            .ok_or("--env requires KEY=VALUE")?;
        env.insert(key.into(), value.into());
    }
    Ok(env)
}
