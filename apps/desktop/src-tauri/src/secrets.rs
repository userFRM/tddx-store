//! Persistent secret storage backed by tauri-plugin-stronghold.
//!
//! The vault password is derived from a per-machine 32-byte salt stored at
//! `$HOME/tddx-store/.salt` (created on first run). This gives us
//! encrypted-at-rest persistence ("remember me") without prompting the
//! user for a master password every launch — the trade-off being that
//! someone with read access to BOTH the salt file AND the vault could
//! decrypt the secrets. Acceptable for a desktop app secrets store; if
//! you want stronger guarantees, swap the salt for a user-supplied
//! master password and prompt on launch.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use std::sync::OnceLock;

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Process-wide salt cache. The first caller does the disk read /
/// generation; later callers see the same `Vec<u8>` even if two
/// `ensure_salt()` calls race during startup. Without this cache the
/// concurrent `LoginGate` mount and `Settings.svelte` mount both try
/// to write a new salt at the same time, only the last write wins,
/// and any existing vault sealed against the discarded salt becomes
/// unrecoverable.
static SALT_CACHE: OnceLock<Vec<u8>> = OnceLock::new();
static SALT_INIT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Tauri's per-platform `app_data_dir()` resolved at app startup and
/// pushed in via `set_data_dir`. We avoid hardcoding `$HOME/...` paths
/// so the install lives where each OS expects: macOS
/// `~/Library/Application Support/io.userfrm.tddx-store`, Linux
/// `~/.local/share/io.userfrm.tddx-store` (XDG), Windows
/// `%APPDATA%\io.userfrm.tddx-store`.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

const VAULT_FILENAME: &str = "credentials.stronghold";
const SALT_FILENAME: &str = ".salt";

/// Called once during `tauri::Builder::setup` with the resolved
/// `app.path().app_data_dir()`. Idempotent — the first set wins, so
/// any later setup-hook re-runs (hot-reload in dev) leave the value
/// stable.
pub fn set_data_dir(p: PathBuf) {
    let _ = DATA_DIR.set(p);
}

pub fn data_dir() -> PathBuf {
    DATA_DIR.get().cloned().unwrap_or_else(|| {
        // Fallback for tests / CLI callers that don't go through the
        // Tauri runtime. Uses XDG-style data home on Unix, current
        // working dir on platforms without `HOME`.
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(format!("{home}/.local/share/io.userfrm.tddx-store"))
    })
}

pub fn salt_path() -> PathBuf {
    data_dir().join(SALT_FILENAME)
}

pub fn vault_path() -> PathBuf {
    data_dir().join(VAULT_FILENAME)
}

/// Read the per-machine salt. Generates one on first call. Subsequent
/// calls return the cached bytes — never re-reads or re-generates the
/// file once a process has resolved its salt. Concurrent first-callers
/// both go through `OnceLock::get_or_init`, so only one disk write
/// happens per process lifetime.
pub fn ensure_salt() -> std::io::Result<Vec<u8>> {
    if let Some(cached) = SALT_CACHE.get() {
        return Ok(cached.clone());
    }
    let _guard = SALT_INIT_LOCK
        .lock()
        .map_err(|_| std::io::Error::other("salt init lock poisoned"))?;
    if let Some(cached) = SALT_CACHE.get() {
        return Ok(cached.clone());
    }
    let bytes = read_or_create_salt()?;
    match SALT_CACHE.set(bytes.clone()) {
        Ok(()) => Ok(bytes),
        Err(_) => SALT_CACHE
            .get()
            .cloned()
            .ok_or_else(|| std::io::Error::other("salt cache missing after initialization")),
    }
}

