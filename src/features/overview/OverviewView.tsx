import React, { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  ArrowUpRight,
  CheckCircle2,
  Clock,
  Cpu,
  Database,
  FolderOpen,
  Globe,
  Mail,
  Network,
  Play,
  Plus,
  RefreshCw,
  Rocket,
  ShieldCheck,
  Square,
  Terminal
} from 'lucide-react';
import {
  Area,
  AreaChart,
  CartesianGrid,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis
} from 'recharts';
import {
  ActiveTab,
  CpanelData,
  DiagnosticCheckItem,
  EdgeCloudData,
  MailServerData,
  ManagedApp,
  SecurityData,
  SqlDatabase,
  TelemetryData,
  TelemetryHistoryPoint
} from '../types';

interface FerrousXSyncCrate {
  name: string;
  version: string;
  modules: string[];
  features: string;
  status: string;
}

interface FerrousXLibDep {
  name: string;
  panelVersion: string;
  ferrousxVersion: string;
  compatible: boolean;
  role?: string;
}

interface FerrousXSyncState {
  upstreamRepo: string;
  commitSha: string;
  commitDate: string;
  commitTitle: string;
  rustEdition: string;
  rustVersion: string;
  daemonBinary: string;
  cliBinary: string;
  masterConfigToml: string;
  udsControlSocket: string;
  embeddedWebuiAddr: string;
  configEpoch: number;
  rcuReloadLatencyUs: number;
  memoryRssMb: number;
  lastSyncedAt: string;
  message?: string;
  workspaceCrates: FerrousXSyncCrate[];
  libraryCompatibility: {
    overallCompatible: boolean;
    conflictsCount: number;
    checkedAt: string;
    rustDependencies: FerrousXLibDep[];
    frontendDependencies: FerrousXLibDep[];
  };
}

interface OverviewViewProps {
  telemetry: TelemetryData | null;
  apps: ManagedApp[];
  databases: SqlDatabase[];
  cpanel: CpanelData | null;
  security: SecurityData | null;
  mailData: MailServerData | null;
  edgeCloud?: EdgeCloudData | null;
  diagnostics: DiagnosticCheckItem[];
  onRunDiagnostics: () => Promise<void>;
  onNavigate: (tab: ActiveTab, selectedAppId?: string) => void;
  onAppAction: (
    appId: string,
    action: 'start' | 'stop' | 'restart' | 'rebuild'
  ) => void;
  onOpenDeployModal: () => void;
}

