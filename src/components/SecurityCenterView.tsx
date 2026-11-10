import React, { useState } from 'react';
import {
  Ban,
  CheckCircle2,
  Cpu,
  FileCheck2,
  Flame,
  Lock,
  Plus,
  RefreshCw,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  Unlock,
  Zap
} from 'lucide-react';
import { SecurityData } from '../types';

interface SecurityCenterViewProps {
  security: SecurityData | null;
  onAddUfwRule: (rule: {
    port: string;
    protocol: 'TCP' | 'UDP' | 'BOTH';
    action: 'ALLOW' | 'DENY' | 'LIMIT';
    source: string;
    comment: string;
  }) => Promise<void>;
  onToggleUfwRule: (id: string) => Promise<void>;
  onDeleteUfwRule: (id: string) => Promise<void>;
  onBanIp: (jailName: string, ip: string, reason: string) => Promise<void>;
  onUnbanIp: (jailName: string, ip: string) => Promise<void>;
  onToggleHardening: (id: string) => Promise<void>;
  onChangeDdosMode?: (
    mode: 'normal' | 'elevated' | 'under_attack'
  ) => Promise<void>;
  onToggleWafRule?: (id: string) => Promise<void>;
  onRunFimScan?: () => Promise<void>;
}

export const SecurityCenterView: React.FC<SecurityCenterViewProps> = ({
  security,
  onAddUfwRule,
  onToggleUfwRule,
  onDeleteUfwRule,
  onBanIp,
  onUnbanIp,
  onToggleHardening,
  onChangeDdosMode,
  onToggleWafRule,
  onRunFimScan
}) => {
  const [port, setPort] = useState('');
  const [protocol, setProtocol] = useState<'TCP' | 'UDP' | 'BOTH'>('TCP');
  const [action, setAction] = useState<'ALLOW' | 'DENY' | 'LIMIT'>('ALLOW');
  const [source, setSource] = useState('0.0.0.0/0');
  const [comment, setComment] = useState('');

  const [banJail, setBanJail] = useState('nginx-limit-req');
  const [banIpInput, setBanIpInput] = useState('');
  const [banReasonInput, setBanReasonInput] = useState('');
  const [scanningFim, setScanningFim] = useState(false);
  const [switchingMode, setSwitchingMode] = useState<string | null>(null);

  if (!security) {
    return (
      <div className="h-64 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 animate-pulse p-6" />
    );
  }

  const shield = security.antiDdosShield;

  const handleCreateRule = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!port.trim()) return;
    await onAddUfwRule({
      port: port.trim(),
      protocol,
      action,
      source: source.trim() || '0.0.0.0/0',
      comment: comment.trim() || 'Custom Ubuntu UFW Rule'
    });
    setPort('');
    setComment('');
  };

  const handleManualBan = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!banIpInput.trim()) return;
    await onBanIp(
      banJail,
      banIpInput.trim(),
      banReasonInput.trim() || 'Chặn thủ công bởi Quản trị viên FerrousPanel'
    );
    setBanIpInput('');
    setBanReasonInput('');
  };

  const handleModeSwitch = async (
    mode: 'normal' | 'elevated' | 'under_attack'
  ) => {
    if (!onChangeDdosMode) return;
    setSwitchingMode(mode);
    try {
      await onChangeDdosMode(mode);
    } finally {
      setSwitchingMode(null);
    }
  };

  const handleFimScanClick = async () => {
    if (!onRunFimScan) return;
    setScanningFim(true);
    try {
      await onRunFimScan();
    } finally {
      setScanningFim(false);
    }
  };

  return (
    <div className="space-y-8">
      {/* Section 0: Multi-Layer Anti-Hacking Shield (eBPF/XDP DDoS + Rust Pingora L7 WAF + FIM SHA-256) */}
      {shield && (
        <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          <div className="p-6 border-b border-slate-200 dark:border-slate-800 flex flex-col lg:flex-row lg:items-center justify-between gap-4">
            <div className="space-y-1">
              <div className="text-xs font-mono text-orange-600 dark:text-orange-400">
                Ferrous-panel/src/security.rs · eBPF/XDP Driver Filter · Rust Pingora L7 WAF · SHA-256 FIM
              </div>
              <h2 className="text-lg font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                <Shield className="w-5 h-5 text-orange-600 dark:text-orange-400" />
                Lá Chắn Chống Tấn Công Đa Tầng (eBPF/XDP Anti-DDoS &amp; Pingora Layer-7 WAF)
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                Chặn gói tin DDoS ngay tại tầng card mạng Linux (<code className="font-mono">XDP_DROP</code>) và lọc độc hại SQLi, RCE, SSRF, Path Traversal trên luồng proxy Rust trước khi chạm tới ứng dụng.
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-2 shrink-0">
              <div className="inline-flex items-center p-1 rounded-lg bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800">
                <button
                  type="button"
                  onClick={() => handleModeSwitch('normal')}
                  disabled={switchingMode !== null}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    shield.protectionMode === 'normal'
                      ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-white shadow-xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                  }`}
                >
                  Tiêu chuẩn (Normal)
                </button>
                <button
                  type="button"
                  onClick={() => handleModeSwitch('elevated')}
                  disabled={switchingMode !== null}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    shield.protectionMode === 'elevated'
                      ? 'bg-amber-600 text-white'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                  }`}
                >
                  Cảnh giác cao (SYNProxy)
                </button>
                <button
                  type="button"
                  onClick={() => handleModeSwitch('under_attack')}
                  disabled={switchingMode !== null}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                    shield.protectionMode === 'under_attack'
                      ? 'bg-rose-600 text-white'
                      : 'text-rose-600 dark:text-rose-400 hover:bg-rose-50 dark:hover:bg-rose-950/30'
                  }`}
                >
                  <Flame className="w-3.5 h-3.5" />
                  Chống DDoS Khẩn cấp (Lockdown)
                </button>
              </div>

              <button
                type="button"
                onClick={handleFimScanClick}
                disabled={scanningFim}
                className="px-3.5 py-2 text-xs font-semibold text-slate-800 dark:text-slate-100 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
              >
                <RefreshCw
                  className={`w-3.5 h-3.5 text-emerald-500 ${scanningFim ? 'animate-spin' : ''}`}
                />
                {scanningFim ? 'Đang quét SHA-256...' : 'Quét Toàn Vẹn Tệp (FIM)'}
              </button>
            </div>
          </div>

          {/* 4-Metric Anti-Hacking Telemetry Strip */}
          <div className="p-6 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 border-b border-slate-200 dark:border-slate-800 bg-slate-50/40 dark:bg-slate-950/30">
            <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 space-y-1">
              <div className="flex items-center justify-between text-xs text-slate-500">
                <span>Bộ lọc Kernel eBPF / XDP</span>
                <Cpu className="w-4 h-4 text-emerald-500" />
              </div>
              <div className="text-sm font-bold font-mono text-slate-900 dark:text-white truncate">
                {shield.xdpStatus}
              </div>
              <div className="text-[11px] font-mono tabular-nums text-slate-500 dark:text-slate-400">
                SYN Cookies: Bật · Ngưỡng {shield.synRateLimitPps.toLocaleString()} pkt/s
              </div>
            </div>

            <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 space-y-1">
              <div className="flex items-center justify-between text-xs text-slate-500">
                <span>Tấn công L7 WAF Đã Chặn</span>
                <Zap className="w-4 h-4 text-orange-500" />
              </div>
              <div className="text-lg font-bold font-mono tabular-nums text-slate-900 dark:text-white">
                {shield.wafRules.filter((r) => r.enabled).length}/{shield.wafRules.length} Luật Active
              </div>
              <div className="text-[11px] font-mono tabular-nums text-slate-500 dark:text-slate-400">
                Đã chặn {shield.blockedL7AttacksToday} request độc hại · {shield.droppedPacketsTotal} gói XDP
              </div>
            </div>

            <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 space-y-1 sm:col-span-2">
              <div className="flex items-center justify-between text-xs text-slate-500">
                <span>Kiểm toán Toàn vẹn Hệ thống (FIM SHA-256 &amp; Anti-Rootkit)</span>
                <FileCheck2 className="w-4 h-4 text-emerald-500" />
              </div>
              <div className="text-sm font-bold font-mono text-emerald-600 dark:text-emerald-400 truncate">
                {shield.fimIntegrityStatus}
              </div>
              <div className="text-[11px] font-mono text-slate-500 dark:text-slate-400">
                Giám sát: /etc/passwd · /etc/shadow · /etc/ssh/sshd_config · /etc/sudoers · Quét cuối: {shield.lastFimScanAt}
              </div>
            </div>
          </div>

          {/* Rust Pingora Layer-7 WAF Rules Matrix */}
          <div className="divide-y divide-slate-200 dark:divide-slate-800">
            {shield.wafRules.map((rule) => (
              <div
                key={rule.id}
                className="p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4 hover:bg-slate-50/70 dark:hover:bg-slate-800/30 transition-colors"
              >
                <div className="space-y-1 min-w-0">
                  <div className="flex flex-wrap items-center gap-2 text-xs font-mono text-slate-500 dark:text-slate-400">
                    <span className="font-bold text-orange-600 dark:text-orange-400">
                      {rule.code}
                    </span>
                    <span>·</span>
                    <span>{rule.category}</span>
                    <span>·</span>
                    <span className="text-slate-700 dark:text-slate-300">
                      Mẫu nhận diện: {rule.threatType}
                    </span>
                  </div>
                  <div className="text-sm font-bold text-slate-900 dark:text-slate-100">
                    {rule.name}
                  </div>
                  <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                    {rule.description}
                  </p>
                </div>

                <div className="flex items-center gap-3 shrink-0">
                  <button
                    type="button"
                    onClick={() => onToggleWafRule && onToggleWafRule(rule.id)}
                    className={`px-3.5 py-1.5 text-xs font-semibold rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                      rule.enabled
                        ? 'bg-emerald-600 text-white'
                        : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                    }`}
                  >
                    {rule.enabled ? (
                      <>
                        <CheckCircle2 className="w-3.5 h-3.5" />
                        Đang bảo vệ
                      </>
                    ) : (
                      <>
                        <Lock className="w-3.5 h-3.5" />
                        Bật luật WAF
                      </>
                    )}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Section 1: Ubuntu UFW Zero-Trust Firewall */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
        <div className="p-6 border-b border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="text-xs font-mono text-emerald-600 dark:text-emerald-400">
              ufw status numbered · Default: deny (incoming), allow (outgoing)
            </div>
            <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
              Tường lửa Ubuntu UFW (Uncomplicated Firewall)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Đảm bảo mọi cổng nội bộ của Actix-Web (8080–8099) bị chặn truy cập trực tiếp từ bên ngoài, chỉ cho phép lưu lượng qua Nginx (80/443)
            </p>
          </div>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-medium text-slate-500 dark:text-slate-400 bg-slate-50/60 dark:bg-slate-950/40">
                <th className="py-3 px-5">Cổng / Dải cổng</th>
                <th className="py-3 px-4">Giao thức</th>
                <th className="py-3 px-4">Chính sách</th>
                <th className="py-3 px-4">Nguồn (CIDR)</th>
                <th className="py-3 px-4">Mô tả quy tắc</th>
                <th className="py-3 px-5 text-right">Trạng thái / Xóa</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-sm">
              {security.ufwRules.map((rule) => (
                <tr
                  key={rule.id}
                  className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40 transition-colors"
                >
                  <td className="py-3 px-5 font-mono text-xs tabular-nums font-semibold text-slate-900 dark:text-slate-100">
                    {rule.port}
                  </td>
                  <td className="py-3 px-4 font-mono text-xs text-slate-600 dark:text-slate-400">
                    {rule.protocol}
                  </td>
                  <td className="py-3 px-4 font-mono text-xs font-semibold">
                    {rule.action === 'ALLOW' && (
                      <span className="text-emerald-600 dark:text-emerald-400">
                        ALLOW IN
                      </span>
                    )}
                    {rule.action === 'LIMIT' && (
                      <span className="text-amber-600 dark:text-amber-400">
                        LIMIT IN
                      </span>
                    )}
                    {rule.action === 'DENY' && (
                      <span className="text-rose-600 dark:text-rose-400">
                        DENY IN
                      </span>
                    )}
                  </td>
                  <td className="py-3 px-4 font-mono text-xs tabular-nums text-slate-600 dark:text-slate-300">
                    {rule.source}
                  </td>
                  <td className="py-3 px-4 text-xs text-slate-600 dark:text-slate-300">
                    {rule.comment}
                  </td>
                  <td className="py-3 px-5 text-right">
                    <div className="inline-flex items-center gap-2">
                      <button
                        type="button"
                        onClick={() => onToggleUfwRule(rule.id)}
                        className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                          rule.enabled
                            ? 'bg-emerald-50 dark:bg-emerald-950/50 text-emerald-700 dark:text-emerald-300'
                            : 'bg-slate-100 dark:bg-slate-800 text-slate-500'
                        }`}
                      >
                        {rule.enabled ? 'Đang bật' : 'Tạm tắt'}
                      </button>
                      <button
                        type="button"
                        onClick={() => onDeleteUfwRule(rule.id)}
                        className="p-1.5 text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors cursor-pointer"
                        title="Xóa quy tắc UFW"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* Add UFW Rule Form */}
        <form
          onSubmit={handleCreateRule}
          className="p-5 bg-slate-50/60 dark:bg-slate-950/40 border-t border-slate-200 dark:border-slate-800 grid grid-cols-1 sm:grid-cols-12 gap-3 items-center"
        >
          <input
            type="text"
            placeholder="Cổng (VD: 8443)"
            value={port}
            onChange={(e) => setPort(e.target.value)}
            className="sm:col-span-2 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
          />
          <select
            value={protocol}
            onChange={(e) =>
              setProtocol(e.target.value as 'TCP' | 'UDP' | 'BOTH')
            }
            className="sm:col-span-2 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
          >
            <option value="TCP">TCP</option>
            <option value="UDP">UDP</option>
            <option value="BOTH">TCP + UDP</option>
          </select>
          <select
            value={action}
            onChange={(e) =>
              setAction(e.target.value as 'ALLOW' | 'DENY' | 'LIMIT')
            }
            className="sm:col-span-2 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
          >
            <option value="ALLOW">ALLOW</option>
            <option value="LIMIT">LIMIT</option>
            <option value="DENY">DENY</option>
          </select>
          <input
            type="text"
            placeholder="CIDR (VD: 0.0.0.0/0)"
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="sm:col-span-2 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
          />
          <input
            type="text"
            placeholder="Ghi chú quy tắc..."
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            className="sm:col-span-2 px-3 py-2 text-xs rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
          />
          <button
            type="submit"
            className="sm:col-span-2 px-3 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center justify-center gap-1.5 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Thêm UFW Rule
          </button>
        </form>
      </section>

      {/* Section 2: Two-Column Split for Fail2ban Jails & Kernel/SSHD Hardening */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Left: Fail2ban Intrusion Prevention & IP Ban List */}
        <section className="lg:col-span-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          <div className="p-6 border-b border-slate-200 dark:border-slate-800 space-y-1">
            <div className="flex items-center justify-between">
              <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                Hệ thống Chống Xâm nhập Fail2ban
              </h2>
              <ShieldAlert className="w-4 h-4 text-amber-600 dark:text-amber-400" />
            </div>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Tự động khóa IP tấn công Brute-force SSH, vượt ngưỡng Rate Limit Nginx hoặc giả mạo JWT Actix-Web
            </p>
          </div>

          <div className="divide-y divide-slate-200 dark:divide-slate-800">
            {security.fail2banJails.map((jail) => (
              <div key={jail.name} className="p-5 space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <span className="font-mono text-sm font-semibold text-slate-900 dark:text-slate-100">
                      [{jail.name}]
                    </span>
                    <span className="text-xs text-slate-500 dark:text-slate-400 font-mono ml-2">
                      {jail.logPath}
                    </span>
                  </div>
                  <div className="text-xs font-mono tabular-nums text-slate-600 dark:text-slate-300">
                    Đang chặn: {jail.currentlyBanned} · Tổng: {jail.totalBanned}
                  </div>
                </div>

                <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                  maxretry = {jail.maxRetry} · findtime = {jail.findTimeSec}s · bantime = {jail.banTimeSec}s
                </div>

                {jail.bannedIps.length > 0 && (
                  <div className="space-y-2 pt-1">
                    {jail.bannedIps.map((item) => (
                      <div
                        key={item.ip}
                        className="px-3 py-2 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200/80 dark:border-slate-800 flex items-center justify-between gap-3 text-xs"
                      >
                        <div className="min-w-0">
                          <div className="font-mono font-semibold text-rose-600 dark:text-rose-400 tabular-nums">
                            {item.ip} · {item.country} · {item.bannedAt}
                          </div>
                          <div className="text-slate-500 dark:text-slate-400 truncate">
                            {item.reason}
                          </div>
                        </div>
                        <button
                          type="button"
                          onClick={() => onUnbanIp(jail.name, item.ip)}
                          className="px-2.5 py-1 text-xs font-medium text-slate-700 dark:text-slate-200 bg-white dark:bg-slate-800 hover:bg-slate-100 dark:hover:bg-slate-700 border border-slate-200 dark:border-slate-700 rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer"
                        >
                          <Unlock className="w-3 h-3" />
                          Gỡ chặn
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>

          {/* Manual IP Ban Form */}
          <form
            onSubmit={handleManualBan}
            className="p-5 bg-slate-50/60 dark:bg-slate-950/40 border-t border-slate-200 dark:border-slate-800 space-y-3"
          >
            <div className="text-xs font-semibold text-slate-700 dark:text-slate-300">
              Chặn IP thủ công (fail2ban-client set &lt;jail&gt; banip &lt;IP&gt;)
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
              <select
                value={banJail}
                onChange={(e) => setBanJail(e.target.value)}
                className="px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
              >
                {security.fail2banJails.map((j) => (
                  <option key={j.name} value={j.name}>
                    {j.name}
                  </option>
                ))}
              </select>
              <input
                type="text"
                placeholder="Địa chỉ IP (VD: 203.0.113.55)"
                value={banIpInput}
                onChange={(e) => setBanIpInput(e.target.value)}
                className="px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
              />
              <button
                type="submit"
                className="px-3 py-2 text-xs font-medium text-white bg-rose-600 hover:bg-rose-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Ban className="w-3.5 h-3.5" />
                Khóa IP ngay
              </button>
            </div>
          </form>
        </section>

        {/* Right: Ubuntu Kernel, SSHD & Actix-Web Hardening Matrix */}
        <section className="lg:col-span-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          <div className="p-6 border-b border-slate-200 dark:border-slate-800 space-y-1">
            <div className="flex items-center justify-between">
              <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                Kiểm toán Gia cố Nhân Ubuntu (sysctl, SSHD &amp; Systemd)
              </h2>
              <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
            </div>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Các chính sách bảo mật cấp hệ điều hành bảo vệ tiến trình Rust Actix-Web và dữ liệu máy chủ
            </p>
          </div>

          <div className="divide-y divide-slate-200 dark:divide-slate-800">
            {security.hardeningDirectives.map((directive) => (
              <div
                key={directive.id}
                className="p-5 flex items-start justify-between gap-4"
              >
                <div className="space-y-1">
                  <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
                    {directive.category} · Khuyến nghị: {directive.recommendedValue}
                  </div>
                  <div className="text-sm font-semibold font-mono text-slate-900 dark:text-slate-100">
                    {directive.key}
                  </div>
                  <p className="text-xs text-slate-600 dark:text-slate-400 leading-relaxed">
                    {directive.description}
                  </p>
                </div>

                <button
                  type="button"
                  onClick={() => onToggleHardening(directive.id)}
                  className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                    directive.enabled
                      ? 'bg-emerald-600 text-white'
                      : 'bg-slate-200 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {directive.enabled ? (
                    <>
                      <CheckCircle2 className="w-3.5 h-3.5" />
                      Đã khóa chặt
                    </>
                  ) : (
                    <>
                      <Lock className="w-3.5 h-3.5" />
                      Bật ngay
                    </>
                  )}
                </button>
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
};