fn read_or_create_salt() -> std::io::Result<Vec<u8>> {
    let dir = data_dir();
    fs::create_dir_all(&dir)?;
    let path = salt_path();
    // Fast path: file exists with the right length, just read it.
    if path.exists() {
        return read_existing_salt_with_retry(&path);
    }
    // Slow path: try to create the file exclusively. If two threads /
    // processes race, only one wins the create, the loser re-reads the
    // winner's file. Eliminates the "concurrent first-launch racers
    // overwrite each other's salt" failure mode.
    use std::io::Write;
    let mut salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut salt);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut f) => {
            f.write_all(&salt)?;
            f.sync_all()?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Loser of the race: wait for the winner's write+fsync to
            // finish, then read the completed bytes.
            return read_existing_salt_with_retry(&path);
        }
        Err(e) => return Err(e),
    }
    // Tighten permissions on Unix: salt = secret enough.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&path)?.permissions();
        perm.set_mode(0o600);
        fs::set_permissions(&path, perm)?;
    }
    Ok(salt.to_vec())
}

fn read_existing_salt_with_retry(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let mut last_len = None;
    for _ in 0..20 {
        let mut buf = Vec::new();
        fs::File::open(path)?.read_to_end(&mut buf)?;
        if buf.len() == 32 {
            return Ok(buf);
        }
        last_len = Some(buf.len());
        std::thread::sleep(Duration::from_millis(10));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "salt file at {} exists but is {} bytes, expected 32",
            path.display(),
            last_len.unwrap_or(0)
        ),
    ))
}

/// Hash function passed to the Stronghold plugin. We pre-mix the salt
/// once to derive the actual vault key so two installs of the
/// TdDx Store on different machines have different keys.
///
/// Fails closed: if the salt cannot be read or generated, return a
/// sentinel that's distinct from any plausible real salt-derived key.
/// This guarantees we never silently fall back to an empty salt (which
/// would derive a *different* key, stranding any existing vault sealed
/// under the real salt). The Stronghold plugin will then fail to
/// unlock the vault and the UI surfaces the error rather than
/// pretending nothing happened.
pub fn vault_hasher_fn() -> impl Fn(&str) -> Vec<u8> + Send + Sync + 'static {
    move |raw_password: &str| {
        let salt = match ensure_salt() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "salt read failed; refusing to derive key with empty fallback");
                // Distinct, fail-closed sentinel. Stronghold will reject
                // any vault opened with this — that's the point.
                let mut hasher = Sha256::new();
                hasher.update(b"tdds-salt-error-sentinel-v1");
                hasher.update(format!("{e}").as_bytes());
                return hasher.finalize().to_vec();
            }
        };
        let mut hasher = Sha256::new();
        hasher.update(b"tdds-vault-v1");
        hasher.update(&salt);
        hasher.update(raw_password.as_bytes());
        hasher.finalize().to_vec()
    }
}

/// Stable string the frontend sends as the "vault password" — the real
/// entropy comes from `salt`. Fails closed: if the salt is unreadable,
/// returns an Err so the Tauri command can surface the failure to the
/// UI rather than silently returning a key derived from empty bytes.
pub fn auto_password_hex() -> std::io::Result<String> {
    let salt = ensure_salt()?;
    let mut hasher = Sha256::new();
    hasher.update(b"tdds-auto-password-v1");
    hasher.update(&salt);
    let h = hasher.finalize();
    Ok(hex::encode(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_salt_is_race_safe() {
        let home =
            std::env::temp_dir().join(format!("tdds-secrets-tests-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(9));
        let mut joins = Vec::new();
        for _ in 0..8 {
            let barrier = barrier.clone();
            joins.push(std::thread::spawn(move || {
                barrier.wait();
                ensure_salt().unwrap()
            }));
        }
        barrier.wait();

        let salts: Vec<Vec<u8>> = joins.into_iter().map(|join| join.join().unwrap()).collect();
        assert_eq!(salts.len(), 8);
        for salt in &salts {
            assert_eq!(salt.len(), 32);
            assert_eq!(salt, &salts[0]);
        }
        let disk = std::fs::read(salt_path()).unwrap();
        assert_eq!(disk.len(), 32);
        assert_eq!(disk, salts[0]);

        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        std::fs::remove_dir_all(home).unwrap();
    }
}
