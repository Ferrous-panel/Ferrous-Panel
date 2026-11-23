use crate::error::PanelError;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Strictly allowlisted Ubuntu 22.04/24.04 system binaries.
/// Shells (`sh`, `bash`, `zsh`, `dash`) are intentionally absent and forbidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedBinary {
    Systemctl,
    SystemdRun,
    Nginx,
    Ferrousx,
    FerrousxCtl,
    PhpFpm74,
    PhpFpm81,
    PhpFpm82,
    PhpFpm83,
    PhpFpm84,
    Php74,
    Php81,
    Php82,
    Php83,
    Php84,
    Dig,
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
    Useradd,
    Userdel,
    Postmap,
    Postconf,
    Postqueue,
    Sendmail,
    Dovecot,
    Doveadm,
    Rspamd,
    Rspamc,
    Wasmtime,
    Podman,
    Docker,
    Wg,
    WgQuick,
    Zramctl,
    Swapon,
    Swapoff,
    Mkswap,
    NamedCheckzone,
    Rndc,
    Python3,
    Go,
}

impl AllowedBinary {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Systemctl => "systemctl",
            Self::SystemdRun => "systemd-run",
            Self::Nginx => "nginx",
            Self::Ferrousx => "ferrousx-bin",
            Self::FerrousxCtl => "ferrousx-ctl",
            Self::PhpFpm74 => "php-fpm7.4",
            Self::PhpFpm81 => "php-fpm8.1",
            Self::PhpFpm82 => "php-fpm8.2",
            Self::PhpFpm83 => "php-fpm8.3",
            Self::PhpFpm84 => "php-fpm8.4",
            Self::Php74 => "php7.4",
            Self::Php81 => "php8.1",
            Self::Php82 => "php8.2",
            Self::Php83 => "php8.3",
            Self::Php84 => "php8.4",
            Self::Dig => "dig",
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
            Self::Useradd => "useradd",
            Self::Userdel => "userdel",
            Self::Postmap => "postmap",
            Self::Postconf => "postconf",
            Self::Postqueue => "postqueue",
            Self::Sendmail => "sendmail",
            Self::Dovecot => "dovecot",
            Self::Doveadm => "doveadm",
            Self::Rspamd => "rspamd",
            Self::Rspamc => "rspamc",
            Self::Wasmtime => "wasmtime",
            Self::Podman => "podman",
            Self::Docker => "docker",
            Self::Wg => "wg",
            Self::WgQuick => "wg-quick",
            Self::Zramctl => "zramctl",
            Self::Swapon => "swapon",
            Self::Swapoff => "swapoff",
            Self::Mkswap => "mkswap",
            Self::NamedCheckzone => "named-checkzone",
            Self::Rndc => "rndc",
            Self::Python3 => "python3",
            Self::Go => "go",
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
            "systemd-run" => Some(Self::SystemdRun),
            "nginx" => Some(Self::Nginx),
            "ferrousx-bin" | "ferrousx" => Some(Self::Ferrousx),
            "ferrousx-ctl" | "fx" => Some(Self::FerrousxCtl),
            "php-fpm7.4" => Some(Self::PhpFpm74),
            "php-fpm8.1" => Some(Self::PhpFpm81),
            "php-fpm8.2" => Some(Self::PhpFpm82),
            "php-fpm8.3" => Some(Self::PhpFpm83),
            "php-fpm8.4" => Some(Self::PhpFpm84),
            "php7.4" => Some(Self::Php74),
            "php8.1" => Some(Self::Php81),
            "php8.2" => Some(Self::Php82),
            "php8.3" | "php" => Some(Self::Php83),
            "php8.4" => Some(Self::Php84),
            "dig" => Some(Self::Dig),
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
            "useradd" => Some(Self::Useradd),
            "userdel" => Some(Self::Userdel),
            "postmap" => Some(Self::Postmap),
            "postconf" => Some(Self::Postconf),
            "postqueue" => Some(Self::Postqueue),
            "sendmail" => Some(Self::Sendmail),
            "dovecot" => Some(Self::Dovecot),
            "doveadm" => Some(Self::Doveadm),
            "rspamd" => Some(Self::Rspamd),
            "rspamc" => Some(Self::Rspamc),
            "wasmtime" => Some(Self::Wasmtime),
            "podman" => Some(Self::Podman),
            "docker" => Some(Self::Docker),
            "wg" => Some(Self::Wg),
            "wg-quick" => Some(Self::WgQuick),
            "zramctl" => Some(Self::Zramctl),
            "swapon" => Some(Self::Swapon),
            "swapoff" => Some(Self::Swapoff),
            "mkswap" => Some(Self::Mkswap),
            "named-checkzone" => Some(Self::NamedCheckzone),
            "rndc" => Some(Self::Rndc),
            "python3" => Some(Self::Python3),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    #[must_use]
    const fn candidate_paths(self) -> &'static [&'static str] {
        match self {
            Self::Systemctl => &["/usr/bin/systemctl", "/bin/systemctl"],
            Self::SystemdRun => &["/usr/bin/systemd-run", "/bin/systemd-run"],
            Self::Nginx => &["/usr/sbin/nginx", "/usr/bin/nginx"],
            Self::Ferrousx => &["/usr/local/bin/ferrousx-bin", "/usr/local/bin/ferrousx"],
            Self::FerrousxCtl => &["/usr/local/bin/ferrousx-ctl", "/usr/local/bin/fx"],
            Self::PhpFpm74 => &["/usr/sbin/php-fpm7.4"],
            Self::PhpFpm81 => &["/usr/sbin/php-fpm8.1"],
            Self::PhpFpm82 => &["/usr/sbin/php-fpm8.2"],
            Self::PhpFpm83 => &["/usr/sbin/php-fpm8.3"],
            Self::PhpFpm84 => &["/usr/sbin/php-fpm8.4"],
            Self::Php74 => &["/usr/bin/php7.4"],
            Self::Php81 => &["/usr/bin/php8.1"],
            Self::Php82 => &["/usr/bin/php8.2"],
            Self::Php83 => &["/usr/bin/php8.3", "/usr/bin/php"],
            Self::Php84 => &["/usr/bin/php8.4"],
            Self::Dig => &["/usr/bin/dig", "/bin/dig"],
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
            Self::Useradd => &["/usr/sbin/useradd", "/sbin/useradd"],
            Self::Userdel => &["/usr/sbin/userdel", "/sbin/userdel"],
            Self::Postmap => &["/usr/sbin/postmap", "/usr/bin/postmap"],
            Self::Postconf => &["/usr/sbin/postconf", "/usr/bin/postconf"],
            Self::Postqueue => &["/usr/sbin/postqueue", "/usr/bin/postqueue"],
            Self::Sendmail => &["/usr/sbin/sendmail", "/usr/lib/sendmail", "/usr/bin/sendmail"],
            Self::Dovecot => &["/usr/sbin/dovecot", "/usr/bin/dovecot"],
            Self::Doveadm => &["/usr/bin/doveadm", "/usr/sbin/doveadm"],
            Self::Rspamd => &["/usr/bin/rspamd", "/usr/sbin/rspamd"],
            Self::Rspamc => &["/usr/bin/rspamc", "/usr/sbin/rspamc"],
            Self::Wasmtime => &["/usr/local/bin/wasmtime", "/root/.wasmtime/bin/wasmtime", "/usr/bin/wasmtime"],
            Self::Podman => &["/usr/bin/podman", "/usr/local/bin/podman"],
            Self::Docker => &["/usr/bin/docker", "/usr/local/bin/docker"],
            Self::Wg => &["/usr/bin/wg", "/usr/sbin/wg"],
            Self::WgQuick => &["/usr/bin/wg-quick", "/usr/sbin/wg-quick"],
            Self::Zramctl => &["/usr/sbin/zramctl", "/sbin/zramctl", "/usr/bin/zramctl"],
            Self::Swapon => &["/usr/sbin/swapon", "/sbin/swapon"],
            Self::Swapoff => &["/usr/sbin/swapoff", "/sbin/swapoff"],
            Self::Mkswap => &["/usr/sbin/mkswap", "/sbin/mkswap"],
            Self::NamedCheckzone => &["/usr/sbin/named-checkzone", "/usr/bin/named-checkzone"],
            Self::Rndc => &["/usr/sbin/rndc", "/usr/bin/rndc"],
            Self::Python3 => &["/usr/bin/python3", "/usr/local/bin/python3"],
            Self::Go => &["/usr/local/go/bin/go", "/usr/bin/go", "/usr/local/bin/go"],
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

    #[must_use]
    pub fn php_fpm_for_version(version: &str) -> Option<Self> {
        match version.trim() {
            "7.4" => Some(Self::PhpFpm74),
            "8.1" => Some(Self::PhpFpm81),
            "8.2" => Some(Self::PhpFpm82),
            "8.3" => Some(Self::PhpFpm83),
            "8.4" => Some(Self::PhpFpm84),
            _ => None,
        }
    }

    #[must_use]
    pub fn php_cli_for_version(version: &str) -> Option<Self> {
        match version.trim() {
            "7.4" => Some(Self::Php74),
            "8.1" => Some(Self::Php81),
            "8.2" => Some(Self::Php82),
            "8.3" => Some(Self::Php83),
            "8.4" => Some(Self::Php84),
            _ => None,
        }
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

/// Validates a user-supplied positional argument passed to external CLIs
/// (`git`, `docker`, `podman`, `curl`, `tar`, `php`, `bun`, `deno`, `composer`, `wp`, `useradd`).
/// Rejects leading `-`, control characters, and known option/command-execution injection patterns.
pub fn validate_no_flag_injection<'a>(arg: &'a str, field: &str) -> Result<&'a str, PanelError> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return Err(PanelError::Validation(format!("{field} cannot be empty")));
    }
    if trimmed.contains('\0') || trimmed.contains('\r') || trimmed.contains('\n') {
        return Err(PanelError::Forbidden(format!(
            "{field} contains forbidden control character"
        )));
    }
    if trimmed.starts_with('-') {
        return Err(PanelError::Forbidden(format!(
            "{field} cannot start with '-' (argument injection guard)"
        )));
    }
    let lower = trimmed.to_ascii_lowercase();
    for forbidden in [
        "--config",
        "--exec",
        "--to-command",
        "--checkpoint-action",
        "--upload-pack",
        "core.sshcommand",
        "core.fsmonitor",
        "proxycommand",
    ] {
        if lower.contains(forbidden) {
            return Err(PanelError::Forbidden(format!(
                "{field} contains forbidden option pattern '{forbidden}'"
            )));
        }
    }
    Ok(trimmed)
}

