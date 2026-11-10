import React, { useState } from 'react';
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Cpu,
  Flame,
  Gauge,
  HardDrive,
  Layers,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  Sliders,
  Sparkles,
  Terminal,
  Zap,
} from 'lucide-react';
import {
  AntiCrashGuardState,
  OptimizationProfilePreset,
  SystemOptimizerData,
} from '../types';

interface SystemOptimizerViewProps {
  optimizer: SystemOptimizerData;
  onApplyProfile: (profileId: OptimizationProfilePreset['id']) => Promise<void>;
  onToggleSysctl: (id: string) => Promise<void>;
  onReclaimMemory: () => Promise<{ freedMb: number; message: string }>;
  onWatchdogHeal: () => Promise<void>;
  onUpdateAntiCrash?: (payload: Partial<AntiCrashGuardState>) => Promise<void>;
  onEmergencyMitigate?: (
    action:
      | 'purge_syn_backlog'
      | 'isolate_leaking_workers'
      | 'reset_hung_db_queries'
  ) => Promise<{ message?: string } | void>;
}

export const SystemOptimizerView: React.FC<SystemOptimizerViewProps> = ({
  optimizer,
  onApplyProfile,
  onToggleSysctl,
  onReclaimMemory,
  onWatchdogHeal,
  onUpdateAntiCrash,
  onEmergencyMitigate,
}) => {
  const [applyingId, setApplyingId] = useState<string | null>(null);
  const [reclaiming, setReclaiming] = useState(false);
  const [healing, setHealing] = useState(false);
  const [mitigatingAction, setMitigatingAction] = useState<string | null>(null);
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

  const handleToggleAntiCrashOption = async (
    field: 'loadSheddingEnabled' | 'zramEnabled' | 'cgroupsV2Enforced'
  ) => {
    if (!onUpdateAntiCrash || !optimizer.antiCrashGuard) return;
    setStatusBanner(null);
    const nextValue = !optimizer.antiCrashGuard[field];
    await onUpdateAntiCrash({ [field]: nextValue });
    setStatusBanner(
      `[Rust Core Binary] Đã cập nhật cơ chế chống sập ${field} = ${
        nextValue ? 'ENABLED' : 'DISABLED'
      } qua Cgroups v2 & Kernel Guard.`
    );
  };

  const handleRunEmergencyAction = async (
    action:
      | 'purge_syn_backlog'
      | 'isolate_leaking_workers'
      | 'reset_hung_db_queries',
    label: string
  ) => {
    if (!onEmergencyMitigate) return;
    setMitigatingAction(action);
    setStatusBanner(null);
    try {
      const res = await onEmergencyMitigate(action);
      setStatusBanner(
        (res && typeof res === 'object' && res.message) ||
          `[Rust Emergency Guard] Đã thực thi "${label}" thành công.`
      );
    } finally {
      setMitigatingAction(null);
    }
  };

  const { watchdog, antiCrashGuard } = optimizer;

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
              Công Nghệ Chống Sập Server (Anti-Crash Resilience) &amp; Tối Ưu Hóa Hệ Thống
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Bảo vệ VPS khỏi sập nguồn do quá tải lưu lượng hoặc rò rỉ RAM bằng Cgroups v2 Slice Isolation, Adaptive Load Shedding, ZRAM zstd và OOM Killer Immunity.
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
            <span className="font-mono text-[11px] shrink-0">RUST-CORE: OK</span>
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

      {/* Anti-Crash Server Resilience Console */}
      {antiCrashGuard && (
        <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
          <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-4">
            <div className="space-y-1">
              <div className="text-xs font-mono text-orange-600 dark:text-orange-400">
                RUST ANTI-CRASH ENGINE · CGROUPS V2 SLICE ISOLATION · ADAPTIVE BACKPRESSURE · ZRAM ZSTD
              </div>
              <h2 className="text-base font-bold text-slate-900 dark:text-white flex items-center gap-2">
                <Layers className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                Kiến Trúc Chống Sập Server Đa Tầng (Zero-Downtime &amp; Anti-OOM Crash Guard)
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                Ngăn chặn hiệu ứng Domino làm treo toàn bộ VPS khi 1 website bị spike CPU/RAM đột biến hoặc Database bị kẹt Deadlock.
              </p>
            </div>

            {/* 1-Click Emergency Anti-Crash Mitigations */}
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={() =>
                  handleRunEmergencyAction(
                    'purge_syn_backlog',
                    'Xả hàng đợi TCP SYN Backlog & Socket treo'
                  )
                }
                disabled={mitigatingAction !== null}
                className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-amber-500/10 hover:bg-amber-500/20 text-amber-700 dark:text-amber-300 border border-amber-500/30 transition-colors inline-flex items-center gap-1.5 cursor-pointer"
              >
                <Flame className="w-3.5 h-3.5 text-amber-500" />
                {mitigatingAction === 'purge_syn_backlog'
                  ? 'Đang xả SYN Backlog...'
                  : 'Xả TCP SYN Backlog'}
              </button>

              <button
                type="button"
                onClick={() =>
                  handleRunEmergencyAction(
                    'isolate_leaking_workers',
                    'Cô lập Worker rò rỉ RAM (Cgroups v2)'
                  )
                }
                disabled={mitigatingAction !== null}
                className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-orange-500/10 hover:bg-orange-500/20 text-orange-700 dark:text-orange-300 border border-orange-500/30 transition-colors inline-flex items-center gap-1.5 cursor-pointer"
              >
                <AlertTriangle className="w-3.5 h-3.5 text-orange-500" />
                {mitigatingAction === 'isolate_leaking_workers'
                  ? 'Đang cô lập Worker...'
                  : 'Cô lập Worker Rò Rỉ RAM'}
              </button>

              <button
                type="button"
                onClick={() =>
                  handleRunEmergencyAction(
                    'reset_hung_db_queries',
                    'Ngắt truy vấn SQL treo > 15s'
                  )
                }
                disabled={mitigatingAction !== null}
                className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-rose-500/10 hover:bg-rose-500/20 text-rose-700 dark:text-rose-300 border border-rose-500/30 transition-colors inline-flex items-center gap-1.5 cursor-pointer"
              >
                <Zap className="w-3.5 h-3.5 text-rose-500" />
                {mitigatingAction === 'reset_hung_db_queries'
                  ? 'Đang ngắt Deadlock...'
                  : 'Cắt SQL Queries Treo'}
              </button>
            </div>
          </div>

          {/* 3 Core Anti-Crash Pillars */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            {/* Pillar 1: Adaptive Load Shedding & Backpressure */}
            <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/70 flex flex-col justify-between space-y-3">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-mono font-bold text-orange-600 dark:text-orange-400">
                    ADAPTIVE LOAD SHEDDING
                  </span>
                  <span className="text-[11px] font-mono text-slate-500 tabular-nums">
                    SQL Kill &gt; {antiCrashGuard.slowQueryAutoKillSec}s
                  </span>
                </div>
                <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                  Cắt Tải Thông Minh Khi Spike Đột Biến
                </h3>
                <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                  Khi CPU &gt; {antiCrashGuard.cpuShedThresholdPercent}% hoặc RAM &gt;{' '}
                  {antiCrashGuard.ramShedThresholdPercent}%, tầng Rust Proxy tự động trả{' '}
                  <code className="font-mono">503 Retry-After</code> cho bot/request phụ để giữ sống nhân xử lý chính.
                </p>
              </div>
              <div className="pt-2 border-t border-slate-200 dark:border-slate-800 flex items-center justify-between">
                <span className="text-[11px] font-mono text-slate-500 tabular-nums">
                  Can thiệp bảo vệ: {antiCrashGuard.mitigationCount} lần
                </span>
                <button
                  type="button"
                  onClick={() => handleToggleAntiCrashOption('loadSheddingEnabled')}
                  className={`px-3 py-1 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                    antiCrashGuard.loadSheddingEnabled
                      ? 'bg-emerald-600 text-white'
                      : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {antiCrashGuard.loadSheddingEnabled ? 'Đang Bật' : 'Đã Tắt'}
                </button>
              </div>
            </div>

            {/* Pillar 2: ZRAM Compressed Memory Swap */}
            <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/70 flex flex-col justify-between space-y-3">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-mono font-bold text-sky-600 dark:text-sky-400">
                    ZRAM ZSTD MEMORY COMPRESSION
                  </span>
                  <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400 tabular-nums">
                    Thuật toán {antiCrashGuard.zramCompressionAlgo}
                  </span>
                </div>
                <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                  Nén RAM Thực Tế Bằng Thuật Toán {antiCrashGuard.zramCompressionAlgo}
                </h3>
                <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                  Tạo vùng nhớ nén tốc độ cao trực tiếp trên RAM ({antiCrashGuard.zramSizeMb} MB), tránh I/O Thrashing làm đơ ổ cứng NVMe khi VPS gần cạn bộ nhớ vật lý.
                </p>
              </div>
              <div className="pt-2 border-t border-slate-200 dark:border-slate-800 flex items-center justify-between">
                <span className="text-[11px] font-mono text-slate-500 tabular-nums">
                  Dung lượng ZRAM: {antiCrashGuard.zramSizeMb} MB
                </span>
                <button
                  type="button"
                  onClick={() => handleToggleAntiCrashOption('zramEnabled')}
                  className={`px-3 py-1 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                    antiCrashGuard.zramEnabled
                      ? 'bg-emerald-600 text-white'
                      : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {antiCrashGuard.zramEnabled ? 'Đang Bật' : 'Đã Tắt'}
                </button>
              </div>
            </div>

            {/* Pillar 3: Cgroups v2 Hard Isolation & Auto-Respawn */}
            <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/70 flex flex-col justify-between space-y-3">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-mono font-bold text-emerald-600 dark:text-emerald-400">
                    CGROUPS V2 &amp; AUTO-RESPAWN
                  </span>
                  <span className="text-[11px] font-mono text-slate-500 tabular-nums">
                    SLA &lt; 50ms
                  </span>
                </div>
                <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                  Cô Lập Cứng Từng Tiến Trình Theo Slice
                </h3>
                <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                  Áp trần <code className="font-mono">memory.high</code> và{' '}
                  <code className="font-mono">memory.max</code> riêng cho từng dịch vụ. Nếu 1 worker bị crash, Systemd + Rust Supervisor tự khởi chạy lại tức thì.
                </p>
              </div>
              <div className="pt-2 border-t border-slate-200 dark:border-slate-800 flex items-center justify-between">
                <span className="text-[11px] font-mono text-slate-500 truncate max-w-[180px]" title={antiCrashGuard.lastMitigationAction}>
                  {antiCrashGuard.lastMitigationAction}
                </span>
                <button
                  type="button"
                  onClick={() => handleToggleAntiCrashOption('cgroupsV2Enforced')}
                  className={`px-3 py-1 rounded-lg text-xs font-semibold transition-colors cursor-pointer shrink-0 ${
                    antiCrashGuard.cgroupsV2Enforced
                      ? 'bg-emerald-600 text-white'
                      : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {antiCrashGuard.cgroupsV2Enforced ? 'Đang Bật' : 'Đã Tắt'}
                </button>
              </div>
            </div>
          </div>

          {/* OOM Killer Immunity Matrix & Cgroups v2 Slice Table */}
          <div className="rounded-xl border border-slate-200 dark:border-slate-800 overflow-hidden">
            <div className="px-4 py-3 bg-slate-50 dark:bg-slate-950 border-b border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
              <div className="text-xs font-bold text-slate-900 dark:text-white">
                Ma Trận Miễn Nhiễm Linux OOM Killer (<code className="font-mono">/proc/&lt;pid&gt;/oom_score_adj</code>) &amp; Cgroups v2 Limits
              </div>
              <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400">
                Ưu tiên giữ sống SSHD (-950) &amp; Ferrous Core (-900)
              </span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse text-xs">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-slate-800 text-[11px] font-mono text-slate-500 uppercase bg-slate-50/50 dark:bg-slate-950/50">
                    <th className="py-2.5 px-4">Tiến Trình Hệ Thống</th>
                    <th className="py-2.5 px-4">PID</th>
                    <th className="py-2.5 px-4">OOM Score Adj</th>
                    <th className="py-2.5 px-4">Ngưỡng Cảnh Báo (memory.high)</th>
                    <th className="py-2.5 px-4">Giới Hạn Cứng (memory.max)</th>
                    <th className="py-2.5 px-4">Chính Sách Tự Hồi Phục (Cgroups v2 Policy)</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 font-mono">
                  {antiCrashGuard.oomImmunityMap.map((proc) => (
                    <tr
                      key={proc.service}
                      className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40"
                    >
                      <td className="py-2.5 px-4 font-bold text-slate-900 dark:text-white">
                        {proc.service}
                      </td>
                      <td className="py-2.5 px-4 tabular-nums text-slate-600 dark:text-slate-400">
                        {proc.pid}
                      </td>
                      <td className="py-2.5 px-4 tabular-nums">
                        <span
                          className={
                            proc.oomScoreAdj <= -800
                              ? 'text-emerald-600 dark:text-emerald-400 font-bold'
                              : proc.oomScoreAdj < 0
                                ? 'text-sky-600 dark:text-sky-400 font-semibold'
                                : 'text-amber-600 dark:text-amber-400'
                          }
                        >
                          {proc.oomScoreAdj}
                        </span>
                      </td>
                      <td className="py-2.5 px-4 tabular-nums text-slate-600 dark:text-slate-300">
                        {proc.memoryHighMb} MB
                      </td>
                      <td className="py-2.5 px-4 tabular-nums text-slate-700 dark:text-slate-300">
                        {proc.memoryMaxMb} MB
                      </td>
                      <td className="py-2.5 px-4 text-emerald-600 dark:text-emerald-400">
                        {proc.policy}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </section>
      )}

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
              {watchdog.recentEvents.length === 0 ? (
                <div className="p-5 text-center text-slate-500 dark:text-slate-400">
                  Chưa có sự kiện can thiệp khẩn cấp nào trong phiên hiện tại. Nhấn &ldquo;Chạy Watchdog Self-Heal&rdquo; hoặc các nút chống sập ở trên để ghi nhận thực thi.
                </div>
              ) : (
                watchdog.recentEvents.map((ev) => (
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
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
