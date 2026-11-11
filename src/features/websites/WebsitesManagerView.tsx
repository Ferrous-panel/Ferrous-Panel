import React, { useState, useEffect } from 'react';
import {
  CheckCircle2,
  Database,
  ExternalLink,
  FileCode2,
  FolderOpen,
  GitBranch,
  Globe,
  Lock,
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  Save,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Square,
  Terminal,
  Trash2,
  X
} from 'lucide-react';
import {
  ActiveTab,
  DirectoryPrivacyItem,
  EnvVar,
  HotlinkProtectionConfig,
  ManagedApp,
  RedirectRuleItem,
  RewritePresetType,
  WebServerEngineType,
  WebsiteLiveTestResult
} from '../types';

interface WebsitesManagerViewProps {
  apps: ManagedApp[];
  selectedAppId: string | null;
  autoOpenSettingsCounter?: number;
  onSelectApp: (id: string) => void;
  onNavigate: (tab: ActiveTab, appId?: string) => void;
  onAppAction: (
    appId: string,
    action: 'start' | 'stop' | 'restart' | 'rebuild'
  ) => Promise<void>;
  onUpdateConfig: (
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
  ) => Promise<void>;
  onUpdateSiteSettings: (
    appId: string,
    payload: Partial<ManagedApp>
  ) => Promise<void>;
  onTriggerGitDeploy: (appId: string) => Promise<void>;
  onTestWebsiteHttp: (appId: string) => Promise<WebsiteLiveTestResult | null>;
  onRenewSsl: (appId: string) => Promise<void>;
  onDeleteWebsite: (appId: string) => Promise<void>;
  onOpenDeployModal: () => void;
}

const REWRITE_PRESET_LABELS: { id: RewritePresetType; label: string }[] = [
  { id: 'rust_proxy', label: 'Actix-Web / Axum / Rocket Upstream Proxy' },
  {
    id: 'nextjs_proxy',
    label: 'Next.js 15 App Router SSR + _next/static Cache'
  },
  { id: 'nuxt_nitro', label: 'Nuxt 3 (Vue 3 Nitro Engine + _nuxt Cache)' },
  {
    id: 'sveltekit_node',
    label: 'SvelteKit 2 (Svelte 5 Adapter-Node + _app/immutable)'
  },
  {
    id: 'astro_hybrid',
    label: 'Astro 5 (Hybrid SSR + Static _astro Assets)'
  },
  { id: 'laravel', label: 'Laravel 11 (try_files -> /index.php?$query_string)' },
  { id: 'wordpress', label: 'WordPress 6.7 Permalinks Rewrite' },
  { id: 'spa_history', label: 'React / Vue / Vite SPA HTML5 History Fallback' }
];

const CLIENT_REWRITE_SNIPPETS: Record<RewritePresetType, string> = {
  rust_proxy: `location / {
    limit_req zone=rust_zone burst=50 nodelay;
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  nextjs_proxy: `location /_next/static/ {
    alias $WORKING_DIR/.next/static/;
    expires 365d;
    access_log off;
    add_header Cache-Control "public, max-age=31536000, immutable";
}

location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  nuxt_nitro: `location /_nuxt/ {
    alias $WORKING_DIR/.output/public/_nuxt/;
    expires 365d;
    access_log off;
    add_header Cache-Control "public, max-age=31536000, immutable";
}

location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  sveltekit_node: `location /_app/immutable/ {
    alias $WORKING_DIR/build/client/_app/immutable/;
    expires 365d;
    access_log off;
    add_header Cache-Control "public, max-age=31536000, immutable";
}

location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  astro_hybrid: `location /_astro/ {
    alias $WORKING_DIR/dist/client/_astro/;
    expires 365d;
    access_log off;
    add_header Cache-Control "public, max-age=31536000, immutable";
}

location / {
    proxy_pass http://127.0.0.1:$INTERNAL_PORT;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
}`,
  laravel: `location / {
    try_files $uri $uri/ /index.php?$query_string;
}

location ~ \\.php$ {
    include snippets/fastcgi-php.conf;
    fastcgi_pass unix:/run/php/php8.3-fpm.sock;
    fastcgi_param SCRIPT_FILENAME $realpath_root$fastcgi_script_name;
    include fastcgi_params;
}`,
  wordpress: `location / {
    try_files $uri $uri/ /index.php?$args;
}

location ~ \\.php$ {
    include snippets/fastcgi-php.conf;
    fastcgi_pass unix:/run/php/php8.3-fpm.sock;
}`,
  spa_history: `location / {
    try_files $uri $uri/ /index.html;
}`
};

type SiteModalTab =
  | 'domain_dir'
  | 'php_select'
  | 'redirects_hotlink'
  | 'rewrite_proxy'
  | 'ssl'
  | 'git_deploy'
  | 'runtime_env'
  | 'nginx_conf'
  | 'logs';