async fn read_pipe_bounded<R: AsyncRead + Unpin>(mut reader: R, limit: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(limit.min(8192));
    {
        let mut limited = (&mut reader).take(limit as u64);
        let _ = limited.read_to_end(&mut buf).await;
    }
    // Drain excess bytes into a small stack buffer so the child process cannot deadlock on a full pipe.
    let mut discard = [0u8; 4096];
    loop {
        match reader.read(&mut discard).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
    buf
}

async fn wait_child_bounded(
    mut child: tokio::process::Child,
    read_stdout: bool,
    read_stderr: bool,
) -> std::io::Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let stdout_pipe = if read_stdout { child.stdout.take() } else { None };
    let stderr_pipe = if read_stderr { child.stderr.take() } else { None };

    let stdout_fut = async move {
        match stdout_pipe {
            Some(pipe) => read_pipe_bounded(pipe, MAX_OUTPUT_BYTES).await,
            None => Vec::new(),
        }
    };
    let stderr_fut = async move {
        match stderr_pipe {
            Some(pipe) => read_pipe_bounded(pipe, MAX_OUTPUT_BYTES).await,
            None => Vec::new(),
        }
    };

    let (status_res, stdout_bytes, stderr_bytes) =
        tokio::join!(child.wait(), stdout_fut, stderr_fut);
    let status = status_res?;
    Ok((status, stdout_bytes, stderr_bytes))
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

    let (status, stdout_bytes, stderr_bytes) =
        tokio::time::timeout(timeout, wait_child_bounded(child, true, true))
            .await
            .map_err(|_| PanelError::CommandTimeout {
                binary: bin.label(),
                timeout_ms,
            })??;

    Ok(CmdOutput {
        exit_code: status.code(),
        success: status.success(),
        stdout: bounded_utf8(&stdout_bytes),
        stderr: bounded_utf8(&stderr_bytes),
    })
}

