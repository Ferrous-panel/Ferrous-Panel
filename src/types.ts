export interface EnvVar {
  key: string;
  value: string;
}

export type WebsiteRuntimeType =
  | 'Actix-Web (Rust)'
  | 'Axum (Rust)'
  | 'Rocket (Rust)'
  | 'Next.js 15 (React SSR)'
  | 'Nuxt 3 (Vue 3 SSR)'
  | 'SvelteKit 2 (Svelte 5)'
  | 'Astro 5 (Hybrid / Static)'
  | 'Remix / React Router v7'
  | 'NestJS 10 (TypeScript API)'
  | 'Vite + React 19 / Vue 3 SPA'
  | 'Node.js / Next.js'
  | 'PHP 8.3-FPM'
  | 'Python / FastAPI'
  | 'Go / Fiber'
  | 'Static HTML / SPA';

export type RewritePresetType =
  | 'rust_proxy'
  | 'nextjs_proxy'
  | 'nuxt_nitro'
  | 'sveltekit_node'
  | 'astro_hybrid'
  | 'laravel'
  | 'wordpress'
  | 'spa_history';

export interface ManagedApp {
  id: string;
  name: string;
  framework: WebsiteRuntimeType;
  runtimeVersion: string;
  domain: string;
  aliases: string[];
  internalPort: number;
  workingDir: string;
  runSubDir: string;
  execCommand: string;
  workers: number;
  memoryLimitMb: number;
  memoryUsedMb: number;
  cpuPercent: number;
  pid: number | null;
  status: 'running' | 'stopped' | 'rebuilding';
  sslEnabled: boolean;
  sslIssuer: string;
  sslExpiryDays: number;
  forceHttps: boolean;
  http3Quic: boolean;
  websocketEnabled: boolean;
  rateLimitRps: number;
  requestsPerSec: number;
  p99LatencyMs: number;
  uptimeSeconds: number;
  diskUsedMb: number;
  diskQuotaMb: number;
  trafficMonthGb: number;
  linkedDbName: string | null;
  ftpUsername: string | null;
  backupCount: number;
  gitRepoUrl: string;
  gitBranch: string;
  buildScript: string;
  lastDeployCommit: string;
  lastDeployTime: string;
  urlRewritePreset: RewritePresetType;
  urlRewriteRules: string;
  phpVersion?: '8.4' | '8.3' | '8.2' | '8.1' | '7.4' | 'None';
  redirects?: RedirectRuleItem[];
  hotlinkProtection?: HotlinkProtectionConfig;
  directoryAuth?: DirectoryPrivacyItem[];
  envVars: EnvVar[];
  logs: string[];
  systemdUnit: string;
  nginxVhost: string;
}

export interface RedirectRuleItem {
  id: string;
  sourcePath: string;
  targetUrl: string;
  type: '301' | '302';
  keepQuery: boolean;
  enabled: boolean;
}

export interface HotlinkProtectionConfig {
  enabled: boolean;
  protectedExtensions: string;
  allowedDomains: string[];
  blockEmptyReferer: boolean;
}

export interface DirectoryPrivacyItem {
  id: string;
  path: string;
  label: string;
  username: string;
  enabled: boolean;
}

export interface PhpExtensionItem {
  name: string;
  category: 'Cache & Speed' | 'Database' | 'Image & Media' | 'Core & Security';
  enabled: boolean;
  description: string;
}

export interface PhpVersionInstance {
  version: '8.4' | '8.3' | '8.2' | '8.1' | '7.4';
  fullVersion: string;
  status: 'running' | 'stopped' | 'not_installed';
  installed: boolean;
  fpmSocket: string;
  iniPath: string;
  poolPath: string;
  memoryUsageMb: number;
  activeWorkers: number;
  maxChildren: number;
  memoryLimit: string;
  uploadMaxFilesize: string;
  postMaxSize: string;
  maxExecutionTime: number;
  maxInputVars: number;
  displayErrors: boolean;
  opcacheEnabled: boolean;
  opcacheJitBuffer: string;
  disableFunctions: string[];
  extensions: PhpExtensionItem[];
  rawPhpIni: string;
  rawFpmPoolConf: string;
}

