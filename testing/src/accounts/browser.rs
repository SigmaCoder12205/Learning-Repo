//! Authorization Code + PKCE using a bounded loopback callback listener.
use super::provider::{Provider, CLIENT_ID};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

fn random() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "OS randomness unavailable")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub(super) fn open(url: &str, no_browser: bool) {
    eprintln!("Open this URL in your browser:\n{url}");
    if !no_browser {
        // The URL contains only a public authorization request, never passwords or tokens.
        match Command::new("xdg-open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(_) => eprintln!("Could not open a browser automatically; use the URL above."),
        }
    }
}
fn reply(stream: &mut TcpStream, status: &str, message: &str) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{message}", message.len());
}
fn request(stream: &mut TcpStream) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while bytes.len() < 16384 && Instant::now() < deadline {
        match stream.read(&mut byte) {
            Ok(1) => {
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    return String::from_utf8(bytes)
                        .map_err(|_| "invalid callback encoding".into());
                }
            }
            _ => break,
        }
    }
    Err("incomplete or oversized callback".into())
}
fn callback(request: &str, state: &str) -> Result<Option<Result<String, String>>, String> {
    let line = request.lines().next().ok_or("missing callback request")?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3
        || parts[0] != "GET"
        || !parts[1].starts_with('/')
        || parts[1].starts_with("//")
    {
        return Err("invalid callback request".into());
    }
    let url =
        Url::parse(&format!("http://127.0.0.1{}", parts[1])).map_err(|_| "invalid callback URL")?;
    if url.path() != "/callback" {
        return Ok(None);
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let field = |key: &str| -> Result<Option<String>, String> {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == key).collect();
        if values.len() > 1 {
            return Err("duplicate callback parameter".into());
        }
        Ok(values.first().map(|(_, value)| value.to_string()))
    };
    if field("state")?.as_deref() != Some(state) {
        return Err("callback state mismatch".into());
    }
    if field("error")?.is_some() {
        return Ok(Some(Err(
            "browser authorization was canceled or denied; saved login unchanged".into(),
        )));
    }
    let code = field("code")?
        .filter(|s| !s.is_empty())
        .ok_or("callback is missing its authorization code")?;
    Ok(Some(Ok(code)))
}
pub(super) struct Authorization {
    pub code: Zeroizing<String>,
    pub verifier: Zeroizing<String>,
    pub redirect_uri: String,
}
pub(super) fn authorize(
    provider: &Provider,
    register: bool,
    hint: Option<&str>,
    no_browser: bool,
    port: u16,
    timeout: u64,
) -> Result<Authorization, String> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .map_err(|e| format!("cannot listen on 127.0.0.1:{port}: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = random()?;
    let verifier = Zeroizing::new(random()?);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let endpoint = if register { "registrations" } else { "auth" };
    let mut url = Url::parse(&provider.endpoint(endpoint)).map_err(|e| e.to_string())?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("scope", "openid profile email offline_access"),
        ("redirect_uri", &redirect_uri),
        ("state", &state),
        ("code_challenge_method", "S256"),
        ("code_challenge", &challenge),
        ("prompt", "login"),
    ]);
    if let Some(hint) = hint {
        url.query_pairs_mut().append_pair("login_hint", hint);
    }
    open(url.as_str(), no_browser);
    eprintln!("Waiting up to {timeout} seconds for browser sign-in…");
    let deadline = Instant::now() + Duration::from_secs(timeout);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => match request(&mut stream).and_then(|r| callback(&r, &state)) {
                Ok(Some(result)) => {
                    reply(&mut stream, "200 OK", "Authorization received. Return to your terminal for the result. You can close this tab.");
                    return Ok(Authorization {
                        code: Zeroizing::new(result?),
                        verifier,
                        redirect_uri,
                    });
                }
                Ok(None) => reply(&mut stream, "404 Not Found", "Not found."),
                Err(_) => reply(
                    &mut stream,
                    "400 Bad Request",
                    "Invalid authorization callback. Return to the original login page.",
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50))
            }
            Err(e) => return Err(format!("cannot receive browser callback: {e}")),
        }
    }
    Err("browser sign-in timed out; saved login unchanged".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_matching_state_and_unique_parameters() {
        assert!(callback("GET /callback?code=x&state=wrong HTTP/1.1\r\n\r\n", "right").is_err());
        assert!(callback(
            "GET /callback?code=x&state=right&state=wrong HTTP/1.1\r\n\r\n",
            "right"
        )
        .is_err());
        assert!(callback(
            "POST /callback?code=x&state=right HTTP/1.1\r\n\r\n",
            "right"
        )
        .is_err());
        assert!(
            callback("GET /callback?code=x&state=right HTTP/1.1\r\n\r\n", "right")
                .unwrap()
                .unwrap()
                .is_ok()
        );
        assert!(callback(
            "GET /callback?error=access_denied&state=right HTTP/1.1\r\n\r\n",
            "right"
        )
        .unwrap()
        .unwrap()
        .is_err());
        assert!(callback("GET /favicon.ico HTTP/1.1\r\n\r\n", "right")
            .unwrap()
            .is_none());
    }
}
