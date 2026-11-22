use crate::error::PanelError;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Strictly allowlisted Ubuntu 22.04/24.04 system binaries.
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
    Certbot,
    OpenSsl,
    Kill,
    Crontab,
    Zstd,
    Zip,
    Unzip,
    Mysqldump,
    PgDump,
    Journalctl,
    SshKeygen,
    Sshd,
    Who,
    Free,
    Ip,
    Phpenmod,
    Phpdismod,
    Bun,
    Deno,
    TailwindCli,
    Git,
    Cargo,
    Rustc,
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
            Self::Certbot => "certbot",
            Self::OpenSsl => "openssl",
            Self::Kill => "kill",
            Self::Crontab => "crontab",
            Self::Zstd => "zstd",
            Self::Zip => "zip",
            Self::Unzip => "unzip",
            Self::Mysqldump => "mysqldump",
            Self::PgDump => "pg_dump",
            Self::Journalctl => "journalctl",
            Self::SshKeygen => "ssh-keygen",
            Self::Sshd => "sshd",
            Self::Who => "who",
            Self::Free => "free",
            Self::Ip => "ip",
            Self::Phpenmod => "phpenmod",
            Self::Phpdismod => "phpdismod",
            Self::Bun => "bun",
            Self::Deno => "deno",
            Self::TailwindCli => "tailwindcss",
            Self::Git => "git",
            Self::Cargo => "cargo",
            Self::Rustc => "rustc",
        }
    }

    /// Maps an executable token (name or canonical path) to an allowlisted binary.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        let trimmed = token.trim();
        let file_name = Path::new(trimmed)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(trimmed);
        match file_name {
            "systemctl" => Some(Self::Systemctl),
            "nginx" => Some(Self::Nginx),
            "ferrousx-bin" | "ferrousx" => Some(Self::Ferrousx),
            "ferrousx-ctl" | "fx" => Some(Self::FerrousxCtl),
            "php-fpm8.3" => Some(Self::PhpFpm83),
            "php8.3" | "php" => Some(Self::Php83),
            "ufw" => Some(Self::Ufw),
            "fail2ban-client" => Some(Self::Fail2banClient),
            "mysql" | "mariadb" => Some(Self::Mysql),
            "psql" => Some(Self::Psql),
            "composer" => Some(Self::Composer),
            "wp" => Some(Self::WpCli),
            "apt-get" => Some(Self::AptGet),
            "curl" => Some(Self::Curl),
            "tar" => Some(Self::Tar),
            "chown" => Some(Self::Chown),
            "chmod" => Some(Self::Chmod),
            "install" => Some(Self::Install),
            "sysctl" => Some(Self::Sysctl),
            "ss" => Some(Self::Ss),
            "df" => Some(Self::Df),
            "id" => Some(Self::Id),
            "uname" => Some(Self::Uname),
            "certbot" => Some(Self::Certbot),
            "openssl" => Some(Self::OpenSsl),
            "kill" => Some(Self::Kill),
            "crontab" => Some(Self::Crontab),
            "zstd" => Some(Self::Zstd),
            "zip" => Some(Self::Zip),
            "unzip" => Some(Self::Unzip),
            "mysqldump" | "mariadb-dump" => Some(Self::Mysqldump),
            "pg_dump" => Some(Self::PgDump),
            "journalctl" => Some(Self::Journalctl),
            "ssh-keygen" => Some(Self::SshKeygen),
            "sshd" => Some(Self::Sshd),
            "who" => Some(Self::Who),
            "free" => Some(Self::Free),
            "ip" => Some(Self::Ip),
            "phpenmod" => Some(Self::Phpenmod),
            "phpdismod" => Some(Self::Phpdismod),
            "bun" => Some(Self::Bun),
            "deno" => Some(Self::Deno),
            "tailwindcss" => Some(Self::TailwindCli),
            "git" => Some(Self::Git),
            "cargo" => Some(Self::Cargo),
            "rustc" => Some(Self::Rustc),
            _ => None,
        }
    }

    #[must_use]
    const fn candidate_paths(self) -> &'static [&'static str] {
        match self {
            Self::Systemctl => &["/usr/bin/systemctl", "/bin/systemctl"],
            Self::Nginx => &["/usr/sbin/nginx", "/usr/bin/nginx"],
            Self::Ferrousx => &["/usr/local/bin/ferrousx-bin", "/usr/local/bin/ferrousx"],
            Self::FerrousxCtl => &["/usr/local/bin/ferrousx-ctl", "/usr/local/bin/fx"],
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
            Self::Certbot => &["/usr/bin/certbot", "/snap/bin/certbot"],
            Self::OpenSsl => &["/usr/bin/openssl"],
            Self::Kill => &["/usr/bin/kill", "/bin/kill"],
            Self::Crontab => &["/usr/bin/crontab", "/bin/crontab"],
            Self::Zstd => &["/usr/bin/zstd", "/bin/zstd"],
            Self::Zip => &["/usr/bin/zip"],
            Self::Unzip => &["/usr/bin/unzip"],
            Self::Mysqldump => &["/usr/bin/mysqldump", "/usr/bin/mariadb-dump"],
            Self::PgDump => &["/usr/bin/pg_dump"],
            Self::Journalctl => &["/usr/bin/journalctl", "/bin/journalctl"],
            Self::SshKeygen => &["/usr/bin/ssh-keygen"],
            Self::Sshd => &["/usr/sbin/sshd", "/usr/bin/sshd"],
            Self::Who => &["/usr/bin/who", "/bin/who"],
            Self::Free => &["/usr/bin/free", "/bin/free"],
            Self::Ip => &["/usr/sbin/ip", "/sbin/ip", "/usr/bin/ip"],
            Self::Phpenmod => &["/usr/sbin/phpenmod"],
            Self::Phpdismod => &["/usr/sbin/phpdismod"],
            Self::Bun => &["/usr/local/bin/bun", "/root/.bun/bin/bun", "/usr/bin/bun"],
            Self::Deno => &["/usr/local/bin/deno", "/root/.deno/bin/deno", "/usr/bin/deno"],
            Self::TailwindCli => &["/usr/local/bin/tailwindcss", "/usr/bin/tailwindcss"],
            Self::Git => &["/usr/bin/git", "/usr/local/bin/git"],
            Self::Cargo => &["/usr/local/bin/cargo", "/root/.cargo/bin/cargo", "/usr/bin/cargo"],
            Self::Rustc => &["/usr/local/bin/rustc", "/root/.cargo/bin/rustc", "/usr/bin/rustc"],
        }
    }

    #[must_use]
    pub fn is_available(self) -> bool {
        self.resolve_path().is_ok()
    }

    #[must_use]
    pub fn resolved_path_str(self) -> Option<&'static str> {
        for candidate in self.candidate_paths() {
            if Path::new(candidate).is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn resolve_path(self) -> Result<&'static Path, PanelError> {
        for candidate in self.candidate_paths() {
            let p = Path::new(candidate);
            if p.is_file() {
                return Ok(p);
            }
        }
        Err(PanelError::BinaryUnavailable(self.label()))
    }
}

