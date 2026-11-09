import React, { useState, useEffect } from 'react';
import {
  Check,
  CheckCircle2,
  Copy,
  FileCode2,
  Hammer,
  Play,
  Plus,
  RefreshCw,
  Search,
  Sliders,
  Square,
  Terminal,
  Trash2
} from 'lucide-react';
import { EnvVar, ManagedApp } from '../types';

interface AppsManagerViewProps {
  apps: ManagedApp[];
  selectedAppId: string | null;
  onSelectApp: (id: string) => void;
  onAppAction: (appId: string, action: 'start' | 'stop' | 'restart' | 'rebuild') => Promise<void>;
  onUpdateConfig: (
    appId: string,
    payload: {
      workers: number;
      memoryLimitMb: number;
      rateLimitRps: number;
      envVars: EnvVar[];
    }
  ) => Promise<void>;
  onOpenDeployModal: () => void;
}

export const AppsManagerView: React.FC<AppsManagerViewProps> = ({
  apps,
  selectedAppId,
  onSelectApp,
  onAppAction,
  onUpdateConfig,
  onOpenDeployModal
}) => {
  const [filterType, setFilterType] = useState<'all' | 'rust' | 'other'>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [detailTab, setDetailTab] = useState<'config' | 'systemd' | 'logs'>('config');

  const activeApp =
    apps.find((a) => a.id === selectedAppId) || apps[0] || null;

  const [workers, setWorkers] = useState<number>(activeApp?.workers ?? 4);
  const [memoryLimitMb, setMemoryLimitMb] = useState<number>(
    activeApp?.memoryLimitMb ?? 512
  );
  const [rateLimitRps, setRateLimitRps] = useState<number>(
    activeApp?.rateLimitRps ?? 100
  );
  const [envVars, setEnvVars] = useState<EnvVar[]>(activeApp?.envVars ?? []);
  const [newEnvKey, setNewEnvKey] = useState('');
  const [newEnvValue, setNewEnvValue] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [copiedUnit, setCopiedUnit] = useState(false);

  useEffect(() => {
    if (activeApp) {
      setWorkers(activeApp.workers);
      setMemoryLimitMb(activeApp.memoryLimitMb);
      setRateLimitRps(activeApp.rateLimitRps);
      setEnvVars(activeApp.envVars);
    }
  }, [activeApp?.id]);

  const filteredApps = apps.filter((app) => {
    const matchesSearch =
      app.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      app.domain.toLowerCase().includes(searchQuery.toLowerCase()) ||
      app.framework.toLowerCase().includes(searchQuery.toLowerCase()) ||
      String(app.internalPort).includes(searchQuery);

    if (!matchesSearch) return false;
    if (filterType === 'rust') return app.framework.includes('Rust');
    if (filterType === 'other') return !app.framework.includes('Rust');
    return true;
  });

  const handleAddEnv = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newEnvKey.trim()) return;
    setEnvVars((prev) => [
      ...prev.filter((item) => item.key !== newEnvKey.trim()),
      { key: newEnvKey.trim().toUpperCase(), value: newEnvValue.trim() }
    ]);
    setNewEnvKey('');
    setNewEnvValue('');
  };

  const handleRemoveEnv = (key: string) => {
    setEnvVars((prev) => prev.filter((item) => item.key !== key));
  };

  const handleSaveChanges = async () => {
    if (!activeApp) return;
    setSaving(true);
    await onUpdateConfig(activeApp.id, {
      workers,
      memoryLimitMb,
      rateLimitRps,
      envVars
    });
    setSaving(false);
    setSaveSuccess(true);
    setTimeout(() => setSaveSuccess(false), 2500);
  };

  const handleCopySystemd = () => {
    if (!activeApp) return;
    navigator.clipboard.writeText(activeApp.systemdUnit);
    setCopiedUnit(true);
    setTimeout(() => setCopiedUnit(false), 2000);
  };

  return (
    <div className="space-y-6">
      {/* Toolbar: Search, Segmented Runtime Filter & Primary Deploy Button */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className="relative">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Tìm tên dịch vụ, port, domain..."
              className="pl-9 pr-4 py-2 text-xs rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-orange-500/40 w-64"
            />
          </div>

          <div className="flex items-center gap-1 p-1 bg-slate-200/70 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-lg">
            <button
              onClick={() => setFilterType('all')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                filterType === 'all'
                  ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
              }`}
            >
              Tất cả ({apps.length})
            </button>
            <button
              onClick={() => setFilterType('rust')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                filterType === 'rust'
                  ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
              }`}
            >
              Rust Actix / Axum ({apps.filter((a) => a.framework.includes('Rust')).length})
            </button>
            <button
              onClick={() => setFilterType('other')}
              className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                filterType === 'other'
                  ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
              }`}
            >
              Node / Python / Go ({apps.filter((a) => !a.framework.includes('Rust')).length})
            </button>
          </div>
        </div>

        <button
          onClick={onOpenDeployModal}
          className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
        >
          <Plus className="w-3.5 h-3.5" />
          Triển khai Dịch vụ Mới
        </button>
      </div>

      {/* Master-Detail Split Layout */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Left List of Services */}
        <div className="lg:col-span-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 divide-y divide-slate-200 dark:divide-slate-800 overflow-hidden">
          {filteredApps.length === 0 ? (
            <div className="p-8 text-center space-y-3">
              <p className="text-sm text-slate-600 dark:text-slate-400">
                Không tìm thấy dịch vụ nào khớp với bộ lọc hiện tại.
              </p>
              <button
                onClick={() => {
                  setFilterType('all');
                  setSearchQuery('');
                }}
                className="px-3 py-1.5 text-xs font-medium text-orange-600 dark:text-orange-400 hover:underline cursor-pointer"
              >
                Đặt lại bộ lọc
              </button>
            </div>
          ) : (
            filteredApps.map((app) => {
              const isSelected = activeApp?.id === app.id;
              return (
                <button
                  key={app.id}
                  onClick={() => onSelectApp(app.id)}
                  className={`w-full text-left p-5 transition-colors block cursor-pointer ${
                    isSelected
                      ? 'bg-orange-50/60 dark:bg-orange-950/20'
                      : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="space-y-1 min-w-0">
                      <div className="font-semibold text-sm text-slate-900 dark:text-slate-100 truncate">
                        {app.name}
                      </div>
                      <div className="text-xs text-slate-500 dark:text-slate-400">
                        {app.framework} · 127.0.0.1:{app.internalPort}
                      </div>
                    </div>

                    <div className="text-xs shrink-0">
                      {app.status === 'running' ? (
                        <span className="inline-flex items-center gap-1 font-medium text-emerald-600 dark:text-emerald-400">
                          <CheckCircle2 className="w-3.5 h-3.5" />
                          Running
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1 font-medium text-slate-500 dark:text-slate-400">
                          <Square className="w-3.5 h-3.5" />
                          Stopped
                        </span>
                      )}
                    </div>
                  </div>

                  <div className="mt-3 pt-3 border-t border-slate-100 dark:border-slate-800/70 flex items-center justify-between text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                    <span>{app.domain}</span>
                    <span>
                      {app.workers}W · {app.memoryUsedMb}MB · {app.p99LatencyMs}ms
                    </span>
                  </div>
                </button>
              );
            })
          )}
        </div>

        {/* Right Service Inspector & Control Console */}
        {activeApp && (
          <div className="lg:col-span-7 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
            {/* Inspector Header */}
            <div className="p-6 border-b border-slate-200 dark:border-slate-800 space-y-4">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div className="space-y-1">
                  <div className="text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums">
                    {activeApp.framework} · {activeApp.runtimeVersion} ·{' '}
                    {activeApp.pid ? `PID ${activeApp.pid}` : 'Offline'}
                  </div>
                  <h2 className="text-lg font-bold text-slate-900 dark:text-slate-100">
                    {activeApp.name}
                  </h2>
                  <p className="text-xs font-mono text-slate-500 dark:text-slate-400">
                    ExecStart: {activeApp.execCommand}
                  </p>
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  {activeApp.status === 'running' ? (
                    <>
                      <button
                        onClick={() => onAppAction(activeApp.id, 'restart')}
                        className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        <RefreshCw className="w-3.5 h-3.5" />
                        Restart
                      </button>
                      <button
                        onClick={() => onAppAction(activeApp.id, 'rebuild')}
                        className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        <Hammer className="w-3.5 h-3.5" />
                        Rebuild Release
                      </button>
                      <button
                        onClick={() => onAppAction(activeApp.id, 'stop')}
                        className="px-3 py-2 text-xs font-medium text-rose-600 dark:text-rose-400 bg-rose-50 dark:bg-rose-950/40 hover:bg-rose-100 dark:hover:bg-rose-900/50 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        <Square className="w-3.5 h-3.5" />
                        Dừng
                      </button>
                    </>
                  ) : (
                    <button
                      onClick={() => onAppAction(activeApp.id, 'start')}
                      className="px-4 py-2 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                    >
                      <Play className="w-3.5 h-3.5" />
                      Khởi chạy dịch vụ
                    </button>
                  )}
                </div>
              </div>

              {/* Sub-Navigation Tabs */}
              <div className="flex items-center gap-1 p-1 bg-slate-100 dark:bg-slate-950 rounded-lg w-fit">
                <button
                  onClick={() => setDetailTab('config')}
                  className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                    detailTab === 'config'
                      ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                  }`}
                >
                  <Sliders className="w-3.5 h-3.5" />
                  Tài nguyên & Biến môi trường (.env)
                </button>
                <button
                  onClick={() => setDetailTab('systemd')}
                  className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                    detailTab === 'systemd'
                      ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                  }`}
                >
                  <FileCode2 className="w-3.5 h-3.5" />
                  Systemd Sandbox Unit
                </button>
                <button
                  onClick={() => setDetailTab('logs')}
                  className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                    detailTab === 'logs'
                      ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200'
                  }`}
                >
                  <Terminal className="w-3.5 h-3.5" />
                  Nhật ký Journalctl ({activeApp.logs.length})
                </button>
              </div>
            </div>

            {/* Tab 1: Resource Quota & Env Vars */}
            {detailTab === 'config' && (
              <div className="p-6 space-y-6">
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Số luồng Worker (ACTIX_WORKERS)
                    </label>
                    <input
                      type="number"
                      min={1}
                      max={64}
                      value={workers}
                      onChange={(e) => setWorkers(Number(e.target.value))}
                      className="w-full px-3 py-2 text-sm font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Giới hạn RAM (Systemd MemoryMax MB)
                    </label>
                    <input
                      type="number"
                      min={64}
                      max={16384}
                      step={64}
                      value={memoryLimitMb}
                      onChange={(e) => setMemoryLimitMb(Number(e.target.value))}
                      className="w-full px-3 py-2 text-sm font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Giới hạn Rate Limit (Nginx req/s)
                    </label>
                    <input
                      type="number"
                      min={10}
                      max={5000}
                      step={10}
                      value={rateLimitRps}
                      onChange={(e) => setRateLimitRps(Number(e.target.value))}
                      className="w-full px-3 py-2 text-sm font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                {/* Environment Variables Editor */}
                <div className="space-y-3 pt-2 border-t border-slate-200 dark:border-slate-800">
                  <div className="flex items-center justify-between">
                    <h3 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                      Biến Môi trường Thực thi (Systemd Environment)
                    </h3>
                    <span className="text-xs text-slate-500 dark:text-slate-400 font-mono">
                      /etc/systemd/system/{activeApp.name}.service
                    </span>
                  </div>

                  <div className="divide-y divide-slate-200 dark:divide-slate-800 border border-slate-200 dark:border-slate-800 rounded-lg">
                    {envVars.map((env) => (
                      <div
                        key={env.key}
                        className="px-3.5 py-2.5 flex items-center justify-between gap-3 text-xs font-mono"
                      >
                        <div className="truncate">
                          <span className="font-semibold text-slate-900 dark:text-slate-100">
                            {env.key}
                          </span>
                          <span className="text-slate-400 mx-1.5">=</span>
                          <span className="text-slate-600 dark:text-slate-300">
                            {env.value}
                          </span>
                        </div>
                        <button
                          onClick={() => handleRemoveEnv(env.key)}
                          className="text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors shrink-0 cursor-pointer"
                          title="Xóa biến môi trường"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    ))}
                  </div>

                  <form onSubmit={handleAddEnv} className="flex flex-col sm:flex-row gap-2">
                    <input
                      type="text"
                      placeholder="TÊN_BIẾN (VD: RUST_BACKTRACE)"
                      value={newEnvKey}
                      onChange={(e) => setNewEnvKey(e.target.value)}
                      className="sm:w-2/5 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                    <input
                      type="text"
                      placeholder="Giá trị (VD: full)"
                      value={newEnvValue}
                      onChange={(e) => setNewEnvValue(e.target.value)}
                      className="flex-1 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                    <button
                      type="submit"
                      className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer"
                    >
                      + Thêm biến
                    </button>
                  </form>
                </div>

                <div className="pt-4 border-t border-slate-200 dark:border-slate-800 flex items-center justify-between gap-4">
                  <div className="text-xs text-slate-500 dark:text-slate-400">
                    Lưu thay đổi sẽ tự động chạy <code className="font-mono">systemctl daemon-reload</code> và nạp lại Nginx.
                  </div>
                  <button
                    onClick={handleSaveChanges}
                    disabled={saving}
                    className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    {saveSuccess ? (
                      <>
                        <Check className="w-3.5 h-3.5" />
                        Đã áp dụng cấu hình
                      </>
                    ) : saving ? (
                      'Đang áp dụng...'
                    ) : (
                      'Lưu & Reload Systemd'
                    )}
                  </button>
                </div>
              </div>
            )}

            {/* Tab 2: Hardened Systemd Unit Preview */}
            {detailTab === 'systemd' && (
              <div className="p-6 space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-sm font-semibold text-slate-900 dark:text-slate-100">
                      /etc/systemd/system/{activeApp.name}.service
                    </h3>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      Tự động bật cô lập nhân Ubuntu (ProtectSystem=strict, PrivateTmp, NoNewPrivileges)
                    </p>
                  </div>
                  <button
                    onClick={handleCopySystemd}
                    className="px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    {copiedUnit ? (
                      <>
                        <Check className="w-3.5 h-3.5 text-emerald-500" />
                        Đã sao chép
                      </>
                    ) : (
                      <>
                        <Copy className="w-3.5 h-3.5" />
                        Sao chép Unit File
                      </>
                    )}
                  </button>
                </div>

                <pre className="p-4 rounded-lg bg-slate-950 text-slate-100 font-mono text-xs overflow-x-auto leading-relaxed border border-slate-800">
                  {activeApp.systemdUnit}
                </pre>
              </div>
            )}

            {/* Tab 3: Live Systemd Journalctl Logs */}
            {detailTab === 'logs' && (
              <div className="p-6 space-y-4">
                <div className="flex items-center justify-between">
                  <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
                    journalctl -u {activeApp.name}.service -n 50 --no-pager
                  </div>
                  <button
                    onClick={() => onAppAction(activeApp.id, 'restart')}
                    className="px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer"
                  >
                    Ghi log khởi động lại
                  </button>
                </div>

                <div className="p-4 rounded-lg bg-slate-950 text-slate-200 font-mono text-xs space-y-1.5 overflow-x-auto max-h-96 border border-slate-800">
                  {activeApp.logs.map((line, idx) => (
                    <div key={idx} className="leading-relaxed whitespace-pre-wrap">
                      {line}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
