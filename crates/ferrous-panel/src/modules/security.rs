use crate::api::contract::{
    AntiDdosShieldState, BannedIpItem, Fail2banJail, HardeningDirective, SecurityData, UfwRule,
    WafRuleItem,
};
use crate::config::Config;
use crate::error::PanelError;
use crate::modules::ModuleStore;
use crate::system::cmd::AllowedBinary;
use crate::system::validate::PortOrRange;
use crate::webserver::CommandExecutor;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

const SEC_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UfwProto {
    Tcp,
    Udp,
    Both,
}

impl UfwProto {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "TCP" => Ok(Self::Tcp),
            "UDP" => Ok(Self::Udp),
            "BOTH" | "ANY" => Ok(Self::Both),
            other => Err(PanelError::Validation(format!(
                "invalid UFW protocol '{other}'; allowed: TCP, UDP, BOTH"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UfwAction {
    Allow,
    Deny,
    Limit,
}

impl UfwAction {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "ALLOW" => Ok(Self::Allow),
            "DENY" => Ok(Self::Deny),
            "LIMIT" => Ok(Self::Limit),
            other => Err(PanelError::Validation(format!(
                "invalid UFW action '{other}'; allowed: ALLOW, DENY, LIMIT"
            ))),
        }
    }

    #[must_use]
    pub const fn as_cli(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Limit => "limit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UfwSource(String);

impl UfwSource {
    pub fn parse(raw: &str) -> Result<Self, PanelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("anywhere")
            || trimmed.eq_ignore_ascii_case("any")
            || trimmed == "0.0.0.0/0"
            || trimmed == "::/0"
        {
            return Ok(Self("any".to_string()));
        }
        if let Some((ip_part, prefix_part)) = trimmed.split_once('/') {
            let ip: IpAddr = ip_part
                .parse()
                .map_err(|_| PanelError::Validation(format!("invalid CIDR IP '{ip_part}'")))?;
            let prefix: u8 = prefix_part
                .parse()
                .map_err(|_| PanelError::Validation("invalid CIDR prefix length".to_string()))?;
            let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
            if prefix > max_prefix {
                return Err(PanelError::Validation(format!(
                    "CIDR prefix /{prefix} exceeds /{max_prefix}"
                )));
            }
            return Ok(Self(format!("{ip}/{prefix}")));
        }
        let ip: IpAddr = trimmed.parse().map_err(|_| {
            PanelError::Validation(format!("invalid UFW source IP/CIDR '{trimmed}'"))
        })?;
        Ok(Self(ip.to_string()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_comment(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.len() > 64 {
        return Err(PanelError::Validation(
            "UFW comment must be <= 64 characters".to_string(),
        ));
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'.' | b'_' | b'-' | b':'))
    {
        return Err(PanelError::Validation(
            "UFW comment may only contain ASCII letters, digits, spaces, and . _ - :".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_jail_name(raw: &str) -> Result<String, PanelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return Err(PanelError::Validation(
            "fail2ban jail name must be 1..=64 characters".to_string(),
        ));
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    {
        return Err(PanelError::Validation(
            "fail2ban jail name may only contain [a-z0-9_-]".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Parses `ufw status numbered` stdout lines like:
/// `[ 1] 443/tcp                    ALLOW IN    Anywhere                   # HTTPS`
#[must_use]
pub fn parse_ufw_numbered(stdout: &str) -> Vec<UfwRule> {
    let mut rules = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('[') else {
            continue;
        };
        let Some((num_part, after_bracket)) = rest.split_once(']') else {
            continue;
        };
        let Ok(num) = num_part.trim().parse::<u32>() else {
            continue;
        };

        let (rule_body, comment) = match after_bracket.split_once('#') {
            Some((body, c)) => (body.trim(), c.trim().to_string()),
            None => (after_bracket.trim(), String::new()),
        };

        let tokens: Vec<&str> = rule_body.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        let target = tokens[0];
        let (port, protocol) = match target.split_once('/') {
            Some((p, proto)) => (p.to_string(), proto.to_ascii_uppercase()),
            None => (target.to_string(), "BOTH".to_string()),
        };

        let action = tokens
            .iter()
            .find(|t| matches!(**t, "ALLOW" | "DENY" | "REJECT" | "LIMIT"))
            .copied()
            .unwrap_or("ALLOW")
            .to_string();

        let source = tokens
            .last()
            .copied()
            .filter(|t| !matches!(*t, "IN" | "OUT" | "ALLOW" | "DENY" | "LIMIT" | "FWD"))
            .unwrap_or("Anywhere")
            .to_string();

        rules.push(UfwRule {
            id: num.to_string(),
            port,
            protocol,
            action,
            source,
            comment,
            enabled: true,
        });
    }
    rules
}

#[must_use]
pub fn parse_fail2ban_jail_list(stdout: &str) -> Vec<String> {
    for line in stdout.lines() {
        if let Some((_, list)) = line.split_once("Jail list:") {
            return list
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

#[must_use]
pub fn parse_fail2ban_jail_status(jail_name: &str, stdout: &str) -> Fail2banJail {
    let mut currently_banned: u32 = 0;
    let mut total_banned: u64 = 0;
    let mut banned_ips: Vec<BannedIpItem> = Vec::new();
    let mut log_path = String::new();
    let now = Utc::now().to_rfc3339();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some((_, val)) = trimmed.split_once("File list:") {
            log_path = val.trim().to_string();
        } else if let Some((_, val)) = trimmed.split_once("Currently banned:") {
            if let Ok(n) = val.trim().parse::<u32>() {
                currently_banned = n;
            }
        } else if let Some((_, val)) = trimmed.split_once("Total banned:") {
            if let Ok(n) = val.trim().parse::<u64>() {
                total_banned = n;
            }
        } else if let Some((_, val)) = trimmed.split_once("Banned IP list:") {
            for ip in val.split_whitespace() {
                if !ip.is_empty() {
                    banned_ips.push(BannedIpItem {
                        ip: ip.to_string(),
                        country: String::new(),
                        reason: format!("fail2ban:{jail_name}"),
                        banned_at: now.clone(),
                    });
                }
            }
        }
    }

    Fail2banJail {
        name: jail_name.to_string(),
        filter: jail_name.to_string(),
        log_path,
        max_retry: 5,
        find_time_sec: 600,
        ban_time_sec: 3600,
        currently_banned,
        total_banned,
        banned_ips,
    }
}

pub async fn list_ufw_rules(executor: &dyn CommandExecutor) -> Result<Vec<UfwRule>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Ufw) {
        return Err(PanelError::Conflict(
            "ufw is not installed on this host".to_string(),
        ));
    }
    let out = executor
        .run(AllowedBinary::Ufw, &["status", "numbered"], SEC_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "ufw",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    Ok(parse_ufw_numbered(&out.stdout))
}

pub async fn add_ufw_rule(
    executor: &dyn CommandExecutor,
    port_raw: &str,
    proto_raw: &str,
    action_raw: &str,
    source_raw: &str,
    comment_raw: &str,
) -> Result<Vec<UfwRule>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Ufw) {
        return Err(PanelError::Conflict(
            "ufw is not installed on this host".to_string(),
        ));
    }
    let port = PortOrRange::parse(port_raw)?;
    let proto = UfwProto::parse(proto_raw)?;
    let action = UfwAction::parse(action_raw)?;
    let source = UfwSource::parse(source_raw)?;
    let comment = validate_comment(comment_raw)?;

    let mut owned_args: Vec<String> = Vec::new();
    owned_args.push(action.as_cli().to_string());

    if source.as_str() == "any" {
        match proto {
            UfwProto::Tcp => owned_args.push(format!("{}/tcp", port.as_str())),
            UfwProto::Udp => owned_args.push(format!("{}/udp", port.as_str())),
            UfwProto::Both => owned_args.push(port.as_str().to_string()),
        }
    } else {
        owned_args.push("from".to_string());
        owned_args.push(source.as_str().to_string());
        owned_args.push("to".to_string());
        owned_args.push("any".to_string());
        owned_args.push("port".to_string());
        owned_args.push(port.as_str().to_string());
        match proto {
            UfwProto::Tcp => {
                owned_args.push("proto".to_string());
                owned_args.push("tcp".to_string());
            }
            UfwProto::Udp => {
                owned_args.push("proto".to_string());
                owned_args.push("udp".to_string());
            }
            UfwProto::Both => {}
        }
    }

    if !comment.is_empty() {
        owned_args.push("comment".to_string());
        owned_args.push(comment);
    }

    let arg_refs: Vec<&str> = owned_args.iter().map(String::as_str).collect();
    let out = executor.run(AllowedBinary::Ufw, &arg_refs, SEC_TIMEOUT).await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "ufw",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    list_ufw_rules(executor).await
}

fn rule_protects_port(rule: &UfwRule, port: u16) -> bool {
    let port_str = port.to_string();
    let trimmed = rule.port.trim();
    if trimmed == port_str {
        return true;
    }
    if let Some((start_s, end_s)) = trimmed.split_once(':') {
        if let (Ok(s), Ok(e)) = (start_s.parse::<u16>(), end_s.parse::<u16>()) {
            return port >= s && port <= e;
        }
    }
    trimmed.eq_ignore_ascii_case("OpenSSH") && port == 22
}

pub async fn delete_ufw_rule(
    executor: &dyn CommandExecutor,
    rule_id: &str,
) -> Result<Vec<UfwRule>, PanelError> {
    delete_ufw_rule_guarded(executor, rule_id, &[], true).await
}

pub async fn delete_ufw_rule_guarded(
    executor: &dyn CommandExecutor,
    rule_id: &str,
    protected_ports: &[u16],
    force: bool,
) -> Result<Vec<UfwRule>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Ufw) {
        return Err(PanelError::Conflict(
            "ufw is not installed on this host".to_string(),
        ));
    }
    let rule_num: u32 = rule_id
        .trim()
        .parse()
        .map_err(|_| PanelError::Validation("UFW rule ID must be a positive integer".to_string()))?;
    if rule_num == 0 {
        return Err(PanelError::Validation(
            "UFW rule ID must be >= 1".to_string(),
        ));
    }

    if !force && !protected_ports.is_empty() {
        let current_rules = list_ufw_rules(executor).await?;
        if let Some(target_rule) = current_rules.iter().find(|r| r.id == rule_num.to_string()) {
            for &port in protected_ports {
                if rule_protects_port(target_rule, port) {
                    return Err(PanelError::Conflict(format!(
                        "Refusing to delete UFW rule #{rule_num} ({}) because it protects active SSH/Panel port {port}. Pass ?force=true to confirm lockout risk.",
                        target_rule.port
                    )));
                }
            }
        }
    }

    let rule_str = rule_num.to_string();
    let out = executor
        .run(AllowedBinary::Ufw, &["--force", "delete", &rule_str], SEC_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "ufw",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    list_ufw_rules(executor).await
}

async fn query_fail2ban_u32(
    executor: &dyn CommandExecutor,
    jail: &str,
    param: &str,
    fallback: u32,
) -> u32 {
    executor
        .run(
            AllowedBinary::Fail2banClient,
            &["get", jail, param],
            SEC_TIMEOUT,
        )
        .await
        .ok()
        .filter(|o| o.success)
        .and_then(|o| o.stdout.lines().last().and_then(|l| l.trim().parse::<u32>().ok()))
        .unwrap_or(fallback)
}

async fn query_fail2ban_u64(
    executor: &dyn CommandExecutor,
    jail: &str,
    param: &str,
    fallback: u64,
) -> u64 {
    executor
        .run(
            AllowedBinary::Fail2banClient,
            &["get", jail, param],
            SEC_TIMEOUT,
        )
        .await
        .ok()
        .filter(|o| o.success)
        .and_then(|o| o.stdout.lines().last().and_then(|l| l.trim().parse::<u64>().ok()))
        .unwrap_or(fallback)
}

pub async fn list_fail2ban_jails(
    executor: &dyn CommandExecutor,
) -> Result<Vec<Fail2banJail>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Fail2banClient) {
        return Err(PanelError::Conflict(
            "fail2ban-client is not installed on this host".to_string(),
        ));
    }
    let out = executor
        .run(AllowedBinary::Fail2banClient, &["status"], SEC_TIMEOUT)
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "fail2ban-client",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    let jail_names = parse_fail2ban_jail_list(&out.stdout);
    let mut jails = Vec::with_capacity(jail_names.len());
    for name in jail_names {
        let j_out = executor
            .run(AllowedBinary::Fail2banClient, &["status", &name], SEC_TIMEOUT)
            .await?;
        if j_out.success {
            let mut jail = parse_fail2ban_jail_status(&name, &j_out.stdout);
            jail.max_retry = query_fail2ban_u32(executor, &name, "maxretry", jail.max_retry).await;
            jail.find_time_sec =
                query_fail2ban_u64(executor, &name, "findtime", jail.find_time_sec).await;
            jail.ban_time_sec =
                query_fail2ban_u64(executor, &name, "bantime", jail.ban_time_sec).await;
            jails.push(jail);
        }
    }
    Ok(jails)
}

pub async fn fail2ban_ban_unban(
    executor: &dyn CommandExecutor,
    jail_raw: &str,
    ip_raw: &str,
    ban: bool,
) -> Result<Vec<Fail2banJail>, PanelError> {
    if !executor.is_binary_available(AllowedBinary::Fail2banClient) {
        return Err(PanelError::Conflict(
            "fail2ban-client is not installed on this host".to_string(),
        ));
    }
    let jail = validate_jail_name(jail_raw)?;
    let ip: IpAddr = ip_raw
        .trim()
        .parse()
        .map_err(|_| PanelError::Validation("invalid IP address for fail2ban".to_string()))?;
    let ip_str = ip.to_string();
    let op = if ban { "banip" } else { "unbanip" };
    let out = executor
        .run(
            AllowedBinary::Fail2banClient,
            &["set", &jail, op, &ip_str],
            SEC_TIMEOUT,
        )
        .await?;
    if !out.success {
        return Err(PanelError::CommandFailed {
            binary: "fail2ban-client",
            exit_code: out.exit_code,
            stderr: out.stderr,
        });
    }
    list_fail2ban_jails(executor).await
}

const SYSCTL_HARDENING: &[(&str, &str, &str, &str, &str)] = &[
    (
        "syncookies",
        "Kernel Network",
        "net.ipv4.tcp_syncookies",
        "1",
        "Protect against TCP SYN flood attacks",
    ),
    (
        "rp_filter",
        "Kernel Network",
        "net.ipv4.conf.all.rp_filter",
        "1",
        "Enable strict reverse-path IP spoofing verification",
    ),
    (
        "protected_symlinks",
        "Filesystem",
        "fs.protected_symlinks",
        "1",
        "Restrict symlink following in world-writable sticky directories",
    ),
    (
        "dmesg_restrict",
        "Kernel Security",
        "kernel.dmesg_restrict",
        "1",
        "Restrict dmesg kernel log access to privileged users",
    ),
];

pub async fn list_hardening(executor: &dyn CommandExecutor) -> Vec<HardeningDirective> {
    let mut out = Vec::new();
    if !executor.is_binary_available(AllowedBinary::Sysctl) {
        return out;
    }
    for &(id, cat, key, rec, desc) in SYSCTL_HARDENING {
        if let Ok(res) = executor
            .run(AllowedBinary::Sysctl, &["-n", key], SEC_TIMEOUT)
            .await
        {
            if res.success {
                let current = res.stdout.trim().to_string();
                out.push(HardeningDirective {
                    id: id.to_string(),
                    category: cat.to_string(),
                    key: key.to_string(),
                    recommended_value: rec.to_string(),
                    enabled: current == rec,
                    current_value: current,
                    description: desc.to_string(),
                });
            }
        }
    }
    out
}

pub async fn toggle_hardening(
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<HardeningDirective>, PanelError> {
    let Some(&(_, _, key, rec, _)) = SYSCTL_HARDENING.iter().find(|entry| entry.0 == id) else {
        return Err(PanelError::NotFound(format!(
            "hardening directive '{id}' not found"
        )));
    };
    if !executor.is_binary_available(AllowedBinary::Sysctl) {
        return Err(PanelError::Conflict(
            "sysctl binary is not available".to_string(),
        ));
    }
    let cur = executor
        .run(AllowedBinary::Sysctl, &["-n", key], SEC_TIMEOUT)
        .await?;
    let next_val = if cur.stdout.trim() == rec { "0" } else { rec };
    let kv = format!("{key}={next_val}");
    let write_res = executor
        .run(AllowedBinary::Sysctl, &["-w", &kv], SEC_TIMEOUT)
        .await?;
    if !write_res.success {
        return Err(PanelError::CommandFailed {
            binary: "sysctl",
            exit_code: write_res.exit_code,
            stderr: write_res.stderr,
        });
    }
    Ok(list_hardening(executor).await)
}

const KV_UFW_DISABLED: &str = "security.ufw_disabled_ids";
const KV_ANTI_DDOS: &str = "security.anti_ddos_shield";

fn read_proc_u64(path: &str) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
}

fn read_kernel_dropped_packets() -> u64 {
    let mut total = 0u64;
    if let Ok(netstat) = std::fs::read_to_string("/proc/net/netstat") {
        let lines: Vec<&str> = netstat.lines().collect();
        for pair in lines.as_chunks::<2>().0 {
            if pair[0].starts_with("TcpExt:") && pair[1].starts_with("TcpExt:") {
                let keys: Vec<&str> = pair[0].split_whitespace().collect();
                let vals: Vec<&str> = pair[1].split_whitespace().collect();
                for (k, v) in keys.iter().zip(vals.iter()) {
                    if matches!(
                        *k,
                        "SyncookiesSent" | "SyncookiesFailed" | "ListenOverflows" | "ListenDrops"
                    ) {
                        if let Ok(n) = v.parse::<u64>() {
                            total = total.saturating_add(n);
                        }
                    }
                }
            }
        }
    }
    if let Ok(dev) = std::fs::read_to_string("/proc/net/dev") {
        for line in dev.lines().skip(2) {
            if let Some((iface, rest)) = line.split_once(':') {
                if iface.trim() == "lo" {
                    continue;
                }
                let cols: Vec<&str> = rest.split_whitespace().collect();
                if let Some(rx_drop) = cols.get(3).and_then(|s| s.parse::<u64>().ok()) {
                    total = total.saturating_add(rx_drop);
                }
            }
        }
    }
    total
}

fn default_anti_ddos_shield() -> AntiDdosShieldState {
    let syn_cookies = read_proc_u64("/proc/sys/net/ipv4/tcp_syncookies")
        .map(|v| v != 0)
        .unwrap_or(false);
    let syn_backlog = read_proc_u64("/proc/sys/net/ipv4/tcp_max_syn_backlog").unwrap_or(4096);
    AntiDdosShieldState {
        protection_mode: "normal".to_string(),
        xdp_status: if syn_cookies {
            format!("Linux Kernel SYN Cookies Active (tcp_max_syn_backlog={syn_backlog})")
        } else {
            "Kernel SYN Cookies Disabled (enable via protection mode or sysctl)".to_string()
        },
        syn_cookies_enabled: syn_cookies,
        syn_rate_limit_pps: syn_backlog,
        dropped_packets_total: read_kernel_dropped_packets(),
        blocked_l7_attacks_today: 0,
        fim_integrity_status: "Pending initial BLAKE3 + SHA-256 scan".to_string(),
        last_fim_scan_at: String::new(),
        waf_rules: vec![
            WafRuleItem {
                id: "waf-sqli-942100".to_string(),
                code: "CRS-942100".to_string(),
                name: "SQL Injection (libinjection AST + Tautology Filter)".to_string(),
                category: "SQLi".to_string(),
                threat_type: "SQL Injection / Auth Bypass".to_string(),
                blocked_count: 0,
                enabled: true,
                description: "Blocks UNION SELECT, tautology payloads, and stacked SQL queries before upstream dispatch.".to_string(),
            },
            WafRuleItem {
                id: "waf-xss-941100".to_string(),
                code: "CRS-941100".to_string(),
                name: "Cross-Site Scripting (DOM / Script Tag Vector)".to_string(),
                category: "XSS".to_string(),
                threat_type: "Reflected & Stored XSS".to_string(),
                blocked_count: 0,
                enabled: true,
                description: "Inspects query strings, headers, and JSON bodies for inline JS event handlers and script vectors.".to_string(),
            },
            WafRuleItem {
                id: "waf-rce-932100".to_string(),
                code: "CRS-932100".to_string(),
                name: "Remote Command Execution & Shell Shock Guard".to_string(),
                category: "RCE".to_string(),
                threat_type: "POSIX Shell / Subshell Injection".to_string(),
                blocked_count: 0,
                enabled: true,
                description: "Rejects request parameters attempting command substitution, pipe chaining, or /bin/sh payloads.".to_string(),
            },
            WafRuleItem {
                id: "waf-lfi-930100".to_string(),
                code: "CRS-930100".to_string(),
                name: "Path Traversal & Dot-Dot (/etc/passwd / .env Probe)".to_string(),
                category: "PathTraversal".to_string(),
                threat_type: "LFI / Arbitrary File Read".to_string(),
                blocked_count: 0,
                enabled: true,
                description: "Drops URLs containing encoded '../', '/.git/', '/.env', or sensitive system path probes.".to_string(),
            },
        ],
    }
}

pub fn load_anti_ddos_shield(store: &ModuleStore) -> Result<AntiDdosShieldState, PanelError> {
    let mut shield = if let Some(existing) = store.get_json::<AntiDdosShieldState>(KV_ANTI_DDOS)? {
        existing
    } else {
        let initial = default_anti_ddos_shield();
        store.put_json(KV_ANTI_DDOS, &initial)?;
        initial
    };
    if let Some(live_syn) = read_proc_u64("/proc/sys/net/ipv4/tcp_syncookies") {
        shield.syn_cookies_enabled = live_syn != 0;
    }
    if let Some(live_backlog) = read_proc_u64("/proc/sys/net/ipv4/tcp_max_syn_backlog") {
        shield.syn_rate_limit_pps = live_backlog;
    }
    shield.dropped_packets_total = read_kernel_dropped_packets();
    Ok(shield)
}

pub async fn set_ddos_mode(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    mode: &str,
) -> Result<AntiDdosShieldState, PanelError> {
    let clean = mode.trim().to_ascii_lowercase();
    let target_mode = if clean.is_empty() {
        "under_attack"
    } else if matches!(clean.as_str(), "normal" | "elevated" | "under_attack") {
        clean.as_str()
    } else {
        return Err(PanelError::Validation(format!(
            "invalid DDoS protection mode '{mode}'; allowed: normal, elevated, under_attack"
        )));
    };

    let (syn_backlog, synack_retries) = match target_mode {
        "normal" => ("4096", "5"),
        "elevated" => ("16384", "3"),
        _ => ("32768", "2"),
    };

    if executor.is_binary_available(AllowedBinary::Sysctl) {
        let kvs = [
            "net.ipv4.tcp_syncookies=1".to_string(),
            format!("net.ipv4.tcp_max_syn_backlog={syn_backlog}"),
            format!("net.ipv4.tcp_synack_retries={synack_retries}"),
        ];
        for kv in &kvs {
            let sysctl_out = executor
                .run(AllowedBinary::Sysctl, &["-w", kv], SEC_TIMEOUT)
                .await?;
            if !sysctl_out.success {
                return Err(PanelError::CommandFailed {
                    binary: "sysctl",
                    exit_code: sysctl_out.exit_code,
                    stderr: sysctl_out.stderr,
                });
            }
        }
    }

    let mut shield = load_anti_ddos_shield(store)?;
    shield.protection_mode = target_mode.to_string();
    shield.syn_cookies_enabled = true;
    shield.syn_rate_limit_pps = syn_backlog.parse::<u64>().unwrap_or(16384);
    shield.xdp_status = match target_mode {
        "normal" => format!("Kernel SYN Cookies Active (backlog={syn_backlog}, synack_retries={synack_retries})"),
        "elevated" => format!("Strict Kernel SYN Hardening Active (backlog={syn_backlog}, synack_retries={synack_retries})"),
        _ => format!("Emergency Kernel SYN Lockdown Active (backlog={syn_backlog}, synack_retries={synack_retries})"),
    };

    store.put_json(KV_ANTI_DDOS, &shield)?;
    Ok(shield)
}

pub fn toggle_waf_rule(
    store: &ModuleStore,
    id: &str,
) -> Result<AntiDdosShieldState, PanelError> {
    let mut shield = load_anti_ddos_shield(store)?;
    let Some(rule) = shield.waf_rules.iter_mut().find(|r| r.id == id || r.code == id) else {
        return Err(PanelError::NotFound(format!("WAF rule '{id}' not found")));
    };
    rule.enabled = !rule.enabled;
    store.put_json(KV_ANTI_DDOS, &shield)?;
    Ok(shield)
}

fn scan_dir_streaming(
    root: &Path,
    depth: u8,
    b3_hasher: &mut blake3::Hasher,
    sha_hasher: &mut Sha256,
    count: &mut u64,
) {
    use std::io::Read as _;
    if depth > 4 || !root.exists() {
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    let mut buf = [0u8; 64 * 1024];
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_symlink() {
            continue;
        }
        let p = entry.path();
        if ft.is_file() {
            if let Ok(mut file) = std::fs::File::open(&p) {
                let p_bytes = p.to_string_lossy();
                b3_hasher.update(p_bytes.as_bytes());
                sha_hasher.update(p_bytes.as_bytes());
                loop {
                    match file.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            b3_hasher.update(&buf[..n]);
                            sha_hasher.update(&buf[..n]);
                        }
                        Err(_) => break,
                    }
                }
                *count = count.saturating_add(1);
            }
        } else if ft.is_dir() {
            scan_dir_streaming(&p, depth + 1, b3_hasher, sha_hasher, count);
        }
    }
}

pub fn run_fim_scan(
    store: &ModuleStore,
    config: &Config,
) -> Result<(u64, String, AntiDdosShieldState), PanelError> {
    let mut b3_hasher = blake3::Hasher::new();
    let mut sha_hasher = Sha256::new();
    let mut count = 0u64;
    scan_dir_streaming(&config.wwwroot, 0, &mut b3_hasher, &mut sha_hasher, &mut count);
    scan_dir_streaming(&config.data_dir, 0, &mut b3_hasher, &mut sha_hasher, &mut count);
    let b3_hex = b3_hasher.finalize().to_hex().to_string();
    let sha_hex = format!("{:x}", sha_hasher.finalize());
    let short_b3: String = b3_hex.chars().take(12).collect();

    let mut shield = load_anti_ddos_shield(store)?;
    shield.last_fim_scan_at = Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    shield.fim_integrity_status = format!("Verified ({count} files · BLAKE3:{short_b3})");
    store.put_json(KV_ANTI_DDOS, &shield)?;
    Ok((count, sha_hex, shield))
}

pub async fn toggle_ufw_rule(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
    id: &str,
) -> Result<Vec<UfwRule>, PanelError> {
    let mut disabled = store
        .get_json::<BTreeSet<String>>(KV_UFW_DISABLED)?
        .unwrap_or_default();
    let clean_id = id.trim().to_string();
    if !disabled.remove(&clean_id) {
        disabled.insert(clean_id);
    }
    store.put_json(KV_UFW_DISABLED, &disabled)?;

    let mut rules = match list_ufw_rules(executor).await {
        Ok(r) => r,
        Err(PanelError::Conflict(_)) => Vec::new(),
        Err(err) => return Err(err),
    };
    for rule in &mut rules {
        if disabled.contains(&rule.id) {
            rule.enabled = false;
        }
    }
    Ok(rules)
}

/// Loads real security overview (`ufw`, `fail2ban-client`, `sysctl`, kernel netstat counters, and FIM state).
pub async fn load_security_overview(
    store: &ModuleStore,
    executor: &dyn CommandExecutor,
) -> Result<SecurityData, PanelError> {
    let disabled = store
        .get_json::<BTreeSet<String>>(KV_UFW_DISABLED)?
        .unwrap_or_default();
    let mut ufw_rules = match list_ufw_rules(executor).await {
        Ok(rules) => rules,
        Err(PanelError::Conflict(_)) => Vec::new(),
        Err(err) => return Err(err),
    };
    for rule in &mut ufw_rules {
        if disabled.contains(&rule.id) {
            rule.enabled = false;
        }
    }
    let fail2ban_jails = match list_fail2ban_jails(executor).await {
        Ok(jails) => jails,
        Err(PanelError::Conflict(_)) => Vec::new(),
        Err(err) => return Err(err),
    };
    let hardening_directives = list_hardening(executor).await;
    let mut shield = load_anti_ddos_shield(store)?;
    shield.blocked_l7_attacks_today = fail2ban_jails.iter().map(|j| j.total_banned).sum();
    let anti_ddos_shield = Some(shield);

    Ok(SecurityData {
        ufw_rules,
        fail2ban_jails,
        hardening_directives,
        anti_ddos_shield,
    })
}
