import { useState, useEffect, useCallback } from 'react';
import {
  ActiveTab,
  AntiCrashGuardState,
  CpanelData,
  DiagnosticCheckItem,
  EnvVar,
  MailServerData,
  ManagedApp,
  MysqlPhpMyAdminConfig,
  OneClickTemplate,
  OptimizationProfilePreset,
  PanelAdminAccount,
  PhpVersionInstance,
  RustSourceFile,
  SecurityData,
  ServerPackageItem,
  SqlDatabase,
  SqlQueryResponse,
  SshDaemonConfig,
  SshGeneratedKeypair,
  SshPanelData,
  SystemOptimizerData,
  TelemetryData,
  WebServerEngineType,
  WebSshCommandOutput,
  WebsiteLiveTestResult,
  WebsiteRuntimeType
} from '../types';
import { actixGatewayFetch } from '../services/actixGateway';

export function usePanelController(
  activeTab: ActiveTab,
  setActiveTab: (tab: ActiveTab) => void
) {
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [autoOpenSettingsCounter, setAutoOpenSettingsCounter] =
    useState<number>(0);

  const [telemetry, setTelemetry] = useState<TelemetryData | null>(null);
  const [apps, setApps] = useState<ManagedApp[]>([]);
  const [phpVersions, setPhpVersions] = useState<PhpVersionInstance[]>([]);
  const [templates, setTemplates] = useState<OneClickTemplate[]>([]);
  const [packages, setPackages] = useState<ServerPackageItem[]>([]);
  const [sshData, setSshData] = useState<SshPanelData | null>(null);
  const [databases, setDatabases] = useState<SqlDatabase[]>([]);
  const [mysqlConfig, setMysqlConfig] = useState<MysqlPhpMyAdminConfig | null>(
    null
  );
  const [mailData, setMailData] = useState<MailServerData | null>(null);
  const [optimizer, setOptimizer] = useState<SystemOptimizerData | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticCheckItem[]>([]);
  const [cpanel, setCpanel] = useState<CpanelData | null>(null);
  const [security, setSecurity] = useState<SecurityData | null>(null);
  const [rustFiles, setRustFiles] = useState<RustSourceFile[]>([]);
  const [refreshing, setRefreshing] = useState<boolean>(false);

  const fetchTelemetry = useCallback(async () => {
    try {
      const res = await fetch('/api/telemetry');
      if (res.ok) {
        const data = await res.json();
        setTelemetry(data);
      }
    } catch {
      // ignore transient network errors
    }
  }, []);

  const fetchDiagnostics = useCallback(async () => {
    try {
      const res = await fetch('/api/system/diagnostics');
      if (res.ok) {
        setDiagnostics(await res.json());
      }
    } catch {
      // ignore
    }
  }, []);

  const fetchAllData = useCallback(async () => {
    setRefreshing(true);
    try {
      const [
        telRes,
        appsRes,
        phpRes,
        tplRes,
        pkgRes,
        sshRes,
        dbRes,
        mysqlRes,
        mailRes,
        optRes,
        diagRes,
        cpRes,
        secRes,
        rustRes
      ] = await Promise.all([
        fetch('/api/telemetry'),
        actixGatewayFetch('NGINX', '/api/apps'),
        actixGatewayFetch('SYSTEMD', '/api/php'),
        fetch('/api/oneclick/templates'),
        actixGatewayFetch('SYSTEMD', '/api/software'),
        actixGatewayFetch('SYSCALL', '/api/ssh'),
        fetch('/api/databases'),
        fetch('/api/databases/mysql-config'),
        fetch('/api/mail'),
        actixGatewayFetch('SYSCTL', '/api/optimizer'),
        fetch('/api/system/diagnostics'),
        fetch('/api/cpanel'),
        actixGatewayFetch('UFW', '/api/security'),
        fetch('/api/rust-source')
      ]);
      if (telRes.ok) setTelemetry(await telRes.json());
      if (appsRes.ok) setApps(await appsRes.json());
      if (phpRes.ok) setPhpVersions(await phpRes.json());
      if (tplRes.ok) setTemplates(await tplRes.json());
      if (pkgRes.ok) setPackages(await pkgRes.json());
      if (sshRes.ok) setSshData(await sshRes.json());
      if (dbRes.ok) setDatabases(await dbRes.json());
      if (mysqlRes.ok) setMysqlConfig(await mysqlRes.json());
      if (mailRes.ok) setMailData(await mailRes.json());
      if (optRes.ok) setOptimizer(await optRes.json());
      if (diagRes.ok) setDiagnostics(await diagRes.json());
      if (cpRes.ok) setCpanel(await cpRes.json());
      if (secRes.ok) setSecurity(await secRes.json());
      if (rustRes.ok) setRustFiles(await rustRes.json());
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    fetchAllData();
    const timer = setInterval(fetchTelemetry, 3000);
    return () => clearInterval(timer);
  }, [fetchAllData, fetchTelemetry]);

  const handleNavigate = (tab: ActiveTab, appId?: string) => {
    setActiveTab(tab);
    if (appId) {
      setSelectedAppId(appId);
      if (tab === 'websites') {
        setAutoOpenSettingsCounter((c) => c + 1);
      }
    }
  };

  // Systemd Service Unit Lifecycle via Actix-Web Gateway -> ferrous-system-core
  const handleAppAction = async (
    appId: string,
    action: 'start' | 'stop' | 'restart' | 'rebuild'
  ) => {
    const res = await actixGatewayFetch('SYSTEMD', `/api/apps/${appId}/action`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action })
    });
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
      fetchTelemetry();
    }
  };

  // Nginx / FerrousX + Systemd Cgroups Config via Actix-Web Gateway -> ferrous-system-core
  const handleUpdateAppConfig = async (
    appId: string,
    payload: {
      workers: number;
      memoryLimitMb: number;
      rateLimitRps: number;
      sslEnabled?: boolean;
      http3Quic?: boolean;
      websocketEnabled?: boolean;
      envVars: EnvVar[];
    }
  ) => {
    const res = await actixGatewayFetch('NGINX', `/api/apps/${appId}/config`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  // Nginx / FerrousX VHost Rewrite, Redirects, SSL & Directory Auth via Actix-Web Gateway
  const handleUpdateSiteSettings = async (
    appId: string,
    payload: Partial<ManagedApp>
  ) => {
    const res = await actixGatewayFetch(
      'NGINX',
      `/api/apps/${appId}/site-settings`,
      {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      }
    );
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleTriggerGitDeploy = async (appId: string) => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      `/api/apps/${appId}/git-deploy`,
      {
        method: 'POST'
      }
    );
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleTestWebsiteHttp = async (
    appId: string
  ): Promise<WebsiteLiveTestResult | null> => {
    const res = await actixGatewayFetch(
      'NGINX',
      `/api/apps/${appId}/test-http`,
      {
        method: 'POST'
      }
    );
    if (res.ok) {
      return (await res.json()) as WebsiteLiveTestResult;
    }
    return null;
  };

  const handleRenewSsl = async (appId: string) => {
    const res = await actixGatewayFetch(
      'NGINX',
      `/api/apps/${appId}/renew-ssl`,
      {
        method: 'POST'
      }
    );
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleDeleteWebsite = async (appId: string) => {
    const res = await actixGatewayFetch('NGINX', `/api/apps/${appId}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const remaining: ManagedApp[] = await res.json();
      setApps(remaining);
      if (remaining.length > 0 && selectedAppId === appId) {
        setSelectedAppId(remaining[0].id);
      }
    }
  };

  const handleDeployNewWebsite = async (payload: {
    name: string;
    framework: WebsiteRuntimeType;
    domain: string;
    aliases: string[];
    internalPort: number;
    workingDir: string;
    runSubDir: string;
    execCommand: string;
    workers: number;
    memoryLimitMb: number;
    sslEnabled: boolean;
    http3Quic: boolean;
    websocketEnabled: boolean;
    rateLimitRps: number;
    createDatabase: boolean;
    dbEngine: 'PostgreSQL 17' | 'MariaDB 11.4';
    dbName: string;
    dbUser: string;
    dbPassword: string;
    createFtp: boolean;
    ftpUser: string;
    gitRepoUrl: string;
    gitBranch: string;
    webServerEngine?: WebServerEngineType;
  }) => {
    const res = await actixGatewayFetch('NGINX', '/api/apps', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const created: ManagedApp = await res.json();
      setApps((prev) => [created, ...prev]);
      setSelectedAppId(created.id);
      setActiveTab('websites');
      fetchAllData();
    }
  };

  const handleDeployOneClick = async (payload: {
    templateId: string;
    domain: string;
    createDb: boolean;
    webServerEngine?: WebServerEngineType;
  }) => {
    const res = await actixGatewayFetch('NGINX', '/api/oneclick/deploy', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const created: ManagedApp = await res.json();
      setApps((prev) => [created, ...prev]);
      setSelectedAppId(created.id);
      setActiveTab('websites');
      fetchAllData();
    }
  };

  const handlePackageAction = async (
    pkgId: string,
    action: 'start' | 'stop' | 'restart' | 'install' | 'uninstall'
  ) => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      `/api/software/${pkgId}/action`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action })
      }
    );
    if (res.ok) {
      setPackages(await res.json());
    }
  };

  const handleSavePackageConfig = async (
    pkgId: string,
    configContent: string
  ) => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      `/api/software/${pkgId}/config`,
      {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ configContent })
      }
    );
    if (res.ok) {
      setPackages(await res.json());
    }
  };

  // SQL Database & MySQL phpMyAdmin Handlers
  const handleCreateDatabase = async (payload: {
    name: string;
    engine: SqlDatabase['engine'];
    username: string;
    password?: string;
    collation: string;
    linkedAppId: string;
  }) => {
    const res = await fetch('/api/databases', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setDatabases(await res.json());
    }
  };

  const handleChangeDatabasePassword = async (
    id: string,
    newPassword: string
  ) => {
    const res = await fetch(`/api/databases/${id}/password`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ password: newPassword })
    });
    if (res.ok) {
      setDatabases(await res.json());
      fetchAllData();
    }
  };

  const handleDeleteDatabase = async (id: string) => {
    const res = await fetch(`/api/databases/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      setDatabases(await res.json());
    }
  };

  const handleExecuteSqlQuery = async (
    dbId: string,
    query: string
  ): Promise<SqlQueryResponse> => {
    const res = await fetch(`/api/databases/${dbId}/query`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ query })
    });
    const data: SqlQueryResponse = await res.json();
    const dbRes = await fetch('/api/databases');
    if (dbRes.ok) setDatabases(await dbRes.json());
    return data;
  };

  const handleUpdateMysqlConfig = async (
    payload: Partial<MysqlPhpMyAdminConfig>
  ) => {
    const res = await fetch('/api/databases/mysql-config', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setMysqlConfig(await res.json());
    }
  };

  // Mail Server Handlers
  const handleAddMailDomain = async (domain: string) => {
    const res = await fetch('/api/mail/domains', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ domain })
    });
    if (res.ok) {
      setMailData(await res.json());
    }
  };

  const handleCreateMailbox = async (payload: {
    email: string;
    domain: string;
    displayName: string;
    quotaMb: number;
  }) => {
    const res = await fetch('/api/mail/mailboxes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setMailData(await res.json());
    }
  };

  const handleDeleteMailbox = async (id: string) => {
    const res = await fetch(`/api/mail/mailboxes/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      setMailData(await res.json());
    }
  };

  const handleCreateForwarder = async (payload: {
    sourceEmail: string;
    destinationEmail: string;
    domain: string;
  }) => {
    const res = await fetch('/api/mail/forwarders', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setMailData(await res.json());
    }
  };

  const handleDeleteForwarder = async (id: string) => {
    const res = await fetch(`/api/mail/forwarders/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      setMailData(await res.json());
    }
  };

  const handleSendTestMail = async (payload: {
    from: string;
    to: string;
    subject: string;
    body: string;
  }) => {
    const res = await fetch('/api/mail/send', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setMailData(await res.json());
    }
  };

  // System Optimizer & Backend Watchdog Handlers via Actix-Web Gateway -> ferrous-system-core
  const handleApplyOptimizationProfile = async (
    profileId: OptimizationProfilePreset['id']
  ) => {
    const res = await actixGatewayFetch(
      'SYSCTL',
      '/api/optimizer/apply-profile',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ profileId })
      }
    );
    if (res.ok) {
      setOptimizer(await res.json());
      fetchAllData();
    }
  };

  const handleToggleSysctlParam = async (id: string) => {
    const res = await actixGatewayFetch(
      'SYSCTL',
      `/api/optimizer/sysctl/${id}`,
      {
        method: 'PUT'
      }
    );
    if (res.ok) {
      setOptimizer(await res.json());
    }
  };

  const handleReclaimMemory = async (): Promise<{
    freedMb: number;
    message: string;
  }> => {
    const res = await actixGatewayFetch(
      'SYSCTL',
      '/api/optimizer/reclaim-memory',
      {
        method: 'POST'
      }
    );
    const data = await res.json();
    if (data.optimizer) setOptimizer(data.optimizer);
    fetchTelemetry();
    return { freedMb: data.freedMb, message: data.message };
  };

  const handleWatchdogHeal = async () => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      '/api/optimizer/watchdog-heal',
      {
        method: 'POST'
      }
    );
    if (res.ok) {
      setOptimizer(await res.json());
      fetchAllData();
    }
  };

  const handleUpdateAntiCrash = async (
    payload: Partial<AntiCrashGuardState>
  ) => {
    const res = await actixGatewayFetch('SYSCTL', '/api/optimizer/anti-crash', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setOptimizer(await res.json());
    }
  };

  const handleEmergencyMitigate = async (
    action:
      | 'purge_syn_backlog'
      | 'isolate_leaking_workers'
      | 'reset_hung_db_queries'
  ): Promise<{ message?: string } | void> => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      '/api/optimizer/emergency-mitigate',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action })
      }
    );
    if (res.ok) {
      const data = await res.json();
      if (data.optimizer) setOptimizer(data.optimizer);
      fetchTelemetry();
      return { message: data.message };
    }
  };

  // cPanel Suite Handlers
  const handleAddCron = async (payload: {
    title: string;
    schedule: string;
    command: string;
    user: string;
    targetApp: string;
  }) => {
    const res = await fetch('/api/cpanel/cron', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const cronJobs = await res.json();
      setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
    }
  };

  const handleRunCronNow = async (id: string) => {
    const res = await fetch(`/api/cpanel/cron/${id}/run`, {
      method: 'POST'
    });
    if (res.ok) {
      const cronJobs = await res.json();
      setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
    }
  };

  const handleToggleCron = async (id: string) => {
    const res = await fetch(`/api/cpanel/cron/${id}/toggle`, {
      method: 'PUT'
    });
    if (res.ok) {
      const cronJobs = await res.json();
      setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
    }
  };

  const handleDeleteCron = async (id: string) => {
    const res = await fetch(`/api/cpanel/cron/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const cronJobs = await res.json();
      setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
    }
  };

  const handleAddDns = async (payload: {
    domain: string;
    type: 'A' | 'AAAA' | 'CNAME' | 'MX' | 'TXT' | 'CAA';
    host: string;
    value: string;
    ttl: number;
  }) => {
    const res = await fetch('/api/cpanel/dns', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const dnsRecords = await res.json();
      setCpanel((prev) => (prev ? { ...prev, dnsRecords } : prev));
    }
  };

  const handleDeleteDns = async (id: string) => {
    const res = await fetch(`/api/cpanel/dns/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const dnsRecords = await res.json();
      setCpanel((prev) => (prev ? { ...prev, dnsRecords } : prev));
    }
  };

  const handleCreateBackup = async (payload: {
    targetApp: string;
    databaseIncluded: string;
  }) => {
    const res = await fetch('/api/cpanel/backups', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const backups = await res.json();
      setCpanel((prev) => (prev ? { ...prev, backups } : prev));
    }
  };

  const handleRestoreBackup = async (id: string) => {
    const res = await fetch(`/api/cpanel/backups/${id}/restore`, {
      method: 'POST'
    });
    if (res.ok) {
      const backups = await res.json();
      setCpanel((prev) => (prev ? { ...prev, backups } : prev));
      fetchAllData();
    }
  };

  const handleDeleteBackup = async (id: string) => {
    const res = await fetch(`/api/cpanel/backups/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const backups = await res.json();
      setCpanel((prev) => (prev ? { ...prev, backups } : prev));
    }
  };

  const handleAddSftp = async (payload: {
    username: string;
    targetApp: string;
    authMethod: 'Ed25519 SSH Key' | 'Argon2id Password';
    quotaMb: number;
  }) => {
    const res = await fetch('/api/cpanel/sftp', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const sftpAccounts = await res.json();
      setCpanel((prev) => (prev ? { ...prev, sftpAccounts } : prev));
    }
  };

  const handleDeleteSftp = async (id: string) => {
    const res = await fetch(`/api/cpanel/sftp/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const sftpAccounts = await res.json();
      setCpanel((prev) => (prev ? { ...prev, sftpAccounts } : prev));
    }
  };

  // UFW Firewall, Fail2ban & eBPF/XDP Security Handlers via Actix-Web Gateway -> ferrous-system-core
  const handleAddUfwRule = async (rule: {
    port: string;
    protocol: 'TCP' | 'UDP' | 'BOTH';
    action: 'ALLOW' | 'DENY' | 'LIMIT';
    source: string;
    comment: string;
  }) => {
    const res = await actixGatewayFetch('UFW', '/api/security/ufw', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(rule)
    });
    if (res.ok) {
      const ufwRules = await res.json();
      setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
    }
  };

  const handleToggleUfwRule = async (id: string) => {
    const res = await actixGatewayFetch(
      'UFW',
      `/api/security/ufw/${id}/toggle`,
      {
        method: 'PUT'
      }
    );
    if (res.ok) {
      const ufwRules = await res.json();
      setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
    }
  };

  const handleDeleteUfwRule = async (id: string) => {
    const res = await actixGatewayFetch('UFW', `/api/security/ufw/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const ufwRules = await res.json();
      setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
    }
  };

  const handleBanIp = async (jailName: string, ip: string, reason: string) => {
    const res = await actixGatewayFetch('UFW', '/api/security/fail2ban/ban', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jailName, ip, reason })
    });
    if (res.ok) {
      const fail2banJails = await res.json();
      setSecurity((prev) => (prev ? { ...prev, fail2banJails } : prev));
    }
  };

  const handleUnbanIp = async (jailName: string, ip: string) => {
    const res = await actixGatewayFetch('UFW', '/api/security/fail2ban/unban', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jailName, ip })
    });
    if (res.ok) {
      const fail2banJails = await res.json();
      setSecurity((prev) => (prev ? { ...prev, fail2banJails } : prev));
    }
  };

  const handleToggleHardening = async (id: string) => {
    const res = await actixGatewayFetch(
      'UFW',
      `/api/security/hardening/${id}`,
      {
        method: 'PUT'
      }
    );
    if (res.ok) {
      const hardeningDirectives = await res.json();
      setSecurity((prev) => (prev ? { ...prev, hardeningDirectives } : prev));
    }
  };

  const handleChangeDdosMode = async (
    mode: 'normal' | 'elevated' | 'under_attack'
  ) => {
    const res = await actixGatewayFetch('UFW', '/api/security/ddos-mode', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ mode })
    });
    if (res.ok) {
      const antiDdos = await res.json();
      setSecurity((prev) => (prev ? { ...prev, antiDdos } : prev));
    }
  };

  const handleToggleWafRule = async (id: string) => {
    const res = await actixGatewayFetch(
      'UFW',
      `/api/security/waf/${id}/toggle`,
      {
        method: 'PUT'
      }
    );
    if (res.ok) {
      const antiDdos = await res.json();
      setSecurity((prev) => (prev ? { ...prev, antiDdos } : prev));
    }
  };

  const handleRunFimScan = async (): Promise<void> => {
    const res = await actixGatewayFetch('UFW', '/api/security/fim-scan', {
      method: 'POST'
    });
    if (res.ok) {
      const data = await res.json();
      if (data.antiDdos) {
        setSecurity((prev) =>
          prev ? { ...prev, antiDdos: data.antiDdos } : prev
        );
      }
    }
  };

  // SSH Terminal & External Client Handlers via Actix-Web Gateway -> ferrous-system-core
  const handleUpdateSshConfig = async (payload: Partial<SshDaemonConfig>) => {
    const res = await actixGatewayFetch('SYSCALL', '/api/ssh/config', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setSshData(await res.json());
      fetchAllData();
    }
  };

  const handleAddAuthorizedKey = async (payload: {
    label: string;
    publicKey: string;
    clientHint: string;
  }) => {
    const res = await actixGatewayFetch('SYSCALL', '/api/ssh/keys', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setSshData(await res.json());
    }
  };

  const handleGenerateSshKeypair = async (payload: {
    label: string;
    clientHint: string;
  }): Promise<SshGeneratedKeypair | null> => {
    const res = await actixGatewayFetch('SYSCALL', '/api/ssh/keys/generate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const data = await res.json();
      if (data.sshData) setSshData(data.sshData);
      return data.generated as SshGeneratedKeypair;
    }
    return null;
  };

  const handleDeleteSshKey = async (id: string) => {
    const res = await actixGatewayFetch('SYSCALL', `/api/ssh/keys/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      setSshData(await res.json());
    }
  };

  const handleTerminateSshSession = async (id: string) => {
    const res = await actixGatewayFetch('SYSCALL', `/api/ssh/sessions/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      setSshData(await res.json());
    }
  };

  const handleExecWebSsh = async (
    command: string,
    cwd: string
  ): Promise<WebSshCommandOutput> => {
    const res = await actixGatewayFetch('SYSCALL', '/api/ssh/exec', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ command, cwd })
    });
    return await res.json();
  };

  const handleUpdatePanelAdmin = async (
    payload: Partial<PanelAdminAccount>
  ) => {
    const res = await actixGatewayFetch('SYSCALL', '/api/panel-admin', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      await fetchAllData();
    }
  };

  const handleRunFpCliAction = async (
    option: number,
    value?: string
  ): Promise<{ output: string; panelAdmin: PanelAdminAccount } | null> => {
    const res = await actixGatewayFetch(
      'SYSCALL',
      '/api/panel-admin/cli-action',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ option, value })
      }
    );
    if (res.ok) {
      const data = await res.json();
      await fetchAllData();
      return data;
    }
    return null;
  };

  // Multi-PHP 8.4 / 8.3 / 8.2 / 8.1 / 7.4 FPM Pool Handlers via Actix-Web Gateway -> ferrous-system-core
  const handlePhpAction = async (
    version: string,
    action: 'start' | 'stop' | 'restart' | 'reload' | 'install' | 'uninstall'
  ) => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      `/api/php/${version}/action`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action })
      }
    );
    if (res.ok) {
      setPhpVersions(await res.json());
    }
  };

  const handleUpdatePhpSettings = async (
    version: string,
    payload: Partial<PhpVersionInstance>
  ) => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      `/api/php/${version}/settings`,
      {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      }
    );
    if (res.ok) {
      setPhpVersions(await res.json());
    }
  };

  const handleTogglePhpExtension = async (version: string, extName: string) => {
    const res = await actixGatewayFetch(
      'SYSTEMD',
      `/api/php/${version}/extensions/toggle`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ extName })
      }
    );
    if (res.ok) {
      setPhpVersions(await res.json());
    }
  };

  const handleSwitchSitePhpVersion = async (
    appId: string,
    phpVersion: string
  ) => {
    const res = await actixGatewayFetch('NGINX', '/api/php/switch-site', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ appId, phpVersion })
    });
    if (res.ok) {
      const data = await res.json();
      if (data.app) {
        setApps((prev) =>
          prev.map((a) => (a.id === data.app.id ? data.app : a))
        );
      }
      if (data.phpVersions) {
        setPhpVersions(data.phpVersions);
      }
    }
  };

  return {
    selectedAppId,
    setSelectedAppId,
    autoOpenSettingsCounter,
    telemetry,
    apps,
    phpVersions,
    templates,
    packages,
    sshData,
    databases,
    mysqlConfig,
    mailData,
    optimizer,
    diagnostics,
    cpanel,
    security,
    rustFiles,
    refreshing,
    fetchAllData,
    fetchDiagnostics,
    handleNavigate,
    handleAppAction,
    handleUpdateAppConfig,
    handleUpdateSiteSettings,
    handleTriggerGitDeploy,
    handleTestWebsiteHttp,
    handleRenewSsl,
    handleDeleteWebsite,
    handleDeployNewWebsite,
    handleDeployOneClick,
    handlePackageAction,
    handleSavePackageConfig,
    handleCreateDatabase,
    handleChangeDatabasePassword,
    handleDeleteDatabase,
    handleExecuteSqlQuery,
    handleUpdateMysqlConfig,
    handleAddMailDomain,
    handleCreateMailbox,
    handleDeleteMailbox,
    handleCreateForwarder,
    handleDeleteForwarder,
    handleSendTestMail,
    handleApplyOptimizationProfile,
    handleToggleSysctlParam,
    handleReclaimMemory,
    handleWatchdogHeal,
    handleUpdateAntiCrash,
    handleEmergencyMitigate,
    handleAddCron,
    handleRunCronNow,
    handleToggleCron,
    handleDeleteCron,
    handleAddDns,
    handleDeleteDns,
    handleCreateBackup,
    handleRestoreBackup,
    handleDeleteBackup,
    handleAddSftp,
    handleDeleteSftp,
    handleAddUfwRule,
    handleToggleUfwRule,
    handleDeleteUfwRule,
    handleBanIp,
    handleUnbanIp,
    handleToggleHardening,
    handleChangeDdosMode,
    handleToggleWafRule,
    handleRunFimScan,
    handleUpdateSshConfig,
    handleAddAuthorizedKey,
    handleGenerateSshKeypair,
    handleDeleteSshKey,
    handleTerminateSshSession,
    handleExecWebSsh,
    handleUpdatePanelAdmin,
    handleRunFpCliAction,
    handlePhpAction,
    handleUpdatePhpSettings,
    handleTogglePhpExtension,
    handleSwitchSitePhpVersion
  };
}
