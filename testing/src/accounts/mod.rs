//! CLI adapter for Keycloak. Account records and password handling live in the service.
mod browser;
mod provider;
mod store;

use clap::{Args, Subcommand};
use provider::{Error, Provider, DEFAULT_ISSUER};
use serde_json::{json, Value};
use std::path::PathBuf;
use store::{Session, Store};

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum Command {
    /// Keycloak accounts: browser registration/login and persistent CLI sessions
    Account(Options),
}
#[derive(Args, Debug, Clone)]
pub(crate) struct Options {
    /// Private CLI session directory (absolute path); contains no account database
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Keycloak realm URL; defaults to the local cppbench service
    #[arg(long, global = true)]
    issuer: Option<String>,
    /// Machine-readable results (browser instructions and errors go to stderr)
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    action: Action,
}
#[derive(Args, Debug, Clone)]
struct BrowserLogin {
    /// Optional username hint; passwords are entered only in Keycloak's browser pages
    username: Option<String>,
    /// Print the browser URL without opening it automatically
    #[arg(long)]
    no_browser: bool,
    /// Seconds to wait for browser authorization (not the lifetime of the account/login)
    #[arg(long, default_value = "300", value_parser = clap::value_parser!(u64).range(1..=600))]
    timeout: u64,
    /// Loopback callback port; custom ports must be allowed in the Keycloak client
    #[arg(long, default_value = "8766", value_parser = clap::value_parser!(u16).range(1..))]
    callback_port: u16,
}
#[derive(Args, Debug, Clone)]
struct Page {
    #[arg(long)]
    no_browser: bool,
}
#[derive(Subcommand, Debug, Clone)]
enum Action {
    /// Register in the browser and sign in; accounts have no expiration date
    Register(BrowserLogin),
    /// Sign in through Keycloak; retain login across terminal sessions
    Login(BrowserLogin),
    /// Revoke the saved CLI login and remove its local credential
    Logout {
        /// Forget only this device's credential while the server is unreachable; does not revoke it
        #[arg(long)]
        local: bool,
    },
    /// Refresh the saved login and show verified identity; exit 1 when signed out
    #[command(alias = "status")]
    Whoami,
    /// Open Keycloak's account management pages
    Manage(Page),
    /// Open the password/security page; finish the change in your browser
    Password(Page),
    /// Open the account page; choose Delete Account and confirm there
    Delete(Page),
}
fn emit(json_output: bool, value: Value, message: &str, code: i32) -> Result<i32, String> {
    if json_output {
        println!("{value}");
    } else {
        println!("{message}");
    }
    Ok(code)
}
fn signed_out(json_output: bool) -> Result<i32, String> {
    emit(
        json_output,
        json!({"authenticated": false, "account": null}),
        "Not signed in. Run cppbench account login.",
        1,
    )
}
fn saved_provider(store: &Store, issuer: Option<&str>) -> Result<Provider, String> {
    let saved = store.state.session.as_ref().map(|s| s.issuer.as_str());
    let provider = Provider::new(issuer.or(saved).unwrap_or(DEFAULT_ISSUER))?;
    if saved.is_some_and(|saved| saved != provider.issuer) {
        return Err(
            "saved login belongs to another issuer; log out first or use a separate --data-dir"
                .into(),
        );
    }
    Ok(provider)
}
pub(crate) fn run(options: Options) -> Result<i32, String> {
    let dir = match options.data_dir {
        Some(dir) => dir,
        None => store::default_dir()?,
    };
    let json_output = options.json;
    match options.action {
        Action::Login(ref login) | Action::Register(ref login) => {
            let store = Store::open(dir.clone())?;
            let provider = saved_provider(&store, options.issuer.as_deref())?;
            if store.state.session.is_some() {
                return Err("a CLI login is already saved; run cppbench account logout before switching accounts".into());
            }
            let revision = store.state.revision;
            drop(store); // Never keep a file lock while waiting for a human in the browser.
            let auth = browser::authorize(
                &provider,
                matches!(options.action, Action::Register(_)),
                login.username.as_deref(),
                login.no_browser,
                login.callback_port,
                login.timeout,
            )?;
            let mut tokens = provider
                .exchange(&auth.code, &auth.verifier, &auth.redirect_uri)
                .map_err(String::from)?;
            if !tokens
                .scope
                .split_whitespace()
                .any(|s| s == "offline_access")
            {
                return Err(
                    "Keycloak did not grant offline_access; enable it for cppbench-cli".into(),
                );
            }
            let account = provider
                .identity(&tokens.access_token)
                .map_err(String::from)?;
            let refresh_token = tokens
                .refresh_token
                .take()
                .filter(|t| !t.is_empty())
                .ok_or("Keycloak did not return a persistent login token")?;
            let mut store = Store::open(dir)?;
            if store.state.revision != revision {
                let _ = provider.revoke(&refresh_token);
                return Err(
                    "account state changed during browser sign-in; retry the command".into(),
                );
            }
            store.state.session = Some(Session {
                issuer: provider.issuer,
                refresh_token,
            });
            store.save()?;
            emit(
                json_output,
                json!({"authenticated": true, "account": account, "persistent_login": true}),
                &format!(
                    "Signed in as {}. CLI login saved; use cppbench account logout to sign out.",
                    account["username"].as_str().unwrap()
                ),
                0,
            )
        }
        Action::Whoami => {
            let mut store = Store::open(dir)?;
            let provider = saved_provider(&store, options.issuer.as_deref())?;
            let Some(session) = &store.state.session else {
                return signed_out(json_output);
            };
            let result = provider.refresh(&session.refresh_token);
            let mut tokens = match result {
                Ok(tokens) => tokens,
                Err(Error::SignedOut) => {
                    store.state.session = None;
                    store.save()?;
                    return signed_out(json_output);
                }
                Err(error) => return Err(error.into()),
            };
            if let Some(refresh_token) = tokens.refresh_token.take() {
                if refresh_token.is_empty() {
                    return Err("Keycloak returned an empty refresh token".into());
                }
                store.state.session = Some(Session {
                    issuer: provider.issuer.clone(),
                    refresh_token,
                });
                store.save()?;
            }
            let account = match provider.identity(&tokens.access_token) {
                Ok(account) => account,
                Err(Error::SignedOut) => {
                    store.state.session = None;
                    store.save()?;
                    return signed_out(json_output);
                }
                Err(error) => return Err(error.into()),
            };
            emit(
                json_output,
                json!({"authenticated": true, "account": account, "persistent_login": true}),
                &format!(
                    "Signed in as {} (account {}).",
                    account["username"].as_str().unwrap(),
                    account["id"].as_str().unwrap()
                ),
                0,
            )
        }
        Action::Logout { local } => {
            let mut store = Store::open(dir)?;
            if !local {
                let provider = saved_provider(&store, options.issuer.as_deref())?;
                if let Some(session) = &store.state.session {
                    provider
                        .revoke(&session.refresh_token)
                        .map_err(String::from)?;
                }
            }
            store.state.session = None;
            store.save()?;
            let message = if local {
                "Local login forgotten. Server-side authorization was not revoked; revoke it in Keycloak's account console when available."
            } else {
                "Signed out of the CLI; saved authorization revoked. Browser sessions are managed separately in Keycloak."
            };
            emit(
                json_output,
                json!({"ok": true, "authenticated": false, "remote_revocation": !local}),
                message,
                0,
            )
        }
        Action::Manage(ref page) | Action::Password(ref page) | Action::Delete(ref page) => {
            let store = Store::open(dir)?;
            let provider = saved_provider(&store, options.issuer.as_deref())?;
            let suffix = if matches!(options.action, Action::Password(_)) {
                "account-security/signing-in"
            } else {
                ""
            };
            let url = format!("{}/account/{suffix}", provider.issuer);
            browser::open(&url, page.no_browser);
            let message = match options.action {
                Action::Delete(_) => "Choose Delete Account in the browser and confirm there. The account is not deleted by opening this page. Run whoami afterward to clear a revoked CLI login.",
                Action::Password(_) => "Change your password under Signing in in the browser. This command only opens the page.",
                _ => "Manage your account, sessions, and app access in the browser.",
            };
            emit(
                json_output,
                json!({"opened": true, "url": url, "action_completed": false}),
                message,
                0,
            )
        }
    }
}
