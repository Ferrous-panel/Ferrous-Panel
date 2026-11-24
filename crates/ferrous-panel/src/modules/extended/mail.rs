use super::common::*;
use std::os::unix::fs::PermissionsExt as _;

// ============================================================================
// 5. Mail Server (Real Postfix / Dovecot / Rspamd Integration)
// ============================================================================

const DKIM_SELECTOR_NAME: &str = "ferrous";
const DKIM_SELECTOR_HOST: &str = "ferrous._domainkey";

async fn generate_real_dkim_public_key(
    config: &Config,
    executor: &dyn CommandExecutor,
    domain: &Domain,
) -> Result<String, PanelError> {
    let dkim_dir = config.data_dir.join("mail").join("dkim");
    std::fs::create_dir_all(&dkim_dir)?;
    let priv_path = dkim_dir.join(format!("{}.key", domain.as_str()));
    let pub_path = dkim_dir.join(format!("{}.pub", domain.as_str()));

    if !executor.is_binary_available(AllowedBinary::OpenSsl) {
        if is_production_vps(config) {
            return Err(PanelError::Conflict(
                "thiếu binary openssl để tạo khóa DKIM RSA-2048 (gợi ý: sudo apt install -y openssl postfix dovecot-core rspamd)".to_string(),
            ));
        }
        return Ok("v=DKIM1; k=rsa; p=PENDING_OPENSSL_UNAVAILABLE".to_string());
    }

    let priv_str = priv_path.display().to_string();
    if !priv_path.is_file() {
        let gen = executor
            .run(
                AllowedBinary::OpenSsl,
                &[
                    "genpkey",
                    "-algorithm",
                    "RSA",
                    "-pkeyopt",
                    "rsa_keygen_bits:2048",
                    "-out",
                    &priv_str,
                ],
                CMD_TIMEOUT,
            )
            .await?;
        if !gen.success {
            return Err(PanelError::CommandFailed {
                binary: "openssl",
                exit_code: gen.exit_code,
                stderr: if gen.stderr.trim().is_empty() {
                    gen.stdout
                } else {
                    gen.stderr
                },
            });
        }
        if priv_path.is_file() {
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&priv_path, perms)?;
        }
    }

    let pub_out = executor
        .run(
            AllowedBinary::OpenSsl,
            &["pkey", "-in", &priv_str, "-pubout"],
            CMD_TIMEOUT,
        )
        .await?;
    if !pub_out.success {
        return Err(PanelError::CommandFailed {
            binary: "openssl",
            exit_code: pub_out.exit_code,
            stderr: if pub_out.stderr.trim().is_empty() {
                pub_out.stdout
            } else {
                pub_out.stderr
            },
        });
    }

    let b64: String = pub_out
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("-----"))
        .collect();
    if b64.is_empty() {
        if is_production_vps(config) {
            return Err(PanelError::Conflict(
                "openssl pkey -pubout không trả về public key hợp lệ".to_string(),
            ));
        }
        return Ok("v=DKIM1; k=rsa; p=PENDING_OPENSSL_TEST".to_string());
    }

    let record = format!("v=DKIM1; k=rsa; p={b64}");
    atomic_write_file(&pub_path, format!("{record}\n").as_bytes(), 0o644)?;
    Ok(record)
}

async fn check_txt_record_contains(
    executor: &dyn CommandExecutor,
    hostname: &str,
    needle: &str,
) -> String {
    if !executor.is_binary_available(AllowedBinary::Dig) {
        return "unverified".to_string();
    }
    match executor
        .run(
            AllowedBinary::Dig,
            &["+short", "+time=3", "+tries=1", "TXT", hostname],
            CMD_TIMEOUT,
        )
        .await
    {
        Ok(out) if out.success => {
            let normalized = out.stdout.replace('"', "").to_ascii_lowercase();
            if normalized.contains(&needle.to_ascii_lowercase()) {
                "verified".to_string()
            } else {
                "missing".to_string()
            }
        }
        _ => "unverified".to_string(),
    }
}

