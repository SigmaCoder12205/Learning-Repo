use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr, OsString},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        raw::c_int,
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
    fn bench_measure(
        helper: c_int,
        request: c_int,
        reply: c_int,
        input: c_int,
        output: c_int,
        error: c_int,
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

/// Versioned measurement provenance; old fork-from-Rust baselines are incompatible.
pub(crate) const BACKEND: &str = "isolated-c-wait4-v1";

/// A between-run pause that preserves INT/TERM as a reportable interruption.
/// Like the native launcher, this is called only from the single execution thread.
pub(crate) fn cooldown(milliseconds: u64) -> Result<i32, String> {
    // SAFETY: all signal sets/timespecs are initialized, pointers stay live for each
    // synchronous call, and the original signal mask is restored on every path.
    unsafe {
        let mut signals: libc::sigset_t = std::mem::zeroed();
        let mut old: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut signals);
        libc::sigaddset(&mut signals, libc::SIGINT);
        libc::sigaddset(&mut signals, libc::SIGTERM);
        if libc::sigprocmask(libc::SIG_BLOCK, &signals, &mut old) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(milliseconds);
        let result = loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let timeout = libc::timespec {
                tv_sec: remaining.as_secs() as libc::time_t,
                tv_nsec: remaining.subsec_nanos() as libc::c_long,
            };
            let signal = libc::sigtimedwait(&signals, std::ptr::null_mut(), &timeout);
            if signal >= 0 {
                break Ok(signal);
            }
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::EAGAIN) => break Ok(0),
                Some(libc::EINTR) => continue,
                _ => break Err(error.to_string()),
            }
        };
        if libc::sigprocmask(libc::SIG_SETMASK, &old, std::ptr::null_mut()) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        result
    }
}