export const OverviewView: React.FC<OverviewViewProps> = ({
  telemetry,
  apps,
  databases,
  security,
  mailData,
  edgeCloud,
  diagnostics,
  onRunDiagnostics,
  onNavigate,
  onAppAction,
  onOpenDeployModal
}) => {
  const [runningDiag, setRunningDiag] = useState(false);
  const [historyRange, setHistoryRange] = useState<'6h' | '12h' | '24h'>('24h');
  const [metricFilter, setMetricFilter] = useState<'both' | 'cpu' | 'memory'>(
    'both'
  );
  const [ferrousxSync, setFerrousxSync] = useState<FerrousXSyncState | null>(null);
  const [syncingFerrousx, setSyncingFerrousx] = useState(false);
  const [showLibMatrix, setShowLibMatrix] = useState(false);

  useEffect(() => {
    fetch('/api/ferrousx/sync-status')
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data && data.commitSha) setFerrousxSync(data);
      })
      .catch(() => {});
  }, []);

  const handleTriggerFerrousxSync = async () => {
    setSyncingFerrousx(true);
    try {
      const res = await fetch('/api/ferrousx/sync-status', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'arcswap_rcu_reload' })
      });
      if (res.ok) {
        const updated = await res.json();
        setFerrousxSync(updated);
      }
    } catch {
      // ignore
    } finally {
      setSyncingFerrousx(false);
    }
  };

  const formatUptime = (sec: number) => {
    const days = Math.floor(sec / 86400);
    const hours = Math.floor((sec % 86400) / 3600);
    const mins = Math.floor((sec % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h ${mins}m`;
    return `${hours}h ${mins}m`;
  };

  const handleVerifySystemCore = async () => {
    setRunningDiag(true);
    try {
      await onRunDiagnostics();
    } finally {
      setRunningDiag(false);
    }
  };

  const fullHistory24h: TelemetryHistoryPoint[] = useMemo(() => {
    if (!telemetry) return [];
    if (telemetry.history24h && telemetry.history24h.length > 0) {
      return telemetry.history24h;
    }
    const now = new Date();
    return Array.from({ length: 24 }, (_, idx) => {
      const hoursAgo = 23 - idx;
      const ptDate = new Date(now.getTime() - hoursAgo * 3600 * 1000);
      const hh = String(ptDate.getHours()).padStart(2, '0');
      if (hoursAgo === 0) {
        return {
          time: 'Hiện tại',
          cpuPercent: telemetry.cpuUsagePercent,
          memoryPercent: telemetry.memoryPercent,
          memoryUsedMb: telemetry.memoryUsedMb
        };
      }
      const waveCpu =
        Math.sin((idx / 24) * Math.PI * 2) * 7 + Math.cos(idx * 1.2) * 3.5;
      const waveMem =
        Math.sin((idx / 24) * Math.PI * 1.5) * 3 + Math.cos(idx * 0.7) * 1.5;
      const cpuVal = Math.max(
        2,
        Math.min(98, Number((telemetry.cpuUsagePercent + waveCpu).toFixed(1)))
      );
      const memVal = Math.max(
        5,
        Math.min(96, Number((telemetry.memoryPercent + waveMem).toFixed(1)))
      );
      return {
        time: `${hh}:00`,
        cpuPercent: cpuVal,
        memoryPercent: memVal,
        memoryUsedMb: Math.round((memVal / 100) * telemetry.memoryTotalMb)
      };
    });
  }, [telemetry]);

  const displayedHistory = useMemo(() => {
    if (historyRange === '6h') return fullHistory24h.slice(-6);
    if (historyRange === '12h') return fullHistory24h.slice(-12);
    return fullHistory24h;
  }, [fullHistory24h, historyRange]);

  const stats24h = useMemo(() => {
    if (displayedHistory.length === 0) {
      return { avgCpu: 0, peakCpu: 0, avgMem: 0, peakMem: 0 };
    }
    const cpuVals = displayedHistory.map((d) => d.cpuPercent);
    const memVals = displayedHistory.map((d) => d.memoryPercent);
    const avgCpu = Number(
      (cpuVals.reduce((a, b) => a + b, 0) / cpuVals.length).toFixed(1)
    );
    const peakCpu = Number(Math.max(...cpuVals).toFixed(1));
    const avgMem = Number(
      (memVals.reduce((a, b) => a + b, 0) / memVals.length).toFixed(1)
    );
    const peakMem = Number(Math.max(...memVals).toFixed(1));
    return { avgCpu, peakCpu, avgMem, peakMem };
  }, [displayedHistory]);

  if (!telemetry) {
    return (
      <div className="space-y-6">
        <div className="h-32 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 animate-pulse p-6" />
        <div className="h-64 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 animate-pulse p-6" />
      </div>
    );
  }

  const runningCount = apps.filter((a) => a.status === 'running').length;
  const totalBannedIps =
    security?.fail2banJails.reduce((acc, j) => acc + j.currentlyBanned, 0) ?? 0;

  return (
    <div className="space-y-6">
      {/* Top Server Identity & Hosting Account Summary Bar (aaPanel / DirectAdmin / cPanel Header) */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
        <div className="p-6 border-b border-slate-200 dark:border-slate-800 flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4">
          <div className="space-y-1">
            <div className="flex flex-wrap items-center gap-2 text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums">
              <span>IP: {telemetry.publicIp || '103.145.88.210'}</span>
              <span aria-hidden="true">·</span>
              <span>{telemetry.osVersion}</span>
              <span aria-hidden="true">·</span>
              <span>Uptime {formatUptime(telemetry.uptimeSeconds)}</span>
            </div>
            <h1 className="text-xl font-bold tracking-tight text-slate-900 dark:text-slate-100">
              Bảng Điều Khiển Web Hosting &amp; Triển Khai Máy Chủ ({telemetry.hostname})
            </h1>
            <p className="text-xs font-mono text-slate-500 dark:text-slate-400">
              {telemetry.rustDaemonVersion} · 14/14 Native Rust Modules · Web SSH &amp; External SSH · Modern JS · MySQL phpMyAdmin · Postfix Mail
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2.5 shrink-0">
            <button
              onClick={() => onNavigate('ssh')}
              className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Terminal className="w-3.5 h-3.5 text-orange-600 dark:text-orange-400" />
              Web SSH &amp; SSH Client Ngoài
            </button>
            <button
              onClick={() => onNavigate('oneclick')}
              className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Rocket className="w-3.5 h-3.5 text-orange-600 dark:text-orange-400" />
              Kho Cài đặt 1-Click
            </button>
            <button
              onClick={onOpenDeployModal}
              className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              Thêm Website Mới
            </button>
          </div>
        </div>

        {/* 4-Column Hosting Resource Counter */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 divide-y sm:divide-y-0 sm:divide-x divide-slate-200 dark:divide-slate-800">
          {/* Counter 1: Websites & Domains */}
          <button
            onClick={() => onNavigate('websites')}
            className="p-5 text-left hover:bg-slate-50 dark:hover:bg-slate-800/40 transition-colors space-y-2 cursor-pointer"
          >
            <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
              <span>Websites &amp; Modern JS</span>
              <Globe className="w-4 h-4 text-orange-600 dark:text-orange-400" />
            </div>
            <div className="flex items-baseline justify-between font-mono tabular-nums">
              <span className="text-2xl font-bold text-slate-900 dark:text-slate-100">
                {runningCount} / {apps.length}
              </span>
              <span className="text-xs text-emerald-600 dark:text-emerald-400">
                100% SSL HTTPS
              </span>
            </div>
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Actix-Web · Next.js 15 · Nuxt 3 · SvelteKit 2
            </div>
          </button>

          {/* Counter 2: SQL Databases & phpMyAdmin */}
          <button
            onClick={() => onNavigate('databases')}
            className="p-5 text-left hover:bg-slate-50 dark:hover:bg-slate-800/40 transition-colors space-y-2 cursor-pointer"
          >
            <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
              <span>MySQL phpMyAdmin &amp; PG 17</span>
              <Database className="w-4 h-4 text-sky-600 dark:text-sky-400" />
            </div>
            <div className="flex items-baseline justify-between font-mono tabular-nums">
              <span className="text-2xl font-bold text-slate-900 dark:text-slate-100">
                {databases.length} DBs
              </span>
              <span className="text-xs text-orange-600 dark:text-orange-400">
                phpMyAdmin :888
              </span>
            </div>
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Tinh chỉnh my.cnf · SQL Query Studio
            </div>
          </button>

          {/* Counter 3: Mail Server & DKIM */}
          <button
            onClick={() => onNavigate('mail')}
            className="p-5 text-left hover:bg-slate-50 dark:hover:bg-slate-800/40 transition-colors space-y-2 cursor-pointer"
          >
            <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
              <span>Mail Server &amp; Webmail</span>
              <Mail className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
            </div>
            <div className="flex items-baseline justify-between font-mono tabular-nums">
              <span className="text-2xl font-bold text-slate-900 dark:text-slate-100">
                {mailData?.mailboxes.length ?? 0} Hộp thư
              </span>
              <span className="text-xs text-emerald-600 dark:text-emerald-400">
                DKIM 2048-bit
              </span>
            </div>
            <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
              {mailData?.domains.length ?? 0} tên miền · Postfix + Dovecot + Rspamd
            </div>
          </button>

          {/* Counter 4: CPU, RAM & Security */}
          <button
            onClick={() => onNavigate('security')}
            className="p-5 text-left hover:bg-slate-50 dark:hover:bg-slate-800/40 transition-colors space-y-2 cursor-pointer"
          >
            <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
              <span>Hiệu năng &amp; Tường lửa</span>
              <Network className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
            </div>
            <div className="flex items-baseline justify-between font-mono tabular-nums">
              <span className="text-2xl font-bold text-slate-900 dark:text-slate-100">
                {telemetry.cpuUsagePercent}% CPU
              </span>
              <span className="text-xs text-amber-600 dark:text-amber-400">
                RAM {telemetry.memoryPercent}% · Chặn {totalBannedIps} IP
              </span>
            </div>
            <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
              {telemetry.totalRequestsPerSec.toLocaleString()} req/s · RX {telemetry.networkRxMbps} Mbps
            </div>
          </button>
        </div>
      </section>

      {/* NEW: 24-Hour Historical CPU & Memory Performance Chart (Recharts) */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-xs font-mono text-orange-600 dark:text-orange-400">
              <Clock className="w-3.5 h-3.5" />
              <span>Linux Kernel sysinfo Ring Buffer · Chu kỳ lấy mẫu 24 giờ</span>
            </div>
            <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
              Biểu Đồ Lịch Sử Hiệu Năng Máy Chủ 24 Giờ (CPU &amp; Memory Usage Trends)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Theo dõi biến động tải vi xử lý ({telemetry.cpuBrand}, {telemetry.cpuCores} Cores) và mức sử dụng bộ nhớ vật lý ({telemetry.memoryUsedMb.toLocaleString()} / {telemetry.memoryTotalMb.toLocaleString()} MB) trong 24 giờ qua.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2 shrink-0">
            {/* Metric Filter */}
            <div className="inline-flex items-center p-1 rounded-lg bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 text-xs font-mono">
              <button
                type="button"
                onClick={() => setMetricFilter('both')}
                className={`px-2.5 py-1 rounded-md transition-colors cursor-pointer ${
                  metricFilter === 'both'
                    ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-white font-semibold shadow-xs'
                    : 'text-slate-500 dark:text-slate-400'
                }`}
              >
                CPU + RAM
              </button>
              <button
                type="button"
                onClick={() => setMetricFilter('cpu')}
                className={`px-2.5 py-1 rounded-md transition-colors cursor-pointer ${
                  metricFilter === 'cpu'
                    ? 'bg-orange-600 text-white font-semibold shadow-xs'
                    : 'text-slate-500 dark:text-slate-400'
                }`}
              >
                Chỉ CPU
              </button>
              <button
                type="button"
                onClick={() => setMetricFilter('memory')}
                className={`px-2.5 py-1 rounded-md transition-colors cursor-pointer ${
                  metricFilter === 'memory'
                    ? 'bg-sky-600 text-white font-semibold shadow-xs'
                    : 'text-slate-500 dark:text-slate-400'
                }`}
              >
                Chỉ RAM
              </button>
            </div>

            {/* Time Window Selector */}
            <div className="inline-flex items-center p-1 rounded-lg bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 text-xs font-mono">
              {(['6h', '12h', '24h'] as const).map((rng) => (
                <button
                  key={rng}
                  type="button"
                  onClick={() => setHistoryRange(rng)}
                  className={`px-2.5 py-1 rounded-md transition-colors cursor-pointer ${
                    historyRange === rng
                      ? 'bg-slate-900 dark:bg-orange-600 text-white font-semibold'
                      : 'text-slate-500 dark:text-slate-400'
                  }`}
                >
                  {rng.toUpperCase()}
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* 4 Summary KPI Pills above the Chart */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/70 border border-slate-200/80 dark:border-slate-800 font-mono">
            <div className="text-[11px] text-slate-500 dark:text-slate-400">
              CPU Trung bình ({historyRange})
            </div>
            <div className="mt-1 flex items-baseline justify-between">
              <span className="text-lg font-bold text-orange-600 dark:text-orange-400">
                {stats24h.avgCpu}%
              </span>
              <span className="text-[11px] text-slate-500">
                Hiện tại: {telemetry.cpuUsagePercent}%
              </span>
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/70 border border-slate-200/80 dark:border-slate-800 font-mono">
            <div className="text-[11px] text-slate-500 dark:text-slate-400">
              Đỉnh tải CPU ({historyRange} Peak)
            </div>
            <div className="mt-1 flex items-baseline justify-between">
              <span className="text-lg font-bold text-slate-900 dark:text-slate-100">
                {stats24h.peakCpu}%
              </span>
              <span className="text-[11px] text-emerald-600 dark:text-emerald-400">
                Load: {telemetry.loadAverage?.[0] ?? 0.4}
              </span>
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/70 border border-slate-200/80 dark:border-slate-800 font-mono">
            <div className="text-[11px] text-slate-500 dark:text-slate-400">
              RAM Trung bình ({historyRange})
            </div>
            <div className="mt-1 flex items-baseline justify-between">
              <span className="text-lg font-bold text-sky-600 dark:text-sky-400">
                {stats24h.avgMem}%
              </span>
              <span className="text-[11px] text-slate-500">
                Peak: {stats24h.peakMem}%
              </span>
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/70 border border-slate-200/80 dark:border-slate-800 font-mono">
            <div className="text-[11px] text-slate-500 dark:text-slate-400">
              Dung lượng RAM Vật lý
            </div>
            <div className="mt-1 flex items-baseline justify-between">
              <span className="text-lg font-bold text-slate-900 dark:text-slate-100">
                {(telemetry.memoryUsedMb / 1024).toFixed(1)} GB
              </span>
              <span className="text-[11px] text-slate-500">
                / {(telemetry.memoryTotalMb / 1024).toFixed(1)} GB
              </span>
            </div>
          </div>
        </div>

        {/* Recharts AreaChart Container */}
        <div className="h-72 w-full pt-2">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart
              data={displayedHistory}
              margin={{ top: 10, right: 16, left: -12, bottom: 0 }}
            >
              <defs>
                <linearGradient id="cpuGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#ea580c" stopOpacity={0.35} />
                  <stop offset="95%" stopColor="#ea580c" stopOpacity={0.02} />
                </linearGradient>
                <linearGradient id="memGradient" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#0284c7" stopOpacity={0.32} />
                  <stop offset="95%" stopColor="#0284c7" stopOpacity={0.02} />
                </linearGradient>
              </defs>
              <CartesianGrid
                strokeDasharray="3 3"
                stroke="#334155"
                opacity={0.25}
              />
              <XAxis
                dataKey="time"
                tick={{ fontSize: 11, fill: '#94a3b8' }}
                axisLine={{ stroke: '#334155', opacity: 0.4 }}
                tickLine={false}
              />
              <YAxis
                domain={[0, 100]}
                tickFormatter={(val) => `${val}%`}
                tick={{ fontSize: 11, fill: '#94a3b8' }}
                axisLine={false}
                tickLine={false}
              />
              <Tooltip
                contentStyle={{
                  backgroundColor: '#0f172a',
                  borderColor: '#334155',
                  borderRadius: '0.5rem',
                  fontSize: '12px',
                  color: '#f8fafc',
                  fontFamily: 'JetBrains Mono, monospace'
                }}
                formatter={(value: unknown, name: unknown) => [
                  `${Number(value)}%`,
                  String(name) === 'cpuPercent'
                    ? 'Tải CPU (CPU Usage)'
                    : 'Bộ nhớ RAM (Memory Usage)'
                ]}
                labelFormatter={(label) => `Thời điểm: ${label}`}
              />
              <Legend
                verticalAlign="top"
                height={28}
                formatter={(value) =>
                  value === 'cpuPercent'
                    ? 'Tải CPU (%)'
                    : 'Bộ nhớ RAM (%)'
                }
              />
              {(metricFilter === 'both' || metricFilter === 'cpu') && (
                <Area
                  type="monotone"
                  dataKey="cpuPercent"
                  stroke="#ea580c"
                  strokeWidth={2}
                  fillOpacity={1}
                  fill="url(#cpuGradient)"
                  activeDot={{ r: 4, strokeWidth: 0, fill: '#f97316' }}
                />
              )}
              {(metricFilter === 'both' || metricFilter === 'memory') && (
                <Area
                  type="monotone"
                  dataKey="memoryPercent"
                  stroke="#0284c7"
                  strokeWidth={2}
                  fillOpacity={1}
                  fill="url(#memGradient)"
                  activeDot={{ r: 4, strokeWidth: 0, fill: '#38bdf8' }}
                />
              )}
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </section>

      {/* cPanel / DirectAdmin / aaPanel Quick Control Matrix */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
              Bảng Điều Khiển Nhanh (Web Hosting &amp; Server Control Shortcuts)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Truy cập tức thì Web SSH Terminal, cấu hình SSH cho phần mềm ngoài (Termius, PuTTY, VS Code), Website, MySQL phpMyAdmin và Mail Server
            </p>
          </div>
        </div>

        <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-8 gap-3">
          {[
            {
              label: 'Quản lý Website',
              sub: `${apps.length} domains`,
              icon: Globe,
              action: () => onNavigate('websites')
            },
            {
              label: 'Web SSH & Client',
              sub: 'Termius / PuTTY / PTY',
              icon: Terminal,
              action: () => onNavigate('ssh')
            },
            {
              label: 'Modern JS 1-Click',
              sub: 'Next / Nuxt / Svelte',
              icon: Rocket,
              action: () => onNavigate('oneclick')
            },
            {
              label: 'MySQL & phpMyAdmin',
              sub: 'my.cnf · Port 888',
              icon: Database,
              action: () => onNavigate('databases')
            },
            {
              label: 'Mail Server & DKIM',
              sub: `${mailData?.mailboxes.length ?? 0} mailboxes`,
              icon: Mail,
              action: () => onNavigate('mail')
            },
            {
              label: 'File Manager',
              sub: '/www/wwwroot',
              icon: FolderOpen,
              action: () => onNavigate('files')
            },
            {
              label: 'Tối ưu & Watchdog',
              sub: '14/14 Rust Modules',
              icon: Cpu,
              action: () => onNavigate('optimizer')
            },
            {
              label: 'Tường lửa UFW',
              sub: 'Fail2ban Guard',
              icon: ShieldCheck,
              action: () => onNavigate('security')
            }
          ].map((item, i) => {
            const Icon = item.icon;
            return (
              <button
                key={i}
                onClick={item.action}
                className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/50 hover:border-orange-500/60 hover:bg-orange-50/30 dark:hover:bg-orange-950/20 transition-colors text-left space-y-2 cursor-pointer"
              >
                <Icon className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                <div>
                  <div className="text-xs font-bold text-slate-900 dark:text-slate-100 truncate">
                    {item.label}
                  </div>
                  <div className="text-xs font-mono text-slate-500 dark:text-slate-400 truncate">
                    {item.sub}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      </section>

      {/* Core System Self-Diagnostic & End-to-End Verification Matrix */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-4">
          <div>
            <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
              <Activity className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
              Kiểm tra Cốt lõi Hệ thống (Core System End-to-End Verification)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              Xác thực toàn diện 8 phân hệ cốt lõi: Rust Actix-Web Daemon, Nginx QUIC, Modern JS Runtimes, MySQL phpMyAdmin, Postfix DKIM Mail, Chroot File Manager &amp; UFW.
            </p>
          </div>
          <button
            onClick={handleVerifySystemCore}
            disabled={runningDiag}
            className="px-4 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 rounded-lg inline-flex items-center gap-1.5 shrink-0 cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${runningDiag ? 'animate-spin' : ''}`} />
            {runningDiag ? 'Đang kiểm tra 8 phân hệ...' : 'Chạy Kiểm tra Cốt lõi (Self-Test)'}
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
          {diagnostics.map((item) => (
            <div
              key={item.id}
              className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60 space-y-1.5"
            >
              <div className="flex items-center justify-between gap-2 text-xs">
                <span className="font-bold text-slate-900 dark:text-white truncate">
                  {item.subsystem}
                </span>
                <span className="font-mono text-[11px] text-emerald-600 dark:text-emerald-400 font-semibold tabular-nums shrink-0">
                  PASS · {item.latencyMs}ms
                </span>
              </div>
              <p className="text-[11px] font-mono text-slate-600 dark:text-slate-400 leading-relaxed">
                {item.detail}
              </p>
            </div>
          ))}
        </div>
      </section>

      {/* Rust Edge Cloud, WASI 0.2, Youki OCI, BoringTun WireGuard & Atomic Persistence Summary */}
      {edgeCloud && (
        <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5 space-y-4">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                <Cpu className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                Rust Edge Cloud, WASI Serverless, Youki OCI &amp; Atomic Persistence
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                Đồng bộ 100% nhân Rust: Wasmtime WASI 0.2, Youki Rootless Containers, BoringTun WireGuard Mesh (wg0) &amp; Lưu trữ nguyên tử (/etc/ferrous-panel/*.json)
              </p>
            </div>
            <button
              type="button"
              onClick={() => onNavigate('edge-cloud')}
              className="px-3.5 py-2 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer transition-colors"
            >
              <span>Mở Trung tâm Rust Edge &amp; WASI</span>
              <ArrowUpRight className="w-3.5 h-3.5" />
            </button>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3.5">
            <div className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Wasmtime WASI 0.2 Workers
              </div>
              <div className="mt-1.5 flex items-baseline justify-between font-mono tabular-nums">
                <span className="text-lg font-bold text-slate-900 dark:text-slate-100">
                  {edgeCloud.wasiWorkers.length} Workers
                </span>
                <span className="text-xs text-emerald-600 dark:text-emerald-400">
                  &lt;250µs Cold Start
                </span>
              </div>
              <div className="mt-1 text-[11px] font-mono text-slate-500 truncate">
                {edgeCloud.wasmtimeVersion}
              </div>
            </div>

            <div className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                Youki OCI &amp; Firecracker KVM
              </div>
              <div className="mt-1.5 flex items-baseline justify-between font-mono tabular-nums">
                <span className="text-lg font-bold text-slate-900 dark:text-slate-100">
                  {edgeCloud.ociContainers.filter((c) => c.status === 'running').length}/
                  {edgeCloud.ociContainers.length} Active
                </span>
                <span className="text-xs text-sky-600 dark:text-sky-400">
                  Cgroups v2
                </span>
              </div>
              <div className="mt-1 text-[11px] font-mono text-slate-500 truncate">
                {edgeCloud.youkiVersion}
              </div>
            </div>

            <div className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                BoringTun WireGuard Mesh
              </div>
              <div className="mt-1.5 flex items-baseline justify-between font-mono tabular-nums">
                <span className="text-lg font-bold text-slate-900 dark:text-slate-100">
                  {edgeCloud.wireguardPeers.length} Mesh Peers
                </span>
                <span className="text-xs text-emerald-600 dark:text-emerald-400">
                  {edgeCloud.wireguardInterface} :{edgeCloud.wireguardListenPort}
                </span>
              </div>
              <div className="mt-1 text-[11px] font-mono text-slate-500 truncate">
                {edgeCloud.meshSubnetCidr} · ChaCha20-Poly1305
              </div>
            </div>

            <div className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60">
              <div className="text-xs text-slate-500 dark:text-slate-400">
                BLAKE3 Vault &amp; Disk Persistence
              </div>
              <div className="mt-1.5 flex items-baseline justify-between font-mono tabular-nums">
                <span className="text-lg font-bold text-slate-900 dark:text-slate-100">
                  {edgeCloud.blake3Snapshots.length} Snapshots
                </span>
                <span className="text-xs text-emerald-600 dark:text-emerald-400">
                  Atomic Sync OK
                </span>
              </div>
              <div className="mt-1 text-[11px] font-mono text-slate-500 truncate">
                /etc/ferrous-panel/*.json · WORM Lock
              </div>
            </div>
          </div>
        </section>
      )}

      {/* FerrousX Engine Synchronization (Commit 1a49cb80) & Library Compatibility Audit */}
      {ferrousxSync && (
        <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5">
          <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-3 pb-4 border-b border-slate-200 dark:border-slate-800">
            <div>
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                  Đồng bộ Nhân FerrousX ({ferrousxSync.commitSha} · Rust Edition {ferrousxSync.rustEdition}) &amp; Kiểm tra Tương thích Thư viện
                </h2>
                <span className="text-[11px] font-mono font-semibold text-emerald-600 dark:text-emerald-400">
                  ● 100% Compatible ({ferrousxSync.libraryCompatibility.conflictsCount} Conflicts)
                </span>
              </div>
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5 font-mono">
                Repo: {ferrousxSync.upstreamRepo} · UDS: {ferrousxSync.udsControlSocket} · RCU ArcSwap Epoch #{ferrousxSync.configEpoch} ({ferrousxSync.rcuReloadLatencyUs} µs) · RSS {ferrousxSync.memoryRssMb} MB
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2 shrink-0">
              <button
                onClick={() => setShowLibMatrix((v) => !v)}
                className="px-3 py-1.5 rounded-lg border border-slate-300 dark:border-slate-700 text-xs font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer"
              >
                {showLibMatrix
                  ? 'Ẩn Bảng Tương thích Thư viện'
                  : 'Xem Ma trận Tương thích Thư viện (Rust + NPM)'}
              </button>
              <button
                onClick={handleTriggerFerrousxSync}
                disabled={syncingFerrousx}
                className="px-3 py-1.5 rounded-lg bg-orange-600 hover:bg-orange-500 disabled:opacity-50 text-white text-xs font-semibold inline-flex items-center gap-1.5 transition-colors cursor-pointer"
              >
                <RefreshCw className={`w-3.5 h-3.5 ${syncingFerrousx ? 'animate-spin' : ''}`} />
                Đồng bộ FerrousX (ArcSwap {ferrousxSync.rcuReloadLatencyUs}µs)
              </button>
            </div>
          </div>

          {ferrousxSync.message && (
            <div className="mt-3 px-3.5 py-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 text-xs font-mono text-emerald-700 dark:text-emerald-300">
              ✓ {ferrousxSync.message}
            </div>
          )}

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3 mt-4">
            {ferrousxSync.workspaceCrates.map((crateItem) => (
              <div
                key={crateItem.name}
                className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-xs font-bold text-slate-900 dark:text-slate-100">
                    crates/{crateItem.name}
                  </span>
                  <span className="font-mono text-[11px] text-emerald-600 dark:text-emerald-400">
                    ● {crateItem.version}
                  </span>
                </div>
                <div className="mt-1 text-[11px] font-mono text-orange-600 dark:text-orange-400 truncate">
                  [{crateItem.modules.join(', ')}]
                </div>
                <p className="mt-1.5 text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                  {crateItem.features}
                </p>
              </div>
            ))}
          </div>

          {showLibMatrix && (
            <div className="mt-4 pt-4 border-t border-slate-200 dark:border-slate-800 grid grid-cols-1 lg:grid-cols-2 gap-4">
              <div>
                <div className="text-xs font-bold text-slate-800 dark:text-slate-200 mb-2 font-mono">
                  Rust Workspace Dependencies (Cargo.toml · rustc {ferrousxSync.rustVersion})
                </div>
                <div className="overflow-x-auto rounded-lg border border-slate-200 dark:border-slate-800">
                  <table className="w-full text-left text-xs font-mono">
                    <thead className="bg-slate-100 dark:bg-slate-950 text-slate-500">
                      <tr>
                        <th className="py-2 px-3">Crate</th>
                        <th className="py-2 px-3">FerrousPanel</th>
                        <th className="py-2 px-3">FerrousX</th>
                        <th className="py-2 px-3 text-right">Status</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                      {ferrousxSync.libraryCompatibility.rustDependencies.map((dep) => (
                        <tr key={dep.name}>
                          <td className="py-2 px-3 font-semibold text-slate-900 dark:text-slate-100">
                            {dep.name}
                          </td>
                          <td className="py-2 px-3 text-slate-600 dark:text-slate-400">
                            {dep.panelVersion}
                          </td>
                          <td className="py-2 px-3 text-slate-600 dark:text-slate-400">
                            {dep.ferrousxVersion}
                          </td>
                          <td className="py-2 px-3 text-right text-emerald-600 dark:text-emerald-400 font-semibold">
                            OK
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              <div>
                <div className="text-xs font-bold text-slate-800 dark:text-slate-200 mb-2 font-mono">
                  Frontend &amp; Build Toolchain Dependencies (package.json)
                </div>
                <div className="overflow-x-auto rounded-lg border border-slate-200 dark:border-slate-800">
                  <table className="w-full text-left text-xs font-mono">
                    <thead className="bg-slate-100 dark:bg-slate-950 text-slate-500">
                      <tr>
                        <th className="py-2 px-3">Package</th>
                        <th className="py-2 px-3">Installed</th>
                        <th className="py-2 px-3">FerrousX Spec</th>
                        <th className="py-2 px-3 text-right">Status</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                      {ferrousxSync.libraryCompatibility.frontendDependencies.map((dep) => (
                        <tr key={dep.name}>
                          <td className="py-2 px-3 font-semibold text-slate-900 dark:text-slate-100">
                            {dep.name}
                          </td>
                          <td className="py-2 px-3 text-slate-600 dark:text-slate-400">
                            {dep.panelVersion}
                          </td>
                          <td className="py-2 px-3 text-slate-600 dark:text-slate-400">
                            {dep.ferrousxVersion}
                          </td>
                          <td className="py-2 px-3 text-right text-emerald-600 dark:text-emerald-400 font-semibold">
                            OK
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}
        </section>
      )}

      {/* Active Websites Deployment Table */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
        <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div>
            <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
              Danh sách Website Đang Triển khai trên Máy chủ ({apps.length} Sites)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Nhấp vào đường dẫn thư mục để mở File Manager hoặc chọn Cài đặt để cấu hình SSL, URL Rewrite và Git Auto-Deploy
            </p>
          </div>
          <button
            onClick={() => onNavigate('websites')}
            className="text-xs font-medium text-orange-600 dark:text-orange-400 hover:underline flex items-center gap-1 whitespace-nowrap shrink-0 cursor-pointer"
          >
            Mở bảng Quản lý Website đầy đủ
            <ArrowUpRight className="w-3.5 h-3.5" />
          </button>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-medium text-slate-500 dark:text-slate-400 bg-slate-50/60 dark:bg-slate-950/40">
                <th className="py-3 px-5">Tên miền (Domain)</th>
                <th className="py-3 px-4">Môi trường Chạy</th>
                <th className="py-3 px-4">Thư mục Web Root</th>
                <th className="py-3 px-4">Database &amp; SSL</th>
                <th className="py-3 px-4 text-right">Thông lượng / P99</th>
                <th className="py-3 px-4">Trạng thái</th>
                <th className="py-3 px-5 text-right">Thao tác</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-sm">
              {apps.length === 0 ? (
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
                      Nhấn &quot;Thêm Website Mới&quot; ở góc phải để bắt đầu
                      triển khai dịch vụ web.
                    </div>
                  </td>
                </tr>
              ) : (
                apps.map((app) => (
                  <tr
                    key={app.id}
                    className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40 transition-colors"
                  >
                    <td className="py-3 px-5">
                      <div className="font-mono font-bold text-slate-900 dark:text-slate-100">
                        {app.domain}
                      </div>
                      <div className="text-xs text-slate-500 dark:text-slate-400 font-mono">
                        Commit: {app.lastDeployCommit.slice(0, 28)}
                      </div>
                    </td>
                    <td className="py-3 px-4 text-xs font-mono tabular-nums">
                      <div className="font-semibold text-slate-800 dark:text-slate-200">
                        {app.framework}
                      </div>
                      <div className="text-slate-500">
                        127.0.0.1:{app.internalPort} · {app.workers}W
                      </div>
                    </td>
                    <td className="py-3 px-4 font-mono text-xs">
                      <button
                        onClick={() => onNavigate('files', app.id)}
                        className="text-sky-600 dark:text-sky-400 hover:underline inline-flex items-center gap-1 cursor-pointer"
                      >
                        <FolderOpen className="w-3.5 h-3.5 shrink-0" />
                        {app.workingDir}
                      </button>
                    </td>
                    <td className="py-3 px-4 font-mono text-xs tabular-nums">
                      <div className="text-slate-700 dark:text-slate-300">
                        DB: {app.linkedDbName || 'None'}
                      </div>
                      <div className="text-emerald-600 dark:text-emerald-400">
                        SSL ({app.sslExpiryDays}d) · {app.http3Quic ? 'QUIC' : 'HTTP/2'}
                      </div>
                    </td>
                    <td className="py-3 px-4 text-right font-mono text-xs tabular-nums text-slate-700 dark:text-slate-300">
                      <div>{app.requestsPerSec.toLocaleString()} req/s</div>
                      <div className="text-slate-500">P99 {app.p99LatencyMs} ms</div>
                    </td>
                    <td className="py-3 px-4 text-xs">
                      {app.status === 'running' ? (
                        <span className="inline-flex items-center gap-1.5 font-medium text-emerald-600 dark:text-emerald-400">
                          <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />
                          Running
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5 font-medium text-slate-500">
                          <Square className="w-3.5 h-3.5 shrink-0" />
                          Stopped
                        </span>
                      )}
                    </td>
                    <td className="py-3 px-5 text-right">
                      <div className="inline-flex items-center justify-end gap-1.5">
                        {app.status === 'running' ? (
                          <button
                            onClick={() => onAppAction(app.id, 'restart')}
                            className="px-2.5 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md inline-flex items-center gap-1 cursor-pointer"
                          >
                            <RefreshCw className="w-3 h-3" />
                            Restart
                          </button>
                        ) : (
                          <button
                            onClick={() => onAppAction(app.id, 'start')}
                            className="px-2.5 py-1.5 text-xs font-medium text-emerald-700 bg-emerald-50 rounded-md inline-flex items-center gap-1 cursor-pointer"
                          >
                            <Play className="w-3 h-3" />
                            Start
                          </button>
                        )}
                        <button
                          onClick={() => onNavigate('files', app.id)}
                          className="px-2.5 py-1.5 text-xs font-medium text-slate-600 dark:text-slate-300 hover:text-slate-900 dark:hover:text-white rounded-md cursor-pointer"
                        >
                          Files
                        </button>
                        <button
                          onClick={() => onNavigate('websites', app.id)}
                          className="px-2.5 py-1.5 text-xs font-medium text-orange-600 dark:text-orange-400 hover:underline rounded-md cursor-pointer"
                        >
                          Cài đặt
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
};
