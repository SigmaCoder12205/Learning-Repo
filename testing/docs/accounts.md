# Keycloak accounts

cppbench 0.8 uses **Keycloak** for account records, registration, password handling, login pages, account management, and revocation. PostgreSQL stores the service's data. The Rust CLI is only an OAuth client; it no longer creates accounts or hashes passwords. Benchmarking stays Rust/C and requires neither login nor a running account service.

## Start locally

Requirements: Docker with Compose, permission to access the Docker daemon, and a browser. `xdg-open` opens the browser automatically when available. No Java, PostgreSQL, Node, or Python runtime needs to be installed for the application: Keycloak and PostgreSQL run in containers.

From this checkout:

```bash
./auth/scripts/setup.sh
make install
cppbench account register
cppbench account whoami
cppbench account logout
cppbench account login
```

The setup script generates private bootstrap credentials in `auth/.env`, starts the containers, and waits for readiness. It is safe to rerun and does not replace credentials or database contents. The service is available only on the loopback address `http://127.0.0.1:8787`. This is a local development deployment; exposing it publicly requires a separate HTTPS/production configuration.

Registration opens Keycloak's registration page and signs the CLI in on completion. Passwords are entered only in the browser. The default policy requires at least 12 characters. Keycloak provides the account form, including username/profile/email fields. Email verification and email-based password recovery are not configured yet because no SMTP service has been set up.

`login` opens the browser login page. An optional username is a hint: `cppbench account login alice`. Registration also accepts a hint. Use `--no-browser` to print the URL and open it manually **on the same machine**. The CLI listens on `127.0.0.1:8766/callback` while authorizing; if this port is occupied, free it or configure another allowed redirect URI and pass `--callback-port PORT`.

Authentication uses Authorization Code with PKCE (S256), a random state value, and a public client with no embedded client secret. Password/direct-access grants are disabled for `cppbench-cli`. There is no `--password-stdin` or CLI password storage anymore. Each browser attempt waits up to five minutes; `--timeout SECONDS` changes that wait only.

## Account and login lifetime

**Accounts have no expiration date.** They remain in PostgreSQL across terminal exits, reboots, logouts, and service restarts until deleted or disabled by a user/administrator. There is no seven-day account or CLI login deadline.

The CLI requests `offline_access` and stores the offline refresh credential. On `whoami`, it refreshes through Keycloak and fetches verified identity from the userinfo endpoint. Access tokens are short-lived (five minutes) and are never printed. Their expiry does not mean the account expires or require a new interactive login.

