use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::{
        fd::AsRawFd,
        raw::{c_char, c_int},
        unix::ffi::OsStrExt,
    },
    path::Path,
};

#[repr(C)]
#[derive(Default)]
struct Measurement {
    metrics: [f64; 15],
    exit_code: c_int,
    reason: c_int,
    launch_errno: c_int,
    interrupt_signal: c_int,
}
extern "C" {
    fn cppbench_measure(
        executable: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
        cwd: *const c_char,
        input: c_int,
        output: c_int,
        error: c_int,
        timeout: f64,
        output_limit: f64,
        units: f64,
        cpus: *const u32,
        cpu_count: usize,
        result: *mut Measurement,
    ) -> c_int;
}

pub struct RunOptions<'a> {
    pub cwd: &'a Path,
    pub env: &'a BTreeMap<OsString, OsString>,
    pub input: Option<&'a Path>,
    pub expected: Option<&'a Path>,
    pub expected_exit: i32,
    pub timeout: f64,
    pub output_limit: f64,
    pub units: f64,
    pub affinity: &'a [u32],
}

fn cstring(value: &OsStr) -> Result<CString, String> {
    CString::new(value.as_bytes()).map_err(|_| "path or argument contains a NUL byte".into())
}
fn same_contents(a: &mut File, b: &mut File) -> std::io::Result<bool> {
    if a.metadata()?.len() != b.metadata()?.len() {
        return Ok(false);
    }
    a.seek(SeekFrom::Start(0))?;
    let mut left = [0; 65536];
    let mut right = [0; 65536];
    let mut remaining = a.metadata()?.len();
    while remaining > 0 {
        let count = remaining.min(left.len() as u64) as usize;
        a.read_exact(&mut left[..count])?;
        b.read_exact(&mut right[..count])?;
        if left[..count] != right[..count] {
            return Ok(false);
        }
        remaining -= count as u64;
    }
    Ok(true)
}

pub fn run(
    command: &[OsString],
    opts: &RunOptions<'_>,
    selected: &[usize],
) -> Result<Value, String> {
    run_logged(command, opts, selected, None)
}

/// Optional persistent stdout/stderr captures for diagnostic tools.
pub fn run_logged(
    command: &[OsString],
    opts: &RunOptions<'_>,
    selected: &[usize],
    logs: Option<(&Path, &Path)>,
) -> Result<Value, String> {
    let work = || -> Result<Value, Box<dyn std::error::Error>> {
        let argv = command
            .iter()
            .map(|a| cstring(a))
            .collect::<Result<Vec<_>, _>>()?;
        let mut argv_ptrs: Vec<_> = argv.iter().map(|a| a.as_ptr()).collect();
        argv_ptrs.push(std::ptr::null());
        let env = opts
            .env
            .iter()
            .map(|(key, value)| {
                let mut pair = key.clone();
                pair.push("=");
                pair.push(value);
                cstring(&pair)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut env_ptrs: Vec<_> = env.iter().map(|v| v.as_ptr()).collect();
        env_ptrs.push(std::ptr::null());
        let cwd = cstring(opts.cwd.as_os_str())?;
        let input = File::open(opts.input.unwrap_or(Path::new("/dev/null")))?;
        let capture = |path: &Path| {
            File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .open(path)
        };
        let (mut output, mut error) = if let Some((stdout, stderr)) = logs {
            (capture(stdout)?, capture(stderr)?)
        } else {
            (tempfile::tempfile()?, tempfile::tempfile()?)
        };
        let mut measured = Measurement::default();
        // SAFETY: all C strings, NULL-terminated pointer arrays, file descriptors, and
        // result storage live until the synchronous C call returns. This CLI invokes
        // the process-wide signal API only from its single main thread.
        let errno = unsafe {
            cppbench_measure(
                argv[0].as_ptr(),
                argv_ptrs.as_ptr(),
                env_ptrs.as_ptr(),
                cwd.as_ptr(),
                input.as_raw_fd(),
                output.as_raw_fd(),
                error.as_raw_fd(),
                opts.timeout,
                opts.output_limit,
                opts.units,
                opts.affinity.as_ptr(),
                opts.affinity.len(),
                &mut measured,
            )
        };
        if errno != 0 {
            return Err(std::io::Error::from_raw_os_error(errno).into());
        }
        if measured.launch_errno != 0 {
            return Err(format!(
                "cannot launch {:?}: {}",
                command[0],
                std::io::Error::from_raw_os_error(measured.launch_errno)
            )
            .into());
        }
        let mut status = match measured.reason {
            1 => "timeout",
            2 => "output_limit",
            3 => "interrupted",
            _ => "ok",
        };
        if status == "ok" && measured.exit_code != opts.expected_exit {
            status = "unexpected_exit";
        }
        if status == "ok" {
            if let Some(path) = opts.expected {
                if !same_contents(&mut output, &mut File::open(path)?)? {
                    status = "output_mismatch";
                }
            }
        }
        error.seek(SeekFrom::Start(0))?;
        let mut preview = Vec::new();
        error.take(4096).read_to_end(&mut preview)?;
        let mut metrics = serde_json::Map::new();
        for &index in selected {
            metrics.insert(
                crate::metrics::METRICS[index].0.into(),
                json!(measured.metrics[index]),
            );
        }
        Ok(
            json!({"status": status, "exit_code": measured.exit_code, "metrics": metrics,
                  "interrupt_signal": measured.interrupt_signal,
                  "stderr_preview": String::from_utf8_lossy(&preview)}),
        )
    };
    work().map_err(|e| e.to_string())
}

pub(crate) fn display_command(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}
