import React, { useEffect, useState, useCallback } from 'react';
import {
  Activity,
  Code2,
  Database,
  FileCode2,
  FolderKanban,
  Gauge,
  Globe,
  Mail,
  Moon,
  Plus,
  RefreshCw,
  Rocket,
  ShieldCheck,
  Sun,
  Terminal,
  LogOut,
  User,
  Wrench
} from 'lucide-react';
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
  WebSshCommandOutput,
  WebsiteLiveTestResult,
  WebsiteRuntimeType
} from './types';
import { OverviewView } from './components/OverviewView';
import { WebsitesManagerView } from './components/WebsitesManagerView';
import { PhpManagerView } from './components/PhpManagerView';
import { OneClickAppsView } from './components/OneClickAppsView';
import { SshTerminalView } from './components/SshTerminalView';
import { FileManagerView } from './components/FileManagerView';
import { SqlDatabaseView } from './components/SqlDatabaseView';
import { MailServerView } from './components/MailServerView';
import { SystemOptimizerView } from './components/SystemOptimizerView';
import { CpanelSuiteView } from './components/CpanelSuiteView';
import { SecurityCenterView } from './components/SecurityCenterView';
import { RustSourceView } from './components/RustSourceView';
import { DeployModal } from './components/DeployModal';
import { LoginView } from './components/LoginView';
import { FerrousLogo } from './components/FerrousLogo';

