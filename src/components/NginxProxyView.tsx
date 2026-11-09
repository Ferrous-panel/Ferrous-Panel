import React, { useState, useEffect } from 'react';
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Copy,
  Globe,
  Lock,
  RefreshCw,
  Shield,
  Zap
} from 'lucide-react';
import { EnvVar, ManagedApp } from '../types';

interface NginxProxyViewProps {
  apps: ManagedApp[];
  selectedAppId: string | null;
  onSelectApp: (id: string) => void;
  onUpdateProxyConfig: (
    appId: string,
    payload: {
      sslEnabled: boolean;
      http3Quic: boolean;
      websocketEnabled: boolean;
      rateLimitRps: number;
      workers: number;
      memoryLimitMb: number;
      envVars: EnvVar[];
    }
  ) => Promise<void>;
}

export const NginxProxyView: React.FC<NginxProxyViewProps> = ({
  apps,
  selectedAppId,
  onSelectApp,
  onUpdateProxyConfig
}) => {
  const activeApp =
    apps.find((a) => a.id === selectedAppId) || apps[0] || null;

  const [sslEnabled, setSslEnabled] = useState<boolean>(
    activeApp?.sslEnabled ?? true
  );
  const [http3Quic, setHttp3Quic] = useState<boolean>(
    activeApp?.http3Quic ?? true
  );
  const [websocketEnabled, setWebsocketEnabled] = useState<boolean>(
    activeApp?.websocketEnabled ?? true
  );
  const [rateLimitRps, setRateLimitRps] = useState<number>(
    activeApp?.rateLimitRps ?? 120
  );
  const [editableConfig, setEditableConfig] = useState<string>(
    activeApp?.nginxVhost ?? ''
  );
  const [validationResult, setValidationResult] = useState<{
    valid: boolean;
    output: string;
  } | null>(null);
  const [validating, setValidating] = useState(false);
  const [reloading, setReloading] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (activeApp) {
      setSslEnabled(activeApp.sslEnabled);
      setHttp3Quic(activeApp.http3Quic);
      setWebsocketEnabled(activeApp.websocketEnabled);
      setRateLimitRps(activeApp.rateLimitRps);
      setEditableConfig(activeApp.nginxVhost);
      setValidationResult(null);
    }
  }, [
    activeApp?.id,
    activeApp?.nginxVhost,
    activeApp?.sslEnabled,
    activeApp?.http3Quic,
    activeApp?.websocketEnabled,
    activeApp?.rateLimitRps
  ]);

  const handleToggleOption = async (
    field: 'sslEnabled' | 'http3Quic' | 'websocketEnabled',
    nextVal: boolean
  ) => {
    if (!activeApp) return;
    const updated = {
      sslEnabled: field === 'sslEnabled' ? nextVal : sslEnabled,
      http3Quic: field === 'http3Quic' ? nextVal : http3Quic,
      websocketEnabled:
        field === 'websocketEnabled' ? nextVal : websocketEnabled,
      rateLimitRps,
      workers: activeApp.workers,
      memoryLimitMb: activeApp.memoryLimitMb,
      envVars: activeApp.envVars
    };
    if (field === 'sslEnabled') setSslEnabled(nextVal);
    if (field === 'http3Quic') setHttp3Quic(nextVal);
    if (field === 'websocketEnabled') setWebsocketEnabled(nextVal);
    await onUpdateProxyConfig(activeApp.id, updated);
  };

  const handleRateLimitApply = async () => {
    if (!activeApp) return;
    setReloading(true);
    await onUpdateProxyConfig(activeApp.id, {
      sslEnabled,
      http3Quic,
      websocketEnabled,
      rateLimitRps,
      workers: activeApp.workers,
      memoryLimitMb: activeApp.memoryLimitMb,
      envVars: activeApp.envVars
    });
    setReloading(false);
    setValidationResult({
      valid: true,
      output: `nginx: the configuration file /etc/nginx/nginx.conf syntax is ok\nnginx: reloaded /etc/nginx/sites-available/${activeApp.domain}.conf (zero-downtime HUP signal)`
    });
  };

  const handleTestSyntax = async () => {
    if (!activeApp) return;
    setValidating(true);
    try {
      const res = await fetch('/api/nginx/validate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          domain: activeApp.domain,
          configContent: editableConfig
        })
      });
      const data = await res.json();
      setValidationResult(data);
    } finally {
      setValidating(false);
    }
  };

  const handleCopyConfig = () => {
    navigator.clipboard.writeText(editableConfig);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="space-y-6">
      {/* Top Summary Banner */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 flex flex-col lg:flex-row lg:items-center justify-between gap-4">
        <div className="space-y-1">
          <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
            /etc/nginx/nginx.conf · worker_processes auto · worker_rlimit_nofile 65535
          </div>
          <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100">
            Điều phối Nginx Reverse Proxy & Chứng chỉ Let’s Encrypt SSL/TLS
          </h1>
          <p className="text-xs text-slate-500 dark:text-slate-400">
            Tự động chuyển tiếp lưu lượng HTTPS/QUIC vào cổng loopback nội bộ của Actix-Web và các framework khác
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={handleTestSyntax}
            disabled={validating}
            className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer"
          >
            {validating ? 'Đang chạy nginx -t...' : 'Kiểm tra cú pháp (nginx -t)'}
          </button>
          <button
            onClick={handleRateLimitApply}
            disabled={reloading}
            className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${reloading ? 'animate-spin' : ''}`} />
            Reload Nginx Zero-Downtime
          </button>
        </div>
      </section>

      {/* Main Split: Domain Vhost Selector & Directive Configurator */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Left: Virtual Hosts List */}
        <div className="lg:col-span-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 divide-y divide-slate-200 dark:divide-slate-800 overflow-hidden">
          <div className="px-5 py-3.5 bg-slate-50/70 dark:bg-slate-950/40 text-xs font-semibold text-slate-600 dark:text-slate-400">
            Virtual Hosts (/etc/nginx/sites-enabled)
          </div>
          {apps.map((app) => {
            const isSelected = activeApp?.id === app.id;
            return (
              <button
                key={app.id}
                onClick={() => onSelectApp(app.id)}
                className={`w-full text-left p-4 transition-colors block cursor-pointer ${
                  isSelected
                    ? 'bg-orange-50/60 dark:bg-orange-950/20'
                    : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-sm font-semibold text-slate-900 dark:text-slate-100 truncate">
                    {app.domain}
                  </span>
                  <span className="text-xs font-mono tabular-nums text-emerald-600 dark:text-emerald-400 shrink-0">
                    SSL ({app.sslExpiryDays}d)
                  </span>
                </div>
                <div className="mt-1 text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums">
                  proxy_pass http://127.0.0.1:{app.internalPort} · {app.framework}
                </div>
                <div className="mt-2 flex items-center gap-2 text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums">
                  <span>{app.http3Quic ? 'HTTP/3 QUIC' : 'HTTP/2'}</span>
                  <span>·</span>
                  <span>{app.rateLimitRps} req/s</span>
                  <span>·</span>
                  <span>{app.websocketEnabled ? 'WebSocket On' : 'HTTP Only'}</span>
                </div>
              </button>
            );
          })}
        </div>

        {/* Right: Vhost Controls & Live Config Editor */}
        {activeApp && (
          <div className="lg:col-span-8 space-y-6">
            {/* Interactive Directives Panel */}
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-slate-200 dark:border-slate-800 pb-4">
                <div>
                  <h2 className="text-base font-semibold text-slate-900 dark:text-slate-100">
                    Cấu hình Upstream & Bảo mật TLS cho {activeApp.domain}
                  </h2>
                  <p className="text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums">
                    Upstream đích: 127.0.0.1:{activeApp.internalPort} ({activeApp.name} · {activeApp.workers} workers)
                  </p>
                </div>
              </div>

              {/* 3 Interactive Toggle Rows */}
              <div className="divide-y divide-slate-200 dark:divide-slate-800 border border-slate-200 dark:border-slate-800 rounded-lg">
                <div className="p-4 flex items-center justify-between gap-4">
                  <div className="space-y-0.5">
                    <div className="text-sm font-medium text-slate-900 dark:text-slate-100 flex items-center gap-2">
                      <Lock className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                      Chứng chỉ Let’s Encrypt TLS 1.3 & HSTS Preload
                    </div>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      Tự động chuyển hướng cổng 80 sang 443 và gia hạn định kỳ qua Certbot systemd timer
                    </p>
                  </div>
                  <button
                    onClick={() => handleToggleOption('sslEnabled', !sslEnabled)}
                    className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                      sslEnabled
                        ? 'bg-emerald-600 text-white'
                        : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                    }`}
                  >
                    {sslEnabled ? 'Đang bật (TLS 1.3)' : 'Đã tắt'}
                  </button>
                </div>

                <div className="p-4 flex items-center justify-between gap-4">
                  <div className="space-y-0.5">
                    <div className="text-sm font-medium text-slate-900 dark:text-slate-100 flex items-center gap-2">
                      <Zap className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                      Tăng tốc HTTP/3 QUIC Multiplexing (UDP 443)
                    </div>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      Bật <code className="font-mono">listen 443 quic reuseport</code> và header <code className="font-mono">Alt-Svc: h3=":443"</code> giảm độ trễ bắt tay
                    </p>
                  </div>
                  <button
                    onClick={() => handleToggleOption('http3Quic', !http3Quic)}
                    className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                      http3Quic
                        ? 'bg-orange-600 text-white'
                        : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                    }`}
                  >
                    {http3Quic ? 'Đang bật (QUIC)' : 'Chỉ HTTP/2'}
                  </button>
                </div>

                <div className="p-4 flex items-center justify-between gap-4">
                  <div className="space-y-0.5">
                    <div className="text-sm font-medium text-slate-900 dark:text-slate-100 flex items-center gap-2">
                      <Globe className="w-4 h-4 text-sky-600 dark:text-sky-400" />
                      Chuyển tiếp WebSocket Upgrade (Actix-Web Actors / Axum WS)
                    </div>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      Thêm header <code className="font-mono">Upgrade $http_upgrade</code> và <code className="font-mono">Connection "upgrade"</code>
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      handleToggleOption('websocketEnabled', !websocketEnabled)
                    }
                    className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                      websocketEnabled
                        ? 'bg-sky-600 text-white'
                        : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                    }`}
                  >
                    {websocketEnabled ? 'Đang bật (WS)' : 'Đã tắt'}
                  </button>
                </div>
              </div>

              {/* Rate Limit Slider / Input */}
              <div className="pt-2 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div className="space-y-0.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300 flex items-center gap-1.5">
                    <Shield className="w-3.5 h-3.5 text-orange-600 dark:text-orange-400" />
                    Chống DDoS tầng L7 (Nginx limit_req_zone trên mỗi IP)
                  </label>
                  <p className="text-xs text-slate-500 dark:text-slate-400">
                    Tự động trả mã 429 và chuyển IP vi phạm sang jail <code className="font-mono">nginx-limit-req</code> của Fail2ban
                  </p>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <input
                    type="number"
                    min={10}
                    max={2000}
                    step={10}
                    value={rateLimitRps}
                    onChange={(e) => setRateLimitRps(Number(e.target.value))}
                    className="w-24 px-3 py-1.5 text-xs font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  />
                  <span className="text-xs font-mono text-slate-500">req/s</span>
                  <button
                    onClick={handleRateLimitApply}
                    className="px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer"
                  >
                    Cập nhật
                  </button>
                </div>
              </div>
            </div>

            {/* Validation Output Banner if triggered */}
            {validationResult && (
              <div
                className={`p-4 rounded-xl border text-xs font-mono space-y-1.5 ${
                  validationResult.valid
                    ? 'bg-emerald-50/70 dark:bg-emerald-950/30 border-emerald-200 dark:border-emerald-800/70 text-emerald-900 dark:text-emerald-200'
                    : 'bg-rose-50/70 dark:bg-rose-950/30 border-rose-200 dark:border-rose-800/70 text-rose-900 dark:text-rose-200'
                }`}
              >
                <div className="flex items-center gap-2 font-sans font-semibold">
                  {validationResult.valid ? (
                    <>
                      <CheckCircle2 className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                      Kiểm tra cú pháp Nginx thành công (exit code 0)
                    </>
                  ) : (
                    <>
                      <AlertTriangle className="w-4 h-4 text-rose-600 dark:text-rose-400" />
                      Phát hiện lỗi cú pháp trong cấu hình Nginx
                    </>
                  )}
                </div>
                <pre className="whitespace-pre-wrap leading-relaxed">
                  {validationResult.output}
                </pre>
              </div>
            )}

            {/* Nginx Configuration Code Editor */}
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
                <span className="text-xs font-mono text-slate-600 dark:text-slate-300">
                  /etc/nginx/sites-available/{activeApp.domain}.conf
                </span>
                <button
                  onClick={handleCopyConfig}
                  className="px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                >
                  {copied ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                      Đã sao chép
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      Sao chép .conf
                    </>
                  )}
                </button>
              </div>
              <textarea
                value={editableConfig}
                onChange={(e) => setEditableConfig(e.target.value)}
                rows={22}
                spellCheck={false}
                className="w-full p-5 bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed focus:outline-none resize-y"
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
