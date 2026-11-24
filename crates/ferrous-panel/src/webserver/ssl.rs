use crate::error::PanelError;
use crate::system::cmd::AllowedBinary;
use crate::system::paths::SafePath;
use crate::system::validate::Domain;
use crate::webserver::{atomic_write_file, AcmeEmail, BoxFuture, CommandExecutor};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CMD_TIMEOUT: Duration = Duration::from_secs(30);
const CERTBOT_TIMEOUT: Duration = Duration::from_secs(120);
const APT_INSTALL_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_PEM_BYTES: usize = 256 * 1024;
const EXPIRING_SOON_DAYS: i64 = 14;
static SSL_TMP_SEQ: AtomicU64 = AtomicU64::new(1);

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub san_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SslDnsPreflight {
    pub domain: String,
    pub resolved: bool,
    pub ips: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
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

    /// Ensures `/www/acme/.well-known/acme-challenge` exists with world-traversable `0755` permissions
    /// so both `www-data` (Nginx) and `ferrousx` can serve HTTP-01 challenge tokens.
    pub fn ensure_acme_webroot_permissions(&self) -> Result<PathBuf, PanelError> {
        let root = self.paths.acme_webroot.as_path();
        let well_known = root.join(".well-known");
        let challenge_dir = well_known.join("acme-challenge");
        std::fs::create_dir_all(&challenge_dir)?;
        for dir in [root, well_known.as_path(), challenge_dir.as_path()] {
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
        }
        Ok(challenge_dir)
    }

    /// Reads real certificate status, issuer, subject, and expiration directly from disk via `openssl x509`.
    /// Degrades gracefully to `Missing` if the certificate file is corrupt/unreadable so the SSL Manager UI stays usable.
    pub fn read_cert_status<'a>(
        &'a self,
        domain: &'a Domain,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        Box::pin(async move {
            let (cert_path, key_path) = self.site_cert_paths(domain)?;
            let cert_str = cert_path.display().to_string();
            let key_str = key_path.display().to_string();
            let missing_status = || SiteSslStatus {
                domain: domain.clone(),
                present: false,
                status: SslCertState::Missing,
                cert_path: cert_str.clone(),
                key_path: key_str.clone(),
                issuer: None,
                subject: None,
                not_before: None,
                not_after: None,
                days_remaining: None,
                provider: None,
                san_domains: Vec::new(),
                sha256_fingerprint: None,
            };

            if !cert_path.is_file() || !key_path.is_file() {
                return Ok(missing_status());
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
                        "-fingerprint",
                        "-sha256",
                        "-ext",
                        "subjectAltName",
                    ],
                    Duration::from_secs(5),
                )
                .await?;

            let out = if out.success {
                out
            } else {
                // Fallback for older OpenSSL builds without `-ext subjectAltName`
                self.executor
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
                    .await?
            };

            if !out.success {
                tracing::warn!(
                    domain = %domain.as_str(),
                    cert = %cert_str,
                    stderr = %out.stderr.trim(),
                    "corrupt or unreadable SSL certificate on disk; treating as missing"
                );
                return Ok(missing_status());
            }

            match parse_openssl_x509_output(domain, &cert_str, &key_str, &out.stdout, Utc::now()) {
                Ok(parsed) => Ok(parsed),
                Err(err) => {
                    tracing::warn!(
                        domain = %domain.as_str(),
                        cert = %cert_str,
                        error = %err,
                        "failed to parse openssl x509 metadata; treating as missing"
                    );
                    Ok(missing_status())
                }
            }
        })
    }

    async fn resolve_public_dns_via_dig(&self, name: &str) -> Vec<String> {
        if !self.executor.is_binary_available(AllowedBinary::Dig) {
            return Vec::new();
        }
        let mut ips = Vec::new();
        for resolver in ["@1.1.1.1", "@8.8.8.8"] {
            for rtype in ["A", "AAAA"] {
                if let Ok(out) = self
                    .executor
                    .run(
                        AllowedBinary::Dig,
                        &[resolver, "+short", "+time=3", "+tries=1", name, rtype],
                        Duration::from_secs(4),
                    )
                    .await
                {
                    if out.success {
                        for line in out.stdout.lines() {
                            let candidate = line.trim();
                            if let Ok(ip) = candidate.parse::<IpAddr>() {
                                if !ip.is_loopback() && !ip.is_unspecified() {
                                    let s = ip.to_string();
                                    if !ips.contains(&s) {
                                        ips.push(s);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !ips.is_empty() {
                break;
            }
        }
        ips
    }

    /// Checks live DNS A/AAAA resolution for the primary domain and aliases.
    /// Falls back to public DNS (`dig @1.1.1.1 / @8.8.8.8`) if `/etc/hosts` resolves the host to loopback.
    pub fn check_dns_preflight<'a>(
        &'a self,
        domain: &'a Domain,
        aliases: &'a [Domain],
    ) -> BoxFuture<'a, Vec<SslDnsPreflight>> {
        Box::pin(async move {
            let mut hosts = vec![domain];
            for alias in aliases {
                if alias.as_str() != domain.as_str() {
                    hosts.push(alias);
                }
            }
            let mut results = Vec::with_capacity(hosts.len());
            for host in hosts {
                let name = host.as_str().to_string();
                match tokio::time::timeout(
                    Duration::from_secs(4),
                    tokio::net::lookup_host((host.as_str(), 80u16)),
                )
                .await
                {
                    Ok(Ok(addrs)) => {
                        let mut ips: Vec<String> = Vec::new();
                        let mut all_local = true;
                        for sa in addrs {
                            let ip = sa.ip();
                            if !ip.is_loopback() && !ip.is_unspecified() {
                                all_local = false;
                            }
                            let s = ip.to_string();
                            if !ips.contains(&s) {
                                ips.push(s);
                            }
                        }
                        if !ips.is_empty() && !all_local {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: true,
                                ips,
                                warning: None,
                            });
                            continue;
                        }
                        let public_ips = self.resolve_public_dns_via_dig(&name).await;
                        if !public_ips.is_empty() {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: true,
                                ips: public_ips,
                                warning: if all_local && !ips.is_empty() {
                                    Some("DNS công cộng hợp lệ (1.1.1.1/8.8.8.8); /etc/hosts cục bộ đang trỏ loopback.".to_string())
                                } else {
                                    None
                                },
                            });
                        } else if ips.is_empty() {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: false,
                                ips,
                                warning: Some(
                                    "Chưa có bản ghi DNS A/AAAA trỏ về máy chủ.".to_string(),
                                ),
                            });
                        } else {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: false,
                                ips,
                                warning: Some(
                                    "Tên miền đang trỏ về IP loopback/nội bộ; Let's Encrypt yêu cầu IP công cộng.".to_string(),
                                ),
                            });
                        }
                    }
                    Ok(Err(err)) => {
                        let public_ips = self.resolve_public_dns_via_dig(&name).await;
                        if !public_ips.is_empty() {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: true,
                                ips: public_ips,
                                warning: None,
                            });
                        } else {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: false,
                                ips: Vec::new(),
                                warning: Some(format!("Không phân giải được DNS: {err}")),
                            });
                        }
                    }
                    Err(_) => {
                        let public_ips = self.resolve_public_dns_via_dig(&name).await;
                        if !public_ips.is_empty() {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: true,
                                ips: public_ips,
                                warning: None,
                            });
                        } else {
                            results.push(SslDnsPreflight {
                                domain: name,
                                resolved: false,
                                ips: Vec::new(),
                                warning: Some("Quá thời gian kiểm tra DNS (4s).".to_string()),
                            });
                        }
                    }
                };
            }
            results
        })
    }

    /// Performs a real local HTTP-01 self-test against `probe_addr` to verify that the active web server
    /// serves `/.well-known/acme-challenge/<token>` from `acme_webroot` with HTTP 200 before calling Certbot.
    pub fn verify_acme_http01_reachable<'a>(
        &'a self,
        domain: &'a Domain,
        probe_addr: SocketAddr,
    ) -> BoxFuture<'a, Result<(), PanelError>> {
        Box::pin(async move {
            let challenge_dir = self.ensure_acme_webroot_permissions()?;
            let seq = SSL_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let token_name = format!("ferrous-acme-check-{}-{seq}-{ts}", std::process::id());
            let token_body = format!("ferrous-acme-ok-{seq}-{ts}");
            let token_path = challenge_dir.join(&token_name);
            atomic_write_file(&token_path, token_body.as_bytes(), 0o644)?;
            let _cleanup = TempFileGuard(token_path);

            let host_hdr = domain.as_str().to_string();
            let req_path = format!("/.well-known/acme-challenge/{token_name}");
            let expected_body = token_body.clone();
            let webroot_str = self.paths.acme_webroot.as_path().display().to_string();

            tokio::task::spawn_blocking(move || {
                let mut stream = TcpStream::connect_timeout(&probe_addr, Duration::from_secs(4))
                    .map_err(|err| {
                        PanelError::Conflict(format!(
                            "Không thể kết nối HTTP cổng {} để xác minh ACME challenge cho '{host_hdr}': {err}. Hãy đảm bảo Nginx hoặc FerrousX đang chạy và mở cổng 80.",
                            probe_addr.port()
                        ))
                    })?;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(4)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(4)));
                let req = format!(
                    "GET {req_path} HTTP/1.1\r\nHost: {host_hdr}\r\nUser-Agent: FerrousPanel-ACME-Check/1.0\r\nConnection: close\r\n\r\n"
                );
                stream.write_all(req.as_bytes()).map_err(|err| {
                    PanelError::Conflict(format!(
                        "Gửi yêu cầu kiểm tra ACME HTTP-01 tới {probe_addr} thất bại: {err}"
                    ))
                })?;

                let mut buf = [0u8; 4096];
                let mut total = 0usize;
                loop {
                    match stream.read(&mut buf[total..]) {
                        Ok(0) => break,
                        Ok(n) => {
                            total += n;
                            if total == buf.len() {
                                break;
                            }
                        }
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                || e.kind() == std::io::ErrorKind::TimedOut =>
                        {
                            break;
                        }
                        Err(err) => {
                            return Err(PanelError::Conflict(format!(
                                "Đọc phản hồi kiểm tra ACME HTTP-01 thất bại: {err}"
                            )));
                        }
                    }
                }

                let raw = String::from_utf8_lossy(&buf[..total]);
                let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_ref(), ""));
                let status_line = head.lines().next().unwrap_or("");
                let status_code = status_line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(0);

                if status_code == 200 && body.trim().contains(expected_body.as_str()) {
                    return Ok(());
                }
                if (300..400).contains(&status_code) {
                    return Err(PanelError::Conflict(format!(
                        "Đường dẫn ACME HTTP-01 ({req_path}) cho '{host_hdr}' đang bị chuyển hướng HTTP {status_code} thay vì trả trực tiếp tệp từ {webroot_str}. Hãy kiểm tra lại vhost cổng 80."
                    )));
                }
                Err(PanelError::Conflict(format!(
                    "Kiểm tra ACME HTTP-01 cục bộ cho '{host_hdr}' thất bại (HTTP {status_code}). Máy chủ web tại {probe_addr} không phục vụ đúng thư mục {webroot_str}/.well-known/acme-challenge/ — vui lòng kiểm tra lại cấu hình vhost."
                )))
            })
            .await
            .map_err(|_| PanelError::Internal("ACME HTTP-01 self-check task cancelled".to_string()))?
        })
    }

    /// Ensures `certbot` is installed, optionally installing it via `apt-get` on Ubuntu/Debian.
    pub fn ensure_certbot_installed(
        &self,
        auto_install: bool,
    ) -> BoxFuture<'_, Result<(), PanelError>> {
        Box::pin(async move {
            if self.executor.is_binary_available(AllowedBinary::Certbot) {
                return Ok(());
            }
            if !auto_install {
                return Err(PanelError::Conflict(
                    "Certbot chưa được cài đặt trên máy chủ. Hãy bật 'Tự động cài Certbot' hoặc chạy: sudo apt-get install -y certbot".to_string(),
                ));
            }
            if !self.executor.is_binary_available(AllowedBinary::AptGet) {
                return Err(PanelError::Conflict(
                    "Không tìm thấy apt-get để tự động cài Certbot. Vui lòng cài certbot thủ công."
                        .to_string(),
                ));
            }

            let env = [("DEBIAN_FRONTEND", "noninteractive")];
            let _ = self
                .executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &["update", "-qq"],
                    &env,
                    None,
                    APT_INSTALL_TIMEOUT,
                )
                .await;

            let install_out = self
                .executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &[
                        "install",
                        "-y",
                        "-qq",
                        "--no-install-recommends",
                        "certbot",
                    ],
                    &env,
                    None,
                    APT_INSTALL_TIMEOUT,
                )
                .await?;

            if !install_out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::AptGet.label(),
                    exit_code: install_out.exit_code,
                    stderr: install_out.stderr,
                });
            }

            if !self.executor.is_binary_available(AllowedBinary::Certbot) {
                return Err(PanelError::Conflict(
                    "Đã chạy apt-get install certbot nhưng chưa thấy binary /usr/bin/certbot."
                        .to_string(),
                ));
            }
            Ok(())
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
        self.issue_or_renew_letsencrypt_with_options(domain, aliases, email, false, false, false)
    }

    /// Full production Let's Encrypt issuance/expansion/renewal flow (aaPanel/CloudPanel style).
    pub fn issue_or_renew_letsencrypt<'a>(
        &'a self,
        domain: &'a Domain,
        aliases: &'a [Domain],
        email: Option<&'a AcmeEmail>,
        auto_install_certbot: bool,
        force_renewal: bool,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        self.issue_or_renew_letsencrypt_with_options(
            domain,
            aliases,
            email,
            auto_install_certbot,
            force_renewal,
            false,
        )
    }

    /// Full production Let's Encrypt issuance/expansion/renewal flow with optional DNS preflight bypass
    /// and automatic rollback of prior non-certbot (self-signed/custom) certificates on Certbot failure.
    pub fn issue_or_renew_letsencrypt_with_options<'a>(
        &'a self,
        domain: &'a Domain,
        aliases: &'a [Domain],
        email: Option<&'a AcmeEmail>,
        auto_install_certbot: bool,
        force_renewal: bool,
        skip_dns_check: bool,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        Box::pin(async move {
            self.ensure_certbot_installed(auto_install_certbot).await?;

            // Pre-check DNS A/AAAA resolution for primary domain and aliases before burning Let's Encrypt rate limit.
            if !skip_dns_check {
                let preflight = self.check_dns_preflight(domain, aliases).await;
                for item in &preflight {
                    if !item.resolved {
                        return Err(PanelError::Conflict(format!(
                            "Kiểm tra DNS thất bại cho '{}': {}. Hãy trỏ bản ghi A/AAAA về IP máy chủ (hoặc đánh dấu 'Bỏ qua DNS Preflight' nếu dùng Split-DNS / Cloudflare) trước khi cấp Let's Encrypt.",
                            item.domain,
                            item.warning.as_deref().unwrap_or("chưa phân giải được")
                        )));
                    }
                }
            }

            let _ = self.ensure_acme_webroot_permissions()?;

            // If live/<domain>/fullchain.pem exists as a non-symlink regular file (self-signed or custom PEM),
            // or a dangling symlink, or live/<domain> exists without a matching renewal/<domain>.conf,
            // back it up first and remove the stale live/<domain> directory so certbot writes directly to
            // live/<domain> instead of failing with "live directory exists for <domain>" or creating <domain>-0001.
            // If Certbot fails for any reason, restore the backed-up certificate so HTTPS is never broken.
            let (cert_path, key_path) = self.site_cert_paths(domain)?;
            let is_non_certbot_cert = cert_path
                .symlink_metadata()
                .map(|m| m.file_type().is_file())
                .unwrap_or(false);
            let is_dangling_symlink =
                cert_path.symlink_metadata().is_ok() && !cert_path.exists();
            let live_dir = cert_path.parent().map(|p| p.to_path_buf());
            let renewal_missing = self
                .paths
                .letsencrypt_live
                .as_path()
                .parent()
                .map(|le_root| {
                    let renewal_dir = le_root.join("renewal");
                    renewal_dir.is_dir()
                        && !renewal_dir
                            .join(format!("{}.conf", domain.as_str()))
                            .is_file()
                })
                .unwrap_or(false);
            let should_clean_live_dir = is_non_certbot_cert
                || is_dangling_symlink
                || (renewal_missing
                    && live_dir.as_ref().map(|d| d.exists()).unwrap_or(false));

            let mut backup_cert_bytes: Option<Vec<u8>> = None;
            let mut backup_key_bytes: Option<Vec<u8>> = None;
            if should_clean_live_dir {
                backup_cert_bytes = std::fs::read(&cert_path).ok();
                backup_key_bytes = std::fs::read(&key_path).ok();
                let _ = std::fs::remove_file(&cert_path);
                let _ = std::fs::remove_file(&key_path);
                if let Some(ref parent) = live_dir {
                    let _ = std::fs::remove_dir_all(parent);
                }
            }

            let restore_backup = || {
                if let (Some(ref c), Some(ref k)) = (&backup_cert_bytes, &backup_key_bytes) {
                    let _ = atomic_write_file(&cert_path, c, 0o644);
                    let _ = atomic_write_file(&key_path, k, 0o600);
                }
            };

            let mut owned_args = match self.build_certbot_args(domain, aliases, email) {
                Ok(args) => args,
                Err(err) => {
                    restore_backup();
                    return Err(err);
                }
            };
            // Avoid failing Certbot when reload_hook_path binary does not exist on disk (e.g. non-standard install path).
            if !self.paths.reload_hook_path.exists() {
                if let Some(pos) = owned_args.iter().position(|a| a == "--deploy-hook") {
                    owned_args.drain(pos..=(pos + 1).min(owned_args.len() - 1));
                }
            }
            owned_args.push("--cert-name".to_string());
            owned_args.push(domain.as_str().to_string());
            owned_args.push("--expand".to_string());
            if force_renewal {
                owned_args.push("--force-renewal".to_string());
            } else {
                owned_args.push("--keep-until-expiring".to_string());
            }
            let arg_refs: Vec<&str> = owned_args.iter().map(String::as_str).collect();

            let out = match self
                .executor
                .run(AllowedBinary::Certbot, &arg_refs, CERTBOT_TIMEOUT)
                .await
            {
                Ok(out) => out,
                Err(err) => {
                    restore_backup();
                    return Err(err);
                }
            };

            // Check if Certbot actually wrote valid certificate files even if a post-hook exited non-zero.
            let cert_written_by_certbot = cert_path.is_file()
                && key_path.is_file()
                && cert_path
                    .symlink_metadata()
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);

            if !out.success && !cert_written_by_certbot {
                restore_backup();
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

            #[cfg(unix)]
            if key_path.is_file() {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(canon_key) = std::fs::canonicalize(&key_path) {
                    let _ = std::fs::set_permissions(
                        &canon_key,
                        std::fs::Permissions::from_mode(0o600),
                    );
                }
            }

            if self.executor.is_binary_available(AllowedBinary::Systemctl) {
                let _ = self.install_renewal_timer().await;
            }

            self.read_cert_status(domain).await
        })
    }

    /// Cryptographically validates a user-supplied PEM certificate chain + private key via `openssl`
    /// (ensuring both are valid AND that the certificate's public key matches the private key),
    /// then installs them atomically (`0644` cert, `0600` key) into `live/<domain>/`.
    pub fn install_custom_pem<'a>(
        &'a self,
        domain: &'a Domain,
        fullchain_pem: &'a str,
        private_key_pem: &'a str,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        Box::pin(async move {
            let cert_trimmed = fullchain_pem.trim();
            let key_trimmed = private_key_pem.trim();

            if cert_trimmed.is_empty() || cert_trimmed.len() > MAX_PEM_BYTES {
                return Err(PanelError::Validation(
                    "Chứng chỉ PEM (Fullchain) phải có kích thước từ 1 byte đến 256 KiB.".to_string(),
                ));
            }
            if key_trimmed.is_empty() || key_trimmed.len() > MAX_PEM_BYTES {
                return Err(PanelError::Validation(
                    "Khóa bí mật PEM (Private Key) phải có kích thước từ 1 byte đến 256 KiB."
                        .to_string(),
                ));
            }
            if cert_trimmed.contains('\0') || key_trimmed.contains('\0') {
                return Err(PanelError::Forbidden(
                    "Nội dung PEM không được chứa ký tự NUL.".to_string(),
                ));
            }
            if !cert_trimmed.contains("-----BEGIN CERTIFICATE-----")
                || !cert_trimmed.contains("-----END CERTIFICATE-----")
            {
                return Err(PanelError::Validation(
                    "Chứng chỉ không đúng định dạng PEM (thiếu -----BEGIN CERTIFICATE-----)."
                        .to_string(),
                ));
            }
            if !key_trimmed.contains("PRIVATE KEY-----") {
                return Err(PanelError::Validation(
                    "Khóa bí mật không đúng định dạng PEM (thiếu -----BEGIN ... PRIVATE KEY-----)."
                        .to_string(),
                ));
            }
            if !self.executor.is_binary_available(AllowedBinary::OpenSsl) {
                return Err(PanelError::BinaryUnavailable(AllowedBinary::OpenSsl.label()));
            }

            // Stage to temporary files (key mode 0600) to validate with OpenSSL before touching live certs.
            let seq = SSL_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let tmp_dir = std::env::temp_dir().join(format!(
                "ferrous-ssl-verify-{}-{seq}",
                std::process::id()
            ));
            std::fs::create_dir_all(&tmp_dir)?;
            let _ = std::fs::set_permissions(&tmp_dir, std::fs::Permissions::from_mode(0o700));
            let _cleanup = TempDirGuard(tmp_dir.clone());

            let tmp_cert = tmp_dir.join("fullchain.pem");
            let tmp_key = tmp_dir.join("privkey.pem");
            let cert_normalized = format!("{cert_trimmed}\n");
            let key_normalized = format!("{key_trimmed}\n");
            atomic_write_file(&tmp_cert, cert_normalized.as_bytes(), 0o644)?;
            atomic_write_file(&tmp_key, key_normalized.as_bytes(), 0o600)?;

            let tmp_cert_str = tmp_cert.display().to_string();
            let tmp_key_str = tmp_key.display().to_string();

            // 1. Validate X.509 certificate and extract its public key
            let cert_pub = self
                .executor
                .run(
                    AllowedBinary::OpenSsl,
                    &["x509", "-in", &tmp_cert_str, "-pubkey", "-noout"],
                    Duration::from_secs(10),
                )
                .await?;
            if !cert_pub.success {
                return Err(PanelError::Validation(format!(
                    "Chứng chỉ X.509 không hợp lệ (openssl x509): {}",
                    cert_pub.stderr.trim()
                )));
            }

            // 2. Validate private key and extract its public key
            let key_pub = self
                .executor
                .run(
                    AllowedBinary::OpenSsl,
                    &["pkey", "-in", &tmp_key_str, "-pubout"],
                    Duration::from_secs(10),
                )
                .await?;
            if !key_pub.success {
                return Err(PanelError::Validation(format!(
                    "Private Key không hợp lệ hoặc bị khóa bằng passphrase (openssl pkey): {}",
                    key_pub.stderr.trim()
                )));
            }

            // 3. Verify cryptographic match between certificate public key and private key
            if cert_pub.stdout.trim().is_empty()
                || cert_pub.stdout.trim() != key_pub.stdout.trim()
            {
                return Err(PanelError::Validation(
                    "Certificate và Private Key KHÔNG khớp nhau (public key mismatch). Vui lòng kiểm tra lại đúng cặp tệp .pem và .key.".to_string(),
                ));
            }

            // 4. Atomically install verified certificate (0644) and private key (0600)
            let (cert_path, key_path) = self.site_cert_paths(domain)?;
            atomic_write_file(&cert_path, cert_normalized.as_bytes(), 0o644)?;
            atomic_write_file(&key_path, key_normalized.as_bytes(), 0o600)?;

            self.read_cert_status(domain).await
        })
    }

    /// Generates a real X.509 v3 RSA-2048 SHA-256 self-signed certificate with SANs via `openssl req`
    /// (useful for intranet, local HTTPS testing, or Cloudflare SSL Mode = Full).
    pub fn issue_self_signed<'a>(
        &'a self,
        domain: &'a Domain,
        aliases: &'a [Domain],
        validity_days: u32,
    ) -> BoxFuture<'a, Result<SiteSslStatus, PanelError>> {
        Box::pin(async move {
            if !self.executor.is_binary_available(AllowedBinary::OpenSsl) {
                return Err(PanelError::BinaryUnavailable(AllowedBinary::OpenSsl.label()));
            }
            let days = validity_days.clamp(1, 3650);
            let days_str = days.to_string();

            let mut san_entries = vec![format!("DNS:{}", domain.as_str())];
            for alias in aliases {
                if alias.as_str() != domain.as_str() {
                    san_entries.push(format!("DNS:{}", alias.as_str()));
                }
            }
            let san_ext = format!("subjectAltName={}", san_entries.join(","));
            let subj = format!("/CN={}", domain.as_str());

            let seq = SSL_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let tmp_dir = std::env::temp_dir().join(format!(
                "ferrous-ssl-selfsigned-{}-{seq}",
                std::process::id()
            ));
            std::fs::create_dir_all(&tmp_dir)?;
            let _ = std::fs::set_permissions(&tmp_dir, std::fs::Permissions::from_mode(0o700));
            let _cleanup = TempDirGuard(tmp_dir.clone());

            let tmp_cert = tmp_dir.join("fullchain.pem");
            let tmp_key = tmp_dir.join("privkey.pem");
            let tmp_cert_str = tmp_cert.display().to_string();
            let tmp_key_str = tmp_key.display().to_string();

            let out = self
                .executor
                .run(
                    AllowedBinary::OpenSsl,
                    &[
                        "req",
                        "-x509",
                        "-newkey",
                        "rsa:2048",
                        "-sha256",
                        "-days",
                        &days_str,
                        "-nodes",
                        "-keyout",
                        &tmp_key_str,
                        "-out",
                        &tmp_cert_str,
                        "-subj",
                        &subj,
                        "-addext",
                        &san_ext,
                    ],
                    CMD_TIMEOUT,
                )
                .await?;

            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::OpenSsl.label(),
                    exit_code: out.exit_code,
                    stderr: out.stderr,
                });
            }

            let cert_bytes = std::fs::read(&tmp_cert)?;
            let key_bytes = std::fs::read(&tmp_key)?;
            let (cert_path, key_path) = self.site_cert_paths(domain)?;
            atomic_write_file(&cert_path, &cert_bytes, 0o644)?;
            atomic_write_file(&key_path, &key_bytes, 0o600)?;

            self.read_cert_status(domain).await
        })
    }

    /// Removes `fullchain.pem` and `privkey.pem` from `live/<domain>/` safely inside `SafePath`.
    pub fn remove_site_cert_files(&self, domain: &Domain) -> Result<bool, PanelError> {
        let (cert_path, key_path) = self.site_cert_paths(domain)?;
        let mut removed = false;
        for path in [&cert_path, &key_path] {
            if path.symlink_metadata().is_ok() {
                std::fs::remove_file(path)?;
                removed = true;
            }
        }
        Ok(removed)
    }

    /// Runs `certbot renew --non-interactive` (with `--deploy-hook` when the reload hook binary exists).
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
            let mut args = vec!["renew".to_string(), "--non-interactive".to_string()];
            if self.paths.reload_hook_path.exists() {
                args.push("--deploy-hook".to_string());
                args.push(hook_cmd);
            }
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = self
                .executor
                .run(AllowedBinary::Certbot, &arg_refs, CERTBOT_TIMEOUT)
                .await?;
            if !out.success {
                return Err(PanelError::CommandFailed {
                    binary: AllowedBinary::Certbot.label(),
                    exit_code: out.exit_code,
                    stderr: if !out.stderr.trim().is_empty() {
                        out.stderr
                    } else {
                        out.stdout
                    },
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

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn detect_cert_provider(issuer: Option<&str>, subject: Option<&str>) -> Option<String> {
    let iss = issuer?.trim();
    let lower = iss.to_ascii_lowercase();
    if lower.contains("let's encrypt") || lower.contains("lets encrypt") {
        return Some("Let's Encrypt".to_string());
    }
    if lower.contains("cloudflare") {
        return Some("Cloudflare Origin CA".to_string());
    }
    if lower.contains("zerossl") {
        return Some("ZeroSSL".to_string());
    }
    if lower.contains("digicert") {
        return Some("DigiCert".to_string());
    }
    if lower.contains("sectigo") || lower.contains("comodo") {
        return Some("Sectigo".to_string());
    }
    if lower.contains("google trust services") {
        return Some("Google Trust Services".to_string());
    }
    if let Some(sub) = subject {
        if sub.trim() == iss {
            return Some("Self-Signed".to_string());
        }
    }
    Some("Custom CA".to_string())
}

/// Parses `openssl x509 -in ... -noout -dates -issuer -subject [-fingerprint -sha256 -ext subjectAltName]` output into real cert metadata.
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
    let mut sha256_fingerprint: Option<String> = None;
    let mut san_domains: Vec<String> = Vec::new();

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
        } else if let Some(val) = trimmed
            .strip_prefix("sha256 Fingerprint=")
            .or_else(|| trimmed.strip_prefix("SHA256 Fingerprint="))
        {
            sha256_fingerprint = Some(val.trim().to_string());
        } else if trimmed.contains("DNS:") {
            for part in trimmed.split(',') {
                if let Some(dns) = part.trim().strip_prefix("DNS:") {
                    let clean = dns.trim().to_string();
                    if !clean.is_empty() && !san_domains.contains(&clean) {
                        san_domains.push(clean);
                    }
                }
            }
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

    let provider = detect_cert_provider(issuer.as_deref(), subject.as_deref());

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
        provider,
        san_domains,
        sha256_fingerprint,
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