export default function App() {
  const [activeTab, setActiveTab] = useState<ActiveTab>('overview');
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [autoOpenSettingsCounter, setAutoOpenSettingsCounter] =
    useState<number>(0);
  const [isDark, setIsDark] = useState<boolean>(true);
  const [isDeployModalOpen, setIsDeployModalOpen] = useState<boolean>(false);

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

  // Authentication & Mandatory Login State
  const [sessionToken, setSessionToken] = useState<string | null>(() => {
    return localStorage.getItem('fp_admin_session_token') || null;
  });
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(() => {
    return Boolean(localStorage.getItem('fp_admin_session_token'));
  });
  const [currentUser, setCurrentUser] = useState<{ username: string } | null>(
    () => {
      const savedUser = localStorage.getItem('fp_admin_user');
      return savedUser ? { username: savedUser } : null;
    }
  );

  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
  }, [isDark]);

  // Verify session on mount
  useEffect(() => {
    const verifySession = async () => {
      const token = localStorage.getItem('fp_admin_session_token');
      if (!token) {
        setIsAuthenticated(false);
        setCurrentUser(null);
        return;
      }
      try {
        const res = await fetch('/api/auth/session', {
          headers: { Authorization: `Bearer ${token}` }
        });
        if (res.ok) {
          const data = await res.json();
          if (data.authenticated) {
            setIsAuthenticated(true);
            setCurrentUser({ username: data.user?.username || 'admin_ferrous' });
          } else {
            setIsAuthenticated(false);
            setCurrentUser(null);
            localStorage.removeItem('fp_admin_session_token');
            localStorage.removeItem('fp_admin_user');
          }
        } else {
          setIsAuthenticated(false);
          setCurrentUser(null);
          localStorage.removeItem('fp_admin_session_token');
          localStorage.removeItem('fp_admin_user');
        }
      } catch {
        // network issue - maintain offline session if present
      }
    };
    verifySession();
  }, []);

  const handleLoginSuccess = (token: string, user: { username: string }) => {
    localStorage.setItem('fp_admin_session_token', token);
    localStorage.setItem('fp_admin_user', user.username);
    setSessionToken(token);
    setCurrentUser(user);
    setIsAuthenticated(true);
    fetchAllData();
  };

  const handleLogout = async () => {
    try {
      if (sessionToken) {
        await fetch('/api/auth/logout', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${sessionToken}`
          },
          body: JSON.stringify({ token: sessionToken })
        });
      }
    } catch {
      // ignore
    } finally {
      localStorage.removeItem('fp_admin_session_token');
      localStorage.removeItem('fp_admin_user');
      setSessionToken(null);
      setCurrentUser(null);
      setIsAuthenticated(false);
    }
  };

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
        fetch('/api/apps'),
        fetch('/api/php'),
        fetch('/api/oneclick/templates'),
        fetch('/api/software'),
        fetch('/api/ssh'),
        fetch('/api/databases'),
        fetch('/api/databases/mysql-config'),
        fetch('/api/mail'),
        fetch('/api/optimizer'),
        fetch('/api/system/diagnostics'),
        fetch('/api/cpanel'),
        fetch('/api/security'),
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

  const handleAppAction = async (
    appId: string,
    action: 'start' | 'stop' | 'restart' | 'rebuild'
  ) => {
    const res = await fetch(`/api/apps/${appId}/action`, {
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
    const res = await fetch(`/api/apps/${appId}/config`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleUpdateSiteSettings = async (
    appId: string,
    payload: Partial<ManagedApp>
  ) => {
    const res = await fetch(`/api/apps/${appId}/site-settings`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleTriggerGitDeploy = async (appId: string) => {
    const res = await fetch(`/api/apps/${appId}/git-deploy`, {
      method: 'POST'
    });
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleTestWebsiteHttp = async (
    appId: string
  ): Promise<WebsiteLiveTestResult | null> => {
    const res = await fetch(`/api/apps/${appId}/test-http`, {
      method: 'POST'
    });
    if (res.ok) {
      return (await res.json()) as WebsiteLiveTestResult;
    }
    return null;
  };

  const handleRenewSsl = async (appId: string) => {
    const res = await fetch(`/api/apps/${appId}/renew-ssl`, {
      method: 'POST'
    });
    if (res.ok) {
      const updated: ManagedApp = await res.json();
      setApps((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    }
  };

  const handleDeleteWebsite = async (appId: string) => {
    const res = await fetch(`/api/apps/${appId}`, {
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
  }) => {
    const res = await fetch('/api/apps', {
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
  }) => {
    const res = await fetch('/api/oneclick/deploy', {
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
    const res = await fetch(`/api/software/${pkgId}/action`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action })
    });
    if (res.ok) {
      setPackages(await res.json());
    }
  };

  const handleSavePackageConfig = async (
    pkgId: string,
    configContent: string
  ) => {
    const res = await fetch(`/api/software/${pkgId}/config`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ configContent })
    });
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

  // System Optimizer & Backend Watchdog Handlers
  const handleApplyOptimizationProfile = async (
    profileId: OptimizationProfilePreset['id']
  ) => {
    const res = await fetch('/api/optimizer/apply-profile', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ profileId })
    });
    if (res.ok) {
      setOptimizer(await res.json());
      fetchAllData();
    }
  };

  const handleToggleSysctlParam = async (id: string) => {
    const res = await fetch(`/api/optimizer/sysctl/${id}`, {
      method: 'PUT'
    });
    if (res.ok) {
      setOptimizer(await res.json());
    }
  };

  const handleReclaimMemory = async (): Promise<{
    freedMb: number;
    message: string;
  }> => {
    const res = await fetch('/api/optimizer/reclaim-memory', {
      method: 'POST'
    });
    const data = await res.json();
    if (data.optimizer) setOptimizer(data.optimizer);
    fetchTelemetry();
    return { freedMb: data.freedMb, message: data.message };
  };

  const handleWatchdogHeal = async () => {
    const res = await fetch('/api/optimizer/watchdog-heal', {
      method: 'POST'
    });
    if (res.ok) {
      setOptimizer(await res.json());
      fetchAllData();
    }
  };

  const handleUpdateAntiCrash = async (
    payload: Partial<AntiCrashGuardState>
  ) => {
    const res = await fetch('/api/optimizer/anti-crash', {
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
    const res = await fetch('/api/optimizer/emergency-mitigate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action })
    });
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

  // Security Handlers
  const handleAddUfwRule = async (rule: {
    port: string;
    protocol: 'TCP' | 'UDP' | 'BOTH';
    action: 'ALLOW' | 'DENY' | 'LIMIT';
    source: string;
    comment: string;
  }) => {
    const res = await fetch('/api/security/ufw', {
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
    const res = await fetch(`/api/security/ufw/${id}/toggle`, {
      method: 'PUT'
    });
    if (res.ok) {
      const ufwRules = await res.json();
      setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
    }
  };

  const handleDeleteUfwRule = async (id: string) => {
    const res = await fetch(`/api/security/ufw/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const ufwRules = await res.json();
      setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
    }
  };

  const handleBanIp = async (jailName: string, ip: string, reason: string) => {
    const res = await fetch('/api/security/fail2ban/ban', {
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
    const res = await fetch('/api/security/fail2ban/unban', {
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
    const res = await fetch(`/api/security/hardening/${id}`, {
      method: 'PUT'
    });
    if (res.ok) {
      const hardeningDirectives = await res.json();
      setSecurity((prev) => (prev ? { ...prev, hardeningDirectives } : prev));
    }
  };

  const handleChangeDdosMode = async (
    mode: 'normal' | 'elevated' | 'under_attack'
  ) => {
    const res = await fetch('/api/security/ddos-mode', {
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
    const res = await fetch(`/api/security/waf/${id}/toggle`, {
      method: 'PUT'
    });
    if (res.ok) {
      const antiDdos = await res.json();
      setSecurity((prev) => (prev ? { ...prev, antiDdos } : prev));
    }
  };

  const handleRunFimScan = async (): Promise<void> => {
    const res = await fetch('/api/security/fim-scan', {
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

  // SSH Terminal & External Client Handlers
  const handleUpdateSshConfig = async (payload: Partial<SshDaemonConfig>) => {
    const res = await fetch('/api/ssh/config', {
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
    const res = await fetch('/api/ssh/keys', {
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
    const res = await fetch('/api/ssh/keys/generate', {
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
    const res = await fetch(`/api/ssh/keys/${id}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      setSshData(await res.json());
    }
  };

  const handleTerminateSshSession = async (id: string) => {
    const res = await fetch(`/api/ssh/sessions/${id}`, {
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
    const res = await fetch('/api/ssh/exec', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ command, cwd })
    });
    return await res.json();
  };

  const handleUpdatePanelAdmin = async (
    payload: Partial<PanelAdminAccount>
  ) => {
    const res = await fetch('/api/panel-admin', {
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
    const res = await fetch('/api/panel-admin/cli-action', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ option, value })
    });
    if (res.ok) {
      const data = await res.json();
      await fetchAllData();
      return data;
    }
    return null;
  };

  // Multi-PHP 8.4 / 8.3 / 8.2 / 8.1 / 7.4 Handlers
  const handlePhpAction = async (
    version: string,
    action: 'start' | 'stop' | 'restart' | 'reload' | 'install' | 'uninstall'
  ) => {
    const res = await fetch(`/api/php/${version}/action`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action })
    });
    if (res.ok) {
      setPhpVersions(await res.json());
    }
  };

  const handleUpdatePhpSettings = async (
    version: string,
    payload: Partial<PhpVersionInstance>
  ) => {
    const res = await fetch(`/api/php/${version}/settings`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
    if (res.ok) {
      setPhpVersions(await res.json());
    }
  };

  const handleTogglePhpExtension = async (version: string, extName: string) => {
    const res = await fetch(`/api/php/${version}/extensions/toggle`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ extName })
    });
    if (res.ok) {
      setPhpVersions(await res.json());
    }
  };

  const handleSwitchSitePhpVersion = async (
    appId: string,
    phpVersion: string
  ) => {
    const res = await fetch('/api/php/switch-site', {
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

  const sidebarItems: {
    id: ActiveTab;
    label: string;
    icon: React.FC<{ className?: string }>;
  }[] = [
    { id: 'overview', label: 'Trang chủ Panel', icon: Activity },
    { id: 'websites', label: 'Websites & Modern JS', icon: Globe },
    { id: 'php', label: 'Multi-PHP (7.4-8.4) & INI', icon: FileCode2 },
    { id: 'files', label: 'File Manager & Giải nén ZIP', icon: FolderKanban },
    { id: 'ssh', label: 'Web SSH & Panel CLI (fp)', icon: Terminal },
    { id: 'oneclick', label: 'Package Rust & 1-Click', icon: Rocket },
    { id: 'databases', label: 'MySQL phpMyAdmin & SQL', icon: Database },
    { id: 'mail', label: 'Mail Server & Webmail', icon: Mail },
    { id: 'optimizer', label: 'Models Tối ưu & Watchdog', icon: Gauge },
    { id: 'cpanel', label: 'Cron, DNS, Backup & FTP', icon: Wrench },
    { id: 'security', label: 'Tường lửa & Bảo mật', icon: ShieldCheck },
    { id: 'rust-source', label: 'Nhân Rust & Đa Ngôn Ngữ', icon: Code2 }
  ];

  // Mandatory Login Gate: If not authenticated, render LoginView only
  if (!isAuthenticated) {
    return (
      <LoginView
        onLoginSuccess={handleLoginSuccess}
        panelAdminConfig={sshData?.panelAdmin}
        serverHost={sshData?.serverHost || '103.145.88.210'}
        isDark={isDark}
        onToggleTheme={() => setIsDark(!isDark)}
      />
    );
  }

  return (
    <div className="min-h-screen flex flex-col bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
      {/* Top Bar Contract: 3 zones separated by gap-8 */}
      <header className="flex items-center justify-between gap-8 px-6 py-3.5 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 sticky top-0 z-30">
        {/* Zone 1: Brand lockup */}
        <a
          href="#overview"
          onClick={(e) => {
            e.preventDefault();
            setActiveTab('overview');
          }}
          className="whitespace-nowrap shrink-0"
        >
          <FerrousLogo size="md" showWordmark={true} />
        </a>

        {/* Zone 2: 5 clean single-line text navigation links */}
        <nav className="hidden md:flex items-center gap-6 text-sm font-medium text-slate-600 dark:text-slate-400">
          <a
            href="#websites"
            onClick={(e) => {
              e.preventDefault();
              setActiveTab('websites');
            }}
            className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
              activeTab === 'websites'
                ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
                : ''
            }`}
          >
            Websites
          </a>
          <a
            href="#php"
            onClick={(e) => {
              e.preventDefault();
              setActiveTab('php');
            }}
            className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
              activeTab === 'php'
                ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
                : ''
            }`}
          >
            Multi-PHP &amp; INI
          </a>
          <a
            href="#files"
            onClick={(e) => {
              e.preventDefault();
              setActiveTab('files');
            }}
            className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
              activeTab === 'files'
                ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
                : ''
            }`}
          >
            File Manager (.ZIP)
          </a>
          <a
            href="#databases"
            onClick={(e) => {
              e.preventDefault();
              setActiveTab('databases');
            }}
            className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
              activeTab === 'databases'
                ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
                : ''
            }`}
          >
            MySQL &amp; phpMyAdmin
          </a>
          <a
            href="#rust-source"
            onClick={(e) => {
              e.preventDefault();
              setActiveTab('rust-source');
            }}
            className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
              activeTab === 'rust-source'
                ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
                : ''
            }`}
          >
            Cầu nối Đa ngôn ngữ
          </a>
        </nav>

        {/* Zone 3: 1 primary action & Admin profile / logout */}
        <div className="flex items-center gap-3 shrink-0">
          <div className="hidden sm:flex items-center gap-2 pl-3 border-l border-slate-200 dark:border-slate-800">
            <div className="w-7 h-7 rounded-lg bg-orange-600/10 dark:bg-orange-500/20 text-orange-600 dark:text-orange-400 flex items-center justify-center">
              <User className="w-3.5 h-3.5" />
            </div>
            <div className="text-left hidden lg:block">
              <div className="text-xs font-semibold text-slate-900 dark:text-slate-100 leading-tight">
                {currentUser?.username || sshData?.panelAdmin?.adminUsername || 'admin_ferrous'}
              </div>
              <div className="text-[10px] font-mono text-emerald-600 dark:text-emerald-400 leading-tight">
                VPS SuperAdmin
              </div>
            </div>
          </div>

          <button
            onClick={() => setIsDeployModalOpen(true)}
            className="px-3.5 py-1.5 text-xs font-medium text-white bg-orange-600 rounded-lg hover:bg-orange-500 transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Thêm Website Mới
          </button>

          <button
            type="button"
            onClick={handleLogout}
            className="px-2.5 py-1.5 text-xs font-medium rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950/60 hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/30 dark:hover:text-rose-400 text-slate-600 dark:text-slate-400 transition-colors inline-flex items-center gap-1.5 cursor-pointer shrink-0"
            title="Đăng xuất khỏi FerrousPanel"
          >
            <LogOut className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Đăng xuất</span>
          </button>
        </div>
      </header>

      {/* Workspace Body: Left Sidebar + Main Viewport */}
      <div className="flex-1 flex">
        {/* Sidebar Navigation (260px) */}
        <aside className="hidden lg:flex w-64 shrink-0 flex-col justify-between border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4">
          <div className="space-y-6">
            <div className="px-3 py-2 space-y-1">
              <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                Ubuntu Rust Control Panel
              </div>
              <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                100% Rust Core · 10 Ngôn ngữ FFI
              </div>
            </div>

            <div className="space-y-1">
              {sidebarItems.map((item) => {
                const Icon = item.icon;
                const active = activeTab === item.id;
                return (
                  <button
                    key={item.id}
                    onClick={() => setActiveTab(item.id)}
                    className={`w-full flex items-center gap-3 px-3 py-2.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                      active
                        ? 'bg-orange-600 text-white'
                        : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-slate-100'
                    }`}
                  >
                    <Icon className="w-4 h-4 shrink-0" />
                    <span>{item.label}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Sidebar Footer: Workspace Utilities (Refresh & Theme Toggle) */}
          <div className="pt-4 border-t border-slate-200 dark:border-slate-800 space-y-2">
            <div className="flex items-center gap-2">
              <button
                onClick={fetchAllData}
                className="flex-1 px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer"
              >
                <RefreshCw
                  className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin' : ''}`}
                />
                Làm mới
              </button>
              <button
                onClick={() => setIsDark(!isDark)}
                className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer"
                title="Chuyển đổi giao diện Sáng / Tối"
              >
                {isDark ? (
                  <>
                    <Sun className="w-3.5 h-3.5 text-amber-400" />
                    Sáng
                  </>
                ) : (
                  <>
                    <Moon className="w-3.5 h-3.5 text-slate-600" />
                    Tối
                  </>
                )}
              </button>
            </div>
            <div className="px-2 text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
              15/15 Rust Modules · SSH Port {sshData?.config.port ?? 22}
            </div>
          </div>
        </aside>

        {/* Main Viewport */}
        <main className="flex-1 min-w-0 p-6 lg:p-8 max-w-7xl mx-auto w-full space-y-6">
          {/* Mobile Tab Bar */}
          <div className="flex lg:hidden items-center gap-1 overflow-x-auto pb-2 border-b border-slate-200 dark:border-slate-800">
            {sidebarItems.map((item) => (
              <button
                key={item.id}
                onClick={() => setActiveTab(item.id)}
                className={`px-3 py-1.5 text-xs font-medium rounded-lg whitespace-nowrap shrink-0 cursor-pointer ${
                  activeTab === item.id
                    ? 'bg-orange-600 text-white'
                    : 'text-slate-600 dark:text-slate-400 bg-slate-100 dark:bg-slate-900'
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>

          {activeTab === 'overview' && (
            <OverviewView
              telemetry={telemetry}
              apps={apps}
              databases={databases}
              cpanel={cpanel}
              security={security}
              mailData={mailData}
              diagnostics={diagnostics}
              onRunDiagnostics={fetchDiagnostics}
              onNavigate={handleNavigate}
              onAppAction={handleAppAction}
              onOpenDeployModal={() => setIsDeployModalOpen(true)}
            />
          )}

          {activeTab === 'websites' && (
            <WebsitesManagerView
              apps={apps}
              selectedAppId={selectedAppId}
              autoOpenSettingsCounter={autoOpenSettingsCounter}
              onSelectApp={setSelectedAppId}
              onNavigate={handleNavigate}
              onAppAction={handleAppAction}
              onUpdateConfig={handleUpdateAppConfig}
              onUpdateSiteSettings={handleUpdateSiteSettings}
              onTriggerGitDeploy={handleTriggerGitDeploy}
              onTestWebsiteHttp={handleTestWebsiteHttp}
              onRenewSsl={handleRenewSsl}
              onDeleteWebsite={handleDeleteWebsite}
              onOpenDeployModal={() => setIsDeployModalOpen(true)}
            />
          )}

          {activeTab === 'php' && (
            <PhpManagerView
              phpVersions={phpVersions}
              apps={apps}
              onPhpAction={handlePhpAction}
              onUpdatePhpSettings={handleUpdatePhpSettings}
              onTogglePhpExtension={handleTogglePhpExtension}
              onSwitchSitePhpVersion={handleSwitchSitePhpVersion}
            />
          )}

          {activeTab === 'ssh' && sshData && (
            <SshTerminalView
              sshData={sshData}
              onUpdateConfig={handleUpdateSshConfig}
              onAddAuthorizedKey={handleAddAuthorizedKey}
              onGenerateKeypair={handleGenerateSshKeypair}
              onDeleteKey={handleDeleteSshKey}
              onTerminateSession={handleTerminateSshSession}
              onExecCommand={handleExecWebSsh}
              onUpdatePanelAdmin={handleUpdatePanelAdmin}
              onRunFpCliAction={handleRunFpCliAction}
              onNavigate={handleNavigate}
            />
          )}

          {activeTab === 'oneclick' && (
            <OneClickAppsView
              templates={templates}
              packages={packages}
              onDeployTemplate={handleDeployOneClick}
              onPackageAction={handlePackageAction}
              onSavePackageConfig={handleSavePackageConfig}
            />
          )}

          {activeTab === 'files' && (
            <FileManagerView
              apps={apps}
              selectedAppId={selectedAppId}
              onSelectApp={setSelectedAppId}
            />
          )}

          {activeTab === 'databases' && (
            <SqlDatabaseView
              databases={databases}
              apps={apps}
              mysqlConfig={mysqlConfig}
              onCreateDatabase={handleCreateDatabase}
              onChangeDatabasePassword={handleChangeDatabasePassword}
              onDeleteDatabase={handleDeleteDatabase}
              onExecuteQuery={handleExecuteSqlQuery}
              onUpdateMysqlConfig={handleUpdateMysqlConfig}
            />
          )}

          {activeTab === 'mail' && mailData && (
            <MailServerView
              mailData={mailData}
              onAddDomain={handleAddMailDomain}
              onCreateMailbox={handleCreateMailbox}
              onDeleteMailbox={handleDeleteMailbox}
              onCreateForwarder={handleCreateForwarder}
              onDeleteForwarder={handleDeleteForwarder}
              onSendTestMail={handleSendTestMail}
            />
          )}

          {activeTab === 'optimizer' && optimizer && (
            <SystemOptimizerView
              optimizer={optimizer}
              onApplyProfile={handleApplyOptimizationProfile}
              onToggleSysctl={handleToggleSysctlParam}
              onReclaimMemory={handleReclaimMemory}
              onWatchdogHeal={handleWatchdogHeal}
              onUpdateAntiCrash={handleUpdateAntiCrash}
              onEmergencyMitigate={handleEmergencyMitigate}
            />
          )}

          {activeTab === 'cpanel' && (
            <CpanelSuiteView
              cpanel={cpanel}
              apps={apps}
              databases={databases}
              onAddCron={handleAddCron}
              onRunCronNow={handleRunCronNow}
              onToggleCron={handleToggleCron}
              onDeleteCron={handleDeleteCron}
              onAddDns={handleAddDns}
              onDeleteDns={handleDeleteDns}
              onCreateBackup={handleCreateBackup}
              onRestoreBackup={handleRestoreBackup}
              onDeleteBackup={handleDeleteBackup}
              onAddSftp={handleAddSftp}
              onDeleteSftp={handleDeleteSftp}
            />
          )}

          {activeTab === 'security' && (
            <SecurityCenterView
              security={security}
              onAddUfwRule={handleAddUfwRule}
              onToggleUfwRule={handleToggleUfwRule}
              onDeleteUfwRule={handleDeleteUfwRule}
              onBanIp={handleBanIp}
              onUnbanIp={handleUnbanIp}
              onToggleHardening={handleToggleHardening}
              onChangeDdosMode={handleChangeDdosMode}
              onToggleWafRule={handleToggleWafRule}
              onRunFimScan={handleRunFimScan}
            />
          )}

          {activeTab === 'rust-source' && <RustSourceView files={rustFiles} />}
        </main>
      </div>

      {/* Add Website (aaPanel / DirectAdmin / cPanel Style) Modal */}
      <DeployModal
        isOpen={isDeployModalOpen}
        onClose={() => setIsDeployModalOpen(false)}
        onDeploy={handleDeployNewWebsite}
      />
    </div>
  );
}
