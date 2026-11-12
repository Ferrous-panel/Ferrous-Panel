import { useState, useEffect, useCallback } from 'react';
import {
  ActiveTab,
  AntiCrashGuardState,
  CpanelData,
  DiagnosticCheckItem,
  EdgeCloudData,
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
  WasiWorkerInvokeResult,
  WebServerEngineType,
  WebSshCommandOutput,
  WebsiteLiveTestResult,
  WebsiteRuntimeType
} from '../types';
import { actixGatewayFetch } from '../services/actixGateway';

const PANEL_CACHE_KEY = 'fp_panel_state_cache_v1';

function readPanelCache<T>(field: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(PANEL_CACHE_KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw);
    return parsed && parsed[field] !== undefined ? (parsed[field] as T) : fallback;
  } catch {
    return fallback;
  }
}

export function usePanelController(
  activeTab: ActiveTab,
  setActiveTab: (tab: ActiveTab) => void
) {
  const [selectedAppId, setSelectedAppIdState] = useState<string | null>(() => {
    try {
      return localStorage.getItem('fp_selected_app_id') || null;
    } catch {
      return null;
    }
  });
  const setSelectedAppId = useCallback(
    (
      nextOrUpdater:
        | string
        | null
        | ((prevId: string | null) => string | null)
    ) => {
      setSelectedAppIdState((prevId) => {
        const nextId =
          typeof nextOrUpdater === 'function'
            ? nextOrUpdater(prevId)
            : nextOrUpdater;
        try {
          if (nextId) localStorage.setItem('fp_selected_app_id', nextId);
          else localStorage.removeItem('fp_selected_app_id');
        } catch {
          // ignore
        }
        return nextId;
      });
    },
    []
  );

  const [autoOpenSettingsCounter, setAutoOpenSettingsCounter] =
    useState<number>(0);

  const [telemetry, setTelemetry] = useState<TelemetryData | null>(null);
  const [apps, setApps] = useState<ManagedApp[]>(() =>
    readPanelCache<ManagedApp[]>('apps', [])
  );
  const [phpVersions, setPhpVersions] = useState<PhpVersionInstance[]>(() =>
    readPanelCache<PhpVersionInstance[]>('phpVersions', [])
  );
  const [templates, setTemplates] = useState<OneClickTemplate[]>(() =>
    readPanelCache<OneClickTemplate[]>('templates', [])
  );
  const [packages, setPackages] = useState<ServerPackageItem[]>(() =>
    readPanelCache<ServerPackageItem[]>('packages', [])
  );
  const [sshData, setSshData] = useState<SshPanelData | null>(() =>
    readPanelCache<SshPanelData | null>('sshData', null)
  );
  const [databases, setDatabases] = useState<SqlDatabase[]>(() =>
    readPanelCache<SqlDatabase[]>('databases', [])
  );
  const [mysqlConfig, setMysqlConfig] = useState<MysqlPhpMyAdminConfig | null>(
    () => readPanelCache<MysqlPhpMyAdminConfig | null>('mysqlConfig', null)
  );
  const [mailData, setMailData] = useState<MailServerData | null>(() =>
    readPanelCache<MailServerData | null>('mailData', null)
  );
  const [optimizer, setOptimizer] = useState<SystemOptimizerData | null>(() =>
    readPanelCache<SystemOptimizerData | null>('optimizer', null)
  );
  const [diagnostics, setDiagnostics] = useState<DiagnosticCheckItem[]>([]);
  const [cpanel, setCpanel] = useState<CpanelData | null>(() =>
    readPanelCache<CpanelData | null>('cpanel', null)
  );
  const [security, setSecurity] = useState<SecurityData | null>(() =>
    readPanelCache<SecurityData | null>('security', null)
  );
  const [edgeCloud, setEdgeCloud] = useState<EdgeCloudData | null>(() =>
    readPanelCache<EdgeCloudData | null>('edgeCloud', null)
  );
  const [rustFiles, setRustFiles] = useState<RustSourceFile[]>([]);
  const [refreshing, setRefreshing] = useState<boolean>(false);

  useEffect(() => {
    try {
      localStorage.setItem(
        PANEL_CACHE_KEY,
        JSON.stringify({
          apps,
          phpVersions,
          templates,
          packages,
          sshData,
          databases,
          mysqlConfig,
          mailData,
          optimizer,
          cpanel,
          security,
          edgeCloud
        })
      );
    } catch {
      // ignore quota errors
    }
  }, [
    apps,
    phpVersions,
    templates,
    packages,
    sshData,
    databases,
    mysqlConfig,
    mailData,
    optimizer,
    cpanel,
    security,
    edgeCloud
  ]);

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
      const results = await Promise.allSettled([
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
        fetch('/api/rust-source'),
        fetch('/api/edge-cloud')
      ]);

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
        rustRes,
        edgeRes
      ] = results;

      const clientHost =
        typeof window !== 'undefined' &&
        window.location.hostname &&
        window.location.hostname !== 'localhost' &&
        window.location.hostname !== '127.0.0.1'
          ? window.location.hostname
          : '';

      if (telRes.status === 'fulfilled' && telRes.value.ok) {
        try {
          const data = await telRes.value.json();
          if (
            clientHost &&
            (!data.publicIp ||
              data.publicIp === '103.145.88.210' ||
              data.publicIp === '127.0.0.1')
          ) {
            data.publicIp = clientHost;
          }
          setTelemetry(data);
        } catch {}
      }
      if (appsRes.status === 'fulfilled' && appsRes.value.ok) {
        try {
          const data = await appsRes.value.json();
          setApps(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (phpRes.status === 'fulfilled' && phpRes.value.ok) {
        try {
          const data = await phpRes.value.json();
          setPhpVersions(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (tplRes.status === 'fulfilled' && tplRes.value.ok) {
        try {
          const data = await tplRes.value.json();
          setTemplates(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (pkgRes.status === 'fulfilled' && pkgRes.value.ok) {
        try {
          const data = await pkgRes.value.json();
          setPackages(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (sshRes.status === 'fulfilled' && sshRes.value.ok) {
        try {
          const data = await sshRes.value.json();
          if (
            clientHost &&
            (!data.serverHost ||
              data.serverHost === '103.145.88.210' ||
              data.serverHost === '127.0.0.1')
          ) {
            data.serverHost = clientHost;
          }
          setSshData(data);
        } catch {}
      }
      if (dbRes.status === 'fulfilled' && dbRes.value.ok) {
        try {
          const data = await dbRes.value.json();
          setDatabases(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (mysqlRes.status === 'fulfilled' && mysqlRes.value.ok) {
        try {
          const data = await mysqlRes.value.json();
          setMysqlConfig(data);
        } catch {}
      }
      if (mailRes.status === 'fulfilled' && mailRes.value.ok) {
        try {
          const data = await mailRes.value.json();
          setMailData(data);
        } catch {}
      }
      if (optRes.status === 'fulfilled' && optRes.value.ok) {
        try {
          const data = await optRes.value.json();
          setOptimizer(data);
        } catch {}
      }
      if (diagRes.status === 'fulfilled' && diagRes.value.ok) {
        try {
          const data = await diagRes.value.json();
          setDiagnostics(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (cpRes.status === 'fulfilled' && cpRes.value.ok) {
        try {
          const data = await cpRes.value.json();
          setCpanel(data);
        } catch {}
      }
      if (secRes.status === 'fulfilled' && secRes.value.ok) {
        try {
          const data = await secRes.value.json();
          setSecurity(data);
        } catch {}
      }
      if (rustRes.status === 'fulfilled' && rustRes.value.ok) {
        try {
          const data = await rustRes.value.json();
          setRustFiles(Array.isArray(data) ? data : []);
        } catch {}
      }
      if (edgeRes && edgeRes.status === 'fulfilled' && edgeRes.value.ok) {
        try {
          const data = await edgeRes.value.json();
          setEdgeCloud(data);
        } catch {}
      }
    } catch (err) {
      console.error('[FerrousPanel] fetchAllData failed:', err);
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
    try {
      const res = await actixGatewayFetch(
        'SYSTEMD',
        `/api/apps/${appId}/action`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ action })
        }
      );
      if (res.ok) {
        const updated: ManagedApp = await res.json();
        setApps((prev) =>
          prev.map((a) => (a.id === updated.id ? updated : a))
        );
        fetchTelemetry();
      }
    } catch (err) {
      console.error(`[FerrousPanel] handleAppAction(${action}) error:`, err);
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
    try {
      const res = await actixGatewayFetch('NGINX', `/api/apps/${appId}/config`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        const updated: ManagedApp = await res.json();
        setApps((prev) =>
          prev.map((a) => (a.id === updated.id ? updated : a))
        );
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdateAppConfig error:', err);
    }
  };

  // Nginx / FerrousX VHost Rewrite, Redirects, SSL & Directory Auth via Actix-Web Gateway
  const handleUpdateSiteSettings = async (
    appId: string,
    payload: Partial<ManagedApp>
  ) => {
    try {
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
        setApps((prev) =>
          prev.map((a) => (a.id === updated.id ? updated : a))
        );
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdateSiteSettings error:', err);
    }
  };

  const handleTriggerGitDeploy = async (appId: string) => {
    try {
      const res = await actixGatewayFetch(
        'SYSTEMD',
        `/api/apps/${appId}/git-deploy`,
        {
          method: 'POST'
        }
      );
      if (res.ok) {
        const updated: ManagedApp = await res.json();
        setApps((prev) =>
          prev.map((a) => (a.id === updated.id ? updated : a))
        );
      }
    } catch (err) {
      console.error('[FerrousPanel] handleTriggerGitDeploy error:', err);
    }
  };

  const handleTestWebsiteHttp = async (
    appId: string
  ): Promise<WebsiteLiveTestResult | null> => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleTestWebsiteHttp error:', err);
    }
    return null;
  };

  const handleRenewSsl = async (appId: string) => {
    try {
      const res = await actixGatewayFetch(
        'NGINX',
        `/api/apps/${appId}/renew-ssl`,
        {
          method: 'POST'
        }
      );
      if (res.ok) {
        const updated: ManagedApp = await res.json();
        setApps((prev) =>
          prev.map((a) => (a.id === updated.id ? updated : a))
        );
      }
    } catch (err) {
      console.error('[FerrousPanel] handleRenewSsl error:', err);
    }
  };

  const handleDeleteWebsite = async (appId: string) => {
    try {
      const res = await actixGatewayFetch('NGINX', `/api/apps/${appId}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const remaining: ManagedApp[] = await res.json();
        const safeRemaining = Array.isArray(remaining) ? remaining : [];
        setApps(safeRemaining);
        setSelectedAppId((prevId) => {
          if (!prevId || prevId === appId) {
            return safeRemaining.length > 0 ? safeRemaining[0].id : null;
          }
          return safeRemaining.some((a) => a.id === prevId)
            ? prevId
            : safeRemaining.length > 0
              ? safeRemaining[0].id
              : null;
        });
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteWebsite error:', err);
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
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleDeployNewWebsite error:', err);
    }
  };

  const handleDeployOneClick = async (payload: {
    templateId: string;
    domain: string;
    createDb: boolean;
    webServerEngine?: WebServerEngineType;
  }) => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleDeployOneClick error:', err);
    }
  };

  const handlePackageAction = async (
    pkgId: string,
    action: 'start' | 'stop' | 'restart' | 'install' | 'uninstall'
  ) => {
    try {
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
        const pkgs = await res.json();
        setPackages(Array.isArray(pkgs) ? pkgs : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handlePackageAction error:', err);
    }
  };

  const handleSavePackageConfig = async (
    pkgId: string,
    configContent: string
  ) => {
    try {
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
        const pkgs = await res.json();
        setPackages(Array.isArray(pkgs) ? pkgs : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handleSavePackageConfig error:', err);
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
    try {
      const res = await fetch('/api/databases', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        const dbs = await res.json();
        setDatabases(Array.isArray(dbs) ? dbs : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handleCreateDatabase error:', err);
    }
  };

  const handleChangeDatabasePassword = async (
    id: string,
    newPassword: string
  ) => {
    try {
      const res = await fetch(`/api/databases/${id}/password`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password: newPassword })
      });
      if (res.ok) {
        const dbs = await res.json();
        setDatabases(Array.isArray(dbs) ? dbs : []);
        fetchAllData();
      }
    } catch (err) {
      console.error('[FerrousPanel] handleChangeDatabasePassword error:', err);
    }
  };

  const handleDeleteDatabase = async (id: string) => {
    try {
      const res = await fetch(`/api/databases/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const dbs = await res.json();
        setDatabases(Array.isArray(dbs) ? dbs : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteDatabase error:', err);
    }
  };

  const handleExecuteSqlQuery = async (
    dbId: string,
    query: string
  ): Promise<SqlQueryResponse> => {
    try {
      const res = await fetch(`/api/databases/${dbId}/query`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query })
      });
      const data: SqlQueryResponse = await res.json();
      const dbRes = await fetch('/api/databases');
      if (dbRes.ok) {
        const dbs = await dbRes.json();
        setDatabases(Array.isArray(dbs) ? dbs : []);
      }
      return data;
    } catch (err) {
      console.error('[FerrousPanel] handleExecuteSqlQuery error:', err);
      return {
        success: false,
        commandTag: 'ERROR',
        executionTimeMs: 0,
        affectedRows: 0,
        columns: ['error'],
        rows: [{ error: 'Failed to execute query: Network error or server timeout' }],
        error: String(err)
      };
    }
  };

  const handleUpdateMysqlConfig = async (
    payload: Partial<MysqlPhpMyAdminConfig>
  ) => {
    try {
      const res = await fetch('/api/databases/mysql-config', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setMysqlConfig(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdateMysqlConfig error:', err);
    }
  };

  // Mail Server Handlers
  const handleAddMailDomain = async (domain: string) => {
    try {
      const res = await fetch('/api/mail/domains', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ domain })
      });
      if (res.ok) {
        setMailData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddMailDomain error:', err);
    }
  };

  const handleCreateMailbox = async (payload: {
    email: string;
    domain: string;
    displayName: string;
    quotaMb: number;
  }) => {
    try {
      const res = await fetch('/api/mail/mailboxes', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setMailData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleCreateMailbox error:', err);
    }
  };

  const handleDeleteMailbox = async (id: string) => {
    try {
      const res = await fetch(`/api/mail/mailboxes/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setMailData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteMailbox error:', err);
    }
  };

  const handleCreateForwarder = async (payload: {
    sourceEmail: string;
    destinationEmail: string;
    domain: string;
  }) => {
    try {
      const res = await fetch('/api/mail/forwarders', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setMailData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleCreateForwarder error:', err);
    }
  };

  const handleDeleteForwarder = async (id: string) => {
    try {
      const res = await fetch(`/api/mail/forwarders/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setMailData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteForwarder error:', err);
    }
  };

  const handleSendTestMail = async (payload: {
    from: string;
    to: string;
    subject: string;
    body: string;
  }) => {
    try {
      const res = await fetch('/api/mail/send', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setMailData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleSendTestMail error:', err);
    }
  };

  // System Optimizer & Backend Watchdog Handlers via Actix-Web Gateway -> ferrous-system-core
  const handleApplyOptimizationProfile = async (
    profileId: OptimizationProfilePreset['id']
  ) => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleApplyOptimizationProfile error:', err);
    }
  };

  const handleToggleSysctlParam = async (id: string) => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleToggleSysctlParam error:', err);
    }
  };

  const handleReclaimMemory = async (): Promise<{
    freedMb: number;
    message: string;
  }> => {
    try {
      const res = await actixGatewayFetch(
        'SYSCTL',
        '/api/optimizer/reclaim-memory',
        {
          method: 'POST'
        }
      );
      if (res.ok) {
        const data = await res.json();
        if (data.optimizer) setOptimizer(data.optimizer);
        fetchTelemetry();
        return { freedMb: data.freedMb, message: data.message };
      }
    } catch (err) {
      console.error('[FerrousPanel] handleReclaimMemory error:', err);
    }
    return { freedMb: 0, message: 'Memory reclaim completed' };
  };

  const handleWatchdogHeal = async () => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleWatchdogHeal error:', err);
    }
  };

  const handleUpdateAntiCrash = async (
    payload: Partial<AntiCrashGuardState>
  ) => {
    try {
      const res = await actixGatewayFetch('SYSCTL', '/api/optimizer/anti-crash', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setOptimizer(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdateAntiCrash error:', err);
    }
  };

  const handleEmergencyMitigate = async (
    action:
      | 'purge_syn_backlog'
      | 'isolate_leaking_workers'
      | 'reset_hung_db_queries'
  ): Promise<{ message?: string } | void> => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleEmergencyMitigate error:', err);
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
    try {
      const res = await fetch('/api/cpanel/cron', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        const cronJobs = await res.json();
        setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddCron error:', err);
    }
  };

  const handleRunCronNow = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/cron/${id}/run`, {
        method: 'POST'
      });
      if (res.ok) {
        const cronJobs = await res.json();
        setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleRunCronNow error:', err);
    }
  };

  const handleToggleCron = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/cron/${id}/toggle`, {
        method: 'PUT'
      });
      if (res.ok) {
        const cronJobs = await res.json();
        setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleToggleCron error:', err);
    }
  };

  const handleDeleteCron = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/cron/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const cronJobs = await res.json();
        setCpanel((prev) => (prev ? { ...prev, cronJobs } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteCron error:', err);
    }
  };

  const handleAddDns = async (payload: {
    domain: string;
    type: 'A' | 'AAAA' | 'CNAME' | 'MX' | 'TXT' | 'CAA';
    host: string;
    value: string;
    ttl: number;
  }) => {
    try {
      const res = await fetch('/api/cpanel/dns', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        const dnsRecords = await res.json();
        setCpanel((prev) => (prev ? { ...prev, dnsRecords } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddDns error:', err);
    }
  };

  const handleDeleteDns = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/dns/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const dnsRecords = await res.json();
        setCpanel((prev) => (prev ? { ...prev, dnsRecords } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteDns error:', err);
    }
  };

  const handleCreateBackup = async (payload: {
    targetApp: string;
    databaseIncluded: string;
  }) => {
    try {
      const res = await fetch('/api/cpanel/backups', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        const backups = await res.json();
        setCpanel((prev) => (prev ? { ...prev, backups } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleCreateBackup error:', err);
    }
  };

  const handleRestoreBackup = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/backups/${id}/restore`, {
        method: 'POST'
      });
      if (res.ok) {
        const backups = await res.json();
        setCpanel((prev) => (prev ? { ...prev, backups } : prev));
        fetchAllData();
      }
    } catch (err) {
      console.error('[FerrousPanel] handleRestoreBackup error:', err);
    }
  };

  const handleDeleteBackup = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/backups/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const backups = await res.json();
        setCpanel((prev) => (prev ? { ...prev, backups } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteBackup error:', err);
    }
  };

  const handleAddSftp = async (payload: {
    username: string;
    targetApp: string;
    authMethod: 'Ed25519 SSH Key' | 'Argon2id Password';
    quotaMb: number;
  }) => {
    try {
      const res = await fetch('/api/cpanel/sftp', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        const sftpAccounts = await res.json();
        setCpanel((prev) => (prev ? { ...prev, sftpAccounts } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddSftp error:', err);
    }
  };

  const handleDeleteSftp = async (id: string) => {
    try {
      const res = await fetch(`/api/cpanel/sftp/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const sftpAccounts = await res.json();
        setCpanel((prev) => (prev ? { ...prev, sftpAccounts } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteSftp error:', err);
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
    try {
      const res = await actixGatewayFetch('UFW', '/api/security/ufw', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(rule)
      });
      if (res.ok) {
        const ufwRules = await res.json();
        setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddUfwRule error:', err);
    }
  };

  const handleToggleUfwRule = async (id: string) => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleToggleUfwRule error:', err);
    }
  };

  const handleDeleteUfwRule = async (id: string) => {
    try {
      const res = await actixGatewayFetch('UFW', `/api/security/ufw/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        const ufwRules = await res.json();
        setSecurity((prev) => (prev ? { ...prev, ufwRules } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteUfwRule error:', err);
    }
  };

  const handleBanIp = async (jailName: string, ip: string, reason: string) => {
    try {
      const res = await actixGatewayFetch('UFW', '/api/security/fail2ban/ban', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jailName, ip, reason })
      });
      if (res.ok) {
        const fail2banJails = await res.json();
        setSecurity((prev) => (prev ? { ...prev, fail2banJails } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleBanIp error:', err);
    }
  };

  const handleUnbanIp = async (jailName: string, ip: string) => {
    try {
      const res = await actixGatewayFetch('UFW', '/api/security/fail2ban/unban', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jailName, ip })
      });
      if (res.ok) {
        const fail2banJails = await res.json();
        setSecurity((prev) => (prev ? { ...prev, fail2banJails } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUnbanIp error:', err);
    }
  };

  const handleToggleHardening = async (id: string) => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleToggleHardening error:', err);
    }
  };

  const handleChangeDdosMode = async (
    mode: 'normal' | 'elevated' | 'under_attack'
  ) => {
    try {
      const res = await actixGatewayFetch('UFW', '/api/security/ddos-mode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode })
      });
      if (res.ok) {
        const antiDdos = await res.json();
        setSecurity((prev) => (prev ? { ...prev, antiDdos } : prev));
      }
    } catch (err) {
      console.error('[FerrousPanel] handleChangeDdosMode error:', err);
    }
  };

  const handleToggleWafRule = async (id: string) => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleToggleWafRule error:', err);
    }
  };

  const handleRunFimScan = async (): Promise<void> => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleRunFimScan error:', err);
    }
  };

  // SSH Terminal & External Client Handlers via Actix-Web Gateway -> ferrous-system-core
  const handleUpdateSshConfig = async (payload: Partial<SshDaemonConfig>) => {
    try {
      const res = await actixGatewayFetch('SYSCALL', '/api/ssh/config', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setSshData(await res.json());
        fetchAllData();
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdateSshConfig error:', err);
    }
  };

  const handleAddAuthorizedKey = async (payload: {
    label: string;
    publicKey: string;
    clientHint: string;
  }) => {
    try {
      const res = await actixGatewayFetch('SYSCALL', '/api/ssh/keys', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setSshData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddAuthorizedKey error:', err);
    }
  };

  const handleGenerateSshKeypair = async (payload: {
    label: string;
    clientHint: string;
  }): Promise<SshGeneratedKeypair | null> => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleGenerateSshKeypair error:', err);
    }
    return null;
  };

  const handleDeleteSshKey = async (id: string) => {
    try {
      const res = await actixGatewayFetch('SYSCALL', `/api/ssh/keys/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setSshData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteSshKey error:', err);
    }
  };

  const handleTerminateSshSession = async (id: string) => {
    try {
      const res = await actixGatewayFetch('SYSCALL', `/api/ssh/sessions/${id}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setSshData(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleTerminateSshSession error:', err);
    }
  };

  const handleExecWebSsh = async (
    command: string,
    cwd: string
  ): Promise<WebSshCommandOutput> => {
    try {
      const res = await actixGatewayFetch('SYSCALL', '/api/ssh/exec', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ command, cwd })
      });
      if (res.ok) {
        return await res.json();
      }
    } catch (err) {
      console.error('[FerrousPanel] handleExecWebSsh error:', err);
    }
    return {
      id: `ssh_out_${Date.now()}`,
      cwd,
      command,
      output: 'Failed to communicate with SSH terminal daemon',
      exitCode: 1,
      timestamp: new Date().toLocaleTimeString()
    };
  };

  const handleUpdatePanelAdmin = async (
    payload: Partial<PanelAdminAccount>
  ) => {
    try {
      const res = await actixGatewayFetch('SYSCALL', '/api/panel-admin', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        await fetchAllData();
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdatePanelAdmin error:', err);
    }
  };

  const handleRunFpCliAction = async (
    option: number,
    value?: string
  ): Promise<{ output: string; panelAdmin: PanelAdminAccount } | null> => {
    try {
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
    } catch (err) {
      console.error('[FerrousPanel] handleRunFpCliAction error:', err);
    }
    return null;
  };

  // Multi-PHP 8.4 / 8.3 / 8.2 / 8.1 / 7.4 FPM Pool Handlers via Actix-Web Gateway -> ferrous-system-core
  const handlePhpAction = async (
    version: string,
    action: 'start' | 'stop' | 'restart' | 'reload' | 'install' | 'uninstall'
  ) => {
    try {
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
        const list = await res.json();
        setPhpVersions(Array.isArray(list) ? list : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handlePhpAction error:', err);
    }
  };

  const handleUpdatePhpSettings = async (
    version: string,
    payload: Partial<PhpVersionInstance>
  ) => {
    try {
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
        const list = await res.json();
        setPhpVersions(Array.isArray(list) ? list : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handleUpdatePhpSettings error:', err);
    }
  };

  const handleTogglePhpExtension = async (version: string, extName: string) => {
    try {
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
        const list = await res.json();
        setPhpVersions(Array.isArray(list) ? list : []);
      }
    } catch (err) {
      console.error('[FerrousPanel] handleTogglePhpExtension error:', err);
    }
  };

  const handleSwitchSitePhpVersion = async (
    appId: string,
    phpVersion: string
  ) => {
    try {
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
        if (data.phpVersions && Array.isArray(data.phpVersions)) {
          setPhpVersions(data.phpVersions);
        }
      }
    } catch (err) {
      console.error('[FerrousPanel] handleSwitchSitePhpVersion error:', err);
    }
  };

  const handleDeployWasiWorker = async (payload: {
    name: string;
    routePattern: string;
    targetDomain: string;
    wasiTarget: 'wasm32-wasip2' | 'wasm32-wasip1';
    memoryLimitKb: number;
    fuelLimit: number;
    kvNamespace: string;
    rustSourceSnippet: string;
  }) => {
    try {
      const res = await fetch('/api/edge-cloud/workers', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeployWasiWorker error:', err);
    }
  };

  const handleInvokeWasiWorker = async (
    workerId: string
  ): Promise<WasiWorkerInvokeResult | null> => {
    try {
      const res = await fetch(`/api/edge-cloud/workers/${workerId}/invoke`, {
        method: 'POST'
      });
      if (res.ok) {
        const data = await res.json();
        if (data.edgeCloud) setEdgeCloud(data.edgeCloud);
        return data.result || null;
      }
    } catch (err) {
      console.error('[FerrousPanel] handleInvokeWasiWorker error:', err);
    }
    return null;
  };

  const handleToggleWasiWorker = async (workerId: string) => {
    try {
      const res = await fetch(`/api/edge-cloud/workers/${workerId}/toggle`, {
        method: 'POST'
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleToggleWasiWorker error:', err);
    }
  };

  const handleDeleteWasiWorker = async (workerId: string) => {
    try {
      const res = await fetch(`/api/edge-cloud/workers/${workerId}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteWasiWorker error:', err);
    }
  };

  const handleDeployOciContainer = async (payload: {
    name: string;
    image: string;
    runtimeEngine: 'Youki OCI (Rust Rootless)' | 'Firecracker MicroVM (Rust KVM)';
    hostPort: number;
    containerPort: number;
    memoryMaxMb: number;
    cpuQuotaPercent: number;
  }) => {
    try {
      const res = await fetch('/api/edge-cloud/containers', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeployOciContainer error:', err);
    }
  };

  const handleOciContainerAction = async (
    containerId: string,
    action: 'start' | 'stop' | 'restart' | 'delete'
  ) => {
    try {
      const res = await fetch(
        `/api/edge-cloud/containers/${containerId}/action`,
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ action })
        }
      );
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleOciContainerAction error:', err);
    }
  };

  const handleAddWireGuardPeer = async (payload: {
    name: string;
    region: string;
    endpoint: string;
    allowedIps: string;
  }) => {
    try {
      const res = await fetch('/api/edge-cloud/wireguard', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleAddWireGuardPeer error:', err);
    }
  };

  const handleToggleWireGuardPeer = async (peerId: string) => {
    try {
      const res = await fetch(`/api/edge-cloud/wireguard/${peerId}/toggle`, {
        method: 'POST'
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleToggleWireGuardPeer error:', err);
    }
  };

  const handleDeleteWireGuardPeer = async (peerId: string) => {
    try {
      const res = await fetch(`/api/edge-cloud/wireguard/${peerId}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteWireGuardPeer error:', err);
    }
  };

  const handleCreateBlake3Snapshot = async (payload: {
    snapshotName: string;
    targetResource: string;
    s3Endpoint: string;
    bucket: string;
    immutableLock: boolean;
  }) => {
    try {
      const res = await fetch('/api/edge-cloud/snapshots', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleCreateBlake3Snapshot error:', err);
    }
  };

  const handleDeleteBlake3Snapshot = async (snapshotId: string) => {
    try {
      const res = await fetch(`/api/edge-cloud/snapshots/${snapshotId}`, {
        method: 'DELETE'
      });
      if (res.ok) {
        setEdgeCloud(await res.json());
      }
    } catch (err) {
      console.error('[FerrousPanel] handleDeleteBlake3Snapshot error:', err);
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
    edgeCloud,
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
    handleSwitchSitePhpVersion,
    handleDeployWasiWorker,
    handleInvokeWasiWorker,
    handleToggleWasiWorker,
    handleDeleteWasiWorker,
    handleDeployOciContainer,
    handleOciContainerAction,
    handleAddWireGuardPeer,
    handleToggleWireGuardPeer,
    handleDeleteWireGuardPeer,
    handleCreateBlake3Snapshot,
    handleDeleteBlake3Snapshot
  };
}
