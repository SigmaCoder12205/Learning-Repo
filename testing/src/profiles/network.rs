use crate::{diagnostics, runner::RunOptions};
use serde_json::{json, Value};
use std::{collections::BTreeMap, ffi::OsString, path::Path};

const SOCKET_CALLS: &[&str] = &[
    "socket",
    "socketpair",
    "connect",
    "accept",
    "accept4",
    "bind",
    "listen",
    "getsockname",
    "getpeername",
    "sendto",
    "sendmsg",
    "recvfrom",
    "recvmsg",
    "shutdown",
    "setsockopt",
    "getsockopt",
    "sendmmsg",
    "recvmmsg",
];

pub fn run(target: &[OsString], opts: &RunOptions<'_>, dir: &Path) -> Result<Value, String> {
    let tool = diagnostics::find_tool("strace", opts.env)?;
    let path = dir.join("network.strace");
    let mut command = vec![
        tool,
        "-f".into(),
        "-qq".into(),
        "-yy".into(),
        "-T".into(),
        "-s".into(),
        "0".into(),
        "-e".into(),
        "trace=%network,read,write,readv,writev,sendfile,splice,io_uring_enter".into(),
        "-e".into(),
        "signal=none".into(),
        "-o".into(),
        path.clone().into_os_string(),
        "--".into(),
    ];
    command.extend_from_slice(target);
    let row = diagnostics::command_run(&command, opts, dir, "network")?;
    if let Some(failure) = diagnostics::invocation_failed(&row) {
        return Ok(failure);
    }
    let mut result = parse(&diagnostics::read_report(&path)?)?;
    result["raw_file"] = json!(path);
    result["invocation"] = row;
    Ok(result)
}

