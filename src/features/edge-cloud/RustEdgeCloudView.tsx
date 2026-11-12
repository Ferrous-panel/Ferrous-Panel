import React, { useState } from 'react';
import {
  Box,
  CheckCircle2,
  Cpu,
  HardDriveDownload,
  Lock,
  Network,
  Pause,
  Play,
  Plus,
  RefreshCw,
  Shield,
  Terminal,
  Trash2,
  Zap
} from 'lucide-react';
import {
  EdgeCloudData,
  WasiWorkerInvokeResult
} from '../../types';
import { usePanelLanguage } from '../../i18n';

interface RustEdgeCloudViewProps {
  edgeCloud: EdgeCloudData | null;
  onDeployWasiWorker: (payload: {
    name: string;
    routePattern: string;
    targetDomain: string;
    wasiTarget: 'wasm32-wasip2' | 'wasm32-wasip1';
    memoryLimitKb: number;
    fuelLimit: number;
    kvNamespace: string;
    rustSourceSnippet: string;
  }) => Promise<void>;
  onInvokeWasiWorker: (workerId: string) => Promise<WasiWorkerInvokeResult | null>;
  onToggleWasiWorker: (workerId: string) => Promise<void>;
  onDeleteWasiWorker: (workerId: string) => Promise<void>;
  onDeployOciContainer: (payload: {
    name: string;
    image: string;
    runtimeEngine: 'Youki OCI (Rust Rootless)' | 'Firecracker MicroVM (Rust KVM)';
    hostPort: number;
    containerPort: number;
    memoryMaxMb: number;
    cpuQuotaPercent: number;
  }) => Promise<void>;
  onOciContainerAction: (
    containerId: string,
    action: 'start' | 'stop' | 'restart' | 'delete'
  ) => Promise<void>;
  onAddWireGuardPeer: (payload: {
    name: string;
    region: string;
    endpoint: string;
    allowedIps: string;
  }) => Promise<void>;
  onToggleWireGuardPeer: (peerId: string) => Promise<void>;
  onDeleteWireGuardPeer: (peerId: string) => Promise<void>;
  onCreateBlake3Snapshot: (payload: {
    snapshotName: string;
    targetResource: string;
    s3Endpoint: string;
    bucket: string;
    immutableLock: boolean;
  }) => Promise<void>;
  onDeleteBlake3Snapshot: (snapshotId: string) => Promise<void>;
}

