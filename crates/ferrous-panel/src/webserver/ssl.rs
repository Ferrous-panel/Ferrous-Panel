use crate::error::PanelError;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::Domain;
use crate::webserver::{atomic_write_file, AcmeEmail, BoxFuture, CommandExecutor};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const CMD_TIMEOUT: Duration = Duration::from_secs(30);
const EXPIRING_SOON_DAYS: i64 = 14;

#[derive(Debug, Clone)]
pub struct SslPaths {
    pub acme_webroot: SafePath,
    pub letsencrypt_live: SafePath,
    pub reload_hook_path: PathBuf,
    pub renewal_service_path: PathBuf,
    pub renewal_timer_path: PathBuf,
}

impl SslPaths {
    pub fn default_system(reload_hook_path: PathBuf) -> Result<Self, PanelError> {
        Ok(Self {
            acme_webroot: SafePath::under_root(Path::new("/www"), "acme")?,
            letsencrypt_live: SafePath::under_root(Path::new("/etc/letsencrypt"), "live")?,
            reload_hook_path,
            renewal_service_path: PathBuf::from(
                "/etc/systemd/system/ferrous-certbot-renew.service",
            ),
            renewal_timer_path: PathBuf::from("/etc/systemd/system/ferrous-certbot-renew.timer"),
        })
    }

    pub fn for_dirs(
        acme_webroot: SafePath,
        letsencrypt_live: SafePath,
        reload_hook_path: PathBuf,
        renewal_service_path: PathBuf,
        renewal_timer_path: PathBuf,
    ) -> Self {
        Self {
            acme_webroot,
            letsencrypt_live,
            reload_hook_path,
            renewal_service_path,
            renewal_timer_path,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SslCertState {
    Valid,
    ExpiringSoon,
    Expired,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteSslStatus {
    pub domain: Domain,
    pub present: bool,
    pub status: SslCertState,
    pub cert_path: String,
    pub key_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_remaining: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SslRenewalTimerStatus {
    pub unit: String,
    pub enabled: bool,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SslManager {
    paths: SslPaths,
    executor: Arc<dyn CommandExecutor>,
}

impl SslManager {
    #[must_use]
    pub fn new(paths: SslPaths, executor: Arc<dyn CommandExecutor>) -> Self {
        Self { paths, executor }
    }

    #[must_use]
    pub fn paths(&self) -> &SslPaths {
        &self.paths
    }

    pub fn site_cert_paths(&self, domain: &Domain) -> Result<(PathBuf, PathBuf), PanelError> {
        let live_dir = SafePath::under_root(self.paths.letsencrypt_live.as_path(), domain.as_str())?;
        let fullchain = live_dir.as_path().join("fullchain.pem");
        let privkey = live_dir.as_path().join("privkey.pem");
        Ok((fullchain, privkey))
    }

    /// Reads real certificate status, issuer, subject, and expiration directly from disk via `openssl x509`.
    pub fn read_cert_status<'a>(
        &'a self,
        domain: &'a Domain,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        Box::pin(async move {
            let (cert_path, key_path) = self.site_cert_paths(domain)?;
            let cert_str = cert_path.display().to_string();
            let key_str = key_path.display().to_string();

            if !cert_path.is_file() || !key_path.is_file() {
                return Ok(SiteSslStatus {
                    domain: domain.clone(),
                    present: false,
                    status: SslCertState::Missing,
                    cert_path: cert_str,
                    key_path: key_str,
                    issuer: None,
                    subject: None,
                    not_before: None,
                    not_after: None,
                    days_remaining: None,
                });
            }

            if !self.executor.is_binary_available(AllowedBinary::OpenSsl) {
                return Err(PanelError::BinaryUnavailable(AllowedBinary::OpenSsl.label()));
            }

            let out = self
                .executor
                .run(
                    AllowedBinary::OpenSsl,
                    &[
                        "x509",
                        "-in",
                        &cert_str,
                        "-noout",
                        "-dates",
                        "-issuer",
                        "-subject",
                    ],
                    Duration::from_secs(5),
                )
                .await?;

            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::OpenSsl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }

            parse_openssl_x509_output(domain, &cert_str, &key_str, &out.stdout, Utc::now())
        })
    }

    /// Ensures `/www/acme/.well-known/acme-challenge` exists and builds exact `certbot certonly --webroot` args.
    pub fn build_certbot_args(
        &self,
        domain: &Domain,
        aliases: &[Domain],
        email: Option<&AcmeEmail>,
    ) -> Result<Vec<String>, PanelError> {
        let hook_cmd = format!("{} --reload-webserver", self.paths.reload_hook_path.display());
        if hook_cmd.contains('\0') {
            return Err(PanelError::Forbidden("invalid deploy-hook path".to_string()));
        }

        let mut args = vec![
            "certonly".to_string(),
            "--non-interactive".to_string(),
            "--agree-tos".to_string(),
            "--webroot".to_string(),
            "-w".to_string(),
            self.paths.acme_webroot.as_path().display().to_string(),
            "-d".to_string(),
            domain.as_str().to_string(),
        ];

        for alias in aliases {
            if alias.as_str() != domain.as_str() {
                args.push("-d".to_string());
                args.push(alias.as_str().to_string());
            }
        }

        match email {
            Some(addr) => {
                args.push("--email".to_string());
                args.push(addr.as_str().to_string());
            }
            None => {
                args.push("--register-unsafely-without-email".to_string());
            }
        }

        args.push("--deploy-hook".to_string());
        args.push(hook_cmd);

        Ok(args)
    }

    /// Runs `certbot certonly --webroot -w /www/acme ... --deploy-hook ...` and reads resulting real certificate.
    pub fn issue_certificate<'a>(
        &'a self,
        domain: &'a Domain,
        aliases: &'a [Domain],
        email: Option<&'a AcmeEmail>,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        Box::pin(async move {
            if !self.executor.is_binary_available(AllowedBinary::Certbot) {
                return Err(PanelError::Conflict(
                    "certbot is not installed on this server (install: sudo apt install -y certbot)"
                        .to_string(),
                ));
            }

            // Pre-check DNS A/AAAA resolution for primary domain and aliases before burning Let's Encrypt rate limit.
            let mut all_hosts = vec![domain];
            for alias in aliases {
                if alias.as_str() != domain.as_str() {
                    all_hosts.push(alias);
                }
            }
            for host in all_hosts {
                let name = host.as_str();
                let resolved = tokio::net::lookup_host((name, 80u16)).await.map_err(|err| {
                    PanelError::Conflict(format!(
                        "DNS pre-check failed for '{name}': domain does not resolve ({err}). Point A/AAAA record to this VPS before requesting Let's Encrypt SSL."
                    ))
                })?;
                let ips: Vec<std::net::IpAddr> = resolved.map(|sa| sa.ip()).collect();
                if ips.is_empty() {
                    return Err(PanelError::Conflict(format!(
                        "DNS pre-check failed for '{name}': no A/AAAA records found. Point DNS to this VPS first."
                    )));
                }
                if ips.iter().all(|ip| ip.is_loopback() || ip.is_unspecified()) {
                    return Err(PanelError::Conflict(format!(
                        "DNS pre-check failed for '{name}': resolves only to loopback/unspecified IP ({ips:?}). Point public DNS A/AAAA record to this VPS."
                    )));
                }
            }

            let challenge_dir = self
                .paths
                .acme_webroot
                .as_path()
                .join(".well-known")
                .join("acme-challenge");
            std::fs::create_dir_all(&challenge_dir)?;

            let owned_args = self.build_certbot_args(domain, aliases, email)?;
            let arg_refs: Vec<&str> = owned_args.iter().map(String::as_str).collect();

            let out = self
                .executor
                .run(AllowedBinary::Certbot, &arg_refs, CMD_TIMEOUT)
                .await?;

            if !out.success {
                let reason = if !out.stderr.trim().is_empty() {
                    out.stderr.trim().to_string()
                } else if !out.stdout.trim().is_empty() {
                    out.stdout.trim().to_string()
                } else {
                    "certbot failed without diagnostic output".to_string()
                };
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Certbot.label(),
                    exit_code: out.exit_code,
                    stderr: reason,
                });
            }

