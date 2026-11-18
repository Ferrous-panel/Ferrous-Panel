use crate::auth::hex_encode;
use crate::error::PanelError;
use argon2::password_hash::rand_core::OsRng as ArgonOsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use subtle::ConstantTimeEq;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const INITIAL_PASSWORD_LEN: usize = 24;
const PASSWORD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#%^&*";

/// Wrapper that prevents secrets from ever appearing in Debug logs.
#[derive(Clone, PartialEq, Eq)]
pub struct RedactedSecret(String);

impl RedactedSecret {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RedactedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RedactedSecret([REDACTED])")
    }
}

/// Persisted administrator credentials file containing only Argon2id hash (never plaintext).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCredentialsFile {
    pub username: String,
    pub password_hash: String,
    pub created_at: String,
}

/// Constant-time slice comparison backed by `subtle`.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

/// Generates a 32-byte (`64` hex chars) bearer session token from kernel CSPRNG `OsRng`.
pub fn generate_session_token() -> Result<RedactedSecret, PanelError> {
    let mut bytes = [0u8; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| PanelError::Internal("OS entropy source unavailable".to_string()))?;
    Ok(RedactedSecret::new(hex_encode(&bytes)))
}

/// Generates an initial random administrator password from `OsRng` (24 chars, unbiased).
pub fn generate_initial_password() -> Result<RedactedSecret, PanelError> {
    let mut raw = [0u8; INITIAL_PASSWORD_LEN];
    OsRng
        .try_fill_bytes(&mut raw)
        .map_err(|_| PanelError::Internal("OS entropy source unavailable".to_string()))?;

    let mut out = String::with_capacity(INITIAL_PASSWORD_LEN);
    for b in raw {
        let idx = usize::from(b & 0x3f);
        if let Some(&ch) = PASSWORD_ALPHABET.get(idx) {
            out.push(char::from(ch));
        }
    }
    Ok(RedactedSecret::new(out))
}

/// Hashes a password with Argon2id and an OS-entropy salt.
pub fn hash_password(plain: &str) -> Result<String, PanelError> {
    if !(8..=256).contains(&plain.len()) {
        return Err(PanelError::Validation(
            "password length must be between 8 and 256 characters".to_string(),
        ));
    }
    let salt = SaltString::generate(&mut ArgonOsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| PanelError::Internal("failed to hash password".to_string()))
}

/// Verifies a candidate password against an Argon2id PHC hash string.
#[must_use]
pub fn verify_password(plain: &str, phc_hash: &str) -> bool {
    if plain.is_empty() || phc_hash.is_empty() {
        return false;
    }
    let Ok(parsed) = PasswordHash::new(phc_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

/// Loads existing `0600` credentials or creates a new random-password Argon2id file on first run.
///
/// Returns `Some(RedactedSecret)` containing the one-time generated plaintext password
/// ONLY when creating the credentials file for the first time.
pub fn load_or_init_credentials(
    path: &Path,
    default_username: &str,
) -> Result<(AdminCredentialsFile, Option<RedactedSecret>), PanelError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            std::fs::set_permissions(parent, PermissionsExt::from_mode(0o700))?;
        }
    }

    if path.exists() {
        #[cfg(unix)]
        std::fs::set_permissions(path, PermissionsExt::from_mode(0o600))?;
        let raw = std::fs::read_to_string(path)?;
        let creds: AdminCredentialsFile = serde_json::from_str(&raw)?;
        PasswordHash::new(&creds.password_hash).map_err(|_| {
            PanelError::Internal("stored credentials contain invalid Argon2id hash".to_string())
        })?;
        return Ok((creds, None));
    }

    let clean_user = default_username.trim();
    if clean_user.is_empty() {
        return Err(PanelError::Validation(
            "admin username cannot be empty".to_string(),
        ));
    }

    let generated = generate_initial_password()?;
    let password_hash = hash_password(generated.expose())?;
    let creds = AdminCredentialsFile {
        username: clean_user.to_string(),
        password_hash,
        created_at: Utc::now().to_rfc3339(),
    };
    let json_bytes = serde_json::to_vec_pretty(&creds)?;

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    opts.mode(0o600);

    match opts.open(path) {
        Ok(mut file) => {
            file.write_all(&json_bytes)?;
            file.sync_all()?;
            #[cfg(unix)]
            std::fs::set_permissions(path, PermissionsExt::from_mode(0o600))?;
            Ok((creds, Some(generated)))
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let raw = std::fs::read_to_string(path)?;
            let existing: AdminCredentialsFile = serde_json::from_str(&raw)?;
            Ok((existing, None))
        }
        Err(err) => Err(PanelError::Io(err)),
    }
}

fn write_credentials_atomic(path: &Path, creds: &AdminCredentialsFile) -> Result<(), PanelError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            std::fs::set_permissions(parent, PermissionsExt::from_mode(0o700))?;
        }
    }
    let json_bytes = serde_json::to_vec_pretty(creds)?;
    let tmp_path = path.with_extension("json.tmp");
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut file = opts.open(&tmp_path)?;
    file.write_all(&json_bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp_path, path)?;
    #[cfg(unix)]
    std::fs::set_permissions(path, PermissionsExt::from_mode(0o600))?;
    Ok(())
}

/// Atomically updates the administrator Argon2id password hash on disk (`0600`).
pub fn update_admin_password(
    path: &Path,
    default_username: &str,
    new_plain_password: &str,
) -> Result<AdminCredentialsFile, PanelError> {
    let (mut creds, _) = load_or_init_credentials(path, default_username)?;
    creds.password_hash = hash_password(new_plain_password)?;
    write_credentials_atomic(path, &creds)?;
    Ok(creds)
}

/// Atomically updates the administrator username in the `0600` credentials file.
pub fn update_admin_username(
    path: &Path,
    default_username: &str,
    new_username: &str,
) -> Result<AdminCredentialsFile, PanelError> {
    let clean_user = new_username.trim();
    if clean_user.len() < 3 || clean_user.len() > 64 || !clean_user.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(PanelError::Validation(
            "username must be 3..=64 ASCII alphanumeric, '_' or '-' characters".to_string(),
        ));
    }
    let (mut creds, _) = load_or_init_credentials(path, default_username)?;
    creds.username = clean_user.to_string();
    write_credentials_atomic(path, &creds)?;
    Ok(creds)
}
