pub mod middleware;
pub mod password;
pub mod rate_limit;
pub mod store;

/// Lowercase hex encoding without extra crate dependencies.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
