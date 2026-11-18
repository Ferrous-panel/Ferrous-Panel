#![forbid(unsafe_code)]

pub mod error;
pub mod security;
pub mod system;
pub mod types;

#[cfg(test)]
mod tests {
    use crate::error::CoreError;
    use crate::security::password::{
        constant_time_eq, generate_random_hex_token, hash_password, verify_password, RedactedSecret,
    };
    use crate::system::cmd::{self, AllowedBinary};
    use crate::types::{Domain, Port, SafePath, ServiceName, WebServerEngine};
    use std::path::Path;
    use std::time::Duration;

    fn ensure(cond: bool, msg: &str) -> Result<(), CoreError> {
        if cond {
            Ok(())
        } else {
            Err(CoreError::Validation(msg.to_string()))
        }
    }

    #[test]
    fn domain_validation_rejects_injection_and_accepts_fqdn() -> Result<(), CoreError> {
        let ok = Domain::parse("Blog.Example-Host.vn.")?;
        ensure(ok.as_str() == "blog.example-host.vn", "domain should normalize lowercase")?;

        ensure(Domain::parse("localhost").is_err(), "single label must fail")?;
        ensure(Domain::parse("-bad.example.com").is_err(), "leading hyphen must fail")?;
        ensure(Domain::parse("bad;rm -rf.com").is_err(), "shell chars must fail")?;
        ensure(Domain::parse("a..b.com").is_err(), "empty label must fail")?;
        Ok(())
    }

    #[test]
    fn service_name_and_port_validation() -> Result<(), CoreError> {
        let svc = ServiceName::parse("My-App.service")?;
        ensure(svc.as_str() == "my-app", "strip .service suffix")?;
        ensure(svc.unit_file_name() == "my-app.service", "unit filename matches")?;

        ensure(ServiceName::parse("../evil").is_err(), "traversal must fail")?;
        ensure(ServiceName::parse("-leading").is_err(), "leading dash must fail")?;
        ensure(ServiceName::parse("bad name").is_err(), "space must fail")?;

        let port = Port::new(443)?;
        ensure(port.get() == 443 && port.is_privileged(), "443 is valid privileged port")?;
        ensure(Port::new(0).is_err(), "port 0 must be rejected")?;
        Ok(())
    }

    #[test]
    fn safe_path_blocks_traversal_and_prefix_escape() -> Result<(), CoreError> {
        let root = Path::new("/www/wwwroot");
        let ok = SafePath::under_root(root, "example.com/public")?;
        ensure(
            ok.as_path() == Path::new("/www/wwwroot/example.com/public"),
            "relative subpath joins cleanly",
        )?;

        ensure(
            SafePath::under_root(root, "../etc/passwd").is_err(),
            "parent traversal must be blocked",
        )?;
        ensure(
            SafePath::under_root(root, "/www/wwwroot-other/file").is_err(),
            "sibling prefix escape must be blocked",
        )?;
        ensure(
            SafePath::under_root(root, "site\0evil").is_err(),
            "NUL byte must be blocked",
        )?;
        Ok(())
    }

    #[test]
    fn webserver_enum_matches_ui_json_strings() -> Result<(), CoreError> {
        let json_fx = serde_json::to_string(&WebServerEngine::FerrousX)?;
        let json_nx = serde_json::to_string(&WebServerEngine::Nginx)?;
        ensure(
            json_fx == "\"FerrousX (Rust Web Server)\"",
            "FerrousX label must match UI",
        )?;
        ensure(json_nx == "\"Nginx 1.26\"", "Nginx label must match UI")?;
        Ok(())
    }

    #[test]
    fn argon2_password_and_redacted_secret() -> Result<(), CoreError> {
        let hash = hash_password("StrongTestPass!2026")?;
        ensure(hash.starts_with("$argon2id$"), "must produce Argon2id PHC hash")?;
        ensure(verify_password("StrongTestPass!2026", &hash)?, "valid password verifies")?;
        ensure(!verify_password("WrongPass!2026", &hash)?, "wrong password rejected")?;
        ensure(hash_password("short").is_err(), "short password rejected")?;

        let token = generate_random_hex_token(32)?;
        ensure(token.expose().len() == 64, "32 bytes = 64 hex chars")?;
        let dbg = format!("{token:?}");
        ensure(!dbg.contains(token.expose()), "Debug output must redact secret")?;

        let secret = RedactedSecret::new("hidden-val".to_string());
        ensure(
            format!("{secret:?}") == "RedactedSecret([REDACTED])",
            "RedactedSecret Debug must not leak content",
        )?;
        ensure(constant_time_eq(b"abc123", b"abc123"), "equal slices match")?;
        ensure(!constant_time_eq(b"abc123", b"abc124"), "different slices fail")?;
        Ok(())
    }

    #[tokio::test]
    async fn allowlisted_cmd_runs_uname() -> Result<(), CoreError> {
        let out = cmd::run_checked(AllowedBinary::Uname, &["-s"], Duration::from_secs(3)).await?;
        ensure(!out.stdout.is_empty(), "uname -s must return OS name")?;
        ensure(
            cmd::run(AllowedBinary::Uname, &["bad\0arg"], Duration::from_secs(1))
                .await
                .is_err(),
            "NUL byte in arg must be rejected",
        )?;
        Ok(())
    }
}