export const RustEdgeCloudView: React.FC<RustEdgeCloudViewProps> = ({
  edgeCloud,
  onDeployWasiWorker,
  onInvokeWasiWorker,
  onToggleWasiWorker,
  onDeleteWasiWorker,
  onDeployOciContainer,
  onOciContainerAction,
  onAddWireGuardPeer,
  onToggleWireGuardPeer,
  onDeleteWireGuardPeer,
  onCreateBlake3Snapshot,
  onDeleteBlake3Snapshot
}) => {
  const { tr } = usePanelLanguage();
  const [subTab, setSubTab] = useState<
    'wasi' | 'containers' | 'wireguard' | 'snapshots'
  >('wasi');

  // WASI Form State
  const [workerName, setWorkerName] = useState('edge-rate-limiter-rs');
  const [routePattern, setRoutePattern] = useState('/api/v2/*');
  const [targetDomain, setTargetDomain] = useState('*.ferrous-edge.internal');
  const [wasiTarget, setWasiTarget] = useState<'wasm32-wasip2' | 'wasm32-wasip1'>(
    'wasm32-wasip2'
  );
  const [memoryLimitKb, setMemoryLimitKb] = useState(16384);
  const [fuelLimit, setFuelLimit] = useState(500000);
  const [kvNamespace, setKvNamespace] = useState('kv://edge_ratelimit_atomic');
  const [rustSnippet, setRustSnippet] = useState(
    `#[wasi_http::proxy::export]\npub fn handle(req: IncomingRequest) -> ResponseOutparam {\n    let key = req.remote_ip();\n    if ferrous_kv::incr(&key, 1) > 250 { return ResponseOutparam::too_many_requests(); }\n    ResponseOutparam::pass_upstream(req)\n}`
  );
  const [invokeResult, setInvokeResult] =
    useState<WasiWorkerInvokeResult | null>(null);
  const [invokingId, setInvokingId] = useState<string | null>(null);

  // OCI Container Form State
  const [containerName, setContainerName] = useState('valkey-ferrouskv-node');
  const [containerImage, setContainerImage] = useState(
    'ghcr.io/ferrous-panel/ferrouskv-server:1.4-rust'
  );
  const [runtimeEngine, setRuntimeEngine] = useState<
    'Youki OCI (Rust Rootless)' | 'Firecracker MicroVM (Rust KVM)'
  >('Youki OCI (Rust Rootless)');
  const [hostPort, setHostPort] = useState(6380);
  const [containerPort, setContainerPort] = useState(6379);
  const [memoryMaxMb, setMemoryMaxMb] = useState(1024);
  const [cpuQuotaPercent, setCpuQuotaPercent] = useState(100);

  // WireGuard Form State
  const [peerName, setPeerName] = useState('tyo-edge-relay-03 (Tokyo)');
  const [peerRegion, setPeerRegion] = useState('ap-northeast-1');
  const [peerEndpoint, setPeerEndpoint] = useState('172.104.92.44:51820');
  const [peerAllowedIps, setPeerAllowedIps] = useState('10.66.0.4/32');

  // BLAKE3 Snapshot Form State
  const [snapshotName, setSnapshotName] = useState('full-cluster-merkle-checkpoint');
  const [targetResource, setTargetResource] = useState(
    '/www/wwwroot + PostgreSQL 17 + OCI Volumes'
  );
  const [s3Endpoint, setS3Endpoint] = useState('s3://r2.ferrous-cloud.internal');
  const [bucketName, setBucketName] = useState('ferrous-immutable-vault');
  const [immutableLock, setImmutableLock] = useState(true);

  if (!edgeCloud) {
    return (
      <div className="p-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-sm text-slate-500">
        {tr(
          'Đang tải cấu hình Rust Edge Cloud & WASI Runtime...',
          'Loading Rust Edge Cloud & WASI Runtime...'
        )}
      </div>
    );
  }

  const totalInvocations = edgeCloud.wasiWorkers.reduce(
    (acc, w) => acc + w.invocations24h,
    0
  );
  const runningContainers = edgeCloud.ociContainers.filter(
    (c) => c.status === 'running'
  ).length;
  const connectedPeers = edgeCloud.wireguardPeers.filter(
    (p) => p.status === 'connected'
  ).length;

  const handleInvokeClick = async (id: string) => {
    setInvokingId(id);
    try {
      const res = await onInvokeWasiWorker(id);
      if (res) setInvokeResult(res);
    } finally {
      setInvokingId(null);
    }
  };

  const applyContainerPreset = (
    presetName: string,
    img: string,
    engine: 'Youki OCI (Rust Rootless)' | 'Firecracker MicroVM (Rust KVM)',
    hPort: number,
    cPort: number,
    mem: number
  ) => {
    setContainerName(presetName);
    setContainerImage(img);
    setRuntimeEngine(engine);
    setHostPort(hPort);
    setContainerPort(cPort);
    setMemoryMaxMb(mem);
  };

  return (
    <div className="space-y-6">
      {/* Header Banner */}
      <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4 pb-4 border-b border-slate-200 dark:border-slate-800">
        <div>
          <h1 className="text-xl sm:text-2xl font-bold tracking-tight text-slate-900 dark:text-slate-100">
            {tr(
              'Rust Edge Cloud, WASI Serverless & Youki Containers',
              'Rust Edge Cloud, WASI Serverless & Youki Containers'
            )}
          </h1>
          <p className="text-xs sm:text-sm text-slate-600 dark:text-slate-400 mt-1">
            {tr(
              'Kiến trúc đám mây thế hệ mới phát triển 100% bằng Rust: Wasmtime WASI 0.2, Rootless Youki OCI / Firecracker MicroVM, BoringTun WireGuard & BLAKE3 Merkle.',
              'Next-generation cloud stack written 100% in Rust: Wasmtime WASI 0.2, Rootless Youki OCI / Firecracker MicroVM, BoringTun WireGuard & BLAKE3 Merkle.'
            )}
          </p>
        </div>

        {/* Sub-navigation Segmented Control */}
        <div className="flex items-center gap-1.5 overflow-x-auto pb-1 sm:pb-0">
          <button
            type="button"
            onClick={() => setSubTab('wasi')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
              subTab === 'wasi'
                ? 'bg-orange-600 text-white'
                : 'bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-300 border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <Zap className="w-3.5 h-3.5" />
            <span>WASI Edge Workers ({edgeCloud.wasiWorkers.length})</span>
          </button>

          <button
            type="button"
            onClick={() => setSubTab('containers')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
              subTab === 'containers'
                ? 'bg-orange-600 text-white'
                : 'bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-300 border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <Box className="w-3.5 h-3.5" />
            <span>Youki OCI & MicroVM ({edgeCloud.ociContainers.length})</span>
          </button>

          <button
            type="button"
            onClick={() => setSubTab('wireguard')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
              subTab === 'wireguard'
                ? 'bg-orange-600 text-white'
                : 'bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-300 border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <Network className="w-3.5 h-3.5" />
            <span>BoringTun Mesh ({edgeCloud.wireguardPeers.length})</span>
          </button>

          <button
            type="button"
            onClick={() => setSubTab('snapshots')}
            className={`px-3 py-1.5 rounded-lg text-xs font-semibold whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
              subTab === 'snapshots'
                ? 'bg-orange-600 text-white'
                : 'bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-300 border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <HardDriveDownload className="w-3.5 h-3.5" />
            <span>BLAKE3 Snapshots ({edgeCloud.blake3Snapshots.length})</span>
          </button>
        </div>
      </div>

      {/* KPI Summary Strip */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
            <span>{tr('Wasmtime WASI 0.2 Workers', 'Wasmtime WASI 0.2 Workers')}</span>
            <Zap className="w-4 h-4 text-orange-500" />
          </div>
          <div className="mt-2 flex items-baseline justify-between">
            <span className="text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100">
              {edgeCloud.wasiWorkers.length}
            </span>
            <span className="text-xs font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
              {totalInvocations.toLocaleString()} req/24h
            </span>
          </div>
          <p className="mt-1 text-[11px] font-mono text-slate-500 truncate">
            {edgeCloud.wasmtimeVersion}
          </p>
        </div>

        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
            <span>{tr('Youki OCI & Firecracker', 'Youki OCI & Firecracker')}</span>
            <Box className="w-4 h-4 text-sky-500" />
          </div>
          <div className="mt-2 flex items-baseline justify-between">
            <span className="text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100">
              {runningContainers}/{edgeCloud.ociContainers.length}
            </span>
            <span className="text-xs font-mono tabular-nums text-sky-600 dark:text-sky-400">
              Cgroups v2 Isolated
            </span>
          </div>
          <p className="mt-1 text-[11px] font-mono text-slate-500 truncate">
            {edgeCloud.youkiVersion}
          </p>
        </div>

        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
            <span>{tr('BoringTun WireGuard Mesh', 'BoringTun WireGuard Mesh')}</span>
            <Network className="w-4 h-4 text-emerald-500" />
          </div>
          <div className="mt-2 flex items-baseline justify-between">
            <span className="text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100">
              {connectedPeers} Peers
            </span>
            <span className="text-xs font-mono tabular-nums text-slate-600 dark:text-slate-300">
              {edgeCloud.wireguardInterface} :{edgeCloud.wireguardListenPort}
            </span>
          </div>
          <p className="mt-1 text-[11px] font-mono text-slate-500 truncate">
            {edgeCloud.boringtunVersion}
          </p>
        </div>

        <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
          <div className="flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
            <span>{tr('BLAKE3 + Zstd Object Vault', 'BLAKE3 + Zstd Object Vault')}</span>
            <Shield className="w-4 h-4 text-violet-500" />
          </div>
          <div className="mt-2 flex items-baseline justify-between">
            <span className="text-2xl font-bold font-mono tabular-nums text-slate-900 dark:text-slate-100">
              {edgeCloud.blake3Snapshots.length}
            </span>
            <span className="text-xs font-mono tabular-nums text-violet-600 dark:text-violet-400">
              3.8x Merkle Dedup
            </span>
          </div>
          <p className="mt-1 text-[11px] font-mono text-slate-500 truncate">
            {edgeCloud.blake3EngineVersion}
          </p>
        </div>
      </div>

      {/* SUB-TAB 1: WASMTIME WASI 0.2 EDGE WORKERS */}
      {subTab === 'wasi' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          {/* Left 7 cols: Active WASI Edge Workers */}
          <div className="lg:col-span-7 space-y-4">
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
                <div>
                  <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                    {tr(
                      'Danh sách Wasmtime WASI Serverless Workers',
                      'Wasmtime WASI Serverless Workers'
                    )}
                  </h2>
                  <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                    {tr(
                      'Thực thi trực tiếp tại tầng Reverse Proxy FerrousX với thời gian khởi động lạnh < 250µs.',
                      'Executed directly inside the FerrousX Reverse Proxy layer with < 250µs cold-start latency.'
                    )}
                  </p>
                </div>
                <span className="text-xs font-mono text-slate-500">
                  wasm32-wasip2
                </span>
              </div>

              <div className="divide-y divide-slate-200 dark:divide-slate-800">
                {edgeCloud.wasiWorkers.map((w) => (
                  <div key={w.id} className="p-5 space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <span
                          className={`px-2 py-0.5 text-[11px] font-mono font-semibold rounded-md ${
                            w.status === 'active'
                              ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                              : 'bg-slate-500/10 text-slate-500'
                          }`}
                        >
                          {w.status.toUpperCase()}
                        </span>
                        <span className="text-sm font-bold font-mono text-slate-900 dark:text-slate-100 truncate">
                          {w.name}
                        </span>
                        <span className="text-xs font-mono text-orange-600 dark:text-orange-400">
                          {w.targetDomain}
                          {w.routePattern}
                        </span>
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        <button
                          type="button"
                          onClick={() => handleInvokeClick(w.id)}
                          disabled={invokingId === w.id}
                          className="px-2.5 py-1 rounded-md text-xs font-semibold bg-orange-600 hover:bg-orange-500 text-white whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer transition-colors"
                        >
                          <Play className="w-3 h-3" />
                          <span>
                            {invokingId === w.id
                              ? tr('Đang chạy...', 'Invoking...')
                              : tr('Test WASI', 'Invoke WASI')}
                          </span>
                        </button>
                        <button
                          type="button"
                          onClick={() => onToggleWasiWorker(w.id)}
                          className="px-2.5 py-1 rounded-md text-xs font-medium border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
                        >
                          {w.status === 'active'
                            ? tr('Tạm dừng', 'Pause')
                            : tr('Kích hoạt', 'Resume')}
                        </button>
                        <button
                          type="button"
                          onClick={() => onDeleteWasiWorker(w.id)}
                          className="p-1.5 rounded-md border border-slate-200 dark:border-slate-700 text-rose-500 hover:bg-rose-500/10 cursor-pointer"
                          title={tr('Xóa Worker', 'Delete Worker')}
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs font-mono tabular-nums text-slate-600 dark:text-slate-400">
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Cold Start / Avg
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {w.coldStartUs}µs / {w.avgExecutionUs}µs
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Linear Memory
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {w.memoryUsedKb} / {w.memoryLimitKb} KB
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Invocations (24h)
                        </span>
                        <span className="text-emerald-600 dark:text-emerald-400 font-semibold">
                          {w.invocations24h.toLocaleString()}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          FerrousKV Binding
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 truncate block">
                          {w.kvNamespace}
                        </span>
                      </div>
                    </div>

                    <pre className="p-3 rounded-lg bg-slate-950 text-slate-200 text-[11px] font-mono overflow-x-auto leading-relaxed">
                      {w.rustSourceSnippet}
                    </pre>

                    <div className="flex items-center justify-between text-[11px] font-mono text-slate-400">
                      <span className="truncate">BLAKE3: {w.blake3Hash}</span>
                      <span className="shrink-0">Last: {w.lastInvokedAt}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            {/* Live WASI Invocation Output Inspector */}
            {invokeResult && (
              <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <CheckCircle2 className="w-4 h-4 text-emerald-500" />
                    <h3 className="text-xs font-bold uppercase tracking-wider text-slate-900 dark:text-slate-100">
                      {tr(
                        'Kết quả Thực thi Wasmtime WASI Trực tiếp',
                        'Live Wasmtime WASI Execution Telemetry'
                      )}
                    </h3>
                  </div>
                  <span className="text-xs font-mono tabular-nums text-emerald-600 dark:text-emerald-400 font-semibold">
                    HTTP {invokeResult.statusCode} · {invokeResult.executionTimeUs}µs
                  </span>
                </div>

                <div className="grid grid-cols-3 gap-3 text-xs font-mono tabular-nums">
                  <div>
                    <span className="text-slate-400 block text-[11px]">Cold Start</span>
                    <span className="font-semibold text-slate-900 dark:text-slate-100">
                      {invokeResult.coldStartUs} µs
                    </span>
                  </div>
                  <div>
                    <span className="text-slate-400 block text-[11px]">Fuel Consumed</span>
                    <span className="font-semibold text-slate-900 dark:text-slate-100">
                      {invokeResult.fuelConsumed.toLocaleString()} units
                    </span>
                  </div>
                  <div>
                    <span className="text-slate-400 block text-[11px]">Peak Memory</span>
                    <span className="font-semibold text-slate-900 dark:text-slate-100">
                      {invokeResult.memoryPeakKb} KB
                    </span>
                  </div>
                </div>

                <pre className="p-3 rounded-lg bg-slate-950 text-emerald-400 text-xs font-mono overflow-x-auto">
                  {invokeResult.responseBody}
                </pre>
              </div>
            )}
          </div>

          {/* Right 5 cols: Deploy New Rust WASI Worker */}
          <div className="lg:col-span-5">
            <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 space-y-4">
              <div>
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                  {tr(
                    'Biên dịch & Triển khai Rust WASI Worker Mới',
                    'Compile & Deploy New Rust WASI Worker'
                  )}
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  {tr(
                    'Biên dịch mã nguồn Rust sang chuẩn Component Model wasm32-wasip2 và gắn vào tuyến FerrousX.',
                    'Compiles Rust code to wasm32-wasip2 Component Model and attaches it to a FerrousX route.'
                  )}
                </p>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Tên Worker (Rust Crate)', 'Worker Name (Rust Crate)')}
                  </label>
                  <input
                    type="text"
                    value={workerName}
                    onChange={(e) => setWorkerName(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      {tr('Tên miền áp dụng', 'Target Domain')}
                    </label>
                    <input
                      type="text"
                      value={targetDomain}
                      onChange={(e) => setTargetDomain(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      {tr('Route Pattern', 'Route Pattern')}
                    </label>
                    <input
                      type="text"
                      value={routePattern}
                      onChange={(e) => setRoutePattern(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                <div className="grid grid-cols-3 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      WASI Target
                    </label>
                    <select
                      value={wasiTarget}
                      onChange={(e) =>
                        setWasiTarget(
                          e.target.value as 'wasm32-wasip2' | 'wasm32-wasip1'
                        )
                      }
                      className="w-full px-2.5 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    >
                      <option value="wasm32-wasip2">wasm32-wasip2</option>
                      <option value="wasm32-wasip1">wasm32-wasip1</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      RAM Limit (KB)
                    </label>
                    <input
                      type="number"
                      value={memoryLimitKb}
                      onChange={(e) => setMemoryLimitKb(Number(e.target.value))}
                      className="w-full px-2.5 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono tabular-nums text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Fuel Budget
                    </label>
                    <input
                      type="number"
                      value={fuelLimit}
                      onChange={(e) => setFuelLimit(Number(e.target.value))}
                      className="w-full px-2.5 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono tabular-nums text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Không gian khóa FerrousKV', 'FerrousKV Namespace Binding')}
                  </label>
                  <input
                    type="text"
                    value={kvNamespace}
                    onChange={(e) => setKvNamespace(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Mã nguồn Rust (wasi-http proxy)', 'Rust Handler Source (wasi-http)')}
                  </label>
                  <textarea
                    rows={5}
                    value={rustSnippet}
                    onChange={(e) => setRustSnippet(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-950 text-slate-100 text-xs font-mono"
                  />
                </div>

                <button
                  type="button"
                  onClick={() =>
                    onDeployWasiWorker({
                      name: workerName,
                      routePattern,
                      targetDomain,
                      wasiTarget,
                      memoryLimitKb,
                      fuelLimit,
                      kvNamespace,
                      rustSourceSnippet: rustSnippet
                    })
                  }
                  className="w-full py-2.5 px-4 rounded-lg bg-orange-600 hover:bg-orange-500 text-white text-xs font-bold inline-flex items-center justify-center gap-2 cursor-pointer transition-colors"
                >
                  <Plus className="w-4 h-4" />
                  <span>
                    {tr(
                      'Biên dịch cargo build --target wasm32-wasip2 & Triển khai',
                      'Compile cargo build --target wasm32-wasip2 & Deploy'
                    )}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* SUB-TAB 2: YOUKI OCI CONTAINERS & FIRECRACKER MICROVMS */}
      {subTab === 'containers' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-7 space-y-4">
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
                <div>
                  <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                    {tr(
                      'Youki OCI Containers & Firecracker KVM MicroVMs',
                      'Youki OCI Containers & Firecracker KVM MicroVMs'
                    )}
                  </h2>
                  <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                    {tr(
                      'Runtime Container viết bằng Rust thay thế runc/Docker, cô lập bằng Linux Cgroups v2 & Seccomp-BPF.',
                      'Rust-native OCI runtime replacing runc/Docker, isolated via Linux Cgroups v2 & Seccomp-BPF.'
                    )}
                  </p>
                </div>
              </div>

              <div className="divide-y divide-slate-200 dark:divide-slate-800">
                {edgeCloud.ociContainers.map((c) => (
                  <div key={c.id} className="p-5 space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <span
                          className={`px-2 py-0.5 text-[11px] font-mono font-semibold rounded-md ${
                            c.status === 'running'
                              ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                              : 'bg-slate-500/10 text-slate-500'
                          }`}
                        >
                          {c.status.toUpperCase()}
                        </span>
                        <span className="text-sm font-bold font-mono text-slate-900 dark:text-slate-100 truncate">
                          {c.name}
                        </span>
                        <span className="text-xs font-mono text-sky-600 dark:text-sky-400">
                          {c.runtimeEngine}
                        </span>
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        {c.status === 'running' ? (
                          <button
                            type="button"
                            onClick={() => onOciContainerAction(c.id, 'stop')}
                            className="px-2.5 py-1 rounded-md text-xs font-medium border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer"
                          >
                            <Pause className="w-3 h-3" />
                            <span>Stop</span>
                          </button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => onOciContainerAction(c.id, 'start')}
                            className="px-2.5 py-1 rounded-md text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer"
                          >
                            <Play className="w-3 h-3" />
                            <span>Start</span>
                          </button>
                        )}
                        <button
                          type="button"
                          onClick={() => onOciContainerAction(c.id, 'restart')}
                          className="px-2.5 py-1 rounded-md text-xs font-medium border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer"
                        >
                          <RefreshCw className="w-3 h-3" />
                          <span>Restart</span>
                        </button>
                        <button
                          type="button"
                          onClick={() => onOciContainerAction(c.id, 'delete')}
                          className="p-1.5 rounded-md border border-slate-200 dark:border-slate-700 text-rose-500 hover:bg-rose-500/10 cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <div className="text-xs font-mono text-slate-500 truncate">
                      Image: <span className="text-slate-800 dark:text-slate-200">{c.image}</span> · Cgroup:{' '}
                      <span>{c.cgroupsV2Path}</span>
                    </div>

                    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs font-mono tabular-nums">
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Port Mapping
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          127.0.0.1:{c.hostPort} → :{c.containerPort}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Cgroups memory.max
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {c.memoryUsedMb} / {c.memoryMaxMb} MB
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          CPU / PIDs Limit
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {c.cpuUsagePercent}% ({c.pidsCurrent}/{c.pidsMax} PIDs)
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Internal Mesh IP
                        </span>
                        <span className="text-emerald-600 dark:text-emerald-400 font-semibold">
                          {c.internalIp} ({c.uptime})
                        </span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* Right 5 cols: Deploy OCI Container / Firecracker MicroVM */}
          <div className="lg:col-span-5 space-y-4">
            <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 space-y-4">
              <div>
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                  {tr(
                    'Khởi tạo Container Youki OCI / Firecracker Mới',
                    'Deploy New Youki OCI / Firecracker Sandbox'
                  )}
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  {tr(
                    'Chọn mẫu dịch vụ Rust có sẵn hoặc nhập OCI Image tùy chỉnh.',
                    'Select a pre-configured Rust service template or enter a custom OCI image.'
                  )}
                </p>
              </div>

              {/* Quick Presets */}
              <div className="flex flex-wrap gap-1.5">
                <button
                  type="button"
                  onClick={() =>
                    applyContainerPreset(
                      'qdrant-vector-db',
                      'qdrant/qdrant:v1.12.1-rust',
                      'Youki OCI (Rust Rootless)',
                      6333,
                      6333,
                      2048
                    )
                  }
                  className="px-2.5 py-1 rounded-md text-xs font-mono border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                >
                  + Qdrant Vector
                </button>
                <button
                  type="button"
                  onClick={() =>
                    applyContainerPreset(
                      'meilisearch-rs',
                      'getmeili/meilisearch:v1.11-rust',
                      'Youki OCI (Rust Rootless)',
                      7700,
                      7700,
                      1024
                    )
                  }
                  className="px-2.5 py-1 rounded-md text-xs font-mono border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                >
                  + Meilisearch
                </button>
                <button
                  type="button"
                  onClick={() =>
                    applyContainerPreset(
                      'ferrouskv-cache',
                      'ghcr.io/ferrous-panel/ferrouskv-server:1.4-rust',
                      'Youki OCI (Rust Rootless)',
                      6380,
                      6379,
                      1024
                    )
                  }
                  className="px-2.5 py-1 rounded-md text-xs font-mono border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                >
                  + FerrousKV
                </button>
                <button
                  type="button"
                  onClick={() =>
                    applyContainerPreset(
                      'firecracker-microvm',
                      'ghcr.io/ferrous-panel/alpine-rust-microvm:6.8-kvm',
                      'Firecracker MicroVM (Rust KVM)',
                      9090,
                      8080,
                      512
                    )
                  }
                  className="px-2.5 py-1 rounded-md text-xs font-mono border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                >
                  + Firecracker KVM
                </button>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Tên Container / MicroVM', 'Container / MicroVM Name')}
                  </label>
                  <input
                    type="text"
                    value={containerName}
                    onChange={(e) => setContainerName(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    OCI Image URI
                  </label>
                  <input
                    type="text"
                    value={containerImage}
                    onChange={(e) => setContainerImage(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Rust Sandbox Engine', 'Rust Sandbox Engine')}
                  </label>
                  <select
                    value={runtimeEngine}
                    onChange={(e) =>
                      setRuntimeEngine(
                        e.target.value as
                          | 'Youki OCI (Rust Rootless)'
                          | 'Firecracker MicroVM (Rust KVM)'
                      )
                    }
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  >
                    <option value="Youki OCI (Rust Rootless)">
                      Youki OCI (Rust Rootless + Cgroups v2)
                    </option>
                    <option value="Firecracker MicroVM (Rust KVM)">
                      Firecracker MicroVM (Rust KVM Hardware Virtualization)
                    </option>
                  </select>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Host Port
                    </label>
                    <input
                      type="number"
                      value={hostPort}
                      onChange={(e) => setHostPort(Number(e.target.value))}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono tabular-nums text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Container Port
                    </label>
                    <input
                      type="number"
                      value={containerPort}
                      onChange={(e) => setContainerPort(Number(e.target.value))}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono tabular-nums text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      memory.max (MB)
                    </label>
                    <input
                      type="number"
                      value={memoryMaxMb}
                      onChange={(e) => setMemoryMaxMb(Number(e.target.value))}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono tabular-nums text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      cpu.max Quota (%)
                    </label>
                    <input
                      type="number"
                      value={cpuQuotaPercent}
                      onChange={(e) => setCpuQuotaPercent(Number(e.target.value))}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono tabular-nums text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                <button
                  type="button"
                  onClick={() =>
                    onDeployOciContainer({
                      name: containerName,
                      image: containerImage,
                      runtimeEngine,
                      hostPort,
                      containerPort,
                      memoryMaxMb,
                      cpuQuotaPercent
                    })
                  }
                  className="w-full py-2.5 px-4 rounded-lg bg-orange-600 hover:bg-orange-500 text-white text-xs font-bold inline-flex items-center justify-center gap-2 cursor-pointer transition-colors"
                >
                  <Plus className="w-4 h-4" />
                  <span>
                    {tr(
                      'Khởi chạy Sandbox qua Youki / Firecracker',
                      'Launch Sandbox via Youki / Firecracker'
                    )}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* SUB-TAB 3: BORINGTUN WIREGUARD ZERO-TRUST MESH */}
      {subTab === 'wireguard' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-7 space-y-4">
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex flex-wrap items-center justify-between gap-2">
                <div>
                  <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                    {tr(
                      'BoringTun WireGuard Zero-Trust Mesh (wg0)',
                      'BoringTun WireGuard Zero-Trust Mesh (wg0)'
                    )}
                  </h2>
                  <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                    {tr(
                      'Kết nối liên cụm máy chủ & Database Replica qua đường hầm mã hóa ChaCha20-Poly1305 / X25519 không cần mở cổng Public.',
                      'Inter-node cluster & DB replica mesh encrypted via ChaCha20-Poly1305 / X25519 without exposing public ports.'
                    )}
                  </p>
                </div>
                <span className="text-xs font-mono text-emerald-600 dark:text-emerald-400">
                  {edgeCloud.meshSubnetCidr}
                </span>
              </div>

              <div className="px-5 py-3 bg-slate-50 dark:bg-slate-950 border-b border-slate-200 dark:border-slate-800 text-xs font-mono flex flex-wrap items-center justify-between gap-2">
                <span className="text-slate-500">
                  Node Public Key:{' '}
                  <strong className="text-slate-900 dark:text-slate-100">
                    {edgeCloud.serverPublicKey}
                  </strong>
                </span>
                <span className="text-slate-500">
                  UDP Listen: <strong>:{edgeCloud.wireguardListenPort}</strong>
                </span>
              </div>

              <div className="divide-y divide-slate-200 dark:divide-slate-800">
                {edgeCloud.wireguardPeers.map((p) => (
                  <div key={p.id} className="p-5 space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex items-center gap-2.5 min-w-0">
                        <span
                          className={`px-2 py-0.5 text-[11px] font-mono font-semibold rounded-md ${
                            p.status === 'connected'
                              ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                              : 'bg-slate-500/10 text-slate-500'
                          }`}
                        >
                          {p.status.toUpperCase()}
                        </span>
                        <span className="text-sm font-bold text-slate-900 dark:text-slate-100 truncate">
                          {p.name}
                        </span>
                        <span className="text-xs font-mono text-slate-500">
                          [{p.region}]
                        </span>
                      </div>

                      <div className="flex items-center gap-1.5 shrink-0">
                        <button
                          type="button"
                          onClick={() => onToggleWireGuardPeer(p.id)}
                          className="px-2.5 py-1 rounded-md text-xs font-medium border border-slate-200 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
                        >
                          {p.status === 'connected'
                            ? tr('Ngắt kết nối', 'Disconnect')
                            : tr('Kết nối lại', 'Reconnect')}
                        </button>
                        <button
                          type="button"
                          onClick={() => onDeleteWireGuardPeer(p.id)}
                          className="p-1.5 rounded-md border border-slate-200 dark:border-slate-700 text-rose-500 hover:bg-rose-500/10 cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs font-mono tabular-nums">
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Mesh IP / Allowed
                        </span>
                        <span className="text-emerald-600 dark:text-emerald-400 font-semibold">
                          {p.allowedIps}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Endpoint & RTT
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {p.endpoint} ({p.latencyMs}ms)
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Traffic RX / TX
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          ↓{p.rxMb} MB / ↑{p.txMb} MB
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Latest Handshake
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {p.latestHandshake}
                        </span>
                      </div>
                    </div>

                    <div className="text-[11px] font-mono text-slate-400 flex flex-wrap items-center justify-between gap-2">
                      <span className="truncate">Peer Key: {p.publicKey}</span>
                      <span className="shrink-0">{p.cipherSuite}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="lg:col-span-5">
            <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 space-y-4">
              <div>
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                  {tr(
                    'Thêm Node WireGuard Mesh Mới (BoringTun)',
                    'Add New WireGuard Mesh Peer (BoringTun)'
                  )}
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  {tr(
                    'Tự động sinh cặp khóa X25519 và PresharedKey ChaCha20-Poly1305 cho Node mới.',
                    'Automatically generates X25519 keypair and ChaCha20-Poly1305 PresharedKey for the peer.'
                  )}
                </p>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Tên Node / Máy chủ Peer', 'Peer Node Name')}
                  </label>
                  <input
                    type="text"
                    value={peerName}
                    onChange={(e) => setPeerName(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Region Code
                    </label>
                    <input
                      type="text"
                      value={peerRegion}
                      onChange={(e) => setPeerRegion(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Allowed Mesh IP (CIDR)
                    </label>
                    <input
                      type="text"
                      value={peerAllowedIps}
                      onChange={(e) => setPeerAllowedIps(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    Endpoint (IP:Port)
                  </label>
                  <input
                    type="text"
                    value={peerEndpoint}
                    onChange={(e) => setPeerEndpoint(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <button
                  type="button"
                  onClick={() =>
                    onAddWireGuardPeer({
                      name: peerName,
                      region: peerRegion,
                      endpoint: peerEndpoint,
                      allowedIps: peerAllowedIps
                    })
                  }
                  className="w-full py-2.5 px-4 rounded-lg bg-orange-600 hover:bg-orange-500 text-white text-xs font-bold inline-flex items-center justify-center gap-2 cursor-pointer transition-colors"
                >
                  <Plus className="w-4 h-4" />
                  <span>
                    {tr(
                      'Sinh khóa X25519 & Kết nối vào Mesh wg0',
                      'Generate X25519 Keypair & Attach to wg0 Mesh'
                    )}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* SUB-TAB 4: BLAKE3 + ZSTD MERKLE OBJECT SNAPSHOTS */}
      {subTab === 'snapshots' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-7 space-y-4">
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800">
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                  {tr(
                    'BLAKE3 Merkle-Tree + Zstandard Incremental Snapshots',
                    'BLAKE3 Merkle-Tree + Zstandard Incremental Snapshots'
                  )}
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  {tr(
                    'Chống trùng lặp khối dữ liệu tốc độ SIMD AVX-512 và khóa bất biến (Immutable Object Lock) chống Ransomware.',
                    'SIMD AVX-512 content-addressable chunk deduplication with Ransomware-proof Immutable Object Lock.'
                  )}
                </p>
              </div>

              <div className="divide-y divide-slate-200 dark:divide-slate-800">
                {edgeCloud.blake3Snapshots.map((s) => (
                  <div key={s.id} className="p-5 space-y-3">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex items-center gap-2 min-w-0">
                        <span className="px-2 py-0.5 text-[11px] font-mono font-semibold rounded-md bg-violet-500/10 text-violet-600 dark:text-violet-400">
                          {s.status.toUpperCase()}
                        </span>
                        <span className="text-sm font-bold font-mono text-slate-900 dark:text-slate-100 truncate">
                          {s.snapshotName}
                        </span>
                        {s.immutableLock && (
                          <span className="px-2 py-0.5 text-[11px] font-mono rounded-md bg-amber-500/10 text-amber-600 dark:text-amber-400 inline-flex items-center gap-1">
                            <Lock className="w-3 h-3" />
                            WORM Locked
                          </span>
                        )}
                      </div>

                      {!s.immutableLock && (
                        <button
                          type="button"
                          onClick={() => onDeleteBlake3Snapshot(s.id)}
                          className="p-1.5 rounded-md border border-slate-200 dark:border-slate-700 text-rose-500 hover:bg-rose-500/10 cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      )}
                    </div>

                    <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 text-xs font-mono tabular-nums">
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Logical → Stored
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {s.logicalSizeMb} MB → {s.storedSizeMb} MB
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Dedup Ratio
                        </span>
                        <span className="text-emerald-600 dark:text-emerald-400 font-semibold">
                          {s.dedupRatio}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          Compression
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 font-semibold">
                          {s.compression}
                        </span>
                      </div>
                      <div>
                        <span className="text-slate-400 block text-[11px]">
                          S3 Bucket Vault
                        </span>
                        <span className="text-slate-900 dark:text-slate-200 truncate block">
                          {s.bucket}
                        </span>
                      </div>
                    </div>

                    <div className="text-[11px] font-mono text-slate-400 flex flex-wrap items-center justify-between gap-2">
                      <span className="truncate">Root: {s.merkleRootBlake3}</span>
                      <span className="shrink-0">
                        {s.chunksCount} chunks · {s.createdAt}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="lg:col-span-5">
            <div className="p-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 space-y-4">
              <div>
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                  {tr(
                    'Tạo Bản Sao Lưu BLAKE3 Merkle Mới',
                    'Create New BLAKE3 Merkle Snapshot'
                  )}
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  {tr(
                    'Băm song song đa luồng bằng BLAKE3 và nén Zstandard lên S3/R2 Object Storage.',
                    'Parallel multi-threaded BLAKE3 hashing and Zstandard compression to S3/R2 Object Storage.'
                  )}
                </p>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    Snapshot Name
                  </label>
                  <input
                    type="text"
                    value={snapshotName}
                    onChange={(e) => setSnapshotName(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                    {tr('Tài nguyên Sao lưu', 'Target Resources')}
                  </label>
                  <input
                    type="text"
                    value={targetResource}
                    onChange={(e) => setTargetResource(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      S3 / R2 Endpoint
                    </label>
                    <input
                      type="text"
                      value={s3Endpoint}
                      onChange={(e) => setS3Endpoint(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Bucket Name
                    </label>
                    <input
                      type="text"
                      value={bucketName}
                      onChange={(e) => setBucketName(e.target.value)}
                      className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-900 dark:text-slate-100"
                    />
                  </div>
                </div>

                <label className="flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300 cursor-pointer pt-1">
                  <input
                    type="checkbox"
                    checked={immutableLock}
                    onChange={(e) => setImmutableLock(e.target.checked)}
                    className="rounded border-slate-300"
                  />
                  <span>
                    {tr(
                      'Bật khóa bất biến WORM (Immutable Lock chống xóa/sửa)',
                      'Enable WORM Immutable Object Lock (Ransomware Protection)'
                    )}
                  </span>
                </label>

                <button
                  type="button"
                  onClick={() =>
                    onCreateBlake3Snapshot({
                      snapshotName,
                      targetResource,
                      s3Endpoint,
                      bucket: bucketName,
                      immutableLock
                    })
                  }
                  className="w-full py-2.5 px-4 rounded-lg bg-orange-600 hover:bg-orange-500 text-white text-xs font-bold inline-flex items-center justify-center gap-2 cursor-pointer transition-colors"
                >
                  <HardDriveDownload className="w-4 h-4" />
                  <span>
                    {tr(
                      'Chạy BLAKE3 Merkle Dedup & Lưu trữ',
                      'Execute BLAKE3 Merkle Dedup & Archive'
                    )}
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