The supplied realm disables the maximum offline-session lifespan. Keycloak still requires an offline **inactivity timeout**: it is set to 315,360,000 seconds (ten 365-day years), renewed whenever the credential is used. This is a long-lived login, not a literal promise that a credential can remain unused forever. In Keycloak 26.8, zero restores the default timeout rather than disabling it. Deletion, revocation, administrative policy changes, or losing the saved credential can require a fresh login. See [Keycloak offline access](https://www.keycloak.org/docs/latest/server_admin/#offline-access) and [the timeout implementation](https://github.com/keycloak/keycloak/blob/26.8.2/server-spi-private/src/main/java/org/keycloak/models/utils/SessionExpirationUtils.java).

## Commands

| Command | Behavior |
| --- | --- |
| `cppbench account register [USERNAME]` | Open registration and retain the resulting CLI login |
| `cppbench account login [USERNAME]` | Open login and retain the resulting CLI login |
| `cppbench account whoami` | Refresh and verify the current identity; alias: `status` |
| `cppbench account logout` | Revoke the CLI authorization in Keycloak, then remove the local credential |
| `cppbench account logout --local` | Forget the credential without contacting Keycloak; remote authorization remains until revoked in the account console |
| `cppbench account manage` | Open Keycloak's account console |
| `cppbench account password` | Open the Signing in page; finish changing your password there |
| `cppbench account delete` | Open the account page; choose Delete Account and confirm there |

`manage`, `password`, and `delete` accept `--no-browser`. Opening a page does not claim that an account mutation completed. These pages use the **browser's** identity, which may differ from the CLI's; check the displayed username before changing or deleting an account. After deletion, `whoami` detects revocation and clears the saved CLI login.

One CLI login is saved per local cache directory. Log out before switching identities. Logout affects the CLI authorization; browser sessions are separate and can be ended in Keycloak's account console. An ordinary logout that cannot reach Keycloak fails while retaining the credential so revocation can be retried. `--local` explicitly bypasses server revocation. Service/network errors never silently erase a saved login.

`--json` is available anywhere after `account`. Browser instructions go to stderr; stdout contains only the public result. Authentication errors and operational errors go to stderr as text. Public identity output includes Keycloak's stable account ID, username, and email if available. Tokens are not included. Exit codes are `0` for success (including opening a management page), `1` for signed-out/revoked identity on `whoami`, and `2` for usage, service, callback, permission, or storage failures.

## Persistence and configuration

Keycloak and PostgreSQL run under Compose project `cppbench-auth`. PostgreSQL uses the named volume `cppbench-auth_database`. Start/stop commands from the checkout:

```bash
docker compose -f auth/compose.yaml ps
docker compose -f auth/compose.yaml stop
./auth/scripts/setup.sh
```

Stopping or recreating containers preserves accounts. `docker compose down -v` removes the database volume and **deletes the service's account data**; do not use it as a routine restart command. Container restart policies bring the services back when Docker runs. Starting Docker automatically at boot is an OS setting, separate from account lifetime.

`auth/realm.json` configures the `cppbench` realm and `cppbench-cli` public client. Startup imports it only when the realm does not already exist. Editing the file does not update a live realm: apply changes through the admin console/API as well, preserving user data. Container images are pinned by digest for reproducibility; update deliberately with the integration test below.

The admin console is at `http://127.0.0.1:8787/admin/`; bootstrap username is `admin`, with its generated password in `auth/.env`. Treat this as an operator credential, separate from your cppbench user. Keep `.env` private and out of Git; it is ignored. Do not regenerate it for an existing database without coordinating the database credential change.

The CLI credential cache defaults to `$XDG_DATA_HOME/cppbench/accounts` if XDG_DATA_HOME is absolute, otherwise `$HOME/.local/share/cppbench/accounts`. It contains `session.json` and `session.lock`, not an account database. The directory is private (`0700`), files are private (`0600`), writes are atomic and locked, and reads are bounded. Symlink/hard-link files and unsafe permissions are rejected. Password/token buffers are cleared where cppbench controls them; this does not guarantee erasure of every OS/library copy. The credential is not encrypted and other programs running as your OS user can access it.

Use `--data-dir /absolute/private/path` for a separate login cache. `--issuer URL` selects another Keycloak realm configured with the same public client and callback. HTTPS is required except on loopback. The issuer is bound to saved credentials; passing a different issuer does not send the old credential to it. Custom realms control their own session and account policies. `whoami` requires the account service even though normal benchmarks work independently.

## Replacing version 0.7

The Rust password database and seven-day login implementation have been removed. Version 0.8 never reads or authenticates against `accounts.json`, and no accounts are migrated into Keycloak. The existing default legacy account store was deleted during this upgrade, as requested. Custom `--data-dir` stores from old versions, if any exist elsewhere, are inert; delete their `accounts.json` separately. Never copy old password hashes into the new service. Existing benchmark artifacts and reports are unaffected.

## Verification

`make verify` exercises OAuth callbacks/state/PKCE, refresh, revocation, failed services, private storage, and the benchmark regression suite using mock HTTP responses. It needs no Docker or browser.

For the optional integration test, start the local service, build the CLI, and run:

```bash
cargo build --locked
python3 auth/tests/smoke.py --restart
```

Python is only a development test dependency. This test drives Keycloak's real browser forms over HTTP, verifies registration/login/refresh/revocation, restarts Keycloak to verify persistence, and deletes its uniquely named temporary account in cleanup. It reads bootstrap credentials privately from `auth/.env`; no credentials or tokens are printed. Run it against the development service, without simultaneous interactive login on callback port 8766.
