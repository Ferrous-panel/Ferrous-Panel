use crate::error::CoreError;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Strictly allowlisted Ubuntu 22.04/24.04 system binaries.
///
/// Shells (`sh`, `bash`, `zsh`, `dash`) are intentionally absent and forbidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedBinary {
    Systemctl,
    Nginx,
    Ferrousx,
    FerrousxCtl,
    PhpFpm83,
    Php83,
    Ufw,
    Fail2banClient,
    Mysql,
    Psql,
    Composer,
    WpCli,
    AptGet,
    Curl,
    Tar,
    Chown,
    Chmod,
    Install,
    Sysctl,
    Ss,
    Df,
    Id,
    Uname,
}

impl AllowedBinary {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Systemctl => "systemctl",
            Self::Nginx => "nginx",
            Self::Ferrousx => "ferrousx-bin",
            Self::FerrousxCtl => "ferrousx-ctl",
            Self::PhpFpm83 => "php-fpm8.3",
            Self::Php83 => "php8.3",
            Self::Ufw => "ufw",
            Self::Fail2banClient => "fail2ban-client",
            Self::Mysql => "mysql",
            Self::Psql => "psql",
            Self::Composer => "composer",
            Self::WpCli => "wp",
            Self::AptGet => "apt-get",
            Self::Curl => "curl",
            Self::Tar => "tar",
            Self::Chown => "chown",
            Self::Chmod => "chmod",
            Self::Install => "install",
            Self::Sysctl => "sysctl",
            Self::Ss => "ss",
            Self::Df => "df",
            Self::Id => "id",
            Self::Uname => "uname",
        }
    }

    #[must_use]
    const fn candidate_paths(self) -> &'static [&'static str] {
        match self {
            Self::Systemctl => &["/usr/bin/systemctl", "/bin/systemctl"],
            Self::Nginx => &["/usr/sbin/nginx", "/usr/bin/nginx"],
            Self::Ferrousx => &["/usr/local/bin/ferrousx-bin", "/usr/local/bin/ferrousx"],
            Self::FerrousxCtl => &["/usr/local/bin/ferrousx-ctl"],
            Self::PhpFpm83 => &["/usr/sbin/php-fpm8.3"],
            Self::Php83 => &["/usr/bin/php8.3", "/usr/bin/php"],
            Self::Ufw => &["/usr/sbin/ufw", "/sbin/ufw"],
            Self::Fail2banClient => &["/usr/bin/fail2ban-client"],
            Self::Mysql => &["/usr/bin/mysql", "/usr/bin/mariadb"],
            Self::Psql => &["/usr/bin/psql"],
            Self::Composer => &["/usr/local/bin/composer", "/usr/bin/composer"],
            Self::WpCli => &["/usr/local/bin/wp", "/usr/bin/wp"],
            Self::AptGet => &["/usr/bin/apt-get"],
            Self::Curl => &["/usr/bin/curl"],
            Self::Tar => &["/usr/bin/tar", "/bin/tar"],
            Self::Chown => &["/usr/bin/chown", "/bin/chown"],
            Self::Chmod => &["/usr/bin/chmod", "/bin/chmod"],
            Self::Install => &["/usr/bin/install"],
            Self::Sysctl => &["/usr/sbin/sysctl", "/sbin/sysctl"],
            Self::Ss => &["/usr/bin/ss", "/bin/ss"],
            Self::Df => &["/usr/bin/df", "/bin/df"],
            Self::Id => &["/usr/bin/id", "/bin/id"],
            Self::Uname => &["/usr/bin/uname", "/bin/uname"],
        }
    }

    fn resolve_path(self) -> Result<&'static Path, CoreError> {
        for candidate in self.candidate_paths() {
            let p = Path::new(candidate);
            if p.is_file() {
                return Ok(p);
            }
        }
        Err(CoreError::BinaryUnavailable(self.label()))
    }
}

/// Result of executing an allowlisted system command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmdOutput {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

fn bounded_utf8(bytes: &[u8]) -> String {
    let slice = if bytes.len() > MAX_OUTPUT_BYTES {
        &bytes[..MAX_OUTPUT_BYTES]
    } else {
        bytes
    };
    String::from_utf8_lossy(slice).trim().to_string()
}

/// Executes an allowlisted system binary directly (`execve` semantics, no shell)
/// with strict argument NUL-byte validation and a mandatory execution timeout.
pub async fn run(
    bin: AllowedBinary,
    args: &[&str],
    timeout: Duration,
) -> Result<CmdOutput, CoreError> {
    if timeout.is_zero() {
        return Err(CoreError::Validation(
            "command timeout must be greater than zero".to_string(),
        ));
    }

    for arg in args {
        if arg.contains('\0') {
            return Err(CoreError::Forbidden("NUL byte in command argument is forbidden"));
        }
    }

    let resolved = bin.resolve_path()?;

    let mut cmd = Command::new(resolved);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn()?;
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| CoreError::CommandTimeout {
            binary: bin.label(),
            timeout_ms,
        })??;

    Ok(CmdOutput {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: bounded_utf8(&output.stdout),
        stderr: bounded_utf8(&output.stderr),
    })
}

/// Executes an allowlisted binary and returns an error if its exit code is non-zero.
pub async fn run_checked(
    bin: AllowedBinary,
    args: &[&str],
    timeout: Duration,
) -> Result<CmdOutput, CoreError> {
    let out = run(bin, args, timeout).await?;
    if !out.success {
        return Err(CoreError::CommandFailed {
            binary: bin.label(),
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}