/// Runs an allowlisted binary directly (`execve`, no shell) and pipes `stdin_bytes` to its stdin.
pub async fn run_with_stdin(
    bin: AllowedBinary,
    args: &[&str],
    stdin_bytes: &[u8],
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
    let resolved = bin.resolve_path()?;
    let mut cmd = Command::new(resolved);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_bytes).await;
        let _ = stdin.shutdown().await;
    }

    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    let (status, stdout_bytes, stderr_bytes) =
        tokio::time::timeout(timeout, wait_child_bounded(child, true, true))
            .await
            .map_err(|_| PanelError::CommandTimeout {
                binary: bin.label(),
                timeout_ms,
            })??;

    Ok(CmdOutput {
        exit_code: status.code(),
        success: status.success(),
        stdout: bounded_utf8(&stdout_bytes),
        stderr: bounded_utf8(&stderr_bytes),
    })
}

/// Runs an allowlisted binary directly (`execve`, no shell) and streams its stdout straight to
/// `output_file` (mode `0600`) without the 64 KiB in-memory stdout cap.
pub async fn run_to_file(
    bin: AllowedBinary,
    args: &[&str],
    output_file: &Path,
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
    let resolved = bin.resolve_path()?;

    if let Some(parent) = output_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let out_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(output_file)?;

    let mut cmd = Command::new(resolved);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn()?;
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    let (status, _stdout_bytes, stderr_bytes) =
        tokio::time::timeout(timeout, wait_child_bounded(child, false, true))
            .await
            .map_err(|_| PanelError::CommandTimeout {
                binary: bin.label(),
                timeout_ms,
            })??;

    Ok(CmdOutput {
        exit_code: status.code(),
        success: status.success(),
        stdout: String::new(),
        stderr: bounded_utf8(&stderr_bytes),
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