export interface OneClickTemplate {
  id: string;
  name: string;
  category:
    | 'Rust Backend'
    | 'Modern JS Frameworks'
    | 'PHP CMS / Framework'
    | 'Python / Go';
  framework: WebsiteRuntimeType;
  version: string;
  description: string;
  defaultPort: number;
  requiresDb: 'PostgreSQL 17' | 'MariaDB 11.4' | 'None';
  buildCommand: string;
  execCommandTemplate: string;
  popularScore: string;
}

export interface ServerPackageItem {
  id: string;
  name: string;
  category:
    | 'Rust Supplemental Package'
    | 'Web Server'
    | 'Runtime'
    | 'Database'
    | 'Cache & Queue'
    | 'Security & Process';
  version: string;
  description: string;
  installed: boolean;
  status: 'running' | 'stopped' | 'not_installed';
  memoryMb: number;
  configPath: string;
  isRustNative: boolean;
  replacedLegacyPackage?: string;
  rustCrate?: string;
  port?: number;
  configContent: string;
  logs: string[];
}

export interface WebsiteLiveTestResult {
  appId: string;
  domain: string;
  url: string;
  statusCode: number;
  statusText: string;
  latencyMs: number;
  protocol: string;
  tlsCipher: string;
  headers: Record<string, string>;
  bodyPreview: string;
  verifiedAt: string;
}

export interface CoreLoad {
  core: number;
  model: string;
  usagePercent: number;
}

export interface TelemetryData {
  hostname: string;
  publicIp: string;
  osVersion: string;
  kernelVersion: string;
  rustDaemonVersion: string;
  nginxVersion: string;
  uptimeSeconds: number;
  cpuBrand: string;
  cpuCores: number;
  cpuUsagePercent: number;
  coreLoads: CoreLoad[];
  loadAverage: number[];
  memoryTotalMb: number;
  memoryUsedMb: number;
  memoryPercent: number;
  swapTotalMb: number;
  swapUsedMb: number;
  diskTotalGb: number;
  diskUsedGb: number;
  networkRxMbps: number;
  networkTxMbps: number;
  totalRequestsPerSec: number;
  activeConnections: number;
  timestamp: string;
}

export interface UfwRule {
  id: string;
  port: string;
  protocol: 'TCP' | 'UDP' | 'BOTH';
  action: 'ALLOW' | 'DENY' | 'LIMIT';
  source: string;
  comment: string;
  enabled: boolean;
}

export interface Fail2banJail {
  name: string;
  filter: string;
  logPath: string;
  maxRetry: number;
  findTimeSec: number;
  banTimeSec: number;
  currentlyBanned: number;
  totalBanned: number;
  bannedIps: {
    ip: string;
    country: string;
    reason: string;
    bannedAt: string;
  }[];
}

export interface HardeningDirective {
  id: string;
  category:
    | 'SSH Daemon'
    | 'Linux Kernel (sysctl)'
    | 'Actix-Web Auth'
    | 'Systemd Sandbox';
  key: string;
  recommendedValue: string;
  currentValue: string;
  enabled: boolean;
  description: string;
}

export interface SecurityData {
  ufwRules: UfwRule[];
  fail2banJails: Fail2banJail[];
  hardeningDirectives: HardeningDirective[];
}

export interface SqlTableColumn {
  name: string;
  dataType: string;
  nullable: boolean;
  isPrimaryKey?: boolean;
}

export interface SqlTableSchema {
  tableName: string;
  rowCount: number;
  sizeKb: number;
  columns: SqlTableColumn[];
  rows: Record<string, string | number | boolean | null>[];
}

export interface SqlDatabase {
  id: string;
  name: string;
  engine: 'PostgreSQL 17' | 'MariaDB 11.4' | 'SQLite 3 (WAL)';
  host: string;
  port: number;
  username: string;
  password?: string;
  collation: string;
  linkedAppId: string;
  linkedAppName: string;
  sizeMb: number;
  activeConnections: number;
  maxConnections: number;
  status: 'online' | 'maintenance';
  createdAt: string;
  tables: SqlTableSchema[];
}