async fn build_mail_domain_item(
    config: &Config,
    executor: &dyn CommandExecutor,
    domain: &Domain,
    mailboxes_count: u32,
) -> Result<MailDomainItem, PanelError> {
    let d = domain.as_str();
    let dkim_public_key = generate_real_dkim_public_key(config, executor, domain).await?;
    let spf_status = check_txt_record_contains(executor, d, "v=spf1").await;
    let dkim_host = format!("{DKIM_SELECTOR_HOST}.{d}");
    let dkim_status = check_txt_record_contains(executor, &dkim_host, "v=dkim1").await;
    let dmarc_host = format!("_dmarc.{d}");
    let dmarc_status = check_txt_record_contains(executor, &dmarc_host, "v=dmarc1").await;

    Ok(MailDomainItem {
        id: format!("md-{}", d.replace('.', "-")),
        domain: d.to_string(),
        spf_status,
        dkim_status,
        dmarc_status,
        dkim_selector: DKIM_SELECTOR_HOST.to_string(),
        dkim_public_key,
        catch_all_address: format!("postmaster@{d}"),
        mailboxes_count,
    })
}

fn default_mail_server_data(config: &Config) -> MailServerData {
    if is_production_vps(config) {
        return MailServerData {
            postfix_version: "not installed".to_string(),
            dovecot_version: "not installed".to_string(),
            rspamd_version: "not installed".to_string(),
            postfix_status: "not-installed".to_string(),
            dovecot_status: "not-installed".to_string(),
            rspamd_status: "not-installed".to_string(),
            queue_count: 0,
            sent_today: 0,
            spam_blocked_today: 0,
            domains: Vec::new(),
            mailboxes: Vec::new(),
            forwarders: Vec::new(),
            messages: Vec::new(),
        };
    }

    let local_domain = resolve_local_domain(Some(config));
    let Ok(dom) = Domain::parse(&local_domain) else {
        return MailServerData {
            postfix_version: "Postfix".to_string(),
            dovecot_version: "Dovecot".to_string(),
            rspamd_version: "Rspamd".to_string(),
            postfix_status: "test-mode".to_string(),
            dovecot_status: "test-mode".to_string(),
            rspamd_status: "test-mode".to_string(),
            queue_count: 0,
            sent_today: 0,
            spam_blocked_today: 0,
            domains: Vec::new(),
            mailboxes: Vec::new(),
            forwarders: Vec::new(),
            messages: Vec::new(),
        };
    };
    let d = dom.as_str();
    MailServerData {
        postfix_version: "Postfix (test mode)".to_string(),
        dovecot_version: "Dovecot (test mode)".to_string(),
        rspamd_version: "Rspamd (test mode)".to_string(),
        postfix_status: "test-mode".to_string(),
        dovecot_status: "test-mode".to_string(),
        rspamd_status: "test-mode".to_string(),
        queue_count: 0,
        sent_today: 0,
        spam_blocked_today: 0,
        domains: vec![MailDomainItem {
            id: format!("md-{}", d.replace('.', "-")),
            domain: d.to_string(),
            spf_status: "pending".to_string(),
            dkim_status: "pending".to_string(),
            dmarc_status: "pending".to_string(),
            dkim_selector: DKIM_SELECTOR_HOST.to_string(),
            dkim_public_key: format!("v=DKIM1; k=rsa; p=PENDING_KEYGEN_{DKIM_SELECTOR_NAME}"),
            catch_all_address: format!("postmaster@{d}"),
            mailboxes_count: 0,
        }],
        mailboxes: Vec::new(),
        forwarders: Vec::new(),
        messages: Vec::new(),
    }
}

fn dir_usage_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() {
                total = total.saturating_add(dir_usage_bytes(&entry.path()));
            }
        }
    }
    total
}

