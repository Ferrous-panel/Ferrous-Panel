use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ============================================================================
// Common Primitives & Auth Contracts
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remember_me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUserDto {
    pub username: String,
    pub panel_port: u16,
    pub security_entrance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub success: bool,
    pub token: String,
    pub user: AuthUserDto,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResponse {
    pub authenticated: bool,
    pub user: AuthUserDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// Websites & Runtime Enums (`src/types.ts`)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebsiteRuntimeType {
    #[serde(rename = "Actix-Web (Rust)")]
    ActixWebRust,
    #[serde(rename = "Axum (Rust)")]
    AxumRust,
    #[serde(rename = "Rocket (Rust)")]
    RocketRust,
    #[serde(rename = "Next.js 15 (React SSR)")]
    NextJs15,
    #[serde(rename = "Nuxt 3 (Vue 3 SSR)")]
    Nuxt3,
    #[serde(rename = "SvelteKit 2 (Svelte 5)")]
    SvelteKit2,
    #[serde(rename = "Astro 5 (Hybrid / Static)")]
    Astro5,
    #[serde(rename = "Remix / React Router v7")]
    RemixReactRouterV7,
    #[serde(rename = "NestJS 10 (TypeScript API)")]
    NestJs10,
    #[serde(rename = "Bun / ElysiaJS (TypeScript)")]
    BunElysiaJs,
    #[serde(rename = "Deno 2 (TypeScript / Fresh)")]
    Deno2,
    #[serde(rename = "Vite + React 19 / Vue 3 SPA")]
    ViteSpa,
    #[serde(rename = "Node.js / Next.js")]
    NodeJsNextJs,
    #[serde(rename = "PHP 8.3-FPM")]
    Php83Fpm,
    #[serde(rename = "PHP / Symfony 7")]
    PhpSymfony7,
    #[serde(rename = "Python / FastAPI")]
    PythonFastApi,
    #[serde(rename = "Python / Django 5")]
    PythonDjango5,
    #[serde(rename = "Python / Flask 3")]
    PythonFlask3,
    #[serde(rename = "Go / Fiber")]
    GoFiber,
    #[serde(rename = "Go / Gin")]
    GoGin,
    #[serde(rename = "Java 21 / Spring Boot 3")]
    JavaSpringBoot3,
    #[serde(rename = "C# / .NET 8 ASP.NET Core")]
    DotNet8AspNetCore,
    #[serde(rename = "Ruby 3.3 / Rails & Elixir")]
    RubyRailsElixir,
    #[serde(rename = "C++20 / Zig Native Daemon")]
    CppZigDaemon,
    #[serde(rename = "WebAssembly (WASI Wasmtime)")]
    WasiWasmtime,
    #[serde(rename = "Static HTML / SPA")]
    StaticHtmlSpa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WebServerEngineType {
    #[default]
    #[serde(rename = "FerrousX (Rust Web Server)")]
    FerrousX,
    #[serde(rename = "Nginx 1.26")]
    Nginx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewritePresetType {
    RustProxy,
    ApiProxy,
    NextjsProxy,
    NuxtNitro,
    SveltekitNode,
    AstroHybrid,
    Laravel,
    Wordpress,
    SpaHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    Running,
    Stopped,
    Rebuilding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhpVersionTag {
    #[serde(rename = "8.4")]
    V84,
    #[serde(rename = "8.3")]
    V83,
    #[serde(rename = "8.2")]
    V82,
    #[serde(rename = "8.1")]
    V81,
    #[serde(rename = "7.4")]
    V74,
    #[serde(rename = "None")]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectRuleItem {
    pub id: String,
    pub source_path: String,
    pub target_url: String,
    #[serde(rename = "type")]
    pub redirect_type: String,
    pub keep_query: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotlinkProtectionConfig {
    pub enabled: bool,
    pub protected_extensions: String,
    pub allowed_domains: Vec<String>,
    pub block_empty_referer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryPrivacyItem {
    pub id: String,
    pub path: String,
    pub label: String,
    pub username: String,
    pub enabled: bool,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedApp {
    pub id: String,
    pub name: String,
    pub framework: WebsiteRuntimeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_server_engine: Option<WebServerEngineType>,
    pub runtime_version: String,
    pub domain: String,
    pub aliases: Vec<String>,
    pub internal_port: u16,
    pub working_dir: String,
    pub run_sub_dir: String,
    pub exec_command: String,
    pub workers: u32,
    pub memory_limit_mb: u64,
    pub memory_used_mb: u64,
    pub cpu_percent: f64,
    pub pid: Option<u32>,
    pub status: AppStatus,
    pub ssl_enabled: bool,
    pub ssl_issuer: String,
    pub ssl_expiry_days: i64,
    pub force_https: bool,
    pub http3_quic: bool,
    pub websocket_enabled: bool,
    pub rate_limit_rps: u32,
    pub requests_per_sec: u64,
    pub p99_latency_ms: f64,
    pub uptime_seconds: u64,
    pub disk_used_mb: f64,
    pub disk_quota_mb: u64,
    pub traffic_month_gb: f64,
    pub linked_db_name: Option<String>,
    pub ftp_username: Option<String>,
    pub backup_count: u32,
    pub git_repo_url: String,
    pub git_branch: String,
    pub build_script: String,
    pub last_deploy_commit: String,
    pub last_deploy_time: String,
    pub url_rewrite_preset: RewritePresetType,
    pub url_rewrite_rules: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub php_version: Option<PhpVersionTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirects: Option<Vec<RedirectRuleItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotlink_protection: Option<HotlinkProtectionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory_auth: Option<Vec<DirectoryPrivacyItem>>,
    pub env_vars: Vec<EnvVar>,
    pub logs: Vec<String>,
    pub systemd_unit: String,
    pub nginx_vhost: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_summary: Option<OneClickSetupSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickSetupSummary {
    pub site_url: String,
    pub admin_url: String,
    pub admin_user: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_password: Option<String>,
    pub admin_email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub db_user: Option<String>,
    pub auto_installed: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteLiveTestResult {
    pub app_id: String,
    pub domain: String,
    pub url: String,
    pub status_code: u16,
    pub status_text: String,
    pub latency_ms: u64,
    pub protocol: String,
    pub tls_cipher: String,
    pub headers: BTreeMap<String, String>,
    pub body_preview: String,
    pub verified_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployWebsiteRequest {
    pub name: String,
    pub framework: WebsiteRuntimeType,
    pub domain: String,
    pub aliases: Vec<String>,
    pub internal_port: u16,
    pub working_dir: String,
    pub run_sub_dir: String,
    pub exec_command: String,
    pub workers: u32,
    pub memory_limit_mb: u64,
    pub ssl_enabled: bool,
    pub http3_quic: bool,
    pub websocket_enabled: bool,
    pub rate_limit_rps: u32,
    pub create_database: bool,
    pub db_engine: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub create_ftp: bool,
    pub ftp_user: String,
    pub git_repo_url: String,
    pub git_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_server_engine: Option<WebServerEngineType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppActionRequest {
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigUpdateRequest {
    pub workers: u32,
    pub memory_limit_mb: u64,
    pub rate_limit_rps: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http3_quic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub websocket_enabled: Option<bool>,
    pub env_vars: Vec<EnvVar>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NginxValidateRequest {
    pub domain: String,
    pub config_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_server_engine: Option<WebServerEngineType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NginxValidateResponse {
    pub valid: bool,
    pub output: String,
}

// ============================================================================
// Multi-PHP, 1-Click Templates & Server Packages
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpExtensionItem {
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpVersionInstance {
    pub version: String,
    pub full_version: String,
    pub status: String,
    pub installed: bool,
    pub fpm_socket: String,
    pub ini_path: String,
    pub pool_path: String,
    pub memory_usage_mb: f64,
    pub active_workers: u32,
    pub max_children: u32,
    pub memory_limit: String,
    pub upload_max_filesize: String,
    pub post_max_size: String,
    pub max_execution_time: u32,
    pub max_input_vars: u32,
    pub display_errors: bool,
    pub opcache_enabled: bool,
    pub opcache_jit_buffer: String,
    pub disable_functions: Vec<String>,
    pub extensions: Vec<PhpExtensionItem>,
    pub raw_php_ini: String,
    pub raw_fpm_pool_conf: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickTemplate {
    pub id: String,
    pub name: String,
    pub category: String,
    pub framework: WebsiteRuntimeType,
    pub version: String,
    pub description: String,
    pub default_port: u16,
    pub requires_db: String,
    pub build_command: String,
    pub exec_command_template: String,
    pub popular_score: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_repo_url: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneClickDeployRequest {
    pub template_id: String,
    pub domain: String,
    #[serde(default, alias = "autoCreateDb")]
    pub create_db: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_server_engine: Option<WebServerEngineType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_password: Option<String>,
    #[serde(default)]
    pub enable_ssl: bool,
    #[serde(default = "default_true")]
    pub auto_install_deps: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackageItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
    pub status: String,
    pub memory_mb: f64,
    pub config_path: String,
    pub is_rust_native: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_legacy_package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_crate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub config_content: String,
    pub logs: Vec<String>,
}

// ============================================================================
// Telemetry & FerrousX Sync (`OverviewView.tsx` & `types.ts`)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreLoad {
    pub core: usize,
    pub model: String,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryHistoryPoint {
    pub time: String,
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub memory_used_mb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests_per_sec: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryData {
    pub hostname: String,
    pub public_ip: String,
    pub os_version: String,
    pub kernel_version: String,
    pub rust_daemon_version: String,
    pub nginx_version: String,
    pub uptime_seconds: u64,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_usage_percent: f64,
    pub core_loads: Vec<CoreLoad>,
    pub load_average: [f64; 3],
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub memory_percent: f64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub disk_total_gb: f64,
    pub disk_used_gb: f64,
    pub network_rx_mbps: f64,
    pub network_tx_mbps: f64,
    pub total_requests_per_sec: u64,
    pub active_connections: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_24h: Option<Vec<TelemetryHistoryPoint>>,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCheckItem {
    pub id: String,
    pub subsystem: String,
    pub target: String,
    pub status: String,
    pub latency_ms: u64,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FerrousXSyncCrate {
    pub name: String,
    pub version: String,
    pub modules: Vec<String>,
    pub features: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FerrousXLibDep {
    pub name: String,
    pub panel_version: String,
    pub ferrousx_version: String,
    pub compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FerrousXLibraryCompatibility {
    pub overall_compatible: bool,
    pub conflicts_count: u32,
    pub checked_at: String,
    pub rust_dependencies: Vec<FerrousXLibDep>,
    pub frontend_dependencies: Vec<FerrousXLibDep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FerrousXSyncState {
    pub upstream_repo: String,
    pub commit_sha: String,
    pub commit_date: String,
    pub commit_title: String,
    pub rust_edition: String,
    pub rust_version: String,
    pub daemon_binary: String,
    pub cli_binary: String,
    pub master_config_toml: String,
    pub uds_control_socket: String,
    pub embedded_webui_addr: String,
    pub config_epoch: u64,
    pub rcu_reload_latency_us: u64,
    pub memory_rss_mb: f64,
    pub last_synced_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub workspace_crates: Vec<FerrousXSyncCrate>,
    pub library_compatibility: FerrousXLibraryCompatibility,
}

// ============================================================================
// Security (`UFW`, `Fail2ban`, `Hardening`, `Anti-DDoS WAF`)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UfwRule {
    pub id: String,
    pub port: String,
    pub protocol: String,
    pub action: String,
    pub source: String,
    pub comment: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BannedIpItem {
    pub ip: String,
    pub country: String,
    pub reason: String,
    pub banned_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fail2banJail {
    pub name: String,
    pub filter: String,
    pub log_path: String,
    pub max_retry: u32,
    pub find_time_sec: u64,
    pub ban_time_sec: u64,
    pub currently_banned: u32,
    pub total_banned: u64,
    pub banned_ips: Vec<BannedIpItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardeningDirective {
    pub id: String,
    pub category: String,
    pub key: String,
    pub recommended_value: String,
    pub current_value: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WafRuleItem {
    pub id: String,
    pub code: String,
    pub name: String,
    pub category: String,
    pub threat_type: String,
    pub blocked_count: u64,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntiDdosShieldState {
    pub protection_mode: String,
    pub xdp_status: String,
    pub syn_cookies_enabled: bool,
    pub syn_rate_limit_pps: u64,
    pub dropped_packets_total: u64,
    pub blocked_l7_attacks_today: u64,
    pub fim_integrity_status: String,
    pub last_fim_scan_at: String,
    pub waf_rules: Vec<WafRuleItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityData {
    pub ufw_rules: Vec<UfwRule>,
    pub fail2ban_jails: Vec<Fail2banJail>,
    pub hardening_directives: Vec<HardeningDirective>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anti_ddos_shield: Option<AntiDdosShieldState>,
}

// ============================================================================
// SQL Databases & phpMyAdmin
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTableColumn {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_primary_key: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlTableSchema {
    pub table_name: String,
    pub row_count: u64,
    pub size_kb: f64,
    pub columns: Vec<SqlTableColumn>,
    pub rows: Vec<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlDatabase {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub collation: String,
    pub linked_app_id: String,
    pub linked_app_name: String,
    pub size_mb: f64,
    pub active_connections: u32,
    pub max_connections: u32,
    pub status: String,
    pub created_at: String,
    pub tables: Vec<SqlTableSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MysqlPhpMyAdminConfig {
    pub mysql_version: String,
    pub socket_path: String,
    pub bind_address: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_password: Option<String>,
    pub innodb_buffer_pool_mb: u64,
    pub max_connections: u32,
    pub slow_query_log: bool,
    pub long_query_time_sec: u32,
    pub default_charset: String,
    pub my_cnf_raw: String,
    pub pma_version: String,
    pub pma_path: String,
    pub pma_port: u16,
    pub pma_ssl_only: bool,
    pub pma_basic_auth: bool,
    pub pma_max_upload_mb: u32,
    pub pma_allowed_ips: String,
    pub pma_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlQueryResponse {
    pub success: bool,
    pub execution_time_ms: f64,
    pub command_tag: String,
    pub affected_rows: u64,
    pub columns: Vec<String>,
    pub rows: Vec<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Files, cPanel Suite & Mail
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebFileNode {
    pub id: String,
    pub app_id: String,
    pub name: String,
    pub path: String,
    pub parent_dir: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub size_bytes: u64,
    pub permissions: String,
    pub symbolic_perms: String,
    pub owner: String,
    pub modified_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobItem {
    pub id: String,
    pub title: String,
    pub schedule: String,
    pub command: String,
    pub user: String,
    pub target_app: String,
    pub enabled: bool,
    pub last_run_at: String,
    pub last_exit_code: i32,
    pub last_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecordItem {
    pub id: String,
    pub domain: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub host: String,
    pub value: String,
    pub ttl: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupArchiveItem {
    pub id: String,
    pub filename: String,
    pub target_app: String,
    pub database_included: String,
    pub size_mb: f64,
    pub compression: String,
    pub sha256: String,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpAccountItem {
    pub id: String,
    pub username: String,
    pub chroot_dir: String,
    pub target_app: String,
    pub auth_method: String,
    pub quota_mb: u64,
    pub used_mb: f64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpanelData {
    pub cron_jobs: Vec<CronJobItem>,
    pub dns_records: Vec<DnsRecordItem>,
    pub backups: Vec<BackupArchiveItem>,
    pub sftp_accounts: Vec<SftpAccountItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailDomainItem {
    pub id: String,
    pub domain: String,
    pub spf_status: String,
    pub dkim_status: String,
    pub dmarc_status: String,
    pub dkim_selector: String,
    pub dkim_public_key: String,
    pub catch_all_address: String,
    pub mailboxes_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxAccount {
    pub id: String,
    pub email: String,
    pub domain: String,
    pub display_name: String,
    pub quota_mb: u64,
    pub used_mb: f64,
    pub imap_server: String,
    pub smtp_server: String,
    pub status: String,
    pub last_login: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailForwarder {
    pub id: String,
    pub source_email: String,
    pub destination_email: String,
    pub domain: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailMessageItem {
    pub id: String,
    pub folder: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub snippet: String,
    pub body: String,
    pub timestamp: String,
    pub dkim_signed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailServerData {
    pub postfix_version: String,
    pub dovecot_version: String,
    pub rspamd_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub postfix_status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dovecot_status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rspamd_status: String,
    pub queue_count: u32,
    pub sent_today: u64,
    pub spam_blocked_today: u64,
    pub domains: Vec<MailDomainItem>,
    pub mailboxes: Vec<MailboxAccount>,
    pub forwarders: Vec<MailForwarder>,
    pub messages: Vec<MailMessageItem>,
}

// ============================================================================
// System Optimizer & Anti-Crash Guard
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationProfilePreset {
    pub id: String,
    pub name: String,
    pub badge: String,
    pub description: String,
    pub actix_workers: u32,
    pub actix_max_connections: u32,
    pub actix_keep_alive_secs: u64,
    pub nginx_worker_connections: u32,
    pub nginx_keepalive_requests: u32,
    pub mariadb_buffer_pool_mb: u64,
    pub postgres_shared_buffers_mb: u64,
    pub redis_max_memory_mb: u64,
    pub expected_p99_reduction_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelSysctlParam {
    pub id: String,
    pub key: String,
    pub category: String,
    pub current_value: String,
    pub recommended_value: String,
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchdogEventItem {
    pub id: String,
    pub timestamp: String,
    pub subsystem: String,
    pub action: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendWatchdogState {
    pub daemon_pid: u32,
    pub oom_score_adj: i32,
    pub allocator_engine: String,
    pub allocated_ram_mb: f64,
    pub resident_ram_mb: f64,
    pub worker_threads_active: u32,
    pub worker_threads_total: u32,
    pub db_pool_active: u32,
    pub db_pool_idle: u32,
    pub db_pool_max: u32,
    pub circuit_breaker_status: String,
    pub auto_heal_enabled: bool,
    pub last_self_heal_at: String,
    pub uptime_stability_percent: f64,
    pub recent_events: Vec<WatchdogEventItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OomImmunityProcessItem {
    pub service: String,
    pub pid: u32,
    pub oom_score_adj: i32,
    pub memory_high_mb: u64,
    pub memory_max_mb: u64,
    pub policy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntiCrashGuardState {
    pub load_shedding_enabled: bool,
    pub cpu_shed_threshold_percent: f64,
    pub ram_shed_threshold_percent: f64,
    pub zram_enabled: bool,
    pub zram_compression_algo: String,
    pub zram_size_mb: u64,
    pub slow_query_auto_kill_sec: u64,
    pub cgroups_v2_enforced: bool,
    pub oom_immunity_map: Vec<OomImmunityProcessItem>,
    pub mitigation_count: u64,
    pub last_mitigation_action: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemOptimizerData {
    pub active_profile_id: String,
    pub profiles: Vec<OptimizationProfilePreset>,
    pub sysctl_params: Vec<KernelSysctlParam>,
    pub sysctl_config_preview: String,
    pub watchdog: BackendWatchdogState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anti_crash_guard: Option<AntiCrashGuardState>,
}

// ============================================================================
// SSH Terminal, Panel Admin, Rust Source & Polyglot Bridge
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDaemonConfig {
    pub port: u16,
    pub listen_address: String,
    pub permit_root_login: String,
    pub password_authentication: bool,
    pub pubkey_authentication: bool,
    pub max_auth_tries: u32,
    pub client_alive_interval_sec: u32,
    pub client_alive_count_max: u32,
    pub allow_tcp_forwarding: bool,
    pub sftp_chroot_enabled: bool,
    pub allow_users: Vec<String>,
    pub kex_algorithms: String,
    pub sshd_config_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthorizedKey {
    pub id: String,
    pub label: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub public_key: String,
    pub added_at: String,
    pub client_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshGeneratedKeypair {
    pub key_id: String,
    pub label: String,
    pub algorithm: String,
    pub fingerprint: String,
    pub public_key_openssh: String,
    pub private_key_pem: String,
    pub vscode_config_snippet: String,
    pub cli_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshActiveSession {
    pub id: String,
    pub pts: String,
    pub user: String,
    pub client_ip: String,
    pub client_software: String,
    pub auth_method: String,
    pub connected_at: String,
    pub idle_time: String,
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSshCommandOutput {
    pub id: String,
    pub cwd: String,
    pub command: String,
    pub output: String,
    pub exit_code: i32,
    pub timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustModuleConversionItem {
    pub id: String,
    pub module_name: String,
    pub rust_file: String,
    pub replaced_legacy_stack: String,
    pub crate_dependencies: String,
    pub memory_footprint_kb: u64,
    pub latency_micros: u64,
    pub status: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelAdminAccount {
    pub admin_username: String,
    pub admin_password: String,
    pub password_hash_algo: String,
    pub panel_port: u16,
    pub security_entrance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_enabled: Option<bool>,
    pub mysql_root_password: String,
    pub postgres_root_password: String,
    pub two_factor_enabled: bool,
    pub allowed_admin_ips: String,
    pub config_file: String,
    pub installed_at: String,
    pub last_modified_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshPanelData {
    pub server_host: String,
    pub config: SshDaemonConfig,
    pub authorized_keys: Vec<SshAuthorizedKey>,
    pub active_sessions: Vec<SshActiveSession>,
    pub rust_modules: Vec<RustModuleConversionItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel_admin: Option<PanelAdminAccount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RustSourceFile {
    pub path: String,
    pub title: String,
    pub lang: String,
    pub description: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolyglotToolchainSpec {
    pub id: String,
    pub language: String,
    pub runtime_version: String,
    pub bridge_protocol: String,
    pub compiler_command: String,
    pub output_artifact: String,
    pub zero_copy_ipc: bool,
    pub systemd_env_vars: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolyglotCompileRequest {
    pub target_language: String,
    pub module_name: String,
    pub optimization_level: String,
    pub enable_zero_copy_ipc: bool,
    pub expose_sys_metrics: bool,
    pub expose_sql_bridge: bool,
    pub custom_functions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolyglotCompileResult {
    pub success: bool,
    pub module_name: String,
    pub target_language: String,
    pub bridge_protocol: String,
    pub compiler_output: String,
    pub rust_core_code: String,
    pub c_header_code: String,
    pub target_binding_code: String,
    pub systemd_unit_snippet: String,
    pub build_script: String,
    pub binary_artifact_path: String,
    pub compiled_at: String,
}

// ============================================================================
// Edge Cloud (`WASI`, `OCI Containers`, `WireGuard`, `BLAKE3 Snapshots`)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasiEdgeWorker {
    pub id: String,
    pub name: String,
    pub route_pattern: String,
    pub target_domain: String,
    pub wasi_target: String,
    pub engine_version: String,
    pub cold_start_us: u64,
    pub memory_limit_kb: u64,
    pub memory_used_kb: u64,
    pub fuel_limit: u64,
    pub invocations_24h: u64,
    pub avg_execution_us: u64,
    pub blake3_hash: String,
    pub kv_namespace: String,
    pub status: String,
    pub rust_source_snippet: String,
    pub last_invoked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasiWorkerInvokeResult {
    pub worker_id: String,
    pub worker_name: String,
    pub route: String,
    pub status_code: u16,
    pub cold_start_us: u64,
    pub execution_time_us: u64,
    pub fuel_consumed: u64,
    pub memory_peak_kb: u64,
    pub blake3_verify: String,
    pub response_headers: BTreeMap<String, String>,
    pub response_body: String,
    pub invoked_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciContainerSandbox {
    pub id: String,
    pub name: String,
    pub image: String,
    pub runtime_engine: String,
    pub cgroups_v2_path: String,
    pub internal_ip: String,
    pub host_port: u16,
    pub container_port: u16,
    pub memory_max_mb: u64,
    pub memory_used_mb: f64,
    pub cpu_quota_percent: f64,
    pub cpu_usage_percent: f64,
    pub pids_current: u32,
    pub pids_max: u32,
    pub seccomp_profile: String,
    pub status: String,
    pub uptime: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireGuardPeerNode {
    pub id: String,
    pub name: String,
    pub region: String,
    pub interface_name: String,
    pub public_key: String,
    pub preshared_key_masked: String,
    pub endpoint: String,
    pub allowed_ips: String,
    pub mesh_ip: String,
    pub cipher_suite: String,
    pub latest_handshake: String,
    pub rx_mb: f64,
    pub tx_mb: f64,
    pub latency_ms: f64,
    pub persistent_keepalive_sec: u32,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blake3ObjectSnapshot {
    pub id: String,
    pub snapshot_name: String,
    pub target_resource: String,
    pub s3_endpoint: String,
    pub bucket: String,
    pub merkle_root_blake3: String,
    pub compression: String,
    pub dedup_ratio: String,
    pub logical_size_mb: f64,
    pub stored_size_mb: f64,
    pub chunks_count: u64,
    pub immutable_lock: bool,
    pub created_at: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeCloudData {
    pub wasmtime_version: String,
    pub youki_version: String,
    pub boringtun_version: String,
    pub blake3_engine_version: String,
    pub wireguard_interface: String,
    pub wireguard_listen_port: u16,
    pub server_public_key: String,
    pub mesh_subnet_cidr: String,
    pub wasi_workers: Vec<WasiEdgeWorker>,
    pub oci_containers: Vec<OciContainerSandbox>,
    pub wireguard_peers: Vec<WireGuardPeerNode>,
    pub blake3_snapshots: Vec<Blake3ObjectSnapshot>,
}