export interface MysqlPhpMyAdminConfig {
  mysqlVersion: string;
  socketPath: string;
  bindAddress: string;
  port: number;
  rootPassword?: string;
  innodbBufferPoolMb: number;
  maxConnections: number;
  slowQueryLog: boolean;
  longQueryTimeSec: number;
  defaultCharset: string;
  myCnfRaw: string;
  pmaVersion: string;
  pmaPath: string;
  pmaPort: number;
  pmaSslOnly: boolean;
  pmaBasicAuth: boolean;
  pmaMaxUploadMb: number;
  pmaAllowedIps: string;
  pmaStatus: 'running' | 'stopped';
}

export interface SqlQueryResponse {
  success: boolean;
  executionTimeMs: number;
  commandTag: string;
  affectedRows: number;
  columns: string[];
  rows: Record<string, string | number | boolean | null>[];
  error?: string;
}

export interface WebFileNode {
  id: string;
  appId: string;
  name: string;
  path: string;
  parentDir: string;
  type: 'file' | 'directory';
  sizeBytes: number;
  permissions: string;
  symbolicPerms: string;
  owner: string;
  modifiedAt: string;
  content?: string;
}

export interface CronJobItem {
  id: string;
  title: string;
  schedule: string;
  command: string;
  user: string;
  targetApp: string;
  enabled: boolean;
  lastRunAt: string;
  lastExitCode: number;
  lastOutput: string;
}

export interface DnsRecordItem {
  id: string;
  domain: string;
  type: 'A' | 'AAAA' | 'CNAME' | 'MX' | 'TXT' | 'CAA';
  host: string;
  value: string;
  ttl: number;
  priority?: number;
}

export interface BackupArchiveItem {
  id: string;
  filename: string;
  targetApp: string;
  databaseIncluded: string;
  sizeMb: number;
  compression: 'zstd (level 9)' | 'gzip';
  sha256: string;
  createdAt: string;
  status: 'verified' | 'restoring';
}

export interface SftpAccountItem {
  id: string;
  username: string;
  chrootDir: string;
  targetApp: string;
  authMethod: 'Ed25519 SSH Key' | 'Argon2id Password';
  quotaMb: number;
  usedMb: number;
  status: 'active' | 'locked';
}

export interface CpanelData {
  cronJobs: CronJobItem[];
  dnsRecords: DnsRecordItem[];
  backups: BackupArchiveItem[];
  sftpAccounts: SftpAccountItem[];
}

export interface MailDomainItem {
  id: string;
  domain: string;
  spfStatus: 'verified' | 'missing';
  dkimStatus: 'verified' | 'missing';
  dmarcStatus: 'verified' | 'missing';
  dkimSelector: string;
  dkimPublicKey: string;
  catchAllAddress: string;
  mailboxesCount: number;
}

export interface MailboxAccount {
  id: string;
  email: string;
  domain: string;
  displayName: string;
  quotaMb: number;
  usedMb: number;
  imapServer: string;
  smtpServer: string;
  status: 'active' | 'suspended';
  lastLogin: string;
}

export interface MailForwarder {
  id: string;
  sourceEmail: string;
  destinationEmail: string;
  domain: string;
  active: boolean;
}

export interface MailMessageItem {
  id: string;
  folder: 'inbox' | 'sent' | 'spam';
  from: string;
  to: string;
  subject: string;
  snippet: string;
  body: string;
  timestamp: string;
  dkimSigned: boolean;
}

export interface MailServerData {
  postfixVersion: string;
  dovecotVersion: string;
  rspamdVersion: string;
  queueCount: number;
  sentToday: number;
  spamBlockedToday: number;
  domains: MailDomainItem[];
  mailboxes: MailboxAccount[];
  forwarders: MailForwarder[];
  messages: MailMessageItem[];
}

export interface DiagnosticCheckItem {
  id: string;
  subsystem: string;
  target: string;
  status: 'passed' | 'warning';
  latencyMs: number;
  detail: string;
}

export interface OptimizationProfilePreset {
  id: 'balanced_cloud' | 'high_throughput_edge' | 'database_heavy' | 'low_memory_vps';
  name: string;
  badge: string;
  description: string;
  actixWorkers: number;
  actixMaxConnections: number;
  actixKeepAliveSecs: number;
  nginxWorkerConnections: number;
  nginxKeepaliveRequests: number;
  mariadbBufferPoolMb: number;
  postgresSharedBuffersMb: number;
  redisMaxMemoryMb: number;
  expectedP99ReductionPercent: number;
}

