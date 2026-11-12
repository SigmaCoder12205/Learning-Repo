//! Evidence-based runtime metadata and separate Python startup calibration.
use crate::{
    cli::Cli,
    python::Target,
    runner::{self, RunOptions},
    stats,
};
use serde_json::{json, Value};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

pub(crate) fn detect(binary: &Path, python: Option<&Target>) -> Value {
    if let Some(target) = python {
        return json!({"kind":"python","evidence":"Python workload selection", "interpreter":target.interpreter,
            "scope":"Full interpreter process; startup, imports, script execution and shutdown"});
    }
    let mut header = [0u8; 512];
    let length = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(binary)
        .and_then(|mut file| {
            if !file.metadata()?.is_file() {
                return Ok(0);
            }
            file.read(&mut header)
        })
        .unwrap_or(0);
    let header = &header[..length];
    if header.starts_with(b"\x7fELF") && header.len() >= 16 {
        json!({"kind":"elf","evidence":"ELF file header", "bits":match header[4] {1=>Some(32),2=>Some(64),_=>None},
            "scope":"Full executable process; source language and embedded runtimes are not inferred"})
    } else if header.starts_with(b"#!") {
        let line = header[2..]
            .split(|&b| b == b'\n')
            .next()
            .unwrap_or_default();
        json!({"kind":"script","evidence":"shebang", "declared_interpreter":String::from_utf8_lossy(line).trim(),
            "scope":"Kernel-selected interpreter and script; shebang metadata does not resolve wrappers"})
    } else {
        json!({"kind":"unknown","evidence":"No recognized ELF header or shebang", "scope":"Full target process"})
    }
}

pub(crate) fn validate_calibration(args: &Cli) -> Result<(), String> {
    if !args.python_overhead {
        return Ok(());
    }
    // Execution selectors and interactive flags could run a module or wait for input
    // instead of the empty script. Keep the calibration's accepted subset explicit.
    let mut value_expected = false;
    for value in &args.python_arg {
        let value = value
            .to_str()
            .ok_or("Python calibration options must be UTF-8")?;
        if value_expected {
            value_expected = false;
            continue;
        }
        if ["-W", "-X"].contains(&value) {
            value_expected = true;
            continue;
        }
        if [
            "-O", "-OO", "-B", "-E", "-I", "-s", "-S", "-u", "-q", "-P", "-b", "-bb", "-d", "-v",
            "-vv",
        ]
        .contains(&value)
            || (value.len() > 2 && (value.starts_with("-W") || value.starts_with("-X")))
        {
            continue;
        }
        return Err(format!("--python-overhead cannot calibrate interpreter option {value:?}; use startup flags such as -O, -B, -I, -S, -W or -X"));
    }
    if value_expected {
        return Err("Python calibration -W/-X needs a value".into());
    }
    Ok(())
}

pub(crate) fn calibrate(
    args: &Cli,
    target: &Target,
    base: &RunOptions<'_>,
    workload: &Value,
) -> Result<Value, String> {
    let mut script = tempfile::Builder::new()
        .prefix("bench-empty-")
        .suffix(".py")
        .tempfile()
        .map_err(|e| e.to_string())?;
    script
        .write_all(b"# Empty script for separate startup calibration.\n")
        .map_err(|e| e.to_string())?;
    let mut command = vec![target.interpreter.clone().into_os_string()];
    command.extend(args.python_arg.clone());
    command.push(script.path().as_os_str().to_owned());
    let opts = RunOptions {
        cwd: base.cwd,
        env: base.env,
        input: None,
        expected: None,
        expected_exit: 0,
        timeout: base.timeout,
        output_limit: base.output_limit,
        units: 1.,
        affinity: base.affinity,
    };
    let mut result = json!({"status":"ok","failed":false,"interrupt_signal":0,"warmups":[],"runs":[],"summary":{},
        "method":"One empty-script warmup followed by fresh interpreter processes; separate from workload samples",
        "limitations":"Estimate only: imports, script-directory search path, startup hooks, caches and scheduling can differ. Full-process metrics remain authoritative. RSS is not subtracted."});
    for index in 0..=args.overhead_runs {
        if args.progress {
            eprintln!(
                "bench: Python startup calibration {}/{}",
                index, args.overhead_runs
            );
        }
        let mut row = runner::run(&command, &opts, &[0, 1, 2, 3, 4, 5])?;
        row["run"] = json!(if index == 0 { 1 } else { index });
        let failed = row["status"] != "ok";
        let signal = row["interrupt_signal"].as_i64().unwrap_or(0);
        result[if index == 0 { "warmups" } else { "runs" }]
            .as_array_mut()
            .unwrap()
            .push(row);
        if failed {
            result["status"] = json!(if signal != 0 { "interrupted" } else { "failed" });
            result["failed"] = json!(true);
            result["interrupt_signal"] = json!(signal);
            break;
        }
    }
    for name in ["wall", "user", "system", "cpu", "cpu_percent", "rss"] {
        let values = result["runs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["status"] == "ok")
            .filter_map(|r| r["metrics"][name].as_f64())
            .collect::<Vec<_>>();
        if !values.is_empty() {
            result["summary"][name] = stats::summarize(values);
        }
    }
    if result["failed"] == false {
        if let (Some(full), Some(empty)) = (
            workload["summary"]["wall"]["median"].as_f64(),
            result["summary"]["wall"]["median"].as_f64(),
        ) {
            result["estimated_workload_wall_seconds"] = json!(full - empty);
            result["startup_fraction_percent"] = if full > 0. {
                json!(empty / full * 100.)
            } else {
                Value::Null
            };
        }
    }
    Ok(result)
}