pub fn parse(text: &str) -> Result<Value, String> {
    let mut pending = BTreeMap::<String, String>::new();
    let mut calls = 0_u64;
    let mut errors = 0_u64;
    let mut connects = 0_u64;
    let mut sent = 0_u64;
    let mut received = 0_u64;
    let mut unaccounted = 0_u64;
    let mut durations = Vec::new();
    let mut by_errno = BTreeMap::<String, u64>::new();
    for raw in text.lines().filter(|l| !l.trim().is_empty()) {
        let raw = raw.trim();
        let (pid, line) = if let Some(rest) = raw.strip_prefix("[pid ") {
            let (pid, line) = rest.split_once(']').ok_or("invalid strace PID")?;
            (pid.trim(), line.trim())
        } else if let Some((first, rest)) = raw.split_once(char::is_whitespace) {
            if first.chars().all(|c| c.is_ascii_digit()) {
                (first, rest.trim())
            } else {
                ("main", raw)
            }
        } else {
            ("main", raw)
        };
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if let Some(first) = line.strip_suffix("<unfinished ...>") {
            if pending.insert(pid.into(), first.into()).is_some() {
                return Err("duplicate unfinished syscall".into());
            }
            continue;
        }
        let complete = if let Some(rest) = line.strip_prefix("<... ") {
            let (name, tail) = rest
                .split_once(" resumed>")
                .ok_or("invalid resumed syscall")?;
            let first = pending.remove(pid).ok_or("resumed syscall without entry")?;
            if !first.starts_with(&format!("{name}(")) {
                return Err("resumed syscall name differs".into());
            }
            format!("{first}{tail}")
        } else {
            line.to_string()
        };
        let (entry, exit) = complete.rsplit_once(" = ").ok_or_else(|| {
            format!(
                "unrecognized strace record: {}",
                complete.chars().take(80).collect::<String>()
            )
        })?;
        let (name, arguments) = entry
            .split_once('(')
            .ok_or("strace call missing arguments")?;
        let annotated_socket = ["<TCP", "<UDP", "<UNIX", "<SCTP", "<NETLINK", "<socket:"]
            .iter()
            .any(|s| arguments.contains(s));
        if name == "io_uring_enter" || (["sendfile", "splice"].contains(&name) && annotated_socket)
        {
            unaccounted += 1;
        }
        let net = SOCKET_CALLS.contains(&name)
            || (["read", "write", "readv", "writev"].contains(&name) && annotated_socket);
        if !net {
            continue;
        }
        calls += 1;
        if name == "connect" {
            connects += 1;
        }
        let result = exit
            .split_whitespace()
            .next()
            .ok_or("missing syscall result")?
            .split('<')
            .next()
            .unwrap_or("")
            .parse::<i64>()
            .map_err(|_| "invalid syscall result")?;
        if result < 0 {
            errors += 1;
            *by_errno
                .entry(exit.split_whitespace().nth(1).unwrap_or("unknown").into())
                .or_default() += 1;
        } else if ["sendto", "sendmsg", "write", "writev"].contains(&name) {
            sent = sent
                .checked_add(result as u64)
                .ok_or("network byte count overflow")?;
        } else if ["recvfrom", "recvmsg", "read", "readv"].contains(&name) {
            received = received
                .checked_add(result as u64)
                .ok_or("network byte count overflow")?;
        } else if ["sendmmsg", "recvmmsg"].contains(&name) && result > 0 {
            unaccounted += 1;
        }
        let seconds = exit
            .rsplit_once('<')
            .and_then(|(_, s)| s.strip_suffix('>'))
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0. && (*v * 1000.).is_finite())
            .ok_or("missing/invalid syscall duration")?;
        durations.push(seconds * 1000.);
        if durations.len() > 100_000 {
            return Err(
                "network trace exceeds 100000 socket calls; reduce the diagnostic workload".into(),
            );
        }
    }
    if !pending.is_empty() {
        return Err("incomplete network trace contains unfinished syscalls".into());
    }
    let mut metrics = serde_json::Map::new();
    for (key, value, unit) in [
        ("networking.socket_calls", calls, "count"),
        ("networking.socket_errors", errors, "count"),
        ("networking.connect_calls", connects, "count"),
        ("networking.network_calls", calls, "count"),
        ("networking.sent_bytes", sent, "B"),
        ("networking.received_bytes", received, "B"),
    ] {
        if unit == "B" && unaccounted > 0 {
            continue;
        }
        metrics.insert(key.into(),json!({"status":"ok","value":value,"unit":unit,"kind":"counter","source":"strace; separate replay including traced descendants"}));
    }
    if !durations.is_empty() {
        let mut summary = crate::summarize_basic(durations.clone());
        durations.sort_by(f64::total_cmp);
        summary["p99"] = json!(crate::percentile(&durations, 0.99));
        metrics.insert("networking.syscall_ms".into(),json!({"status":"ok","unit":"ms","kind":"samples","summary":summary,"source":"strace elapsed syscall time; perturbed by tracing"}));
    }
    Ok(
        json!({"status":"ok","metrics":metrics,"errors_by_errno":by_errno,"unsupported_transfer_calls":unaccounted,
        "scope":"socket syscalls including UNIX/local sockets and traced descendants; byte totals count successful send/receive/read/write results, not wire traffic; both endpoints may be counted; EINPROGRESS/EAGAIN are included in errors and need interpretation; no RTT, packet loss, retransmit or HTTP inference; byte totals withheld for observed batching, zero-copy or io_uring calls"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn annotated_descriptors_resumed_calls_and_errors_are_accounted() {
        let r = parse(include_str!("../../tests/fixtures/network.strace")).unwrap();
        let m = &r["metrics"];
        assert_eq!(m["networking.socket_calls"]["value"], 5);
        assert_eq!(m["networking.socket_errors"]["value"], 1);
        assert_eq!(m["networking.sent_bytes"]["value"], 64);
        assert_eq!(m["networking.received_bytes"]["value"], 64);
        assert_eq!(r["errors_by_errno"]["EINPROGRESS"], 1);
        assert_eq!(m["networking.syscall_ms"]["summary"]["count"], 5);
        assert_eq!(m["networking.syscall_ms"]["summary"]["max"], 2.);
    }
    #[test]
    fn no_guessed_bytes_for_async_or_batched_io_and_no_guessed_zero_for_bad_trace() {
        let r = parse("1 sendmmsg(3<UDP:[123]>, [...], 2, 0) = 2 <0.001>\n").unwrap();
        assert!(r["metrics"]["networking.sent_bytes"].is_null());
        assert_eq!(r["unsupported_transfer_calls"], 1);
        for trace in [
            "1 recvfrom(3, <unfinished ...>\n",
            "garbage",
            "1 <... read resumed>0) = 0 <0.01>\n",
        ] {
            assert!(parse(trace).is_err());
        }
        let empty = parse("").unwrap();
        assert_eq!(empty["metrics"]["networking.sent_bytes"]["value"], 0);
        assert!(empty["metrics"]["networking.syscall_ms"].is_null());
    }
}
