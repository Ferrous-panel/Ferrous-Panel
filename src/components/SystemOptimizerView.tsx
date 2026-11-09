import React, { useState } from 'react';
import {
  Activity,
  CheckCircle2,
  Cpu,
  Gauge,
  HardDrive,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  Sliders,
  Sparkles,
  Terminal,
  Zap,
} from 'lucide-react';
import { OptimizationProfilePreset, SystemOptimizerData } from '../types';

interface SystemOptimizerViewProps {
  optimizer: SystemOptimizerData;
  onApplyProfile: (profileId: OptimizationProfilePreset['id']) => Promise<void>;
  onToggleSysctl: (id: string) => Promise<void>;
  onReclaimMemory: () => Promise<{ freedMb: number; message: string }>;
  onWatchdogHeal: () => Promise<void>;
}

export const SystemOptimizerView: React.FC<SystemOptimizerViewProps> = ({
  optimizer,
  onApplyProfile,
  onToggleSysctl,
  onReclaimMemory,
  onWatchdogHeal,
}) => {
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [reclaiming, setReclaiming] = useState(false);
  const [healing, setHealing] = useState(false);
  const [statusBanner, setStatusBanner] = useState<string | null>(null);

  const handleApply = async (id: OptimizationProfilePreset['id']) => {
    setApplyingId(id);
    setStatusBanner(null);
    try {
      await onApplyProfile(id);
      const found = optimizer.profiles.find((p) => p.id === id);
      setStatusBanner(
        `Đã kích hoạt Model "${found?.name}": Đồng bộ ACTIX_WORKERS=${found?.actixWorkers}, Nginx worker_connections=${found?.nginxWorkerConnections} và MariaDB Buffer Pool=${found?.mariadbBufferPoolMb} MB.`
      );
    } finally {
      setApplyingId(null);
    }
  };

  const handleReclaim = async () => {
    setReclaiming(true);
    setStatusBanner(null);
    try {
      const res = await onReclaimMemory();
      setStatusBanner(res.message);
    } finally {
      setReclaiming(false);
    }
  };

  const handleHeal = async () => {
    setHealing(true);
    setStatusBanner(null);
    try {
      await onWatchdogHeal();
      setStatusBanner(
        'Watchdog Self-Healing hoàn tất: Đã kiểm tra toàn bộ Unix Domain Sockets, làm mới kết nối SQL Pool và xác nhận OOM Guard (-900).'
      );
    } finally {
      setHealing(false);
    }
  };

  const { watchdog } = optimizer;

  return (
    <div className="space-y-6">
      {/* Header & Backend Stability Controls */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-5">
          <div className="space-y-1">
            <div className="text-xs font-mono text-emerald-600 dark:text-emerald-400">
              Ferrous-panel/src/optimizer.rs · Actix-Web Watchdog · Linux Kernel 6.8 TCP BBR · OOM Score -900
            </div>
            <h1 className="text-xl font-bold tracking-tight text-slate-900 dark:text-white flex items-center gap-2">
              <Gauge className="w-5 h-5 text-orange-600 dark:text-orange-400" />
              Models Tối Ưu Hóa Hệ Thống &amp; Bảo Đảm Ổn Định Backend (Watchdog Engine)
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Tự động tính toán thông số Worker Pool theo phần cứng vật lý, tinh chỉnh nhân Linux <code className="font-mono">sysctl</code> và bảo vệ tiến trình điều khiển không bao giờ bị treo hay tràn bộ nhớ.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2.5 shrink-0">
            <button
              onClick={handleReclaim}
              disabled={reclaiming}
              className="px-3.5 py-2 text-xs font-semibold text-slate-800 dark:text-slate-100 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Sparkles className={`w-3.5 h-3.5 text-amber-500 ${reclaiming ? 'animate-spin' : ''}`} />
              {reclaiming ? 'Đang giải phóng PageCache...' : 'Giải phóng PageCache & Idle RAM'}
            </button>
            <button
              onClick={handleHeal}
              disabled={healing}
              className="px-4 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${healing ? 'animate-spin' : ''}`} />
              {healing ? 'Đang chạy Self-Heal...' : 'Chạy Watchdog Self-Heal Ngay'}
            </button>
          </div>
        </div>

        {statusBanner && (
          <div className="p-3.5 rounded-lg bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200 dark:border-emerald-800 text-xs font-medium text-emerald-800 dark:text-emerald-200 flex items-center justify-between gap-2">
            <span>{statusBanner}</span>
            <span className="font-mono text-[11px] shrink-0">STATUS: OK</span>
          </div>
        )}

        {/* 4-Card Backend Stability Telemetry */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          <div className="p-4 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
            <div className="flex items-center justify-between text-xs text-slate-500">
              <span>Tiến trình Panel Daemon</span>
              <ShieldCheck className="w-4 h-4 text-emerald-500" />
            </div>
            <div className="text-lg font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              PID {watchdog.daemonPid} · OOM {watchdog.oomScoreAdj}
            </div>
            <div className="text-[11px] font-mono text-slate-500 dark:text-slate-400">
              Miễn nhiễm Linux OOM Killer · Độ ổn định {watchdog.uptimeStabilityPercent}%
            </div>
          </div>

          <div className="p-4 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
            <div className="flex items-center justify-between text-xs text-slate-500">
              <span>Bộ cấp phát RAM (Allocator)</span>
              <HardDrive className="w-4 h-4 text-sky-500" />
            </div>
            <div className="text-lg font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              {watchdog.allocatedRamMb} MB / {watchdog.residentRamMb} MB
            </div>
            <div className="text-[11px] font-mono text-slate-500 dark:text-slate-400">
              {watchdog.allocatorEngine} · Chống phân mảnh Heap
            </div>
          </div>

          <div className="p-4 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
            <div className="flex items-center justify-between text-xs text-slate-500">
              <span>Luồng Actix Worker &amp; DB Pool</span>
              <Cpu className="w-4 h-4 text-orange-500" />
            </div>
            <div className="text-lg font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              {watchdog.workerThreadsActive}/{watchdog.workerThreadsTotal} Workers · {watchdog.dbPoolActive}/{watchdog.dbPoolMax} DB
            </div>
            <div className="text-[11px] font-mono text-slate-500 dark:text-slate-400">
              Idle Conns: {watchdog.dbPoolIdle} · Non-blocking Tokio I/O
            </div>
          </div>

          <div className="p-4 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
            <div className="flex items-center justify-between text-xs text-slate-500">
              <span>Circuit Breaker &amp; Watchdog</span>
              <Activity className="w-4 h-4 text-emerald-500" />
            </div>
            <div className="text-lg font-bold font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
              {watchdog.circuitBreakerStatus}
            </div>
            <div className="text-[11px] font-mono text-slate-500 dark:text-slate-400">
              Kiểm tra cuối: {watchdog.lastSelfHealAt}
            </div>
          </div>
        </div>
      </section>

      {/* Section 2: Hardware-Adaptive Optimization Models */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div>
            <h2 className="text-base font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <Zap className="w-4 h-4 text-orange-600 dark:text-orange-400" />
              1. Models Tối Ưu Hóa Phần Cứng Tự Động (Hardware-Adaptive Resource Models)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
              Chọn mô hình phù hợp với tải thực tế để đồng bộ đồng thời cấu hình Actix-Web, Nginx Reverse Proxy, PostgreSQL 17, MariaDB 11.4 và Redis.
            </p>
          </div>
          <span className="text-xs font-mono text-slate-500">
            Model đang kích hoạt:{' '}
            <strong className="text-orange-600 dark:text-orange-400">{optimizer.activeProfileId}</strong>
          </span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {optimizer.profiles.map((prof) => {
            const isActive = optimizer.activeProfileId === prof.id;
            return (
              <div
                key={prof.id}
                className={`rounded-xl border p-5 flex flex-col justify-between space-y-4 transition-colors ${
                  isActive
                    ? 'border-orange-500 bg-orange-50/30 dark:bg-orange-950/20'
                    : 'border-slate-200 dark:border-slate-800 bg-slate-50/40 dark:bg-slate-950/40'
                }`}
              >
                <div className="space-y-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs font-mono font-semibold text-orange-600 dark:text-orange-400">
                      {prof.badge}
                    </span>
                    <span className="text-xs font-mono text-emerald-600 dark:text-emerald-400 tabular-nums">
                      Giảm ~{prof.expectedP99ReductionPercent}% độ trễ P99
                    </span>
                  </div>
                  <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                    {prof.name}
                  </h3>
                  <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                    {prof.description}
                  </p>
                </div>

                <div className="space-y-3 pt-3 border-t border-slate-200/80 dark:border-slate-800">
                  <div className="grid grid-cols-2 gap-2 text-[11px] font-mono tabular-nums text-slate-600 dark:text-slate-300">
                    <div>Actix Workers: <strong>{prof.actixWorkers}</strong></div>
                    <div>Max Conns: <strong>{prof.actixMaxConnections.toLocaleString()}</strong></div>
                    <div>Nginx Conns: <strong>{prof.nginxWorkerConnections.toLocaleString()}</strong></div>
                    <div>Keepalive: <strong>{prof.actixKeepAliveSecs}s ({prof.nginxKeepaliveRequests} req)</strong></div>
                    <div>InnoDB Buffer: <strong>{prof.mariadbBufferPoolMb} MB</strong></div>
                    <div>PG Shared / Redis: <strong>{prof.postgresSharedBuffersMb} / {prof.redisMaxMemoryMb} MB</strong></div>
                  </div>

                  <button
                    onClick={() => handleApply(prof.id)}
                    disabled={applyingId === prof.id}
                    className={`w-full py-2 px-4 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                      isActive
                        ? 'bg-emerald-600 text-white'
                        : 'bg-slate-900 dark:bg-orange-600 hover:bg-slate-800 dark:hover:bg-orange-500 text-white'
                    }`}
                  >
                    {applyingId === prof.id
                      ? 'Đang tính toán & đồng bộ cấu hình...'
                      : isActive
                        ? 'Đang áp dụng Model này (Nhấn để đồng bộ lại)'
                        : 'Áp dụng Model Tối ưu Này'}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      {/* Section 3: Kernel Sysctl Tuning + Live Config Preview */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        <div className="lg:col-span-7 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
            <div>
              <h2 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
                <Sliders className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                2. Model Tinh Chỉnh Nhân Linux (<code className="font-mono">sysctl.conf</code>)
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                Tối ưu hóa thuật toán chống nghẽn TCP BBR, hàng đợi Epoll và kiểm soát bộ nhớ ảo Swap.
              </p>
            </div>
          </div>

          <div className="divide-y divide-slate-200 dark:divide-slate-800">
            {optimizer.sysctlParams.map((param) => (
              <div
                key={param.id}
                className="p-4 flex items-center justify-between gap-4 hover:bg-slate-50/70 dark:hover:bg-slate-800/30"
              >
                <div className="space-y-1 min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-mono text-xs font-bold text-slate-900 dark:text-white">
                      {param.key}
                    </span>
                    <span className="text-[11px] font-mono text-slate-500">
                      · {param.category}
                    </span>
                  </div>
                  <p className="text-xs text-slate-600 dark:text-slate-400">
                    {param.description}
                  </p>
                  <div className="text-[11px] font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
                    Giá trị tối ưu: <strong>{param.recommendedValue}</strong> (Hiện tại: {param.currentValue})
                  </div>
                </div>

                <button
                  onClick={() => onToggleSysctl(param.id)}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold shrink-0 transition-colors cursor-pointer ${
                    param.enabled
                      ? 'bg-emerald-600 text-white'
                      : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {param.enabled ? 'Đang bật' : 'Đã tắt'}
                </button>
              </div>
            ))}
          </div>
        </div>

        {/* Right: Generated /etc/sysctl.d/99-ferrous-optimizer.conf & Watchdog Audit Log */}
        <div className="lg:col-span-5 space-y-6">
          <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
            <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <span className="text-xs font-mono font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-1.5">
                <Terminal className="w-3.5 h-3.5 text-orange-500" />
                /etc/sysctl.d/99-ferrous-optimizer.conf
              </span>
              <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400">
                sysctl --system
              </span>
            </div>
            <pre className="p-4 bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed overflow-x-auto max-h-64">
              {optimizer.sysctlConfigPreview}
            </pre>
          </div>

          <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
            <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <span className="text-xs font-bold text-slate-900 dark:text-white flex items-center gap-1.5">
                <ShieldAlert className="w-3.5 h-3.5 text-emerald-500" />
                Nhật ký Watchdog &amp; Tự Phục Hồi Backend
              </span>
              <span className="text-[11px] font-mono text-slate-500">Auto-Heal Active</span>
            </div>
            <div className="divide-y divide-slate-200 dark:divide-slate-800 max-h-64 overflow-y-auto text-xs">
              {watchdog.recentEvents.map((ev) => (
                <div key={ev.id} className="p-3.5 space-y-1">
                  <div className="flex items-center justify-between font-mono text-[11px]">
                    <span className="font-semibold text-slate-900 dark:text-white">
                      [{ev.subsystem}]
                    </span>
                    <span className="text-slate-500 tabular-nums">{ev.timestamp}</span>
                  </div>
                  <div className="text-slate-700 dark:text-slate-300">{ev.action}</div>
                  <div className="font-mono text-[11px] text-emerald-600 dark:text-emerald-400 flex items-center gap-1">
                    <CheckCircle2 className="w-3 h-3" /> {ev.result}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