fn parse_rfc5322_file(path: &Path, id: String, folder: &str) -> Option<MailMessageItem> {
    let raw = std::fs::read(path).ok()?;
    let slice = if raw.len() > 64 * 1024 {
        &raw[..64 * 1024]
    } else {
        &raw[..]
    };
    let text = String::from_utf8_lossy(slice);
    let (headers_part, body_part) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
        .unwrap_or((&text, ""));

    let mut from = String::new();
    let mut to = String::new();
    let mut subject = "(No Subject)".to_string();
    let mut date = String::new();
    let mut dkim_signed = false;

    for line in headers_part.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            match key.as_str() {
                "from" if from.is_empty() => from = val.to_string(),
                "to" if to.is_empty() => to = val.to_string(),
                "subject" => subject = val.to_string(),
                "date" if date.is_empty() => date = val.to_string(),
                "dkim-signature" => dkim_signed = true,
                _ => {}
            }
        }
    }
    if from.is_empty() && to.is_empty() {
        return None;
    }
    let body_trimmed = body_part.trim().chars().take(4096).collect::<String>();
    let snippet = body_trimmed
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect::<String>();

    Some(MailMessageItem {
        id,
        folder: folder.to_string(),
        from,
        to,
        subject,
        snippet,
        body: body_trimmed,
        timestamp: if date.is_empty() {
            Utc::now().format("%Y-%m-%d %H:%M UTC").to_string()
        } else {
            date
        },
        dkim_signed,
    })
}

fn scan_maildir_inbox_messages(config: &Config, mail: &MailServerData) -> Vec<MailMessageItem> {
    let mut found = Vec::new();
    let base_roots = [
        PathBuf::from("/var/mail/vhosts"),
        config.data_dir.join("mail").join("vhosts"),
    ];
    for mb in &mail.mailboxes {
        for base in &base_roots {
            let mdir = base.join(&mb.domain).join(&mb.email);
            if !mdir.is_dir() {
                continue;
            }
            for sub in ["new", "cur"] {
                let sub_dir = mdir.join(sub);
                let Ok(entries) = std::fs::read_dir(&sub_dir) else {
                    continue;
                };
                for entry in entries.flatten().take(20) {
                    let p = entry.path();
                    if !p.is_file() {
                        continue;
                    }
                    let fname = entry.file_name().to_string_lossy().to_string();
                    let msg_id = format!("mdir-{}", blake3_hex(format!("{}:{fname}", mb.email).as_bytes()).chars().take(12).collect::<String>());
                    if let Some(item) = parse_rfc5322_file(&p, msg_id, "inbox") {
                        found.push(item);
                    }
                    if found.len() >= 40 {
                        return found;
                    }
                }
            }
        }
    }
    found
}

fn sync_mail_virtual_files(config: &Config, mail: &MailServerData) -> Result<(), PanelError> {
    let mail_dir = config.data_dir.join("mail");
    std::fs::create_dir_all(&mail_dir)?;
    let domains_txt = mail
        .domains
        .iter()
        .map(|d| format!("{} OK\n", d.domain))
        .collect::<String>();
    atomic_write_file(&mail_dir.join("virtual_domains"), domains_txt.as_bytes(), 0o644)?;

    let mailboxes_txt = mail
        .mailboxes
        .iter()
        .map(|m| format!("{} {}/{}/\n", m.email, m.domain, m.email))
        .collect::<String>();
    atomic_write_file(&mail_dir.join("vmailbox"), mailboxes_txt.as_bytes(), 0o644)?;

    let virtual_alias = mail
        .forwarders
        .iter()
        .map(|f| format!("{} {}\n", f.source_email, f.destination_email))
        .collect::<String>();
    atomic_write_file(&mail_dir.join("virtual_alias"), virtual_alias.as_bytes(), 0o644)?;
    Ok(())
}

