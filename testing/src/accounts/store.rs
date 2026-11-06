//! Private OAuth credential cache. Keycloak owns account records and passwords.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Session {
    pub issuer: String,
    pub refresh_token: String,
}
impl Drop for Session {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.refresh_token.zeroize();
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct State {
    pub schema_version: u32,
    pub revision: u64,
    pub session: Option<Session>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: 2,
            revision: 0,
            session: None,
        }
    }
}
pub(super) struct Store {
    dir: PathBuf,
    _lock: File,
    pub state: State,
}
fn private(metadata: &fs::Metadata, directory: bool) -> Result<(), String> {
    // SAFETY: geteuid has no arguments or memory safety preconditions.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || (directory && !metadata.is_dir())
        || (!directory && (!metadata.is_file() || metadata.nlink() != 1))
    {
        return Err("account storage must be owned by you, with private directories (0700) and regular files (0600); symlinks and hard links are not accepted".into());
    }
    Ok(())
}
fn open_private(path: &Path, create: bool) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| format!("cannot open account file {}: {e}", path.display()))?;
    private(&file.metadata().map_err(|e| e.to_string())?, false)?;
    Ok(file)
}
impl Store {
    pub fn open(dir: PathBuf) -> Result<Self, String> {
        if !dir.is_absolute() {
            return Err("account directory must be an absolute path".into());
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|e| format!("cannot create account directory: {e}"))?;
        private(
            &fs::symlink_metadata(&dir).map_err(|e| e.to_string())?,
            true,
        )?;
        let lock = open_private(&dir.join("session.lock"), true)?;
        lock.lock()
            .map_err(|e| format!("cannot lock account store: {e}"))?;
        let path = dir.join("session.json");
        let state = match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => return Err(e.to_string()),
            Ok(_) => {
                let file = open_private(&path, false)?;
                let mut bytes = zeroize::Zeroizing::new(Vec::new());
                file.take(MAX_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() as u64 > MAX_BYTES {
                    return Err("account store exceeds 1 MiB".into());
                }
                let state: State = serde_json::from_slice(&bytes)
                    .map_err(|_| "invalid account store; existing data has been left intact")?;
                validate(&state)?;
                state
            }
        };
        Ok(Self {
            dir,
            _lock: lock,
            state,
        })
    }
    pub fn save(&mut self) -> Result<(), String> {
        self.state.revision = self
            .state
            .revision
            .checked_add(1)
            .ok_or("session revision overflow")?;
        validate(&self.state)?;
        let bytes = zeroize::Zeroizing::new(
            serde_json::to_vec_pretty(&self.state).map_err(|e| e.to_string())?,
        );
        if bytes.len() as u64 > MAX_BYTES {
            return Err("account store exceeds 1 MiB".into());
        }
        let mut temp = tempfile::NamedTempFile::new_in(&self.dir).map_err(|e| e.to_string())?;
        temp.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        temp.write_all(&bytes).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(self.dir.join("session.json"))
            .map_err(|e| e.to_string())?;
        File::open(&self.dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    }
}
fn validate(state: &State) -> Result<(), String> {
    if state.schema_version != 2 {
        return Err("unsupported session store schema version".into());
    }
    if let Some(session) = &state.session {
        super::provider::Provider::validate_issuer(&session.issuer)?;
        if session.refresh_token.is_empty() || session.refresh_token.len() > 65536 {
            return Err("invalid saved session token".into());
        }
    }
    Ok(())
}
pub(super) fn default_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Ok(path.join("cppbench/accounts"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|p| p.join(".local/share/cppbench/accounts"))
        .ok_or_else(|| {
            "set HOME or XDG_DATA_HOME to an absolute path, or use account --data-dir".into()
        })
}