fn memory_file(name: &std::ffi::CStr, executable: bool) -> std::io::Result<File> {
    let flags = libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING;
    // MFD_EXEC is supported from Linux 6.3; older kernels allow executable memfds by default.
    // SAFETY: name is NUL-terminated, and we take ownership of a successful returned descriptor.
    let mut fd =
        unsafe { libc::memfd_create(name.as_ptr(), flags | if executable { 0x0010 } else { 0 }) };
    if fd < 0 && executable && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL)
    {
        fd = unsafe { libc::memfd_create(name.as_ptr(), flags) };
    }
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn seal(file: &File) -> std::io::Result<()> {
    // SAFETY: the descriptor is live and F_ADD_SEALS takes an integer bitmask.
    let rc = unsafe {
        libc::fcntl(
            file.as_raw_fd(),
            libc::F_ADD_SEALS,
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
        )
    };
    if rc < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
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
        if argv.is_empty() {
            return Err("empty target command".into());
        }
        let mut config = Vec::new();
        config.extend_from_slice(&1u32.to_le_bytes());
        for value in [opts.timeout, opts.output_limit, opts.units] {
            config.extend_from_slice(&value.to_le_bytes());
        }
        for count in [argv.len(), env.len(), opts.affinity.len()] {
            config.extend_from_slice(&u32::try_from(count)?.to_le_bytes());
        }
        for cpu in opts.affinity {
            config.extend_from_slice(&cpu.to_le_bytes());
        }
        for value in std::iter::once(&cwd).chain(argv.iter()).chain(env.iter()) {
            let bytes = value.as_bytes_with_nul();
            config.extend_from_slice(&u32::try_from(bytes.len())?.to_le_bytes());
            config.extend_from_slice(bytes);
        }
        if config.len() > 16 * 1024 * 1024 {
            return Err("measurement request exceeds 16 MiB".into());
        }
        let mut helper = memory_file(c"bench-measure", true)?;
        helper.write_all(include_bytes!(concat!(env!("OUT_DIR"), "/bench-measure")))?;
        seal(&helper)?;
        let mut request = memory_file(c"bench-request", false)?;
        request.write_all(&config)?;
        seal(&request)?;
        let reply = memory_file(c"bench-result", false)?;
        let mut measured = Measurement::default();
        // SAFETY: owned descriptors and result storage outlive this synchronous C call.
        // The launcher installs process-wide handlers only on the single main thread.
        // Target argv/environment are serialized, not accessed across exec through pointers.
        let errno = unsafe {
            bench_measure(
                helper.as_raw_fd(),
                request.as_raw_fd(),
                reply.as_raw_fd(),
                input.as_raw_fd(),
                output.as_raw_fd(),
                error.as_raw_fd(),
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
        let mut row = json!({"status": status, "exit_code": measured.exit_code, "metrics": metrics,
                  "termination": if measured.exit_code < 0 {
                      json!({"kind":"signal", "signal":-measured.exit_code, "name":signal_name(-measured.exit_code)})
                  } else { json!({"kind":"exit", "code":measured.exit_code}) },
                  "interrupt_signal": measured.interrupt_signal,
                  "stderr_preview": String::from_utf8_lossy(&preview)});
        if status != "ok" {
            // Read only after measurement. Failure context is independent of metric filtering.
            output.seek(SeekFrom::Start(0))?;
            let mut stdout = Vec::new();
            output.take(512).read_to_end(&mut stdout)?;
            row["stdout_preview"] = json!(String::from_utf8_lossy(&stdout));
            row["stdout_preview_truncated"] = json!(measured.metrics[12] > 512.0);
            row["captured_output"] = json!({
                "stdout_bytes": measured.metrics[12],
                "stderr_bytes": measured.metrics[13]
            });
        }
        Ok(row)
    };
    work().map_err(|e| e.to_string())
}

fn signal_name(signal: i32) -> Option<&'static str> {
    match signal {
        libc::SIGHUP => Some("SIGHUP"),
        libc::SIGINT => Some("SIGINT"),
        libc::SIGQUIT => Some("SIGQUIT"),
        libc::SIGILL => Some("SIGILL"),
        libc::SIGABRT => Some("SIGABRT"),
        libc::SIGFPE => Some("SIGFPE"),
        libc::SIGKILL => Some("SIGKILL"),
        libc::SIGSEGV => Some("SIGSEGV"),
        libc::SIGPIPE => Some("SIGPIPE"),
        libc::SIGALRM => Some("SIGALRM"),
        libc::SIGTERM => Some("SIGTERM"),
        libc::SIGBUS => Some("SIGBUS"),
        libc::SIGUSR1 => Some("SIGUSR1"),
        libc::SIGUSR2 => Some("SIGUSR2"),
        _ => None,
    }
}

pub(crate) fn display_command(command: &[OsString]) -> Vec<String> {
    command
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_peak_is_independent_of_rust_parent_resident_memory() {
        let env = BTreeMap::new();
        let opts = RunOptions {
            cwd: Path::new("/"),
            env: &env,
            input: None,
            expected: None,
            expected_exit: 0,
            timeout: 5.,
            output_limit: 1024.,
            units: 1.,
            affinity: &[],
        };
        let command = [OsString::from("/bin/true")];
        let before = run(&command, &opts, &[5]).unwrap();
        assert_eq!(before["status"], "ok");
        let mut resident = vec![0u8; 64 * 1024 * 1024];
        for byte in resident.iter_mut().step_by(4096) {
            *byte = 1;
        }
        std::hint::black_box(&resident);
        let after = run(&command, &opts, &[5]).unwrap();
        assert_eq!(after["status"], "ok");
        let before_rss = before["metrics"]["rss"].as_f64().unwrap();
        let after_rss = after["metrics"]["rss"].as_f64().unwrap();
        assert!(
            after_rss < before_rss + 8. * 1024.,
            "parent memory leaked into target RSS: before={before_rss}, after={after_rss}"
        );
        std::hint::black_box(resident);
    }
}