export const WebsitesManagerView: React.FC<WebsitesManagerViewProps> = ({
  apps,
  selectedAppId,
  autoOpenSettingsCounter,
  onSelectApp,
  onNavigate,
  onAppAction,
  onUpdateConfig,
  onUpdateSiteSettings,
  onTriggerGitDeploy,
  onTestWebsiteHttp,
  onRenewSsl,
  onDeleteWebsite,
  onOpenDeployModal
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [runtimeCategory, setRuntimeCategory] = useState<
    'ALL' | 'MODERN_JS' | 'RUST' | 'PHP' | 'POLYGLOT'
  >('ALL');
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [modalTab, setModalTab] = useState<SiteModalTab>('domain_dir');
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [statusToast, setStatusToast] = useState<string | null>(null);

  // Live Website Test Modal State
  const [liveTestResult, setLiveTestResult] =
    useState<WebsiteLiveTestResult | null>(null);
  const [testingSiteId, setTestingSiteId] = useState<string | null>(null);

  const selectedApp =
    apps.find((a) => a.id === selectedAppId) || apps[0] || null;

  // Editable states inside Site Configuration Modal
  const [aliasesText, setAliasesText] = useState('');
  const [runSubDir, setRunSubDir] = useState('/');
  const [webServerEngine, setWebServerEngine] = useState<WebServerEngineType>(
    'FerrousX (Rust Web Server)'
  );
  const [phpVersion, setPhpVersion] = useState<
    '8.4' | '8.3' | '8.2' | '8.1' | '7.4' | 'None'
  >('None');
  const [redirects, setRedirects] = useState<RedirectRuleItem[]>([]);
  const [newRedirectSource, setNewRedirectSource] = useState('/old-url');
  const [newRedirectTarget, setNewRedirectTarget] = useState(
    'https://domain.vn/new-url'
  );
  const [newRedirectType, setNewRedirectType] = useState<'301' | '302'>('301');
  const [hotlinkConfig, setHotlinkConfig] = useState<HotlinkProtectionConfig>({
    enabled: false,
    protectedExtensions: 'jpg|jpeg|png|gif|webp|svg|mp4|zip',
    allowedDomains: [],
    blockEmptyReferer: false
  });
  const [directoryAuth, setDirectoryAuth] = useState<DirectoryPrivacyItem[]>(
    []
  );
  const [newAuthPath, setNewAuthPath] = useState('/admin');
  const [newAuthRealm, setNewAuthRealm] = useState('Restricted Admin Area');
  const [newAuthUser, setNewAuthUser] = useState('admin');
  const [forceHttps, setForceHttps] = useState(true);
  const [sslEnabled, setSslEnabled] = useState(true);
  const [http3Quic, setHttp3Quic] = useState(true);
  const [websocketEnabled, setWebsocketEnabled] = useState(true);
  const [rateLimitRps, setRateLimitRps] = useState(120);
  const [workers, setWorkers] = useState(4);
  const [memoryLimitMb, setMemoryLimitMb] = useState(512);
  const [rewritePreset, setRewritePreset] =
    useState<RewritePresetType>('rust_proxy');
  const [rewriteRules, setRewriteRules] = useState('');
  const [gitRepoUrl, setGitRepoUrl] = useState('');
  const [gitBranch, setGitBranch] = useState('main');
  const [buildScript, setBuildScript] = useState('');
  const [envVars, setEnvVars] = useState<EnvVar[]>([]);
  const [nginxRaw, setNginxRaw] = useState('');
  const [validationResult, setValidationResult] = useState<{
    valid: boolean;
    output: string;
  } | null>(null);

  useEffect(() => {
    if (selectedApp) {
      setAliasesText((selectedApp.aliases || []).join('\n'));
      setRunSubDir(selectedApp.runSubDir || '/');
      setWebServerEngine(
        selectedApp.webServerEngine || 'FerrousX (Rust Web Server)'
      );
      setPhpVersion(
        selectedApp.phpVersion ||
          (selectedApp.framework.includes('PHP') ? '8.3' : 'None')
      );
      setRedirects(selectedApp.redirects || []);
      setHotlinkConfig(
        selectedApp.hotlinkProtection || {
          enabled: false,
          protectedExtensions: 'jpg|jpeg|png|gif|webp|svg|mp4|zip',
          allowedDomains: [selectedApp.domain, `www.${selectedApp.domain}`],
          blockEmptyReferer: false
        }
      );
      setDirectoryAuth(selectedApp.directoryAuth || []);
      setForceHttps(selectedApp.forceHttps ?? true);
      setSslEnabled(selectedApp.sslEnabled ?? true);
      setHttp3Quic(selectedApp.http3Quic ?? true);
      setWebsocketEnabled(selectedApp.websocketEnabled ?? true);
      setRateLimitRps(selectedApp.rateLimitRps || 120);
      setWorkers(selectedApp.workers || 4);
      setMemoryLimitMb(selectedApp.memoryLimitMb || 512);
      setRewritePreset(selectedApp.urlRewritePreset || 'rust_proxy');
      setRewriteRules(selectedApp.urlRewriteRules || '');
      setGitRepoUrl(selectedApp.gitRepoUrl || '');
      setGitBranch(selectedApp.gitBranch || 'main');
      setBuildScript(selectedApp.buildScript || '');
      setEnvVars(selectedApp.envVars.map((e) => ({ ...e })));
      setNginxRaw(selectedApp.nginxVhost || '');
      setValidationResult(null);
    }
  }, [selectedApp]);

  useEffect(() => {
    if (autoOpenSettingsCounter && autoOpenSettingsCounter > 0 && selectedApp) {
      setIsSettingsOpen(true);
    }
  }, [autoOpenSettingsCounter, selectedApp]);

  const isModernJsFramework = (fw: string) =>
    fw.includes('Next.js') ||
    fw.includes('Nuxt') ||
    fw.includes('SvelteKit') ||
    fw.includes('Astro') ||
    fw.includes('Remix') ||
    fw.includes('NestJS') ||
    fw.includes('Vite');

  const isPolyglotFramework = (fw: string) =>
    fw.includes('Python') ||
    fw.includes('Go') ||
    fw.includes('Java') ||
    fw.includes('C#') ||
    fw.includes('.NET') ||
    fw.includes('Ruby') ||
    fw.includes('C++') ||
    fw.includes('WebAssembly');

  const filteredApps = apps.filter((a) => {
    const matchesSearch =
      a.domain.toLowerCase().includes(searchQuery.toLowerCase()) ||
      a.framework.toLowerCase().includes(searchQuery.toLowerCase()) ||
      a.workingDir.toLowerCase().includes(searchQuery.toLowerCase());

    if (!matchesSearch) return false;
    if (runtimeCategory === 'MODERN_JS')
      return isModernJsFramework(a.framework);
    if (runtimeCategory === 'RUST') return a.framework.includes('Rust');
    if (runtimeCategory === 'PHP') return a.framework.includes('PHP');
    if (runtimeCategory === 'POLYGLOT') return isPolyglotFramework(a.framework);
    return true;
  });

  const openSiteModal = (app: ManagedApp, defaultTab: SiteModalTab) => {
    onSelectApp(app.id);
    setModalTab(defaultTab);
    setIsSettingsOpen(true);
  };

  const handleRunLiveHttpTest = async (site: ManagedApp) => {
    setTestingSiteId(site.id);
    try {
      const result = await onTestWebsiteHttp(site.id);
      if (result) {
        setLiveTestResult(result);
      }
    } finally {
      setTestingSiteId(null);
    }
  };

  const handlePresetChange = (preset: RewritePresetType) => {
    setRewritePreset(preset);
    setRewriteRules(CLIENT_REWRITE_SNIPPETS[preset] || '');
  };

  const handleSaveSiteSettings = async () => {
    if (!selectedApp) return;
    setBusyAction('save');
    try {
      await onUpdateSiteSettings(selectedApp.id, {
        aliases: aliasesText
          .split('\n')
          .map((s) => s.trim())
          .filter(Boolean),
        runSubDir,
        webServerEngine,
        phpVersion,
        redirects,
        hotlinkProtection: hotlinkConfig,
        directoryAuth,
        forceHttps,
        sslEnabled,
        http3Quic,
        websocketEnabled,
        rateLimitRps,
        urlRewritePreset: rewritePreset,
        urlRewriteRules: rewriteRules,
        gitRepoUrl,
        gitBranch,
        buildScript,
        nginxVhost: nginxRaw
      });
      await onUpdateConfig(selectedApp.id, {
        workers,
        memoryLimitMb,
        rateLimitRps,
        sslEnabled,
        http3Quic,
        websocketEnabled,
        envVars: envVars.filter((e) => e.key.trim().length > 0)
      });
      setStatusToast(
        `Đã lưu cấu hình Website ${selectedApp.domain} (Web Server: ${webServerEngine} · PHP: ${phpVersion}) & nạp lại cấu hình thành công!`
      );
      setTimeout(() => setStatusToast(null), 4000);
    } finally {
      setBusyAction(null);
    }
  };

  const handleRenewSslClick = async () => {
    if (!selectedApp) return;
    setBusyAction('renew-ssl');
    try {
      await onRenewSsl(selectedApp.id);
      setStatusToast(
        `FerrousACME (Rust): Đã cấp mới chứng chỉ Let's Encrypt ECDSA P-384 (90 ngày) cho ${selectedApp.domain}`
      );
      setTimeout(() => setStatusToast(null), 4000);
    } finally {
      setBusyAction(null);
    }
  };

  const handleDeleteCurrentSite = async () => {
    if (!selectedApp) return;
    setBusyAction('delete-site');
    try {
      const deletedDomain = selectedApp.domain;
      await onDeleteWebsite(selectedApp.id);
      setIsSettingsOpen(false);
      setStatusToast(`Đã xóa Website ${deletedDomain} khỏi hệ thống.`);
      setTimeout(() => setStatusToast(null), 4000);
    } finally {
      setBusyAction(null);
    }
  };

  const handleValidateNginx = async () => {
    if (!selectedApp) return;
    setBusyAction('validate');
    try {
      const res = await fetch('/api/nginx/validate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain: selectedApp.domain,
          configContent: nginxRaw,
          webServerEngine
        })
      });
      if (res.ok) {
        setValidationResult(await res.json());
      }
    } finally {
      setBusyAction(null);
    }
  };

  const handleGitPullDeploy = async () => {
    if (!selectedApp) return;
    setBusyAction('git');
    try {
      await onTriggerGitDeploy(selectedApp.id);
      setStatusToast(
        `Git Atomic Deploy hoàn tất cho ${selectedApp.domain} (Zero-Downtime Reload)`
      );
      setTimeout(() => setStatusToast(null), 4000);
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <div className="space-y-6">
      {/* Top Toolbar (aaPanel / DirectAdmin / cPanel Website Table Header) */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="space-y-1">
            <h1 className="text-lg font-bold tracking-tight text-slate-900 dark:text-slate-100 font-display">
              Quản lý Websites &amp; Đa Ngôn Ngữ Chạy Trên Nhân Rust ({apps.length}{' '}
              Sites)
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Vận hành đồng thời 10 hệ sinh thái (Rust Actix/Axum, Next.js 15, Nuxt 3, SvelteKit 2,
              PHP 7.4–8.4, Python PyO3, Go CGO, Java JNI, .NET 8, Ruby/Elixir, C++, WASI) qua cầu nối <code className="font-mono">libferrous_bridge.so</code>
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2.5">
            <div className="relative">
              <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Tìm tên miền, framework..."
                className="pl-8 pr-3 py-2 text-xs rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100 w-56"
              />
            </div>

            <button
              onClick={onOpenDeployModal}
              className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              Thêm Website Mới
            </button>
          </div>
        </div>

        {/* Runtime Filter Tabs */}
        <div className="flex flex-wrap items-center justify-between gap-3 pt-3 border-t border-slate-100 dark:border-slate-800">
          <div className="flex flex-wrap items-center gap-1.5">
            {[
              { id: 'ALL', label: `Tất cả (${apps.length})` },
              {
                id: 'RUST',
                label: `Rust Actix/Axum (${apps.filter((a) => a.framework.includes('Rust')).length})`
              },
              {
                id: 'MODERN_JS',
                label: `Modern JS/TS (${apps.filter((a) => isModernJsFramework(a.framework)).length})`
              },
              {
                id: 'PHP',
                label: `PHP-FPM (${apps.filter((a) => a.framework.includes('PHP')).length})`
              },
              {
                id: 'POLYGLOT',
                label: `Python/Go/Java/.NET/WASI (${apps.filter((a) => isPolyglotFramework(a.framework)).length})`
              }
            ].map((tab) => (
              <button
                key={tab.id}
                onClick={() =>
                  setRuntimeCategory(tab.id as typeof runtimeCategory)
                }
                className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors cursor-pointer ${
                  runtimeCategory === tab.id
                    ? 'bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900'
                    : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>

          <div className="flex items-center gap-2 text-xs font-mono text-slate-500 dark:text-slate-400">
            <Sparkles className="w-3.5 h-3.5 text-orange-500" />
            <span>
              FerrousWAF + FerrousACME + Polyglot FFI Bridge đang hoạt động
            </span>
          </div>
        </div>
      </div>

      {statusToast && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl px-4 py-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-xs font-medium text-emerald-700 dark:text-emerald-300">
            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            <span>{statusToast}</span>
          </div>
          <button
            onClick={() => setStatusToast(null)}
            className="text-xs text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 cursor-pointer"
          >
            Đóng
          </button>
        </div>
      )}

      {/* Main Hosting Websites Table */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-mono uppercase text-slate-500 dark:text-slate-400 bg-slate-50/60 dark:bg-slate-950/50">
                <th className="py-3.5 px-4">Tên miền (Site Name)</th>
                <th className="py-3.5 px-4">Trạng thái</th>
                <th className="py-3.5 px-4">Framework / Runtime</th>
                <th className="py-3.5 px-4">Thư mục gốc (Document Root)</th>
                <th className="py-3.5 px-4">Database &amp; Backup</th>
                <th className="py-3.5 px-4">SSL (Let&apos;s Encrypt)</th>
                <th className="py-3.5 px-4 text-right">Thao tác Quản trị</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs">
              {filteredApps.length === 0 ? (
                <tr>
                  <td
                    colSpan={7}
                    className="py-12 text-center text-slate-500 dark:text-slate-400"
                  >
                    <Globe className="w-8 h-8 mx-auto mb-2 opacity-40 text-slate-400" />
                    <div className="font-semibold text-xs text-slate-700 dark:text-slate-300">
                      Chưa có Website nào được tạo
                    </div>
                    <div className="text-[11px] mt-1 text-slate-400">
                      Nhấn &quot;Thêm Website Mới&quot; ở trên để bắt đầu thêm
                      website vào hệ thống.
                    </div>
                  </td>
                </tr>
              ) : 
                filteredApps.map((site) => (
                  <tr
                    key={site.id}
                    className={`hover:bg-slate-50 dark:hover:bg-slate-800/40 transition-colors ${
                      site.id === selectedApp?.id
                        ? 'bg-orange-500/5 dark:bg-orange-500/10'
                        : ''
                    }`}
                  >
                  <td className="py-3.5 px-4">
                    <div className="flex items-center gap-2">
                      <Globe className="w-4 h-4 text-orange-500 shrink-0" />
                      <div>
                        <button
                          onClick={() => openSiteModal(site, 'domain_dir')}
                          className="font-mono font-semibold text-slate-900 dark:text-slate-100 hover:text-orange-600 dark:hover:text-orange-400 text-left cursor-pointer"
                        >
                          {site.domain}
                        </button>
                        <div className="text-slate-500 font-mono">
                          127.0.0.1:{site.internalPort} · {site.requestsPerSec}{' '}
                          req/s
                        </div>
                      </div>
                    </div>
                  </td>

                  <td className="py-3.5 px-4 font-mono">
                    <div className="flex items-center gap-2">
                      <span
                        className={`inline-flex items-center gap-1.5 ${
                          site.status === 'running'
                            ? 'text-emerald-600 dark:text-emerald-400'
                            : 'text-slate-400'
                        }`}
                      >
                        <span
                          className={`w-2 h-2 rounded-full ${
                            site.status === 'running'
                              ? 'bg-emerald-500'
                              : 'bg-slate-400'
                          }`}
                        />
                        {site.status === 'running' ? 'Đang chạy' : 'Đã dừng'}
                      </span>
                      {site.status === 'running' ? (
                        <button
                          onClick={() => onAppAction(site.id, 'stop')}
                          title="Dừng Website"
                          className="p-1 text-slate-400 hover:text-rose-500 rounded cursor-pointer"
                        >
                          <Square className="w-3 h-3" />
                        </button>
                      ) : (
                        <button
                          onClick={() => onAppAction(site.id, 'start')}
                          title="Khởi chạy Website"
                          className="p-1 text-slate-400 hover:text-emerald-500 rounded cursor-pointer"
                        >
                          <Play className="w-3 h-3" />
                        </button>
                      )}
                    </div>
                  </td>

                  <td className="py-3.5 px-4">
                    <div className="font-medium text-slate-900 dark:text-slate-100">
                      {site.framework}
                    </div>
                    <div className="text-slate-500 font-mono">
                      <span
                        className={
                          (site.webServerEngine ||
                            'FerrousX (Rust Web Server)') ===
                          'FerrousX (Rust Web Server)'
                            ? 'text-orange-600 dark:text-orange-400 font-semibold'
                            : 'text-sky-600 dark:text-sky-400 font-semibold'
                        }
                      >
                        {site.webServerEngine === 'Nginx 1.26'
                          ? 'Nginx 1.26'
                          : 'FerrousX Rust'}
                      </span>{' '}
                      · RAM: {site.memoryUsedMb}/{site.memoryLimitMb} MB · P99{' '}
                      {site.p99LatencyMs}ms
                    </div>
                  </td>

                  <td className="py-3.5 px-4 font-mono">
                    <button
                      onClick={() => onNavigate('files', site.id)}
                      className="inline-flex items-center gap-1.5 text-sky-600 dark:text-sky-400 hover:underline cursor-pointer"
                      title="Mở trình quản lý tệp tin của Website này"
                    >
                      <FolderOpen className="w-3.5 h-3.5 shrink-0" />
                      <span>{site.workingDir}</span>
                    </button>
                    <div className="text-slate-500">
                      Dung lượng: {site.diskUsedMb} MB / {site.diskQuotaMb} MB
                    </div>
                  </td>

                  <td className="py-3.5 px-4 font-mono">
                    {site.linkedDbName ? (
                      <button
                        onClick={() => onNavigate('databases', site.id)}
                        className="inline-flex items-center gap-1 text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                      >
                        <Database className="w-3.5 h-3.5" />
                        <span>{site.linkedDbName}</span>
                      </button>
                    ) : (
                      <span className="text-slate-400">Không dùng DB</span>
                    )}
                    <div className="text-slate-500">
                      Backup ({site.backupCount}) · Traffic{' '}
                      {site.trafficMonthGb} GB
                    </div>
                  </td>

                  <td className="py-3.5 px-4 font-mono">
                    <button
                      onClick={() => openSiteModal(site, 'ssl')}
                      className={`inline-flex items-center gap-1 cursor-pointer hover:underline ${
                        site.sslEnabled
                          ? 'text-emerald-600 dark:text-emerald-400'
                          : 'text-amber-600 dark:text-amber-400'
                      }`}
                    >
                      <Lock className="w-3.5 h-3.5" />
                      <span>
                        {site.sslEnabled
                          ? `TLS 1.3 (${site.sslExpiryDays} ngày)`
                          : 'Chưa bật SSL'}
                      </span>
                    </button>
                    <div className="text-slate-500">
                      {site.http3Quic ? 'HTTP/3 QUIC Bật' : 'HTTP/2'}
                    </div>
                  </td>

                  <td className="py-3.5 px-4 text-right whitespace-nowrap">
                    <div className="inline-flex items-center gap-1.5">
                      <button
                        onClick={() => handleRunLiveHttpTest(site)}
                        disabled={testingSiteId === site.id}
                        className="px-2.5 py-1.5 text-xs font-medium text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 rounded-lg inline-flex items-center gap-1 cursor-pointer disabled:opacity-50"
                        title="Kiểm thử kết nối HTTP/3 & Xem trước phản hồi Website"
                      >
                        <ExternalLink className="w-3.5 h-3.5" />
                        {testingSiteId === site.id
                          ? 'Đang mở...'
                          : 'Kiểm thử Web'}
                      </button>
                      <button
                        onClick={() => openSiteModal(site, 'domain_dir')}
                        className="px-2.5 py-1.5 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1 cursor-pointer"
                      >
                        <Settings className="w-3.5 h-3.5" />
                        Cấu hình
                      </button>
                      <button
                        onClick={() => openSiteModal(site, 'git_deploy')}
                        className="px-2.5 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg inline-flex items-center gap-1 cursor-pointer"
                      >
                        <GitBranch className="w-3.5 h-3.5" />
                        Git Deploy
                      </button>
                      <button
                        onClick={() => onNavigate('files', site.id)}
                        className="px-2.5 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg cursor-pointer"
                      >
                        Files
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Live Website HTTP/3 Test & Preview Modal */}
      {liveTestResult && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/75 backdrop-blur-xs p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl max-w-3xl w-full overflow-hidden shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Globe className="w-5 h-5 text-emerald-500" />
                <div>
                  <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 font-display">
                    Kiểm thử Trực tiếp Website: {liveTestResult.url}
                  </h2>
                  <p className="text-xs font-mono text-slate-500 dark:text-slate-400">
                    {liveTestResult.protocol} · {liveTestResult.tlsCipher} · Xác
                    minh lúc {liveTestResult.verifiedAt}
                  </p>
                </div>
              </div>
              <button
                onClick={() => setLiveTestResult(null)}
                className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 rounded-lg cursor-pointer"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-xl p-3.5">
                  <div className="text-xs text-slate-500">HTTP Status</div>
                  <div
                    className={`text-base font-bold font-mono mt-0.5 ${
                      liveTestResult.statusCode === 200
                        ? 'text-emerald-600 dark:text-emerald-400'
                        : 'text-rose-600 dark:text-rose-400'
                    }`}
                  >
                    {liveTestResult.statusText}
                  </div>
                </div>
                <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-xl p-3.5">
                  <div className="text-xs text-slate-500">
                    Độ trễ Reverse Proxy
                  </div>
                  <div className="text-base font-bold font-mono text-slate-900 dark:text-slate-100 mt-0.5">
                    {liveTestResult.latencyMs} ms
                  </div>
                </div>
                <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-xl p-3.5">
                  <div className="text-xs text-slate-500">Giao thức Bảo mật</div>
                  <div className="text-base font-bold font-mono text-orange-600 dark:text-orange-400 mt-0.5">
                    {liveTestResult.protocol}
                  </div>
                </div>
              </div>

              <div className="space-y-1.5">
                <div className="text-xs font-semibold text-slate-700 dark:text-slate-300">
                  Response Headers (Nginx 1.26 + FerrousWAF Pingora)
                </div>
                <div className="bg-slate-950 rounded-xl p-3.5 font-mono text-xs text-slate-300 space-y-1">
                  {Object.entries(liveTestResult.headers).map(([k, v]) => (
                    <div key={k} className="flex gap-2">
                      <span className="text-orange-400">{k}:</span>
                      <span className="text-emerald-300">{v}</span>
                    </div>
                  ))}
                </div>
              </div>

              <div className="space-y-1.5">
                <div className="text-xs font-semibold text-slate-700 dark:text-slate-300">
                  Nội dung Phản hồi Trực tiếp từ Upstream (Response Body Preview)
                </div>
                <pre className="p-4 rounded-xl bg-slate-950 text-emerald-300 font-mono text-xs overflow-x-auto max-h-56 leading-relaxed">
                  {liveTestResult.bodyPreview}
                </pre>
              </div>

              <div className="flex items-center justify-end gap-2 pt-2 border-t border-slate-200 dark:border-slate-800">
                <button
                  onClick={() => setLiveTestResult(null)}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg cursor-pointer"
                >
                  Đóng cửa sổ kiểm thử
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Comprehensive Website Configuration Modal (aaPanel / cPanel Site Settings) */}
      {isSettingsOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/75 backdrop-blur-xs p-4 overflow-y-auto">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl max-w-5xl w-full overflow-hidden shadow-2xl my-auto">
            {/* Modal Header */}
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Globe className="w-5 h-5 text-orange-500" />
                <div>
                  <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 font-display">
                    Quản trị Website: {selectedApp.domain} [
                    {selectedApp.framework}]
                  </h2>
                  <p className="text-xs font-mono text-slate-500 dark:text-slate-400">
                    Web Root: {selectedApp.workingDir} · Upstream Port:{' '}
                    127.0.0.1:{selectedApp.internalPort}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => handleRunLiveHttpTest(selectedApp)}
                  className="px-3 py-1.5 text-xs font-medium text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  Kiểm thử HTTP 200 OK
                </button>
                <button
                  onClick={() => setIsSettingsOpen(false)}
                  className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 rounded-lg cursor-pointer"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>
            </div>

            {/* Modal Body: Left Navigation + Right Content (aaPanel Style) */}
            <div className="grid grid-cols-1 md:grid-cols-12 min-h-[480px]">
              {/* Left Vertical Menu */}
              <div className="md:col-span-3 border-r border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/50 p-3 space-y-1">
                {[
                  {
                    id: 'domain_dir',
                    label: 'Tên miền & Thư mục gốc',
                    icon: Globe
                  },
                  {
                    id: 'php_select',
                    label: 'Phiên bản PHP (MultiPHP)',
                    icon: FileCode2
                  },
                  {
                    id: 'redirects_hotlink',
                    label: '301 Redirect & Hotlink',
                    icon: ShieldCheck
                  },
                  {
                    id: 'rewrite_proxy',
                    label: 'URL Rewrite & Reverse Proxy',
                    icon: FileCode2
                  },
                  {
                    id: 'ssl',
                    label: "SSL (Let's Encrypt TLS 1.3)",
                    icon: Lock
                  },
                  {
                    id: 'git_deploy',
                    label: 'Git Auto-Deploy & Build',
                    icon: GitBranch
                  },
                  {
                    id: 'runtime_env',
                    label: 'Workers, RAM & Biến .env',
                    icon: Settings
                  },
                  {
                    id: 'nginx_conf',
                    label: 'VHost (FerrousX / Nginx)',
                    icon: ShieldCheck
                  },
                  {
                    id: 'logs',
                    label: 'Nhật ký Website (Site Logs)',
                    icon: Terminal
                  }
                ].map((item) => {
                  const Icon = item.icon;
                  const active = modalTab === item.id;
                  return (
                    <button
                      key={item.id}
                      onClick={() => setModalTab(item.id as SiteModalTab)}
                      className={`w-full flex items-center gap-2.5 px-3 py-2.5 text-xs font-medium rounded-lg transition-colors text-left cursor-pointer ${
                        active
                          ? 'bg-orange-600 text-white'
                          : 'text-slate-600 dark:text-slate-400 hover:bg-slate-200/60 dark:hover:bg-slate-800'
                      }`}
                    >
                      <Icon className="w-3.5 h-3.5 shrink-0" />
                      <span>{item.label}</span>
                    </button>
                  );
                })}
              </div>

              {/* Right Content Area */}
              <div className="md:col-span-9 p-6 flex flex-col justify-between">
                {modalTab === 'domain_dir' && (
                  <div className="space-y-4">
                    <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                      Quản lý Tên miền phụ (Domain Aliases) &amp; Thư mục thực
                      thi (Site Directory)
                    </h3>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Tên miền chính (Primary Domain)
                        </label>
                        <input
                          type="text"
                          disabled
                          value={selectedApp.domain}
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-800 text-slate-500"
                        />
                      </div>

                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Thư mục chạy Public (Running Directory)
                        </label>
                        <select
                          value={runSubDir}
                          onChange={(e) => setRunSubDir(e.target.value)}
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                        >
                          <option value="/">/ (Mặc định cho Rust / Node)</option>
                          <option value="/public">
                            /public (Chuẩn Laravel / Symfony / PHP)
                          </option>
                          <option value="/dist">
                            /dist (Chuẩn Static SPA / Vite)
                          </option>
                          <option value="/.output/public">
                            /.output/public (Chuẩn Nuxt 3 Nitro)
                          </option>
                          <option value="/build/client">
                            /build/client (Chuẩn SvelteKit 2)
                          </option>
                        </select>
                      </div>
                    </div>

                    <div className="space-y-1.5">
                      <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                        Tùy chọn Web Server Engine (FerrousX vs Nginx)
                      </label>
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                        <button
                          type="button"
                          onClick={() =>
                            setWebServerEngine('FerrousX (Rust Web Server)')
                          }
                          className={`p-3 rounded-lg border text-left transition-colors cursor-pointer ${
                            webServerEngine === 'FerrousX (Rust Web Server)'
                              ? 'border-orange-500 bg-orange-500/10 text-slate-900 dark:text-white'
                              : 'border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-600 dark:text-slate-400'
                          }`}
                        >
                          <div className="text-xs font-bold font-mono text-orange-600 dark:text-orange-400">
                            FerrousX (Rust Web Server)
                          </div>
                          <div className="text-[11px] font-mono text-slate-500 mt-0.5">
                            github.com/Ferrous-panel/FerrousX · Tokio + Hyper + io_uring
                          </div>
                        </button>

                        <button
                          type="button"
                          onClick={() => setWebServerEngine('Nginx 1.26')}
                          className={`p-3 rounded-lg border text-left transition-colors cursor-pointer ${
                            webServerEngine === 'Nginx 1.26'
                              ? 'border-orange-500 bg-orange-500/10 text-slate-900 dark:text-white'
                              : 'border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-600 dark:text-slate-400'
                          }`}
                        >
                          <div className="text-xs font-bold font-mono text-sky-600 dark:text-sky-400">
                            Nginx 1.26 (Mainline C Daemon)
                          </div>
                          <div className="text-[11px] font-mono text-slate-500 mt-0.5">
                            Web Server truyền thống tương thích epoll + FastCGI
                          </div>
                        </button>
                      </div>
                    </div>

                    <div className="space-y-1.5">
                      <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                        Danh sách Tên miền phụ / Aliases (Mỗi dòng 1 domain)
                      </label>
                      <textarea
                        rows={4}
                        value={aliasesText}
                        onChange={(e) => setAliasesText(e.target.value)}
                        placeholder="www.domain.vn&#10;cdn.domain.vn"
                        className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                      />
                    </div>

                    <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg p-3.5 text-xs space-y-1">
                      <div className="font-semibold text-slate-800 dark:text-slate-200">
                        Chống tấn công Cross-Site (Anti-Cross-Site Isolation)
                      </div>
                      <div className="text-slate-500">
                        Đã kích hoạt chỉ thị{' '}
                        <code className="font-mono text-orange-600 dark:text-orange-400">
                          ProtectSystem=strict
                        </code>{' '}
                        và{' '}
                        <code className="font-mono text-orange-600 dark:text-orange-400">
                          ReadWritePaths={selectedApp.workingDir}
                        </code>{' '}
                        để cô lập tiến trình của website này khỏi các website
                        khác trên cùng máy chủ.
                      </div>
                    </div>
                  </div>
                )}

                {modalTab === 'php_select' && (
                  <div className="space-y-4">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div>
                        <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                          Chuyển đổi Phiên bản PHP (MultiPHP Version Selector)
                        </h3>
                        <p className="text-xs text-slate-500">
                          Gán phiên bản PHP-FPM riêng biệt cho website{' '}
                          <span className="font-mono font-semibold text-slate-800 dark:text-slate-200">
                            {selectedApp.domain}
                          </span>{' '}
                          hoặc chọn &quot;Pure Static / Reverse Proxy&quot; cho
                          Rust / Node.js.
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={() => {
                          setIsSettingsOpen(false);
                          onNavigate('php');
                        }}
                        className="px-3 py-1.5 text-xs font-medium text-orange-600 dark:text-orange-400 bg-orange-500/10 hover:bg-orange-500/20 rounded-lg cursor-pointer"
                      >
                        Mở Trình quản lý Multi-PHP &amp; php.ini
                      </button>
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                      {(
                        [
                          {
                            ver: '8.4',
                            title: 'PHP 8.4 (FPM + JIT Latest)',
                            sock: '/run/php/php8.4-fpm.sock',
                            desc: 'Hiệu năng cao nhất, Property Hooks, OPcache JIT 128M'
                          },
                          {
                            ver: '8.3',
                            title: 'PHP 8.3 (FPM LTS Khuyên dùng)',
                            sock: '/run/php/php8.3-fpm.sock',
                            desc: 'Chuẩn ổn định cho Laravel 11, WordPress 6.7, WooCommerce'
                          },
                          {
                            ver: '8.2',
                            title: 'PHP 8.2 (FPM Stable)',
                            sock: '/run/php/php8.2-fpm.sock',
                            desc: 'Tương thích rộng rãi với các plugin/package PHP hiện đại'
                          },
                          {
                            ver: '8.1',
                            title: 'PHP 8.1 (FPM Security Support)',
                            sock: '/run/php/php8.1-fpm.sock',
                            desc: 'Dành cho mã nguồn Laravel 9/10 hoặc Magento 2.4'
                          },
                          {
                            ver: '7.4',
                            title: 'PHP 7.4 (Legacy FPM Hardened)',
                            sock: '/run/php/php7.4-fpm.sock',
                            desc: 'Chạy các theme/plugin WordPress cũ hoặc code PHP 7'
                          },
                          {
                            ver: 'None',
                            title: 'Pure Static / Rust / Node Proxy (Không PHP)',
                            sock: `http://127.0.0.1:${selectedApp.internalPort}`,
                            desc: 'Dành cho Actix-Web, Axum, Next.js, Nuxt, SvelteKit'
                          }
                        ] as const
                      ).map((opt) => (
                        <button
                          key={opt.ver}
                          type="button"
                          onClick={() => setPhpVersion(opt.ver)}
                          className={`p-3.5 rounded-xl border text-left transition-all cursor-pointer ${
                            phpVersion === opt.ver
                              ? 'border-orange-500 bg-orange-500/10'
                              : 'border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/60 hover:border-slate-300 dark:hover:border-slate-700'
                          }`}
                        >
                          <div className="flex items-center justify-between">
                            <span className="text-xs font-bold text-slate-900 dark:text-slate-100">
                              {opt.title}
                            </span>
                            <span
                              className={`text-xs font-mono ${
                                phpVersion === opt.ver
                                  ? 'text-orange-600 dark:text-orange-400 font-bold'
                                  : 'text-slate-400'
                              }`}
                            >
                              {phpVersion === opt.ver ? 'Đang chọn' : opt.ver}
                            </span>
                          </div>
                          <div className="text-xs font-mono text-slate-500 mt-1">
                            {opt.sock}
                          </div>
                          <div className="text-xs text-slate-500 mt-1">
                            {opt.desc}
                          </div>
                        </button>
                      ))}
                    </div>
                  </div>
                )}

                {modalTab === 'redirects_hotlink' && (
                  <div className="space-y-5 max-h-[390px] overflow-y-auto pr-1">
                    {/* 301/302 Redirects */}
                    <div className="space-y-2.5">
                      <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                        1. Chuyển hướng Tên miền &amp; Đường dẫn (301 / 302 URL
                        Redirects)
                      </h3>
                      <div className="grid grid-cols-1 sm:grid-cols-12 gap-2">
                        <select
                          value={newRedirectType}
                          onChange={(e) =>
                            setNewRedirectType(e.target.value as '301' | '302')
                          }
                          className="sm:col-span-2 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        >
                          <option value="301">301 (Vĩnh viễn)</option>
                          <option value="302">302 (Tạm thời)</option>
                        </select>
                        <input
                          type="text"
                          value={newRedirectSource}
                          onChange={(e) => setNewRedirectSource(e.target.value)}
                          placeholder="/duong-dan-cu"
                          className="sm:col-span-4 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                        <input
                          type="text"
                          value={newRedirectTarget}
                          onChange={(e) => setNewRedirectTarget(e.target.value)}
                          placeholder="https://domain-moi.vn/trang-dich"
                          className="sm:col-span-4 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                        <button
                          type="button"
                          onClick={() => {
                            if (
                              !newRedirectSource.trim() ||
                              !newRedirectTarget.trim()
                            )
                              return;
                            setRedirects([
                              ...redirects,
                              {
                                id: `redir-${Date.now()}`,
                                sourcePath: newRedirectSource.trim(),
                                targetUrl: newRedirectTarget.trim(),
                                type: newRedirectType,
                                keepQuery: true,
                                enabled: true
                              }
                            ]);
                          }}
                          className="sm:col-span-2 px-3 py-1.5 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg cursor-pointer"
                        >
                          + Thêm
                        </button>
                      </div>

                      {redirects.length > 0 && (
                        <div className="space-y-1.5">
                          {redirects.map((r) => (
                            <div
                              key={r.id}
                              className="flex items-center justify-between px-3 py-2 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 text-xs font-mono"
                            >
                              <span>
                                [{r.type}] {r.sourcePath} &rarr;{' '}
                                <span className="text-emerald-600 dark:text-emerald-400">
                                  {r.targetUrl}
                                </span>
                              </span>
                              <button
                                type="button"
                                onClick={() =>
                                  setRedirects(
                                    redirects.filter((x) => x.id !== r.id)
                                  )
                                }
                                className="text-rose-500 hover:underline cursor-pointer"
                              >
                                Xóa
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>

                    {/* Hotlink Protection */}
                    <div className="space-y-2.5 pt-3 border-t border-slate-200 dark:border-slate-800">
                      <div className="flex items-center justify-between">
                        <div>
                          <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                            2. Chống trộm Băng thông Ảnh / Video (Hotlink
                            Protection)
                          </h3>
                          <p className="text-xs text-slate-500">
                            Chặn website lạ nhúng trực tiếp file ảnh/video/zip
                            của {selectedApp.domain}
                          </p>
                        </div>
                        <input
                          type="checkbox"
                          checked={hotlinkConfig.enabled}
                          onChange={(e) =>
                            setHotlinkConfig({
                              ...hotlinkConfig,
                              enabled: e.target.checked
                            })
                          }
                          className="rounded text-orange-600"
                        />
                      </div>

                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                        <div className="space-y-1">
                          <label className="text-xs text-slate-500">
                            Đuôi tệp bảo vệ (Nginx location ~)
                          </label>
                          <input
                            type="text"
                            value={hotlinkConfig.protectedExtensions}
                            onChange={(e) =>
                              setHotlinkConfig({
                                ...hotlinkConfig,
                                protectedExtensions: e.target.value
                              })
                            }
                            className="w-full px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                          />
                        </div>
                        <div className="space-y-1">
                          <label className="text-xs text-slate-500">
                            Tên miền hợp lệ (valid_referers)
                          </label>
                          <input
                            type="text"
                            value={hotlinkConfig.allowedDomains.join(', ')}
                            onChange={(e) =>
                              setHotlinkConfig({
                                ...hotlinkConfig,
                                allowedDomains: e.target.value
                                  .split(',')
                                  .map((s) => s.trim())
                                  .filter(Boolean)
                              })
                            }
                            className="w-full px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                          />
                        </div>
                      </div>
                    </div>

                    {/* Password Protected Directories */}
                    <div className="space-y-2.5 pt-3 border-t border-slate-200 dark:border-slate-800">
                      <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                        3. Khóa Mật khẩu Thư mục (Directory Privacy / HTTP Basic
                        Auth)
                      </h3>
                      <div className="grid grid-cols-1 sm:grid-cols-12 gap-2">
                        <input
                          type="text"
                          value={newAuthPath}
                          onChange={(e) => setNewAuthPath(e.target.value)}
                          placeholder="/wp-admin"
                          className="sm:col-span-4 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                        <input
                          type="text"
                          value={newAuthUser}
                          onChange={(e) => setNewAuthUser(e.target.value)}
                          placeholder="User BasicAuth"
                          className="sm:col-span-3 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                        <input
                          type="text"
                          value={newAuthRealm}
                          onChange={(e) => setNewAuthRealm(e.target.value)}
                          placeholder="Tên vùng bảo mật"
                          className="sm:col-span-3 px-2.5 py-1.5 text-xs rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                        <button
                          type="button"
                          onClick={() => {
                            if (!newAuthPath.trim() || !newAuthUser.trim())
                              return;
                            setDirectoryAuth([
                              ...directoryAuth,
                              {
                                id: `auth-${Date.now()}`,
                                path: newAuthPath.trim(),
                                label: newAuthRealm.trim() || 'Protected Area',
                                username: newAuthUser.trim(),
                                enabled: true
                              }
                            ]);
                          }}
                          className="sm:col-span-2 px-3 py-1.5 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg cursor-pointer"
                        >
                          + Khóa
                        </button>
                      </div>

                      {directoryAuth.length > 0 && (
                        <div className="space-y-1.5">
                          {directoryAuth.map((d) => (
                            <div
                              key={d.id}
                              className="flex items-center justify-between px-3 py-2 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 text-xs font-mono"
                            >
                              <span>
                                Thư mục{' '}
                                <strong className="text-orange-600 dark:text-orange-400">
                                  {d.path}
                                </strong>{' '}
                                · User: {d.username} ({d.label})
                              </span>
                              <button
                                type="button"
                                onClick={() =>
                                  setDirectoryAuth(
                                    directoryAuth.filter((x) => x.id !== d.id)
                                  )
                                }
                                className="text-rose-500 hover:underline cursor-pointer"
                              >
                                Gỡ khóa
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                )}

                {modalTab === 'rewrite_proxy' && (
                  <div className="space-y-4">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                        Mẫu URL Rewrite &amp; Reverse Proxy Upstream
                      </h3>
                      <select
                        value={rewritePreset}
                        onChange={(e) =>
                          handlePresetChange(
                            e.target.value as RewritePresetType
                          )
                        }
                        className="px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                      >
                        {REWRITE_PRESET_LABELS.map((p) => (
                          <option key={p.id} value={p.id}>
                            Mẫu: {p.label}
                          </option>
                        ))}
                      </select>
                    </div>

                    <p className="text-xs text-slate-500">
                      Chọn mẫu chuyển hướng URL cho Actix-Web, Next.js 15, Nuxt
                      3, SvelteKit 2, Astro 5, Laravel, WordPress hoặc tùy chỉnh
                      trực tiếp quy tắc Nginx bên dưới:
                    </p>

                    <textarea
                      rows={11}
                      value={rewriteRules}
                      onChange={(e) => setRewriteRules(e.target.value)}
                      className="w-full p-3.5 text-xs font-mono rounded-xl border border-slate-300 dark:border-slate-700 bg-slate-950 text-emerald-300 leading-relaxed"
                    />
                  </div>
                )}

                {modalTab === 'ssl' && (
                  <div className="space-y-4">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                        Chứng chỉ SSL Let&apos;s Encrypt (FerrousACME Rust) &amp;
                        HTTP/3 QUIC
                      </h3>
                      <button
                        type="button"
                        disabled={busyAction === 'renew-ssl'}
                        onClick={handleRenewSslClick}
                        className="px-3.5 py-1.5 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                      >
                        <RefreshCw
                          className={`w-3.5 h-3.5 ${
                            busyAction === 'renew-ssl' ? 'animate-spin' : ''
                          }`}
                        />
                        {busyAction === 'renew-ssl'
                          ? 'Đang cấp phát ACME...'
                          : "Gia hạn / Cấp mới SSL Let's Encrypt"}
                      </button>
                    </div>

                    <div className="bg-emerald-500/10 border border-emerald-500/20 rounded-xl p-4 flex items-center justify-between">
                      <div className="space-y-1">
                        <div className="text-xs font-semibold text-emerald-600 dark:text-emerald-400 flex items-center gap-1.5">
                          <CheckCircle2 className="w-4 h-4" />
                          Chứng chỉ {selectedApp.sslIssuer} đang hoạt động
                        </div>
                        <div className="text-xs font-mono text-slate-600 dark:text-slate-400">
                          Đường dẫn: /etc/letsencrypt/live/{selectedApp.domain}
                          /fullchain.pem · Còn hạn: {selectedApp.sslExpiryDays}{' '}
                          ngày
                        </div>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-2">
                      <label className="flex items-center justify-between p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 cursor-pointer">
                        <div>
                          <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                            Bắt buộc HTTPS (Force HTTPS 301)
                          </div>
                          <div className="text-xs text-slate-500">
                            Tự động chuyển hướng cổng 80 sang 443
                          </div>
                        </div>
                        <input
                          type="checkbox"
                          checked={forceHttps}
                          onChange={(e) => setForceHttps(e.target.checked)}
                          className="rounded text-orange-600"
                        />
                      </label>

                      <label className="flex items-center justify-between p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 cursor-pointer">
                        <div>
                          <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                            Bật giao thức HTTP/3 QUIC (UDP 443)
                          </div>
                          <div className="text-xs text-slate-500">
                            Tăng tốc kết nối TLS 1.3 0-RTT
                          </div>
                        </div>
                        <input
                          type="checkbox"
                          checked={http3Quic}
                          onChange={(e) => setHttp3Quic(e.target.checked)}
                          className="rounded text-orange-600"
                        />
                      </label>
                    </div>
                  </div>
                )}

                {modalTab === 'git_deploy' && (
                  <div className="space-y-4">
                    <div className="flex items-center justify-between">
                      <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                        Triển khai tự động từ Git Repository (Git CI/CD Webhook)
                      </h3>
                      <button
                        type="button"
                        disabled={busyAction === 'git'}
                        onClick={handleGitPullDeploy}
                        className="px-3.5 py-2 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                      >
                        <RotateCw
                          className={`w-3.5 h-3.5 ${
                            busyAction === 'git' ? 'animate-spin' : ''
                          }`}
                        />
                        Pull Code &amp; Build Ngay
                      </button>
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                      <div className="sm:col-span-2 space-y-1.5">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Git Repository URL (GitHub / GitLab / Gitea)
                        </label>
                        <input
                          type="text"
                          value={gitRepoUrl}
                          onChange={(e) => setGitRepoUrl(e.target.value)}
                          placeholder="https://github.com/Ferrous-panel/FerrousX.git"
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                        />
                      </div>

                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Nhánh (Branch)
                        </label>
                        <input
                          type="text"
                          value={gitBranch}
                          onChange={(e) => setGitBranch(e.target.value)}
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                        />
                      </div>
                    </div>

                    <div className="space-y-1.5">
                      <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                        Kịch bản Build trên Server (Build Script)
                      </label>
                      <input
                        type="text"
                        value={buildScript}
                        onChange={(e) => setBuildScript(e.target.value)}
                        className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                      />
                    </div>

                    <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg p-3.5 text-xs font-mono space-y-1">
                      <div className="text-slate-500">
                        Lần Deploy gần nhất:{' '}
                        <span className="text-slate-900 dark:text-slate-100">
                          {selectedApp.lastDeployCommit} (
                          {selectedApp.lastDeployTime})
                        </span>
                      </div>
                      <div className="text-slate-500">
                        Webhook URL:{' '}
                        <span className="text-orange-600 dark:text-orange-400">
                          https://panel.ferroushub.vn/api/webhook/
                          {selectedApp.id}
                        </span>
                      </div>
                    </div>
                  </div>
                )}

                {modalTab === 'runtime_env' && (
                  <div className="space-y-4">
                    <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                      Giới hạn Tài nguyên Tiến trình &amp; Biến môi trường
                      (.env)
                    </h3>

                    <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                      <div className="space-y-1">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Số luồng Worker
                        </label>
                        <input
                          type="number"
                          min={1}
                          max={64}
                          value={workers}
                          onChange={(e) => setWorkers(Number(e.target.value))}
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                      </div>

                      <div className="space-y-1">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Giới hạn RAM (MB)
                        </label>
                        <input
                          type="number"
                          min={64}
                          max={16384}
                          step={64}
                          value={memoryLimitMb}
                          onChange={(e) =>
                            setMemoryLimitMb(Number(e.target.value))
                          }
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                      </div>

                      <div className="space-y-1">
                        <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                          Rate Limit (req/s)
                        </label>
                        <input
                          type="number"
                          min={10}
                          max={5000}
                          value={rateLimitRps}
                          onChange={(e) =>
                            setRateLimitRps(Number(e.target.value))
                          }
                          className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                        />
                      </div>
                    </div>

                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <span className="text-xs font-semibold text-slate-700 dark:text-slate-300">
                          Biến môi trường (.env)
                        </span>
                        <button
                          type="button"
                          onClick={() =>
                            setEnvVars([...envVars, { key: '', value: '' }])
                          }
                          className="text-xs font-medium text-orange-600 dark:text-orange-400 inline-flex items-center gap-1 cursor-pointer"
                        >
                          <Plus className="w-3.5 h-3.5" /> Thêm biến
                        </button>
                      </div>

                      <div className="space-y-2 max-h-48 overflow-y-auto">
                        {envVars.map((env, idx) => (
                          <div key={idx} className="flex items-center gap-2">
                            <input
                              type="text"
                              value={env.key}
                              onChange={(e) => {
                                const next = [...envVars];
                                next[idx].key = e.target.value;
                                setEnvVars(next);
                              }}
                              placeholder="KEY"
                              className="w-1/3 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                            />
                            <input
                              type="text"
                              value={env.value}
                              onChange={(e) => {
                                const next = [...envVars];
                                next[idx].value = e.target.value;
                                setEnvVars(next);
                              }}
                              placeholder="VALUE"
                              className="flex-1 px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                            />
                            <button
                              type="button"
                              onClick={() =>
                                setEnvVars(envVars.filter((_, i) => i !== idx))
                              }
                              className="p-1.5 text-slate-400 hover:text-rose-500 cursor-pointer"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                )}

                {modalTab === 'nginx_conf' && (
                  <div className="space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <span className="text-xs font-mono text-slate-500">
                        {webServerEngine === 'FerrousX (Rust Web Server)'
                          ? `/www/server/panel/vhost/ferrousx/${selectedApp.domain}.conf`
                          : `/www/server/panel/vhost/nginx/${selectedApp.domain}.conf`}
                      </span>
                      <div className="flex items-center gap-2">
                        <select
                          value={webServerEngine}
                          onChange={(e) =>
                            setWebServerEngine(
                              e.target.value as WebServerEngineType
                            )
                          }
                          className="px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                        >
                          <option value="FerrousX (Rust Web Server)">
                            FerrousX (github.com/Ferrous-panel/FerrousX)
                          </option>
                          <option value="Nginx 1.26">Nginx 1.26</option>
                        </select>
                        <button
                          type="button"
                          onClick={handleValidateNginx}
                          className="px-3 py-1.5 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg inline-flex items-center gap-1 cursor-pointer"
                        >
                          <CheckCircle2 className="w-3.5 h-3.5" />
                          Kiểm tra cú pháp (
                          {webServerEngine === 'FerrousX (Rust Web Server)'
                            ? 'ferrousx -t'
                            : 'nginx -t'}
                          )
                        </button>
                      </div>
                    </div>
                    <textarea
                      rows={12}
                      value={nginxRaw}
                      onChange={(e) => setNginxRaw(e.target.value)}
                      className="w-full p-3.5 text-xs font-mono rounded-xl border border-slate-300 dark:border-slate-700 bg-slate-950 text-emerald-300 leading-relaxed"
                    />
                    {validationResult && (
                      <pre
                        className={`p-3 rounded-lg text-xs font-mono ${
                          validationResult.valid
                            ? 'bg-emerald-950/60 text-emerald-300 border border-emerald-800'
                            : 'bg-rose-950/60 text-rose-300 border border-rose-800'
                        }`}
                      >
                        {validationResult.output}
                      </pre>
                    )}
                  </div>
                )}

                {modalTab === 'logs' && (
                  <div className="space-y-2">
                    <div className="text-xs font-mono text-slate-500">
                      /www/wwwlogs/{selectedApp.domain}.log &amp; journalctl -u{' '}
                      {selectedApp.name}.service
                    </div>
                    <pre className="p-4 rounded-xl bg-slate-950 text-slate-200 font-mono text-xs overflow-x-auto max-h-80 leading-relaxed">
                      {selectedApp.logs.join('\n')}
                    </pre>
                  </div>
                )}

                {/* Modal Footer */}
                <div className="flex items-center justify-between gap-2 pt-4 mt-4 border-t border-slate-200 dark:border-slate-800">
                  <button
                    type="button"
                    disabled={busyAction === 'delete-site' || apps.length <= 1}
                    onClick={handleDeleteCurrentSite}
                    className="px-3.5 py-2 text-xs font-medium text-rose-600 dark:text-rose-400 bg-rose-500/10 hover:bg-rose-500/20 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-40"
                    title={
                      apps.length <= 1
                        ? 'Cần giữ lại ít nhất 1 Website trên hệ thống'
                        : 'Xóa Website này'
                    }
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    Xóa Website
                  </button>

                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => setIsSettingsOpen(false)}
                      className="px-4 py-2 text-xs font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg cursor-pointer"
                    >
                      Đóng
                    </button>
                    <button
                      type="button"
                      disabled={busyAction === 'save'}
                      onClick={handleSaveSiteSettings}
                      className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                    >
                      <Save className="w-3.5 h-3.5" />
                      {busyAction === 'save'
                        ? 'Đang lưu & Reload...'
                        : `Lưu Cấu hình & Reload ${
                            webServerEngine === 'FerrousX (Rust Web Server)'
                              ? 'FerrousX'
                              : 'Nginx'
                          }`}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