fn validate_mail_header_value(value: &str, field: &str) -> Result<String, PanelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PanelError::Validation(format!("{field} cannot be empty")));
    }
    if trimmed.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
        return Err(PanelError::Validation(format!(
            "{field} contains forbidden control characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_mail_address(raw: &str, field: &str) -> Result<String, PanelError> {
    let clean = validate_mail_header_value(raw, field)?.to_ascii_lowercase();
    if clean.starts_with('-') || clean.len() > 254 || clean.chars().any(char::is_whitespace) {
        return Err(PanelError::Validation(format!(
            "{field} must be a valid email address"
        )));
    }
    let Some((local, host)) = clean.split_once('@') else {
        return Err(PanelError::Validation(format!(
            "{field} must contain '@'"
        )));
    };
    if local.is_empty()
        || local.len() > 64
        || !local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    {
        return Err(PanelError::Validation(format!(
            "{field} contains invalid characters in local part"
        )));
    }
    let _ = Domain::parse(host)?;
    Ok(clean)
}

async fn sync_mail_virtual_files_live(
    config: &Config,
    mail: &MailServerData,
    executor: &dyn CommandExecutor,
    require_mta_in_prod: bool,
) -> Result<(), PanelError> {
    sync_mail_virtual_files(config, mail)?;

    if !is_production_vps(config) {
        return Ok(());
    }

    if !Path::new("/etc/postfix").is_dir() || !executor.is_binary_available(AllowedBinary::Postmap) {
        if require_mta_in_prod {
            return Err(PanelError::Conflict(
                "Postfix chưa được cài đặt hoặc thiếu /etc/postfix / postmap — hãy vào 'PHP-FPM & Phần mềm' để cài 1-click Postfix + Dovecot + Rspamd".to_string(),
            ));
        }
        return Ok(());
    }

    // Ensure system Maildir directories exist under /var/mail/vhosts
    for mb in &mail.mailboxes {
        let sys_mdir = Path::new("/var/mail/vhosts").join(&mb.domain).join(&mb.email);
        for sub in ["cur", "new", "tmp"] {
            let _ = std::fs::create_dir_all(sys_mdir.join(sub));
        }
    }

    let domains_txt = mail
        .domains
        .iter()
        .map(|d| format!("{} OK\n", d.domain))
        .collect::<String>();
    let mailboxes_txt = mail
        .mailboxes
        .iter()
        .map(|m| format!("{} {}/{}/\n", m.email, m.domain, m.email))
        .collect::<String>();
    let virtual_alias = mail
        .forwarders
        .iter()
        .map(|f| format!("{} {}\n", f.source_email, f.destination_email))
        .collect::<String>();

    let files = [
        (
            Path::new("/etc/postfix/ferrous_virtual_domains"),
            domains_txt.as_bytes(),
        ),
        (
            Path::new("/etc/postfix/ferrous_virtual_mailboxes"),
            mailboxes_txt.as_bytes(),
        ),
        (
            Path::new("/etc/postfix/ferrous_virtual_aliases"),
            virtual_alias.as_bytes(),
        ),
    ];

    let snapshots: Vec<(&Path, Option<Vec<u8>>)> = files
        .iter()
        .map(|(p, _)| (*p, std::fs::read(p).ok()))
        .collect();

    let rollback = || {
        for (p, prev) in &snapshots {
            if let Some(bytes) = prev {
                let _ = atomic_write_file(p, bytes, 0o644);
            } else {
                let _ = std::fs::remove_file(p);
            }
        }
    };

    for (path, bytes) in &files {
        if let Err(err) = atomic_write_file(path, bytes, 0o644) {
            rollback();
            return Err(err);
        }
    }

    for (path, _) in &files {
        let p_str = path.display().to_string();
        let res = executor
            .run(AllowedBinary::Postmap, &[&p_str], CMD_TIMEOUT)
            .await;
        match res {
            Ok(out) if out.success => {}
            Ok(out) => {
                rollback();
                return Err(PanelError::CommandFailed {
                    binary: "postmap",
                    exit_code: out.exit_code,
                    stderr: if out.stderr.trim().is_empty() {
                        out.stdout
                    } else {
                        out.stderr
                    },
                });
            }
            Err(err) => {
                rollback();
                return Err(err);
            }
        }
    }

    // Wire Postfix main.cf virtual maps via postconf -e so Postfix actually loads these tables
    if executor.is_binary_available(AllowedBinary::Postconf) {
        let _ = executor
            .run(
                AllowedBinary::Postconf,
                &[
                    "-e",
                    "virtual_mailbox_domains=hash:/etc/postfix/ferrous_virtual_domains",
                    "virtual_mailbox_maps=hash:/etc/postfix/ferrous_virtual_mailboxes",
                    "virtual_alias_maps=hash:/etc/postfix/ferrous_virtual_aliases",
                    "virtual_mailbox_base=/var/mail/vhosts",
                ],
                CMD_TIMEOUT,
            )
            .await;
    }

    if !executor.is_binary_available(AllowedBinary::Systemctl) {
        rollback();
        return Err(PanelError::Conflict(
            "thiếu systemctl để reload postfix.service (đã rollback)".to_string(),
        ));
    }

    let reload = executor
        .run(
            AllowedBinary::Systemctl,
            &["reload-or-restart", "postfix.service"],
            CMD_TIMEOUT,
        )
        .await;
    match reload {
        Ok(out) if out.success => Ok(()),
        Ok(out) => {
            rollback();
            Err(PanelError::CommandFailed {
                binary: "systemctl",
                exit_code: out.exit_code,
                stderr: if out.stderr.trim().is_empty() {
                    out.stdout
                } else {
                    out.stderr
                },
            })
        }
        Err(err) => {
            rollback();
            Err(err)
        }
    }
}

async fn probe_mail_unit_status(executor: &dyn CommandExecutor, unit: &str) -> String {
    match executor
        .run(AllowedBinary::Systemctl, &["is-active", unit], CMD_TIMEOUT)
        .await
    {
        Ok(out) => {
            let s = out.stdout.trim();
            if s.eq_ignore_ascii_case("active") {
                "active".to_string()
            } else if s.eq_ignore_ascii_case("inactive") || s.eq_ignore_ascii_case("failed") {
                s.to_ascii_lowercase()
            } else {
                "not-installed".to_string()
            }
        }
        Err(_) => "not-installed".to_string(),
    }
}

async fn probe_postfix_queue_count(executor: &dyn CommandExecutor) -> u32 {
    if !executor.is_binary_available(AllowedBinary::Postqueue) {
        return 0;
    }
    match executor
        .run(AllowedBinary::Postqueue, &["-j"], Duration::from_secs(4))
        .await
    {
        Ok(out) if out.success => out
            .stdout
            .lines()
            .filter(|l| l.trim().starts_with('{'))
            .count() as u32,
        _ => 0,
    }
}

async fn probe_rspamd_spam_blocked(executor: &dyn CommandExecutor) -> u64 {
    if !executor.is_binary_available(AllowedBinary::Rspamc) {
        return 0;
    }
    let Ok(out) = executor
        .run(AllowedBinary::Rspamc, &["stat"], Duration::from_secs(4))
        .await
    else {
        return 0;
    };
    if !out.success {
        return 0;
    }
    let mut blocked = 0u64;
    for line in out.stdout.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("action reject:") || lower.contains("action soft reject:") || lower.contains("action add header:") {
            if let Some((_, rhs)) = line.split_once(':') {
                if let Some(num) = rhs
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                {
                    blocked = blocked.saturating_add(num);
                }
            }
        }
    }
    blocked
}

pub async fn load_mail_server(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
) -> Result<MailServerData, PanelError> {
    let mut mail = if let Some(existing) = store.get_json::<MailServerData>(KV_MAIL_SERVER)? {
        existing
    } else {
        let initial = default_mail_server_data(config);
        store.put_json(KV_MAIL_SERVER, &initial)?;
        sync_mail_virtual_files_live(config, &initial, executor, false).await?;
        initial
    };

    if is_production_vps(config) {
        // 1. Live systemd status
        if executor.is_binary_available(AllowedBinary::Systemctl) {
            mail.postfix_status = probe_mail_unit_status(executor, "postfix.service").await;
            mail.dovecot_status = probe_mail_unit_status(executor, "dovecot.service").await;
            mail.rspamd_status = probe_mail_unit_status(executor, "rspamd.service").await;
        }

        // 2. Live versions from real Postfix / Dovecot / Rspamd binaries
        mail.postfix_version = probe_binary_version_line(
            executor,
            AllowedBinary::Postconf,
            &["-h", "mail_version"],
        )
        .await
        .map(|v| format!("Postfix {v}"))
        .unwrap_or_else(|| "not installed".to_string());

        mail.dovecot_version =
            probe_binary_version_line(executor, AllowedBinary::Dovecot, &["--version"])
                .await
                .map(|v| format!("Dovecot {v}"))
                .unwrap_or_else(|| "not installed".to_string());

        mail.rspamd_version =
            probe_binary_version_line(executor, AllowedBinary::Rspamd, &["--version"])
                .await
                .unwrap_or_else(|| "not installed".to_string());

        // 3. Live Postfix queue count & Rspamd spam-blocked statistics
        mail.queue_count = probe_postfix_queue_count(executor).await;
        mail.spam_blocked_today = probe_rspamd_spam_blocked(executor).await;

        // 4. Live Maildir disk usage for each mailbox
        for mb in &mut mail.mailboxes {
            let sys_bytes = dir_usage_bytes(&Path::new("/var/mail/vhosts").join(&mb.domain).join(&mb.email));
            let local_bytes = dir_usage_bytes(&config.data_dir.join("mail").join("vhosts").join(&mb.domain).join(&mb.email));
            let total = sys_bytes.max(local_bytes);
            mb.used_mb = ((total as f64 / 1_048_576.0) * 100.0).round() / 100.0;
        }

        // 5. Merge live incoming Maildir inbox messages
        let inbox_msgs = scan_maildir_inbox_messages(config, &mail);
        for incoming in inbox_msgs {
            if !mail.messages.iter().any(|m| m.id == incoming.id) {
                mail.messages.insert(0, incoming);
            }
        }
        if mail.messages.len() > 100 {
            mail.messages.truncate(100);
        }

        // 6. Live DNS SPF/DKIM/DMARC verification via dig
        if executor.is_binary_available(AllowedBinary::Dig) {
            for dom in &mut mail.domains {
                let d = dom.domain.clone();
                dom.spf_status = check_txt_record_contains(executor, &d, "v=spf1").await;
                dom.dkim_status = check_txt_record_contains(
                    executor,
                    &format!("{}.{d}", dom.dkim_selector),
                    "v=dkim1",
                )
                .await;
                dom.dmarc_status =
                    check_txt_record_contains(executor, &format!("_dmarc.{d}"), "v=dmarc1").await;
            }
        }
    }

    Ok(mail)
}

pub async fn add_mail_domain(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    raw_domain: &str,
) -> Result<MailServerData, PanelError> {
    let domain = Domain::parse(raw_domain)?;
    let mut mail = load_mail_server(store, config, executor).await?;
    if mail
        .domains
        .iter()
        .any(|d| d.domain.eq_ignore_ascii_case(domain.as_str()))
    {
        return Err(PanelError::Conflict(format!(
            "mail domain '{}' already exists",
            domain.as_str()
        )));
    }
    let item = build_mail_domain_item(config, executor, &domain, 0).await?;
    mail.domains.insert(0, item);
    sync_mail_virtual_files_live(config, &mail, executor, false).await?;
    store.put_json(KV_MAIL_SERVER, &mail)?;
    Ok(mail)
}

pub async fn create_mailbox(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    email: &str,
    domain: &str,
    display_name: &str,
    quota_mb: u64,
) -> Result<MailServerData, PanelError> {
    let parsed_domain = Domain::parse(domain)?;
    let clean_email = validate_mail_address(email, "mailbox email")?;
    let mut mail = load_mail_server(store, config, executor).await?;
    if mail
        .mailboxes
        .iter()
        .any(|m| m.email.eq_ignore_ascii_case(&clean_email))
    {
        return Err(PanelError::Conflict(format!(
            "mailbox '{clean_email}' already exists"
        )));
    }
    let id = format!("mb-{:x}", Utc::now().timestamp_millis());
    let maildir_root = config
        .data_dir
        .join("mail")
        .join("vhosts")
        .join(parsed_domain.as_str())
        .join(&clean_email);
    for sub in ["cur", "new", "tmp"] {
        std::fs::create_dir_all(maildir_root.join(sub))?;
    }

    mail.mailboxes.insert(
        0,
        MailboxAccount {
            id,
            email: clean_email,
            domain: parsed_domain.as_str().to_string(),
            display_name: if display_name.trim().is_empty() {
                "Mailbox User".to_string()
            } else {
                display_name.trim().to_string()
            },
            quota_mb: quota_mb.max(64),
            used_mb: 0.0,
            imap_server: format!("mail.{}:993 (SSL/TLS)", parsed_domain.as_str()),
            smtp_server: format!("mail.{}:465 (SMTPS)", parsed_domain.as_str()),
            status: "active".to_string(),
            last_login: "Never".to_string(),
        },
    );
    for d in &mut mail.domains {
        if d.domain.eq_ignore_ascii_case(parsed_domain.as_str()) {
            d.mailboxes_count = d.mailboxes_count.saturating_add(1);
        }
    }
    sync_mail_virtual_files_live(config, &mail, executor, true).await?;
    store.put_json(KV_MAIL_SERVER, &mail)?;
    Ok(mail)
}

pub async fn delete_mailbox(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<MailServerData, PanelError> {
    let mut mail = load_mail_server(store, config, executor).await?;
    let removed = mail
        .mailboxes
        .iter()
        .find(|m| m.id == id)
        .map(|m| (m.domain.clone(), m.email.clone()));
    let before = mail.mailboxes.len();
    mail.mailboxes.retain(|m| m.id != id);
    if mail.mailboxes.len() == before {
        return Err(PanelError::NotFound(format!("mailbox '{id}' not found")));
    }
    if let Some((dom, email)) = removed {
        for d in &mut mail.domains {
            if d.domain.eq_ignore_ascii_case(&dom) {
                d.mailboxes_count = d.mailboxes_count.saturating_sub(1);
            }
        }
        sync_mail_virtual_files_live(config, &mail, executor, true).await?;
        let maildir_root = config.data_dir.join("mail").join("vhosts").join(&dom).join(&email);
        if maildir_root.exists() {
            std::fs::remove_dir_all(&maildir_root)?;
        }
    }
    store.put_json(KV_MAIL_SERVER, &mail)?;
    Ok(mail)
}

pub async fn create_forwarder(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    source_email: &str,
    destination_email: &str,
    domain: &str,
) -> Result<MailServerData, PanelError> {
    let parsed_domain = Domain::parse(domain)?;
    let src = validate_mail_address(source_email, "source email")?;
    let dst = validate_mail_address(destination_email, "destination email")?;
    let mut mail = load_mail_server(store, config, executor).await?;
    let id = format!("fwd-{:x}", Utc::now().timestamp_millis());
    mail.forwarders.insert(
        0,
        MailForwarder {
            id,
            source_email: src,
            destination_email: dst,
            domain: parsed_domain.as_str().to_string(),
            active: true,
        },
    );
    sync_mail_virtual_files_live(config, &mail, executor, true).await?;
    store.put_json(KV_MAIL_SERVER, &mail)?;
    Ok(mail)
}

pub async fn delete_forwarder(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<MailServerData, PanelError> {
    let mut mail = load_mail_server(store, config, executor).await?;
    let before = mail.forwarders.len();
    mail.forwarders.retain(|f| f.id != id);
    if mail.forwarders.len() == before {
        return Err(PanelError::NotFound(format!("forwarder '{id}' not found")));
    }
    sync_mail_virtual_files_live(config, &mail, executor, true).await?;
    store.put_json(KV_MAIL_SERVER, &mail)?;
    Ok(mail)
}

pub async fn send_test_mail(
    store: &ModuleStore,
    config: &Config,
    executor: &dyn CommandExecutor,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<MailServerData, PanelError> {
    let clean_from = validate_mail_address(from, "sender email")?;
    let clean_to = validate_mail_address(to, "recipient email")?;
    let clean_subject = if subject.trim().is_empty() {
        "(No Subject)".to_string()
    } else {
        validate_mail_header_value(subject, "subject")?
    };

    let msg_id = format!("msg-{:x}", Utc::now().timestamp_millis());
    let rfc_date = Utc::now().to_rfc2822();
    let rfc5322 = format!(
        "From: {clean_from}\r\nTo: {clean_to}\r\nSubject: {clean_subject}\r\nDate: {rfc_date}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\nX-Mailer: FerrousPanel\r\n\r\n{body}\r\n"
    );

    let spool_dir = config.data_dir.join("mail").join("spool");
    std::fs::create_dir_all(&spool_dir)?;
    atomic_write_file(
        &spool_dir.join(format!("{msg_id}.eml")),
        rfc5322.as_bytes(),
        0o640,
    )?;

    let (delivery_note, dkim_signed) = if is_production_vps(config) {
        if !executor.is_binary_available(AllowedBinary::Sendmail) {
            return Err(PanelError::Conflict(
                "thiếu binary sendmail/Postfix để gửi email thật — hãy cài Postfix trong mục 'PHP-FPM & Phần mềm'".to_string(),
            ));
        }
        let out = executor
            .run_with_stdin(
                AllowedBinary::Sendmail,
                &["-f", &clean_from, "--", &clean_to],
                rfc5322.as_bytes(),
                CMD_TIMEOUT,
            )
            .await?;
        if !out.success {
            let err = if out.stderr.trim().is_empty() {
                out.stdout.trim().to_string()
            } else {
                out.stderr.trim().to_string()
            };
            return Err(PanelError::CommandFailed {
                binary: "sendmail",
                exit_code: out.exit_code,
                stderr: err,
            });
        }
        ("dispatched via host sendmail/Postfix MTA".to_string(), true)
    } else {
        ("spooled in test mode (FERROUS_MODE=test)".to_string(), false)
    };

    let mut mail = load_mail_server(store, config, executor).await?;
    let trimmed_body = body.trim();
    let snippet_prefix = if trimmed_body.is_empty() {
        delivery_note.clone()
    } else {
        format!("{trimmed_body} [{delivery_note}]")
    };
    let snippet = snippet_prefix.chars().take(120).collect::<String>();
    mail.sent_today = mail.sent_today.saturating_add(1);
    mail.messages.insert(
        0,
        MailMessageItem {
            id: msg_id,
            folder: "sent".to_string(),
            from: clean_from,
            to: clean_to,
            subject: clean_subject,
            snippet,
            body: format!("{body}\n\n---\nDelivery: {delivery_note}"),
            timestamp: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
            dkim_signed,
        },
    );
    store.put_json(KV_MAIL_SERVER, &mail)?;
    Ok(mail)
}
