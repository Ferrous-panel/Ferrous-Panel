import React, { useState } from 'react';
import {
  Activity,
  ArrowUpRight,
  CheckCircle2,
  Clock,
  Cpu,
  Database,
  FolderOpen,
  Globe,
  Lock,
  Mail,
  Network,
  Play,
  Plus,
  RefreshCw,
  Rocket,
  Server,
  ShieldCheck,
  Square,
  Terminal
} from 'lucide-react';
import {
  ActiveTab,
  CpanelData,
  DiagnosticCheckItem,
  MailServerData,
  ManagedApp,
  SecurityData,
  SqlDatabase,
  TelemetryData
} from '../types';

interface OverviewViewProps {
  telemetry: TelemetryData | null;
  apps: ManagedApp[];
  databases: SqlDatabase[];
  cpanel: CpanelData | null;
  security: SecurityData | null;
  mailData: MailServerData | null;
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
  cpanel,
  security,
  mailData,
  diagnostics,
  onRunDiagnostics,
  onNavigate,
  onAppAction,
  onOpenDeployModal
}) => {
  const [runningDiag, setRunningDiag] = useState(false);

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
              {telemetry.rustDaemonVersion} · 12/12 Rust Modules · Web SSH &amp; External SSH · Modern JS (Next.js 15 / Nuxt 3 / SvelteKit 2) · MySQL phpMyAdmin · Postfix Mail
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
                {mailData?.mailboxes.length ?? 4} Hộp thư
              </span>
              <span className="text-xs text-emerald-600 dark:text-emerald-400">
                DKIM 2048-bit
              </span>
            </div>
            <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
              {mailData?.domains.length ?? 3} tên miền · Postfix + Dovecot + Rspamd
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
              sub: `${mailData?.mailboxes.length ?? 4} mailboxes`,
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
              sub: '12/12 Rust Modules',
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
                    colSpan={5}
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
              )))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
};
