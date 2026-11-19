//! Answer-once terminal trial. Human time never enters benchmark samples.
use crate::{
    cli::Cli,
    input_hints::{self, Hint},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::{CString, OsString},
    io::{IsTerminal, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
};

#[repr(C)]
#[derive(Default)]
struct CaptureResult {
    exit_code: i32,
    reason: i32,
    launch_errno: i32,
    interrupt_signal: i32,
}
extern "C" {
    fn bench_capture(
        argv: *const *const libc::c_char,
        env: *const *const libc::c_char,
        cwd: *const libc::c_char,
        terminal: i32,
        timeout: f64,
        output_limit: f64,
        callback: extern "C" fn(*mut libc::c_void, i32, *const u8, usize) -> i32,
        context: *mut libc::c_void,
        result: *mut CaptureResult,
    ) -> i32;
}

struct State {
    bytes: Vec<u8>,
    prompt: String,
    hints: Vec<Hint>,
    source: Vec<Hint>,
    position: usize,
    shown: usize,
}
impl State {
    fn hint(&self) -> Hint {
        let prompt = input_hints::prompt(&self.prompt, self.position);
        if prompt.kind != "unknown" {
            return prompt;
        }
        self.source
            .iter()
            .find(|h| h.channel == "stdin" && h.position == Some(self.position))
            .cloned()
            .unwrap_or(prompt)
    }
    fn event(&mut self, kind: i32, bytes: &[u8]) -> std::io::Result<()> {
        if kind == 0 {
            std::io::stderr().lock().write_all(bytes)?;
            self.prompt.push_str(&String::from_utf8_lossy(bytes));
            self.prompt = self
                .prompt
                .chars()
                .rev()
                .take(4096)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            let hint = self.hint();
            if self.shown != self.position && hint.kind != "unknown" {
                writeln!(
                    std::io::stderr().lock(),
                    "\n[bench answer {}: likely {}; examples {} — inferred]",
                    self.position,
                    hint.kind,
                    json!(hint.suggestions)
                )?;
                self.shown = self.position;
            }
        } else {
            self.bytes.extend_from_slice(bytes);
            for &b in bytes {
                if b == b'\n' {
                    self.hints.push(self.hint());
                    self.position += 1;
                    self.prompt.clear();
                }
            }
        }
        Ok(())
    }
}
extern "C" fn event(context: *mut libc::c_void, kind: i32, bytes: *const u8, len: usize) -> i32 {
    // SAFETY: the native supervisor calls synchronously with its live buffer and
    // the State pointer supplied below. No Rust callback runs in the forked child.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        (&mut *context.cast::<State>())
            .event(kind, std::slice::from_raw_parts(bytes, len))
            .map(|_| 0)
            .unwrap_or(1)
    }))
    .unwrap_or(1)
}

pub(crate) fn terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

pub(crate) fn capture(
    args: &Cli,
    command: &[OsString],
    env: &BTreeMap<OsString, OsString>,
    cwd: &Path,
    discovery: &Value,
) -> Result<(Vec<u8>, Value), String> {
    if !std::io::stdin().is_terminal() {
        return Err("--interactive needs a terminal on stdin; use --input-line, --input-text or --input FILE in scripts".into());
    }
    eprintln!("bench: answer the program's prompts once, including its quit command if needed.\nThis extra trial is not timed as a benchmark; answers will be replayed. Ctrl-D sends EOF; Ctrl-C cancels.");
    eprint!("{}", input_hints::render(discovery));
    let argv = command
        .iter()
        .map(|v| CString::new(v.as_bytes()).map_err(|_| "argument contains NUL"))
        .collect::<Result<Vec<_>, _>>()?;
    let environment = env
        .iter()
        .map(|(k, v)| {
            let mut value = k.clone();
            value.push("=");
            value.push(v);
            CString::new(value.as_bytes()).map_err(|_| "environment contains NUL")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut argv_ptrs = argv.iter().map(|v| v.as_ptr()).collect::<Vec<_>>();
    argv_ptrs.push(std::ptr::null());
    let mut env_ptrs = environment.iter().map(|v| v.as_ptr()).collect::<Vec<_>>();
    env_ptrs.push(std::ptr::null());
    let cwd = CString::new(cwd.as_os_str().as_bytes()).map_err(|_| "cwd contains NUL")?;
    let mut state = State {
        bytes: vec![],
        prompt: String::new(),
        hints: vec![],
        source: input_hints::hints(discovery),
        position: 1,
        shown: 0,
    };
    let mut result = CaptureResult::default();
    // SAFETY: C strings, argv/env pointers and state remain alive for the entire
    // synchronous call. Called only on the main thread, before any measured runs.
    let error = unsafe {
        bench_capture(
            argv_ptrs.as_ptr(),
            env_ptrs.as_ptr(),
            cwd.as_ptr(),
            0,
            args.interactive_timeout,
            args.output_limit * 1048576.,
            event,
            (&mut state as *mut State).cast(),
            &mut result,
        )
    };
    if error != 0 {
        return Err(format!(
            "interactive trial: {}",
            std::io::Error::from_raw_os_error(error)
        ));
    }
    if result.launch_errno != 0 {
        return Err(format!(
            "cannot launch interactive trial: {}",
            std::io::Error::from_raw_os_error(result.launch_errno)
        ));
    }
    if !state.bytes.is_empty() && state.bytes.last() != Some(&b'\n') {
        state.hints.push(state.hint());
    }
    let status = match result.reason {
        1 => "timeout",
        2 => "output_limit",
        3 => "interrupted",
        4 => "input_limit",
        _ => {
            if result.exit_code == args.expect_exit {
                "ok"
            } else {
                "unexpected_exit"
            }
        }
    };
    eprintln!("\nbench: input trial {status}; captured {} bytes. Human time is excluded from the measurements.",state.bytes.len());
    let info = json!({"status":status,"exit_code":result.exit_code,"interrupt_signal":result.interrupt_signal,
        "captured_bytes":state.bytes.len(),"hints":state.hints,"measurement_scope":"Unmeasured terminal trial; measured runs replay bytes through a regular stdin file"});
    Ok((state.bytes, info))
}