export interface KernelSysctlParam {
  id: string;
  key: string;
  category: 'TCP BBR & Network' | 'Memory & Swap' | 'File Descriptors & Epoll';
  currentValue: string;
  recommendedValue: string;
  description: string;
  enabled: boolean;
}

export interface BackendWatchdogState {
  daemonPid: number;
  oomScoreAdj: number;
  allocatorEngine: string;
  allocatedRamMb: number;
  residentRamMb: number;
  workerThreadsActive: number;
  workerThreadsTotal: number;
  dbPoolActive: number;
  dbPoolIdle: number;
  dbPoolMax: number;
  circuitBreakerStatus: 'CLOSED (Optimal)' | 'HALF-OPEN (Recovering)';
  autoHealEnabled: boolean;
  lastSelfHealAt: string;
  uptimeStabilityPercent: number;
  recentEvents: {
    id: string;
    timestamp: string;
    subsystem: string;
    action: string;
    result: string;
  }[];
}

export interface SystemOptimizerData {
  activeProfileId: OptimizationProfilePreset['id'];
  profiles: OptimizationProfilePreset[];
  sysctlParams: KernelSysctlParam[];
  sysctlConfigPreview: string;
  watchdog: BackendWatchdogState;
}

export interface SshDaemonConfig {
  port: number;
  listenAddress: string;
  permitRootLogin: 'prohibit-password' | 'yes' | 'no';
  passwordAuthentication: boolean;
  pubkeyAuthentication: boolean;
  maxAuthTries: number;
  clientAliveIntervalSec: number;
  clientAliveCountMax: number;
  allowTcpForwarding: boolean;
  sftpChrootEnabled: boolean;
  allowUsers: string[];
  kexAlgorithms: string;
  sshdConfigPreview: string;
}

export interface SshAuthorizedKey {
  id: string;
  label: string;
  algorithm: 'ssh-ed25519' | 'rsa-sha2-512' | 'ecdsa-sha2-nistp256';
  fingerprint: string;
  publicKey: string;
  addedAt: string;
  clientHint: string;
}

export interface SshGeneratedKeypair {
  keyId: string;
  label: string;
  algorithm: string;
  fingerprint: string;
  publicKeyOpenssh: string;
  privateKeyPem: string;
  vscodeConfigSnippet: string;
  cliCommand: string;
}

export interface SshActiveSession {
  id: string;
  pts: string;
  user: string;
  clientIp: string;
  clientSoftware: string;
  authMethod: string;
  connectedAt: string;
  idleTime: string;
  pid: number;
}

export interface WebSshCommandOutput {
  id: string;
  cwd: string;
  command: string;
  output: string;
  exitCode: number;
  timestamp: string;
}

export interface RustModuleConversionItem {
  id: string;
  moduleName: string;
  rustFile: string;
  replacedLegacyStack: string;
  crateDependencies: string;
  memoryFootprintKb: number;
  latencyMicros: number;
  status: 'NATIVE_RUST_COMPILED';
  description: string;
}

export interface PanelAdminAccount {
  adminUsername: string;
  adminPassword: string;
  passwordHashAlgo: string;
  panelPort: number;
  securityEntrance: string;
  customDomain?: string;
  sslEnabled?: boolean;
  mysqlRootPassword: string;
  postgresRootPassword: string;
  twoFactorEnabled: boolean;
  allowedAdminIps: string;
  configFile: string;
  installedAt: string;
  lastModifiedAt: string;
}

export interface SshPanelData {
  serverHost: string;
  config: SshDaemonConfig;
  authorizedKeys: SshAuthorizedKey[];
  activeSessions: SshActiveSession[];
  rustModules: RustModuleConversionItem[];
  panelAdmin?: PanelAdminAccount;
}

export interface RustSourceFile {
  path: string;
  title: string;
  lang: string;
  description: string;
  content: string;
}

export type ActiveTab =
  | 'overview'
  | 'websites'
  | 'php'
  | 'oneclick'
  | 'ssh'
  | 'files'
  | 'databases'
  | 'mail'
  | 'optimizer'
  | 'cpanel'
  | 'security'
  | 'rust-source';