            self.read_cert_status(domain).await
        })
    }

    /// Runs `certbot renew --non-interactive --deploy-hook "<hook> --reload-webserver"`.
    pub fn renew_all(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            if !self.executor.is_binary_available(AllowedBinary::Certbot) {
                return Err(PanelError::Conflict(
                    "certbot is not installed on this server".to_string(),
                ));
            }
            let hook_cmd = format!("{} --reload-webserver", self.paths.reload_hook_path.display());
            if hook_cmd.contains('\0') {
                return Err(PanelError::Forbidden("invalid deploy-hook path".to_string()));
            }
            let out = self
                .executor
                .run(
                    AllowedBinary::Certbot,
                    &["renew", "--non-interactive", "--deploy-hook", &hook_cmd],
                    CMD_TIMEOUT,
                )
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Certbot.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }
            Ok(())
        })
    }

    #[must_use]
    pub fn render_renewal_systemd_service(&self) -> String {
        let hook_cmd = format!("{} --reload-webserver", self.paths.reload_hook_path.display());
        format!(
            "[Unit]\n\
Description=FerrousPanel Let's Encrypt Certbot Renewal\n\
After=network-online.target\n\
Wants=network-online.target\n\
\n\
[Service]\n\
Type=oneshot\n\
ExecStart=/usr/bin/certbot renew --quiet --deploy-hook \"{hook_cmd}\"\n"
        )
    }

    #[must_use]
    pub fn render_renewal_systemd_timer(&self) -> &'static str {
        "[Unit]\n\
Description=Run FerrousPanel Certbot renewal twice daily\n\
\n\
[Timer]\n\
OnCalendar=*-*-* 00,12:00:00\n\
RandomizedDelaySec=1h\n\
Persistent=true\n\
\n\
[Install]\n\
WantedBy=timers.target\n"
    }

    /// Writes systemd service + timer unit files atomically and enables `ferrous-certbot-renew.timer`.
    pub fn install_renewal_timer(&self) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            let service_body = self.render_renewal_systemd_service();
            let timer_body = self.render_renewal_systemd_timer();
            atomic_write_file(
                &self.paths.renewal_service_path,
                service_body.as_bytes(),
                0o644,
            )?;
            atomic_write_file(&self.paths.renewal_timer_path, timer_body.as_bytes(), 0o644)?;

            let reload = self
                .executor
                .run(
                    AllowedBinary::Systemctl,
                    &["daemon-reload"],
                    Duration::from_secs(10),
                )
                .await?;
            if !reload.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: reload.exit_code,
                    stderr: reload.stderr,
                });
            }

            let enable = self
                .executor
                .run(
                    AllowedBinary::Systemctl,
                    &["enable", "--now", "ferrous-certbot-renew.timer"],
                    Duration::from_secs(10),
                )
                .await?;
            if !enable.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Systemctl.label(),
                    exit_code: enable.exit_code,
                    stderr: enable.stderr,
                });
            }
            Ok(())
        })
    }

    pub fn renewal_timer_status(&self) -> BoxFuture<'_, SslRenewalTimerStatus> {
        Box::pin(async move {
            for candidate in ["ferrous-certbot-renew.timer", "certbot.timer"] {
                if let Ok(out) = self
                    .executor
                    .run(
                        AllowedBinary::Systemctl,
                        &[
                            "show",
                            candidate,
                            "--property=LoadState,UnitFileState,ActiveState,NextElapseUSecRealtime",
                            "--no-pager",
                        ],
                        Duration::from_secs(5),
                    )
                    .await
                {
                    let mut load_state = String::new();
                    let mut unit_file_state = String::new();
                    let mut active_state = String::new();
                    let mut next_run: Option<String> = None;

                    for line in out.stdout.lines() {
                        if let Some((k, v)) = line.split_once('=') {
                            let val = v.trim();
                            match k.trim() {
                                "LoadState" => load_state = val.to_string(),
                                "UnitFileState" => unit_file_state = val.to_string(),
                                "ActiveState" => active_state = val.to_string(),
                                "NextElapseUSecRealtime" => {
                                    if !val.is_empty() && val != "n/a" {
                                        next_run = Some(val.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    if load_state == "loaded" {
                        return SslRenewalTimerStatus {
                            unit: candidate.to_string(),
                            enabled: unit_file_state == "enabled",
                            active: active_state == "active",
                            next_run,
                        };
                    }
                }
            }

            SslRenewalTimerStatus {
                unit: "ferrous-certbot-renew.timer".to_string(),
                enabled: false,
                active: false,
                next_run: None,
            }
        })
    }
}

/// Parses `openssl x509 -in ... -noout -dates -issuer -subject` output into real cert metadata.
pub fn parse_openssl_x509_output(
    domain: &Domain,
    cert_path: &str,
    key_path: &str,
    stdout: &str,
    now: DateTime<Utc>,
) -> Result<SiteSslStatus, PanelError> {
    let mut issuer: Option<String> = None;
    let mut subject: Option<String> = None;
    let mut not_before_raw: Option<String> = None;
    let mut not_after_raw: Option<String> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("notBefore=") {
            not_before_raw = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("notAfter=") {
            not_after_raw = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("issuer=") {
            issuer = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("subject=") {
            subject = Some(val.trim().to_string());
        }
    }

    let not_before_dt = not_before_raw
        .as_deref()
        .map(parse_openssl_gmt_time)
        .transpose()?;
    let not_after_dt = not_after_raw
        .as_deref()
        .map(parse_openssl_gmt_time)
        .transpose()?
        .ok_or_else(|| {
            PanelError::Internal("openssl x509 output did not contain valid notAfter timestamp".to_string())
        })?;

    let days_remaining = (not_after_dt - now).num_days();
    let status = if not_after_dt <= now {
        SslCertState::Expired
    } else if days_remaining <= EXPIRING_SOON_DAYS {
        SslCertState::ExpiringSoon
    } else {
        SslCertState::Valid
    };

    Ok(SiteSslStatus {
        domain: domain.clone(),
        present: true,
        status,
        cert_path: cert_path.to_string(),
        key_path: key_path.to_string(),
        issuer,
        subject,
        not_before: not_before_dt.map(|d| d.to_rfc3339()),
        not_after: Some(not_after_dt.to_rfc3339()),
        days_remaining: Some(days_remaining),
    })
}

fn parse_openssl_gmt_time(raw: &str) -> Result<DateTime<Utc>, PanelError> {
    let normalized = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let without_tz = normalized
        .strip_suffix(" GMT")
        .or_else(|| normalized.strip_suffix(" UTC"))
        .unwrap_or(&normalized);

    let naive = NaiveDateTime::parse_from_str(without_tz, "%b %e %H:%M:%S %Y")
        .or_else(|_| NaiveDateTime::parse_from_str(without_tz, "%b %d %H:%M:%S %Y"))
        .map_err(|_| {
            PanelError::Internal(format!("unable to parse certificate timestamp: {raw}"))
        })?;

    Ok(naive.and_utc())
}
