//! CLI OAuth contract, tested without Docker or a browser.
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::{symlink, PermissionsExt},
    path::Path,
    process::{Command, Output, Stdio},
    thread,
};

fn dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cppbench"));
    command
        .args(["account", "--json", "--data-dir"])
        .arg(dir)
        .args(args);
    command
}
fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}
fn ok(output: Output) -> Value {
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}
fn cached(dir: &Path, issuer: &str) {
    fs::write(dir.join("session.json"), serde_json::to_vec(&json!({"schema_version":2,"revision":0,"session":{"issuer":issuer,"refresh_token":"private-refresh"}})).unwrap()).unwrap();
    fs::set_permissions(dir.join("session.json"), fs::Permissions::from_mode(0o600)).unwrap();
}
fn state(dir: &Path) -> Value {
    serde_json::from_slice(&fs::read(dir.join("session.json")).unwrap()).unwrap()
}
fn tokens() -> Value {
    json!({"access_token":"private-access","refresh_token":"private-next","token_type":"Bearer","scope":"openid profile offline_access","expires_in":300,"refresh_expires_in":0})
}
fn identity() -> Value {
    json!({"sub":"stable-id","preferred_username":"alice","email":"alice@example.test"})
}
struct Reply {
    route: &'static str,
    status: u16,
    body: Value,
}
fn mock(replies: Vec<Reply>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let issuer = format!("http://{}/realms/cppbench", listener.local_addr().unwrap());
    let thread = thread::spawn(move || {
        let mut requests = Vec::new();
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&stream);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                headers.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            assert!(headers.lines().next().unwrap().contains(reply.route));
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut bytes = vec![0; length];
            reader.read_exact(&mut bytes).unwrap();
            requests.push(String::from_utf8(bytes).unwrap());
            let body = reply.body.to_string();
            write!(stream,"HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reply.status,body.len(),body).unwrap();
        }
        requests
    });
    (issuer, thread)
}
fn reply(route: &'static str, body: Value) -> Reply {
    Reply {
        route,
        status: 200,
        body,
    }
}
#[test]
fn persistent_login_refreshes_and_uses_verified_userinfo() {
    let temp = dir();
    let (issuer, server) = mock(vec![
        reply("/token", tokens()),
        reply("/userinfo", identity()),
        reply("/revoke", json!({})),
    ]);
    cached(temp.path(), &issuer);
    let output = run(temp.path(), &["whoami"]);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-"));
    let result = ok(output);
    assert_eq!(result["account"]["id"], "stable-id");
    assert_eq!(result["authenticated"], true);
    assert_eq!(result["persistent_login"], true);
    assert_eq!(
        state(temp.path())["session"]["refresh_token"],
        "private-next"
    );
    ok(run(temp.path(), &["logout"]));
    assert!(state(temp.path())["session"].is_null());
    let requests = server.join().unwrap();
    assert!(requests[0].contains("grant_type=refresh_token"));
    assert!(requests[0].contains("refresh_token=private-refresh"));
    assert!(requests[2].contains("token=private-next"));
}
#[test]
fn revocation_signs_out_but_network_errors_preserve_credentials() {
    let temp = dir();
    let (issuer, server) = mock(vec![Reply {
        route: "/token",
        status: 400,
        body: json!({"error":"invalid_grant"}),
    }]);
    cached(temp.path(), &issuer);
    assert_eq!(run(temp.path(), &["whoami"]).status.code(), Some(1));
    assert!(state(temp.path())["session"].is_null());
    server.join().unwrap();
    cached(temp.path(), &issuer); // Server has stopped; connection failure must not discard login.
    let before = fs::read(temp.path().join("session.json")).unwrap();
    assert_eq!(run(temp.path(), &["whoami"]).status.code(), Some(2));
    assert_eq!(run(temp.path(), &["logout"]).status.code(), Some(2));
    assert_eq!(fs::read(temp.path().join("session.json")).unwrap(), before);
    let result = ok(run(temp.path(), &["logout", "--local"]));
    assert_eq!(result["remote_revocation"], false);
    assert!(state(temp.path())["session"].is_null());
}
#[test]
fn cache_validation_and_issuer_binding_prevent_credential_leaks() {
    let temp = dir();
    cached(temp.path(), "http://127.0.0.1:9/realms/cppbench");
    let before = fs::read(temp.path().join("session.json")).unwrap();
    for issuer in [
        "http://example.org/realms/test",
        "https://someone:secret@example.org/realm",
        "https://example.org/realm?token=bad",
        "http://127.0.0.1:8/other",
    ] {
        assert_eq!(
            run(temp.path(), &["--issuer", issuer, "whoami"])
                .status
                .code(),
            Some(2)
        );
    }
    assert_eq!(fs::read(temp.path().join("session.json")).unwrap(), before);
    fs::set_permissions(
        temp.path().join("session.json"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_eq!(run(temp.path(), &["whoami"]).status.code(), Some(2));
    fs::set_permissions(
        temp.path().join("session.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::rename(
        temp.path().join("session.json"),
        temp.path().join("other.json"),
    )
    .unwrap();
    symlink(
        temp.path().join("other.json"),
        temp.path().join("session.json"),
    )
    .unwrap();
    assert_eq!(
        run(temp.path(), &["logout", "--local"]).status.code(),
        Some(2)
    );
    fs::remove_file(temp.path().join("session.json")).unwrap();
    fs::write(temp.path().join("session.json"), "{}").unwrap();
    fs::set_permissions(
        temp.path().join("session.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(
        run(temp.path(), &["logout", "--local"]).status.code(),
        Some(2)
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("session.json")).unwrap(),
        "{}"
    );
}
#[test]
fn no_legacy_accounts_and_no_login_requirement_for_benchmarks() {
    let temp = dir();
    fs::write(temp.path().join("accounts.json"),r#"{"schema_version":1,"accounts":[{"username":"old-user"}],"session":{"expires_at":9999999999}}"#).unwrap();
    assert_eq!(run(temp.path(), &["whoami"]).status.code(), Some(1));
    assert_eq!(
        run(temp.path(), &["login", "old-user", "--password-stdin"])
            .status
            .code(),
        Some(2)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args(["/bin/true", "--runs", "1", "--warmup", "0"])
        .env("XDG_DATA_HOME", temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!temp.path().join("cppbench").exists());
    let output = Command::new(env!("CARGO_BIN_EXE_cppbench"))
        .args(["--gpu", "account", "whoami"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
#[test]
fn browser_login_pkce_and_callback_state_end_to_end() {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};
    let temp = dir();
    let (issuer, server) = mock(vec![
        reply("/token", tokens()),
        reply("/userinfo", identity()),
    ]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut child = command(
        temp.path(),
        &[
            "--issuer",
            &issuer,
            "login",
            "alice",
            "--no-browser",
            "--callback-port",
            &port.to_string(),
            "--timeout",
            "10",
        ],
    )
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let auth = loop {
        let mut line = String::new();
        assert!(stderr.read_line(&mut line).unwrap() > 0);
        if line.starts_with("http://") {
            break reqwest::Url::parse(line.trim()).unwrap();
        }
    };
    let query: std::collections::HashMap<_, _> = auth.query_pairs().into_owned().collect();
    assert_eq!(query["code_challenge_method"], "S256");
    assert!(query["scope"].contains("offline_access"));
    assert_eq!(query["login_hint"], "alice");
    for (state, status) in [("wrong", "400"), (query["state"].as_str(), "200")] {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET /callback?state={state}&code=one-use-code HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with(&format!("HTTP/1.1 {status}")));
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(ok(output)["account"]["username"], "alice");
    let requests = server.join().unwrap();
    let form: std::collections::HashMap<_, _> =
        reqwest::Url::parse(&format!("http://localhost/?{}", requests[0]))
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect();
    assert_eq!(form["code"], "one-use-code");
    assert_eq!(
        URL_SAFE_NO_PAD.encode(Sha256::digest(form["code_verifier"].as_bytes())),
        query["code_challenge"]
    );
}
#[test]
fn browser_management_reports_opened_pages_not_completed_mutations() {
    let temp = dir();
    for action in ["manage", "password", "delete"] {
        let result = ok(run(temp.path(), &[action, "--no-browser"]));
        assert_eq!(result["action_completed"], false);
        assert!(result["url"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:8787/realms/cppbench/account/"));
    }
}
