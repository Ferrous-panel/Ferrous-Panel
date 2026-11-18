use crate::error::CoreError;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use std::fmt;

/// Secret-redacting wrapper for credentials/tokens so secrets are never printed to logs.
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

/// Constant-time byte slice comparison (XOR accumulator) to resist timing attacks.
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Hashes a password with Argon2id and a kernel CSPRNG salt (`OsRng`).
pub fn hash_password(plain: &str) -> Result<String, CoreError> {
    if !(8..=256).contains(&plain.len()) {
        return Err(CoreError::Validation(
            "password length must be between 8 and 256 characters".to_string(),
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| CoreError::Auth("failed to hash password"))
}

/// Verifies a plaintext password against a stored Argon2id PHC hash string.
pub fn verify_password(plain: &str, phc_hash: &str) -> Result<bool, CoreError> {
    if plain.is_empty() || phc_hash.is_empty() {
        return Ok(false);
    }
    let parsed =
        PasswordHash::new(phc_hash).map_err(|_| CoreError::Auth("invalid password hash format"))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

/// Generates `num_bytes` of OS entropy formatted as lowercase hex (`2 * num_bytes` chars).
pub fn generate_random_hex_token(num_bytes: usize) -> Result<RedactedSecret, CoreError> {
    if !(16..=128).contains(&num_bytes) {
        return Err(CoreError::Validation(
            "token byte length must be between 16 and 128".to_string(),
        ));
    }
    let mut buf = vec![0u8; num_bytes];
    OsRng
        .try_fill_bytes(&mut buf)
        .map_err(|_| CoreError::Auth("OS entropy source unavailable"))?;

    let mut hex = String::with_capacity(num_bytes * 2);
    for byte in buf {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(RedactedSecret::new(hex))
}