/// Output captured from an allowlisted command execution.
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

/// Runs an allowlisted binary directly (`execve`, no shell) with argument NUL checks and timeout.
pub async fn run(
    bin: AllowedBinary,
    args: &[&str],
    timeout: Duration,
) -> Result<CmdOutput, PanelError> {
    run_with_env(bin, args, &[], None, timeout).await
}

/// Runs an allowlisted binary directly (`execve`, no shell) with optional environment variables and cwd.
pub async fn run_with_env(
    bin: AllowedBinary,
    args: &[&str],
    envs: &[(&str, &str)],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<CmdOutput, PanelError> {
    if timeout.is_zero() {
        return Err(PanelError::Validation(
            "command timeout must be greater than zero".to_string(),
        ));
    }

    for arg in args {
        if arg.contains('\0') {
            return Err(PanelError::Forbidden(
                "NUL byte in command argument is forbidden".to_string(),
            ));
        }
    }
    for (k, v) in envs {
        if k.contains('\0') || v.contains('\0') {
            return Err(PanelError::Forbidden(
                "NUL byte in command environment is forbidden".to_string(),
            ));
        }
    }

    let resolved = bin.resolve_path()?;

    let mut cmd = Command::new(resolved);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }

    let child = cmd.spawn()?;
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| PanelError::CommandTimeout {
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

/// Runs an allowlisted binary and turns non-zero exit codes into `PanelError::CommandFailed`.
pub async fn run_checked(
    bin: AllowedBinary,
    args: &[&str],
    timeout: Duration,
) -> Result<CmdOutput, PanelError> {
    let out = run(bin, args, timeout).await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: bin.label(),
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(out)
}
