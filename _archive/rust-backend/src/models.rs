//! Ferrous System Core — Translated Domain Models from `server.ts` (`rust-backend/src/models.rs`)
//!
//! Strongly-typed Rust structs and enums representing the complete domain model
//! and state structures of Ferrous-panel (`server.ts`), serialized with `camelCase`
//! JSON compatibility for the React/Vite frontend.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteRedirectRule {
    pub id: String,
    pub source_path: String,
    pub target_url: String,
    #[serde(rename = "type")]
    pub redirect_type: String,
    pub keep_query: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotlinkProtectionConfig {
    pub enabled: bool,
    pub protected_extensions: String,
    pub allowed_domains: String,
    pub return_blank_or_403: String,
    pub allow_empty_referer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBasicAuthRule {
    pub id: String,
    pub name: String,
    pub path: String,
    pub username: String,
    pub password_hash: String,
    pub realm_name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDeployConfig {
    pub enabled: bool,
    pub repo_url: String,
    pub branch: String,
    pub deploy_script: String,
    pub webhook_url: String,
    pub last_commit_hash: String,
    pub last_deploy_time: String,
    pub last_deploy_log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedApp {
    pub id: String,
    pub name: String,
    pub framework: String,
    pub runtime_version: String,
    pub web_server_engine: String,
    pub php_version: String,
    pub domain: String,
    pub aliases: Vec<String>,
    pub internal_port: u16,
    pub status: String,
    pub pid: Option<u32>,
    pub workers: u32,
    pub memory_limit_mb: u32,
    pub memory_used_mb: u32,
    pub cpu_usage_percent: f64,
    pub ssl_enabled: bool,
    pub ssl_issuer: String,
    pub ssl_expiry_days: u32,
    pub ssl_cert_pem: String,
    pub ssl_private_key_pem: String,
    pub force_https: bool,
    pub http3_quic: bool,
    pub websocket_enabled: bool,
    pub rate_limit_rps: u32,
    pub working_dir: String,
    pub run_sub_dir: String,
    pub open_basedir_enabled: bool,
    pub default_indexes: String,
    pub exec_command: String,
    pub build_script: String,
    pub url_rewrite_preset: String,
    pub url_rewrite_rules: String,
    pub redirects: Vec<SiteRedirectRule>,
    pub hotlink_protection: HotlinkProtectionConfig,
    pub directory_auth: Vec<DirectoryBasicAuthRule>,
    pub git_deploy: GitDeployConfig,
    pub access_log_path: String,
    pub error_log_path: String,
    pub recent_access_logs: Vec<String>,
    pub recent_error_logs: Vec<String>,
    pub env_vars: Vec<EnvVar>,
    pub systemd_unit_content: String,
    pub nginx_vhost_content: String,
    pub last_deployed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpExtension {
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpVersionInstance {
    pub version: String,
    pub status: String,
    pub fpm_socket: String,
    pub binary_path: String,
    pub ini_path: String,
    pub active_sites_count: u32,
    pub memory_usage_mb: u32,
    pub pm_mode: String,
    pub pm_max_children: u32,
    pub pm_start_servers: u32,
    pub pm_min_spare_servers: u32,
    pub pm_max_spare_servers: u32,
    pub memory_limit: String,
    pub upload_max_filesize: String,
    pub post_max_size: String,
    pub max_execution_time: u32,
    pub max_input_vars: u32,
    pub display_errors: bool,
    pub opcache_enabled: bool,
    pub opcache_memory_mb: u32,
    pub opcache_jit_buffer_mb: u32,
    pub disabled_functions: String,
    pub extensions: Vec<PhpExtension>,
    pub raw_ini_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlDatabase {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub username: String,
    pub password_masked: String,
    pub password_plain: String,
    pub host: String,
    pub port: u16,
    pub size_mb: f64,
    pub tables_count: u32,
    pub active_connections: u32,
    pub max_connections: u32,
    pub collation: String,
    pub linked_app_id: String,
    pub status: String,
    pub last_backup: String,
    pub slow_queries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UfwFirewallRule {
    pub id: String,
    pub port: String,
    pub protocol: String,
    pub action: String,
    pub source: String,
    pub comment: String,
    pub hits: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fail2banBannedIp {
    pub ip: String,
    pub country: String,
    pub banned_at: String,
    pub ban_duration: String,
    pub failed_attempts: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fail2banJail {
    pub name: String,
    pub service: String,
    pub enabled: bool,
    pub max_retry: u32,
    pub find_time: String,
    pub ban_time: String,
    pub currently_banned: u32,
    pub total_banned: u32,
    pub banned_ips: Vec<Fail2banBannedIp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardeningDirective {
    pub id: String,
    pub title: String,
    pub category: String,
    pub config_key: String,
    pub current_value: String,
    pub recommended_value: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WafProtectionRule {
    pub id: String,
    pub code: String,
    pub category: String,
    pub description: String,
    pub action: String,
    pub blocked_24h: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntiDdosShieldState {
    pub protection_mode: String,
    pub ebpf_xdp_enabled: bool,
    pub syn_cookies_enabled: bool,
    pub syn_rate_limit_pps: u32,
    pub conn_limit_per_ip: u32,
    pub waf_enabled: bool,
    pub waf_paranoia_level: u8,
    pub blocked_packets_24h: u64,
    pub mitigated_attacks_count: u32,
    pub current_pps: u32,
    pub fim_last_scan: String,
    pub fim_intact_files: u32,
    pub fim_tampered_files: u32,
    pub waf_rules: Vec<WafProtectionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityData {
    pub firewall_active: bool,
    pub default_incoming_policy: String,
    pub default_outgoing_policy: String,
    pub ufw_rules: Vec<UfwFirewallRule>,
    pub fail2ban_jails: Vec<Fail2banJail>,
    pub hardening_directives: Vec<HardeningDirective>,
    pub anti_ddos: AntiDdosShieldState,
    pub blocked_attacks_24h: u64,
    pub ssh_port: u16,
    pub root_login_disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelAdminAccount {
    pub username: String,
    pub password_plain: String,
    pub password_hash_argon2: String,
    pub panel_port: u16,
    pub security_entrance_path: String,
    pub session_timeout_minutes: u32,
    pub bound_domain: String,
    pub ip_whitelist: String,
    pub basic_auth_enabled: bool,
    pub two_factor_enabled: bool,
    pub ssl_panel_enabled: bool,
    pub last_login_ip: String,
    pub last_login_at: String,
    pub failed_login_attempts: u32,
    pub panel_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminSession {
    pub token: String,
    pub username: String,
    pub created_at: u64,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTranslationVerificationReport {
    pub translated_from: String,
    pub target_rust_module: String,
    pub target_models_module: String,
    pub actix_web_version: String,
    pub total_routes_translated: usize,
    pub route_groups_verified: Vec<String>,
    pub state_consistency_passed: bool,
    pub basic_auth_middleware_active: bool,
    pub clean_initial_state_verified: bool,
    pub abi_signature: String,
    pub verified_at: String,
    pub route_mapping_table: HashMap<String, String>,
}
