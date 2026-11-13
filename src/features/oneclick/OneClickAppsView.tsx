import React, { useState } from 'react';
import {
  CheckCircle2,
  Code2,
  Cpu,
  Download,
  FileCode2,
  Layers,
  Play,
  RotateCw,
  Rocket,
  Save,
  Server,
  Square,
  Terminal,
  Trash2,
  X
} from 'lucide-react';
import {
  OneClickTemplate,
  ServerPackageItem,
  WebServerEngineType
} from '../types';

interface OneClickAppsViewProps {
  templates: OneClickTemplate[];
  packages: ServerPackageItem[];
  onDeployTemplate: (payload: {
    templateId: string;
    domain: string;
    createDb: boolean;
    webServerEngine?: WebServerEngineType;
  }) => Promise<void>;
  onPackageAction: (
    pkgId: string,
    action: 'start' | 'stop' | 'restart' | 'install' | 'uninstall'
  ) => Promise<void>;
  onSavePackageConfig: (pkgId: string, configContent: string) => Promise<void>;
}

export const OneClickAppsView: React.FC<OneClickAppsViewProps> = ({
  templates,
  packages,
  onDeployTemplate,
  onPackageAction,
  onSavePackageConfig
}) => {
  const [section, setSection] = useState<'packages' | 'templates'>('packages');
  const [pkgFilter, setPkgFilter] = useState<'all' | 'rust_only' | 'core'>(
    'all'
  );
  const [categoryFilter, setCategoryFilter] = useState<string>('ALL');
  const [selectedTpl, setSelectedTpl] = useState<OneClickTemplate | null>(null);
  const [domainInput, setDomainInput] = useState('');
  const [createDb, setCreateDb] = useState(true);
  const [webServerEngine, setWebServerEngine] = useState<WebServerEngineType>(
    'FerrousX (Rust Web Server)'
  );
  const [deploying, setDeploying] = useState(false);

  // Package Config & Logs Modal State
  const [configPkg, setConfigPkg] = useState<ServerPackageItem | null>(null);
  const [editedConfig, setEditedConfig] = useState('');
  const [savingConfig, setSavingConfig] = useState(false);
  const [busyPkgId, setBusyPkgId] = useState<string | null>(null);
  const [bannerMsg, setBannerMsg] = useState<string | null>(null);

  const filteredTemplates =
    categoryFilter === 'ALL'
      ? templates
      : templates.filter((t) => t.category === categoryFilter);

  const filteredPackages = packages.filter((pkg) => {
    if (pkgFilter === 'rust_only') return pkg.isRustNative;
    if (pkgFilter === 'core') return !pkg.isRustNative;
    return true;
  });

  const rustPackagesCount = packages.filter((p) => p.isRustNative).length;
  const runningPackagesCount = packages.filter(
    (p) => p.status === 'running'
  ).length;

  const handleOpenConfigModal = (pkg: ServerPackageItem) => {
    setConfigPkg(pkg);
    setEditedConfig(pkg.configContent || '');
  };

  const handleTriggerPkgAction = async (
    pkg: ServerPackageItem,
    action: 'start' | 'stop' | 'restart' | 'install' | 'uninstall'
  ) => {
    setBusyPkgId(pkg.id);
    try {
      await onPackageAction(pkg.id, action);
      const labelMap: Record<string, string> = {
        start: 'Đã khởi chạy dịch vụ',
        stop: 'Đã dừng dịch vụ',
        restart: 'Đã khởi động lại dịch vụ',
        install: 'Đã cài đặt & kích hoạt package Rust',
        uninstall: 'Đã gỡ cài đặt package'
      };
      setBannerMsg(`${labelMap[action]}: ${pkg.name}`);
      setTimeout(() => setBannerMsg(null), 4000);
    } finally {
      setBusyPkgId(null);
    }
  };

  const handleSaveConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!configPkg) return;
    setSavingConfig(true);
    try {
      await onSavePackageConfig(configPkg.id, editedConfig);
      setBannerMsg(
        `Đã lưu tệp cấu hình ${configPkg.configPath} và nạp lại dịch vụ ${configPkg.name}`
      );
      setConfigPkg(null);
      setTimeout(() => setBannerMsg(null), 4000);
    } finally {
      setSavingConfig(false);
    }
  };

  const handleConfirmDeploy = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedTpl || !domainInput.trim()) return;
    setDeploying(true);
    try {
      await onDeployTemplate({
        templateId: selectedTpl.id,
        domain: domainInput.trim(),
        createDb,
        webServerEngine
      });
      setSelectedTpl(null);
    } finally {
      setDeploying(false);
    }
  };

  return (
    <div className="space-y-5 sm:space-y-6 min-w-0 w-full">
      {/* Header & Section Switcher */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-5 flex flex-col lg:flex-row lg:items-center justify-between gap-4 min-w-0 overflow-hidden">
        <div className="space-y-1 min-w-0">
          <h1 className="text-base sm:text-lg font-bold tracking-tight text-slate-900 dark:text-slate-100 font-display">
            Package Bổ Sung Viết Bằng Rust &amp; Kho Ứng Dụng 1-Click
          </h1>
          <p className="text-xs text-slate-500 dark:text-slate-400 leading-relaxed">
            Toàn bộ các package phụ trợ (Cache KV, WAF Proxy, ACME SSL, DNS,
            SFTP, Search, Log Vector) được viết lại bằng Rust giúp tiết kiệm
            75% RAM và vận hành ổn định tuyệt đối cho các Website.
          </p>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 lg:flex items-center gap-2 w-full lg:w-auto shrink-0">
          <button
            onClick={() => setSection('packages')}
            className={`px-3.5 py-2.5 sm:py-2 text-xs font-medium rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer min-w-0 ${
              section === 'packages'
                ? 'bg-orange-600 text-white shadow-xs'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            <Cpu className="w-3.5 h-3.5 shrink-0" />
            <span className="truncate">
              Package Bổ Sung Rust ({rustPackagesCount} Rust / {packages.length}{' '}
              Tổng)
            </span>
          </button>
          <button
            onClick={() => setSection('templates')}
            className={`px-3.5 py-2.5 sm:py-2 text-xs font-medium rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer min-w-0 ${
              section === 'templates'
                ? 'bg-orange-600 text-white shadow-xs'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            <Rocket className="w-3.5 h-3.5 shrink-0" />
            <span className="truncate">
              1-Click Website Templates ({templates.length})
            </span>
          </button>
        </div>
      </div>

      {bannerMsg && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl px-4 py-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-xs font-medium text-emerald-700 dark:text-emerald-300 min-w-0">
            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            <span className="break-words">{bannerMsg}</span>
          </div>
          <button
            onClick={() => setBannerMsg(null)}
            className="text-xs text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 cursor-pointer shrink-0"
          >
            Đóng
          </button>
        </div>
      )}

      {section === 'packages' ? (
        <div className="space-y-4 min-w-0">
          {/* Rust Package Architecture Summary Bar */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3.5 sm:gap-4">
            <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 space-y-1 min-w-0">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Packages Viết Lại Bằng Rust
              </div>
              <div className="text-lg sm:text-xl font-bold font-mono tabular-nums text-orange-600 dark:text-orange-400">
                {rustPackagesCount} Native Rust Daemons
              </div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Thay thế Redis, Certbot, ModSecurity, BIND9, vsftpd, Filebeat
              </div>
            </div>

            <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 space-y-1 min-w-0">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Trạng thái Dịch vụ Đang Chạy
              </div>
              <div className="text-lg sm:text-xl font-bold font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
                {runningPackagesCount} / {packages.length} Active
              </div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Tất cả cổng nội bộ &amp; socket đã kết nối với Website Manager
              </div>
            </div>

            <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 space-y-1 min-w-0">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Tổng RAM {rustPackagesCount} Package Bổ Sung Rust
              </div>
              <div className="text-lg sm:text-xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100">
                {packages
                  .filter((p) => p.isRustNative)
                  .reduce((acc, p) => acc + p.memoryMb, 0)
                  .toFixed(1)}{' '}
                MB RSS
              </div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Giảm ~82% bộ nhớ so với chạy JVM / Python / C truyền thống
              </div>
            </div>
          </div>

          {/* Filter Bar */}
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-2 w-full sm:w-auto">
              <button
                onClick={() => setPkgFilter('all')}
                className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors cursor-pointer ${
                  pkgFilter === 'all'
                    ? 'bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900'
                    : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-800'
                }`}
              >
                Tất cả ({packages.length})
              </button>
              <button
                onClick={() => setPkgFilter('rust_only')}
                className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors cursor-pointer ${
                  pkgFilter === 'rust_only'
                    ? 'bg-orange-600 text-white'
                    : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-800'
                }`}
              >
                Package Bổ Sung Viết Bằng Rust ({rustPackagesCount})
              </button>
              <button
                onClick={() => setPkgFilter('core')}
                className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors cursor-pointer ${
                  pkgFilter === 'core'
                    ? 'bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900'
                    : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-800'
                }`}
              >
                Web Server &amp; SQL Engines (
                {packages.length - rustPackagesCount})
              </button>
            </div>
          </div>

          {/* Packages List */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 min-w-0">
            {filteredPackages.map((pkg) => {
              const isBusy = busyPkgId === pkg.id;
              return (
                <div
                  key={pkg.id}
                  className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-4 sm:p-5 flex flex-col justify-between gap-4 hover:border-slate-300 dark:hover:border-slate-700 transition-colors min-w-0 overflow-hidden"
                >
                  <div className="space-y-2.5">
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="text-xs font-mono text-orange-600 dark:text-orange-400 font-semibold">
                            {pkg.category}
                          </span>
                          {pkg.isRustNative && (
                            <span className="text-xs font-mono text-emerald-600 dark:text-emerald-400">
                              · 100% Native Rust
                            </span>
                          )}
                        </div>
                        <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100 mt-0.5">
                          {pkg.name}
                        </h3>
                        <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
                          {pkg.version}{' '}
                          {pkg.port ? `· Port ${pkg.port}` : ''}
                        </div>
                      </div>
                      <span
                        className={`inline-flex items-center gap-1.5 text-xs font-mono shrink-0 ${
                          pkg.status === 'running'
                            ? 'text-emerald-600 dark:text-emerald-400'
                            : pkg.status === 'stopped'
                              ? 'text-amber-600 dark:text-amber-400'
                              : 'text-slate-400'
                        }`}
                      >
                        <span
                          className={`w-2 h-2 rounded-full ${
                            pkg.status === 'running'
                              ? 'bg-emerald-500'
                              : pkg.status === 'stopped'
                                ? 'bg-amber-500'
                                : 'bg-slate-400'
                          }`}
                        />
                        {pkg.status === 'running'
                          ? 'Đang chạy'
                          : pkg.status === 'stopped'
                            ? 'Đã dừng'
                            : 'Chưa cài đặt'}
                      </span>
                    </div>

                    <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                      {pkg.description}
                    </p>

                    {pkg.replacedLegacyPackage && (
                      <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg px-3 py-2 text-xs font-mono space-y-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-slate-500">
                            Thay thế phần mềm cũ:
                          </span>
                          <span className="text-slate-800 dark:text-slate-200">
                            {pkg.replacedLegacyPackage}
                          </span>
                        </div>
                        {pkg.rustCrate && (
                          <div className="flex items-center justify-between gap-2">
                            <span className="text-slate-500">Rust Crates:</span>
                            <span className="text-orange-600 dark:text-orange-400">
                              {pkg.rustCrate}
                            </span>
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  <div className="pt-3 border-t border-slate-100 dark:border-slate-800 space-y-2.5">
                    <div className="flex items-center justify-between text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                      <span className="truncate max-w-[230px]" title={pkg.configPath}>
                        {pkg.configPath}
                      </span>
                      <span>RAM: {pkg.memoryMb} MB</span>
                    </div>

                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <button
                        onClick={() => handleOpenConfigModal(pkg)}
                        className="px-2.5 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        <FileCode2 className="w-3.5 h-3.5 text-orange-500" />
                        Cấu hình &amp; Logs
                      </button>

                      <div className="flex items-center gap-1.5">
                        {pkg.status === 'not_installed' ? (
                          <button
                            disabled={isBusy}
                            onClick={() => handleTriggerPkgAction(pkg, 'install')}
                            className="px-3 py-1.5 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg inline-flex items-center gap-1 cursor-pointer disabled:opacity-50"
                          >
                            <Download className="w-3.5 h-3.5" />
                            Cài đặt Package Rust
                          </button>
                        ) : (
                          <>
                            {pkg.status === 'running' ? (
                              <button
                                disabled={isBusy}
                                onClick={() => handleTriggerPkgAction(pkg, 'stop')}
                                className="px-2.5 py-1.5 text-xs font-medium text-amber-600 dark:text-amber-400 bg-amber-500/10 hover:bg-amber-500/20 rounded-lg inline-flex items-center gap-1 cursor-pointer disabled:opacity-50"
                              >
                                <Square className="w-3 h-3" />
                                Dừng
                              </button>
                            ) : (
                              <button
                                disabled={isBusy}
                                onClick={() => handleTriggerPkgAction(pkg, 'start')}
                                className="px-2.5 py-1.5 text-xs font-medium text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 hover:bg-emerald-500/20 rounded-lg inline-flex items-center gap-1 cursor-pointer disabled:opacity-50"
                              >
                                <Play className="w-3 h-3" />
                                Bật
                              </button>
                            )}
                            <button
                              disabled={isBusy}
                              onClick={() => handleTriggerPkgAction(pkg, 'restart')}
                              className="px-2.5 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg inline-flex items-center gap-1 cursor-pointer disabled:opacity-50"
                            >
                              <RotateCw
                                className={`w-3 h-3 ${isBusy ? 'animate-spin' : ''}`}
                              />
                              Restart
                            </button>
                            <button
                              disabled={isBusy}
                              onClick={() =>
                                handleTriggerPkgAction(pkg, 'uninstall')
                              }
                              className="p-1.5 text-xs text-rose-600 dark:text-rose-400 bg-rose-500/10 hover:bg-rose-500/20 rounded-lg cursor-pointer disabled:opacity-50"
                              title="Gỡ cài đặt package"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ) : (
        <div className="space-y-4">
          {/* Category Filter */}
          <div className="flex flex-wrap items-center gap-2">
            {[
              'ALL',
              'Modern JS Frameworks',
              'Rust Backend',
              'PHP CMS / Framework',
              'Python / Go',
              'Enterprise & Systems'
            ].map((cat) => (
              <button
                key={cat}
                onClick={() => setCategoryFilter(cat)}
                className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors cursor-pointer ${
                  categoryFilter === cat
                    ? 'bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900'
                    : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-800'
                }`}
              >
                {cat === 'ALL' ? 'Tất cả Framework & Ngôn ngữ' : cat}
              </button>
            ))}
          </div>

          {/* Template Cards Grid */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {filteredTemplates.map((tpl) => (
              <div
                key={tpl.id}
                className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 flex flex-col justify-between gap-4 hover:border-slate-300 dark:hover:border-slate-700 transition-colors"
              >
                <div className="space-y-2.5">
                  <div className="flex items-start justify-between gap-2">
                    <div>
                      <span className="text-xs font-mono text-orange-600 dark:text-orange-400 font-medium">
                        {tpl.category}
                      </span>
                      <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100 mt-0.5">
                        {tpl.name}
                      </h3>
                    </div>
                    <span className="text-xs font-mono text-slate-500 dark:text-slate-400 shrink-0">
                      {tpl.version}
                    </span>
                  </div>

                  <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                    {tpl.description}
                  </p>
                </div>

                <div className="space-y-3 pt-3 border-t border-slate-100 dark:border-slate-800">
                  <div className="flex items-center justify-between text-xs font-mono text-slate-500 dark:text-slate-400">
                    <span>DB: {tpl.requiresDb}</span>
                    <span>Port: :{tpl.defaultPort}</span>
                  </div>

                  <button
                    onClick={() => {
                      setSelectedTpl(tpl);
                      setDomainInput(
                        `${tpl.id.replace('tpl-', '').split('-')[0]}.ferroushub.vn`
                      );
                      setCreateDb(tpl.requiresDb !== 'None');
                    }}
                    className="w-full px-3.5 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer"
                  >
                    <Download className="w-3.5 h-3.5" />
                    Cài đặt 1-Click ngay
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Package Config & Logs Modal */}
      {configPkg && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 backdrop-blur-xs p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl max-w-3xl w-full overflow-hidden shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div>
                <div className="flex items-center gap-2">
                  <Code2 className="w-4 h-4 text-orange-500" />
                  <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 font-display">
                    Cấu hình Package: {configPkg.name}
                  </h2>
                </div>
                <p className="text-xs font-mono text-slate-500 dark:text-slate-400 mt-0.5">
                  Đường dẫn tệp: {configPkg.configPath}
                </p>
              </div>
              <button
                onClick={() => setConfigPkg(null)}
                className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 rounded-lg cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleSaveConfig} className="p-6 space-y-4">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-medium text-slate-700 dark:text-slate-300">
                    Nội dung tệp cấu hình ({configPkg.configPath})
                  </span>
                  <span className="font-mono text-slate-500">
                    {configPkg.rustCrate}
                  </span>
                </div>
                <textarea
                  rows={10}
                  value={editedConfig}
                  onChange={(e) => setEditedConfig(e.target.value)}
                  className="w-full px-3.5 py-3 text-xs font-mono rounded-xl border border-slate-300 dark:border-slate-700 bg-slate-950 text-emerald-300 leading-relaxed focus:outline-hidden focus:border-orange-500"
                />
              </div>

              <div className="space-y-1.5">
                <div className="text-xs font-medium text-slate-700 dark:text-slate-300 flex items-center gap-1.5">
                  <Terminal className="w-3.5 h-3.5 text-orange-500" />
                  Nhật ký hoạt động Package (Systemd Journal)
                </div>
                <pre className="p-3.5 rounded-xl bg-slate-950 text-slate-300 font-mono text-xs overflow-x-auto max-h-36 leading-relaxed">
                  {(configPkg.logs || []).join('\n')}
                </pre>
              </div>

              <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-200 dark:border-slate-800">
                <button
                  type="button"
                  onClick={() => setConfigPkg(null)}
                  className="px-4 py-2 text-xs font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg cursor-pointer"
                >
                  Đóng
                </button>
                <button
                  type="submit"
                  disabled={savingConfig}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                >
                  <Save className="w-3.5 h-3.5" />
                  {savingConfig
                    ? 'Đang ghi & Reload...'
                    : 'Lưu Cấu hình & Gửi SIGHUP'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* 1-Click Deploy Modal */}
      {selectedTpl && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/70 backdrop-blur-xs p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl max-w-lg w-full overflow-hidden shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Layers className="w-4 h-4 text-orange-500" />
                <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 font-display">
                  Triển khai 1-Click: {selectedTpl.name}
                </h2>
              </div>
              <button
                onClick={() => setSelectedTpl(null)}
                className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 rounded-lg cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleConfirmDeploy} className="p-6 space-y-4">
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                  Tên miền Website (Domain)
                </label>
                <input
                  type="text"
                  required
                  value={domainInput}
                  onChange={(e) => setDomainInput(e.target.value)}
                  placeholder="vd: app.ferroushub.vn"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                  Tùy chọn Web Server &amp; Reverse Proxy Engine
                </label>
                <select
                  value={webServerEngine}
                  onChange={(e) =>
                    setWebServerEngine(e.target.value as WebServerEngineType)
                  }
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  <option value="FerrousX (Rust Web Server)">
                    FerrousX (https://github.com/Ferrous-panel/FerrousX) — Rust Web Server
                  </option>
                  <option value="Nginx 1.26">
                    Nginx 1.26 — Traditional C Web Server
                  </option>
                </select>
              </div>

              <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg p-3.5 space-y-2 text-xs font-mono">
                <div className="flex justify-between gap-2">
                  <span className="text-slate-500">Web Server VHost:</span>
                  <span className="text-orange-600 dark:text-orange-400 truncate">
                    {webServerEngine}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-500">Thư mục Web Root:</span>
                  <span className="text-slate-900 dark:text-slate-100">
                    /www/wwwroot/{domainInput || 'domain'}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-500">Lệnh Build tự động:</span>
                  <span className="text-emerald-600 dark:text-emerald-400">
                    {selectedTpl.buildCommand}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-500">Nạp môi trường tự động:</span>
                  <span className="text-sky-600 dark:text-sky-400 text-right">
                    Tự cài Runtime + Build + Systemd + FerrousX VHost
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-slate-500">Chứng chỉ bảo mật:</span>
                  <span className="text-slate-900 dark:text-slate-100">
                    FerrousACME Let&apos;s Encrypt TLS 1.3 + HTTP/3
                  </span>
                </div>
              </div>

              {selectedTpl.requiresDb !== 'None' && (
                <label className="flex items-center gap-2.5 text-xs text-slate-700 dark:text-slate-300 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={createDb}
                    onChange={(e) => setCreateDb(e.target.checked)}
                    className="rounded border-slate-300 text-orange-600 focus:ring-orange-500"
                  />
                  <span>
                    Tự động khởi tạo cơ sở dữ liệu{' '}
                    <strong>{selectedTpl.requiresDb}</strong> và gắn biến môi
                    trường
                  </span>
                </label>
              )}

              <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-200 dark:border-slate-800">
                <button
                  type="button"
                  onClick={() => setSelectedTpl(null)}
                  className="px-4 py-2 text-xs font-medium text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg cursor-pointer"
                >
                  Hủy
                </button>
                <button
                  type="submit"
                  disabled={deploying}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                >
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  {deploying
                    ? 'Đang cài đặt...'
                    : 'Xác nhận Deploy Website'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
