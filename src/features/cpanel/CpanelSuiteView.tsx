import React, { useState } from 'react';
import {
  Archive,
  CheckCircle2,
  Clock,
  Globe,
  KeyRound,
  Pause,
  Play,
  Plus,
  RotateCcw,
  ShieldCheck,
  Trash2
} from 'lucide-react';
import { CpanelData, ManagedApp, SqlDatabase } from '../types';

interface CpanelSuiteViewProps {
  cpanel: CpanelData | null;
  apps: ManagedApp[];
  databases: SqlDatabase[];
  onAddCron: (payload: {
    title: string;
    schedule: string;
    command: string;
    user: string;
    targetApp: string;
  }) => Promise<void>;
  onRunCronNow: (id: string) => Promise<void>;
  onToggleCron: (id: string) => Promise<void>;
  onDeleteCron: (id: string) => Promise<void>;
  onAddDns: (payload: {
    domain: string;
    type: 'A' | 'AAAA' | 'CNAME' | 'MX' | 'TXT' | 'CAA';
    host: string;
    value: string;
    ttl: number;
  }) => Promise<void>;
  onDeleteDns: (id: string) => Promise<void>;
  onCreateBackup: (payload: {
    targetApp: string;
    databaseIncluded: string;
  }) => Promise<void>;
  onRestoreBackup: (id: string) => Promise<void>;
  onDeleteBackup: (id: string) => Promise<void>;
  onAddSftp: (payload: {
    username: string;
    targetApp: string;
    authMethod: 'Ed25519 SSH Key' | 'Argon2id Password';
    quotaMb: number;
  }) => Promise<void>;
  onDeleteSftp: (id: string) => Promise<void>;
}

export const CpanelSuiteView: React.FC<CpanelSuiteViewProps> = ({
  cpanel,
  apps,
  databases,
  onAddCron,
  onRunCronNow,
  onToggleCron,
  onDeleteCron,
  onAddDns,
  onDeleteDns,
  onCreateBackup,
  onRestoreBackup,
  onDeleteBackup,
  onAddSftp,
  onDeleteSftp
}) => {
  const [subTab, setSubTab] = useState<'cron' | 'dns' | 'backups' | 'sftp'>(
    'cron'
  );
  const [bannerMsg, setBannerMsg] = useState<string | null>(null);

  // Cron state
  const [cronTitle, setCronTitle] = useState('');
  const [cronSchedule, setCronSchedule] = useState('*/15 * * * *');
  const [cronCommand, setCronCommand] = useState(
    '/usr/bin/curl -fsS http://127.0.0.1:8081/healthz'
  );
  const [cronUser, setCronUser] = useState('www-data');
  const [cronTargetApp, setCronTargetApp] = useState(
    apps[0]?.domain || 'api.ferroushub.vn'
  );

  // DNS state
  const [dnsDomain, setDnsDomain] = useState('ferroushub.vn');
  const [dnsType, setDnsType] = useState<
    'A' | 'AAAA' | 'CNAME' | 'MX' | 'TXT' | 'CAA'
  >('A');
  const liveIp =
    typeof window !== 'undefined' &&
    window.location.hostname &&
    window.location.hostname !== 'localhost' &&
    window.location.hostname !== '127.0.0.1'
      ? window.location.hostname
      : '127.0.0.1';
  const [dnsHost, setDnsHost] = useState('edge.ferroushub.vn');
  const [dnsValue, setDnsValue] = useState(liveIp);
  const [dnsTtl, setDnsTtl] = useState(300);

  // Backup state
  const [backupApp, setBackupApp] = useState(
    apps[0]?.domain || 'api.ferroushub.vn'
  );
  const [backupDb, setBackupDb] = useState(
    databases[0]
      ? `${databases[0].engine} · ${databases[0].name}`
      : 'PostgreSQL 17 · commerce_prod'
  );
  const [creatingBackup, setCreatingBackup] = useState(false);

  // SFTP state
  const [sftpUsername, setSftpUsername] = useState('');
  const [sftpApp, setSftpApp] = useState(
    apps[0]?.domain || 'api.ferroushub.vn'
  );
  const [sftpAuth, setSftpAuth] = useState<
    'Ed25519 SSH Key' | 'Argon2id Password'
  >('Ed25519 SSH Key');
  const [sftpQuota, setSftpQuota] = useState(5120);

  if (!cpanel) return null;

  const showToast = (msg: string) => {
    setBannerMsg(msg);
    setTimeout(() => setBannerMsg(null), 4000);
  };

  const handleSubmitCron = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!cronTitle.trim() || !cronCommand.trim()) return;
    await onAddCron({
      title: cronTitle.trim(),
      schedule: cronSchedule.trim(),
      command: cronCommand.trim(),
      user: cronUser,
      targetApp: cronTargetApp
    });
    setCronTitle('');
    showToast('Đã thêm tác vụ Cron Job mới vào /etc/cron.d/ferrous-panel');
  };

  const handleSubmitDns = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!dnsHost.trim() || !dnsValue.trim()) return;
    await onAddDns({
      domain: dnsDomain,
      type: dnsType,
      host: dnsHost.trim(),
      value: dnsValue.trim(),
      ttl: dnsTtl
    });
    setDnsHost('');
    showToast(
      `FerrousDNS (Hickory Rust): Đã thêm bản ghi ${dnsType} cho ${dnsHost.trim()}`
    );
  };

  const handleTriggerBackup = async (e: React.FormEvent) => {
    e.preventDefault();
    setCreatingBackup(true);
    try {
      await onCreateBackup({
        targetApp: backupApp,
        databaseIncluded: backupDb
      });
      showToast(
        `Đã tạo bản sao lưu nén Zstandard (.tar.zst) cho ${backupApp}`
      );
    } finally {
      setCreatingBackup(false);
    }
  };

  const handleSubmitSftp = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!sftpUsername.trim()) return;
    await onAddSftp({
      username: sftpUsername.trim(),
      targetApp: sftpApp,
      authMethod: sftpAuth,
      quotaMb: sftpQuota
    });
    setSftpUsername('');
    showToast(
      `FerrousSFTP (russh): Đã tạo tài khoản SFTP Chroot cho ${sftpApp}`
    );
  };

  return (
    <div className="space-y-6">
      {/* Header & Sub-navigation */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
        <div className="space-y-1">
          <h1 className="text-lg font-bold tracking-tight text-slate-900 dark:text-slate-100 font-display">
            Tiện ích Quản trị Web Hosting (cPanel Suite · Rust Powered)
          </h1>
          <p className="text-xs text-slate-500 dark:text-slate-400">
            Quản lý lịch tác vụ Ubuntu Cron, DNS Zone (FerrousDNS Hickory),
            Backup Zstandard và tài khoản SFTP Chroot (FerrousSFTP russh).
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={() => setSubTab('cron')}
            className={`px-3.5 py-2 text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'cron'
                ? 'bg-orange-600 text-white'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
            }`}
          >
            <Clock className="w-3.5 h-3.5" />
            Cron Jobs ({cpanel.cronJobs.length})
          </button>
          <button
            onClick={() => setSubTab('dns')}
            className={`px-3.5 py-2 text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'dns'
                ? 'bg-orange-600 text-white'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
            }`}
          >
            <Globe className="w-3.5 h-3.5" />
            DNS Zone Editor ({cpanel.dnsRecords.length})
          </button>
          <button
            onClick={() => setSubTab('backups')}
            className={`px-3.5 py-2 text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'backups'
                ? 'bg-orange-600 text-white'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
            }`}
          >
            <Archive className="w-3.5 h-3.5" />
            Zstd Backups ({cpanel.backups.length})
          </button>
          <button
            onClick={() => setSubTab('sftp')}
            className={`px-3.5 py-2 text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'sftp'
                ? 'bg-orange-600 text-white'
                : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
            }`}
          >
            <KeyRound className="w-3.5 h-3.5" />
            SFTP Chroot ({cpanel.sftpAccounts.length})
          </button>
        </div>
      </div>

      {bannerMsg && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl px-4 py-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-xs font-medium text-emerald-700 dark:text-emerald-300">
            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            <span>{bannerMsg}</span>
          </div>
          <button
            onClick={() => setBannerMsg(null)}
            className="text-xs text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 cursor-pointer"
          >
            Đóng
          </button>
        </div>
      )}

      {/* 1. CRON JOBS */}
      {subTab === 'cron' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 mb-4 font-display">
              Thêm Tác vụ Định kỳ (/etc/cron.d)
            </h2>
            <form onSubmit={handleSubmitCron} className="space-y-3.5">
              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Mô tả tác vụ
                </label>
                <input
                  type="text"
                  required
                  value={cronTitle}
                  onChange={(e) => setCronTitle(e.target.value)}
                  placeholder="vd: Dọn dẹp phiên JWT hết hạn"
                  className="w-full px-3 py-2 text-xs rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1">
                  <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                    Biểu thức Cron
                  </label>
                  <input
                    type="text"
                    required
                    value={cronSchedule}
                    onChange={(e) => setCronSchedule(e.target.value)}
                    placeholder="*/15 * * * *"
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  />
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                    Linux User
                  </label>
                  <select
                    value={cronUser}
                    onChange={(e) => setCronUser(e.target.value)}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  >
                    <option value="www-data">www-data</option>
                    <option value="postgres">postgres</option>
                    <option value="root">root</option>
                  </select>
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Website áp dụng
                </label>
                <select
                  value={cronTargetApp}
                  onChange={(e) => setCronTargetApp(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  {apps.map((a) => (
                    <option key={a.id} value={a.domain}>
                      {a.domain}
                    </option>
                  ))}
                </select>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Lệnh thực thi (Command)
                </label>
                <input
                  type="text"
                  required
                  value={cronCommand}
                  onChange={(e) => setCronCommand(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <button
                type="submit"
                className="w-full px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                Lưu Cron Job
              </button>
            </form>
          </div>

          <div className="lg:col-span-8 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 space-y-4">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 font-display">
              Danh sách Cron Jobs đang hoạt động
            </h2>
            <div className="space-y-3">
              {cpanel.cronJobs.length === 0 ? (
                <div className="p-8 text-center text-slate-500 dark:text-slate-400">
                  <Clock className="w-8 h-8 mx-auto mb-2 opacity-40 text-slate-400" />
                  <div className="text-xs font-semibold">Chưa có Cron Job nào</div>
                  <div className="text-[11px] mt-1 text-slate-400">
                    Tạo tác vụ định kỳ tự động ở biểu mẫu bên trái.
                  </div>
                </div>
              ) :
                cpanel.cronJobs.map((job) => (
                <div
                  key={job.id}
                  className="border border-slate-200 dark:border-slate-800 rounded-lg p-4 space-y-2.5"
                >
                  <div className="flex flex-wrap items-start justify-between gap-2">
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                          {job.title}
                        </span>
                        <span
                          className={`text-xs font-mono ${
                            job.enabled
                              ? 'text-emerald-600 dark:text-emerald-400'
                              : 'text-amber-600 dark:text-amber-400'
                          }`}
                        >
                          · {job.enabled ? 'Đang bật' : 'Tạm dừng'}
                        </span>
                      </div>
                      <div className="text-xs font-mono text-slate-500 mt-0.5">
                        Lịch:{' '}
                        <span className="text-orange-600 dark:text-orange-400 font-semibold">
                          {job.schedule}
                        </span>{' '}
                        · User: {job.user} · Site: {job.targetApp}
                      </div>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <button
                        onClick={async () => {
                          await onToggleCron(job.id);
                          showToast(
                            `Đã ${job.enabled ? 'tạm dừng' : 'kích hoạt lại'} Cron Job: ${job.title}`
                          );
                        }}
                        className="px-2.5 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg inline-flex items-center gap-1 cursor-pointer"
                      >
                        {job.enabled ? (
                          <>
                            <Pause className="w-3 h-3" /> Tạm dừng
                          </>
                        ) : (
                          <>
                            <Play className="w-3 h-3" /> Bật lại
                          </>
                        )}
                      </button>
                      <button
                        onClick={async () => {
                          await onRunCronNow(job.id);
                          showToast(`Đã chạy tức thì Cron Job: ${job.title}`);
                        }}
                        className="px-2.5 py-1.5 text-xs font-medium text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 rounded-lg inline-flex items-center gap-1 cursor-pointer"
                      >
                        <Play className="w-3 h-3" /> Chạy ngay
                      </button>
                      <button
                        onClick={() => onDeleteCron(job.id)}
                        className="p-1.5 text-slate-400 hover:text-rose-500 rounded-lg cursor-pointer"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>

                  <div className="p-2.5 rounded bg-slate-950 text-slate-200 font-mono text-xs overflow-x-auto">
                    $ {job.command}
                  </div>

                  <div className="flex items-center justify-between text-xs font-mono text-slate-500">
                    <span>Lần chạy cuối: {job.lastRunAt}</span>
                    <span className="text-emerald-600 dark:text-emerald-400">
                      Exit Code: {job.lastExitCode} · {job.lastOutput}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* 2. DNS ZONE EDITOR */}
      {subTab === 'dns' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 mb-4 font-display">
              Thêm Bản ghi DNS (FerrousDNS Hickory)
            </h2>
            <form onSubmit={handleSubmitDns} className="space-y-3.5">
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1">
                  <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                    Loại bản ghi
                  </label>
                  <select
                    value={dnsType}
                    onChange={(e) =>
                      setDnsType(e.target.value as typeof dnsType)
                    }
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  >
                    <option value="A">A (IPv4)</option>
                    <option value="AAAA">AAAA (IPv6)</option>
                    <option value="CNAME">CNAME</option>
                    <option value="MX">MX (Mail)</option>
                    <option value="TXT">TXT (SPF/DKIM)</option>
                    <option value="CAA">CAA (SSL Lock)</option>
                  </select>
                </div>
                <div className="space-y-1">
                  <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                    TTL (Giây)
                  </label>
                  <input
                    type="number"
                    value={dnsTtl}
                    onChange={(e) => setDnsTtl(Number(e.target.value))}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  />
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Tên miền gốc (Zone)
                </label>
                <input
                  type="text"
                  value={dnsDomain}
                  onChange={(e) => setDnsDomain(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Host / Subdomain
                </label>
                <input
                  type="text"
                  required
                  value={dnsHost}
                  onChange={(e) => setDnsHost(e.target.value)}
                  placeholder="api.ferroushub.vn"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Giá trị trỏ tới (Value / Target)
                </label>
                <input
                  type="text"
                  required
                  value={dnsValue}
                  onChange={(e) => setDnsValue(e.target.value)}
                  placeholder={liveIp}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <button
                type="submit"
                className="w-full px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                Thêm Bản ghi DNS
              </button>
            </form>
          </div>

          <div className="lg:col-span-8 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 mb-4 font-display">
              Bảng Phân giải Tên miền (DNS Zone Records)
            </h2>
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-mono uppercase text-slate-500">
                    <th className="py-2.5 px-3">Loại</th>
                    <th className="py-2.5 px-3">Host</th>
                    <th className="py-2.5 px-3">Giá trị (Value)</th>
                    <th className="py-2.5 px-3">TTL</th>
                    <th className="py-2.5 px-3 text-right">Xóa</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs font-mono">
                  {cpanel.dnsRecords.length === 0 ? (
                    <tr>
                      <td
                        colSpan={5}
                        className="py-8 text-center text-slate-500 dark:text-slate-400 font-sans text-xs"
                      >
                        Chưa có bản ghi DNS nào được cấu hình.
                      </td>
                    </tr>
                  ) :
                    cpanel.dnsRecords.map((rec) => (
                    <tr key={rec.id}>
                      <td className="py-2.5 px-3 font-bold text-orange-600 dark:text-orange-400">
                        {rec.type}
                      </td>
                      <td className="py-2.5 px-3 text-slate-900 dark:text-slate-100">
                        {rec.host}
                      </td>
                      <td className="py-2.5 px-3 text-slate-600 dark:text-slate-300 break-all">
                        {rec.value}
                      </td>
                      <td className="py-2.5 px-3 text-slate-500">{rec.ttl}s</td>
                      <td className="py-2.5 px-3 text-right">
                        <button
                          onClick={() => onDeleteDns(rec.id)}
                          className="p-1 text-slate-400 hover:text-rose-500 rounded cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* 3. ZSTD BACKUPS */}
      {subTab === 'backups' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 mb-4 font-display">
              Đóng gói Sao lưu Tức thì (.tar.zst)
            </h2>
            <form onSubmit={handleTriggerBackup} className="space-y-3.5">
              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Chọn Website cần sao lưu
                </label>
                <select
                  value={backupApp}
                  onChange={(e) => setBackupApp(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  {apps.map((a) => (
                    <option key={a.id} value={a.domain}>
                      {a.domain} ({a.workingDir})
                    </option>
                  ))}
                </select>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Đính kèm SQL Database Dump
                </label>
                <select
                  value={backupDb}
                  onChange={(e) => setBackupDb(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  {databases.map((db) => (
                    <option key={db.id} value={`${db.engine} · ${db.name}`}>
                      {db.engine} · {db.name}
                    </option>
                  ))}
                  <option value="Không đính kèm Database">
                    Chỉ sao lưu mã nguồn Web Root
                  </option>
                </select>
              </div>

              <button
                type="submit"
                disabled={creatingBackup}
                className="w-full px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer disabled:opacity-50"
              >
                <Archive className="w-3.5 h-3.5" />
                {creatingBackup
                  ? 'Đang nén Zstd Level 9...'
                  : 'Tạo Bản sao lưu ngay'}
              </button>
            </form>
          </div>

          <div className="lg:col-span-8 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 space-y-4">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 font-display">
              Kho Lưu trữ Bản sao lưu (/var/backups/ferrous-panel)
            </h2>
            <div className="space-y-3">
              {cpanel.backups.length === 0 ? (
                <div className="p-8 text-center text-slate-500 dark:text-slate-400">
                  <ShieldCheck className="w-8 h-8 mx-auto mb-2 opacity-40 text-slate-400" />
                  <div className="text-xs font-semibold">Chưa có bản sao lưu nào</div>
                  <div className="text-[11px] mt-1 text-slate-400">
                    Tạo bản sao lưu nén Zstd đầu tiên ở biểu mẫu bên trái.
                  </div>
                </div>
              ) :
                cpanel.backups.map((bk) => (
                <div
                  key={bk.id}
                  className="border border-slate-200 dark:border-slate-800 rounded-lg p-4 flex flex-wrap items-center justify-between gap-4"
                >
                  <div className="space-y-1">
                    <div className="flex items-center gap-2">
                      <ShieldCheck className="w-4 h-4 text-emerald-500 shrink-0" />
                      <span className="text-xs font-mono font-bold text-slate-900 dark:text-slate-100">
                        {bk.filename}
                      </span>
                    </div>
                    <div className="text-xs font-mono text-slate-500">
                      Site: {bk.targetApp} · DB: {bk.databaseIncluded} · Dung
                      lượng: {bk.sizeMb} MB ({bk.compression})
                    </div>
                    <div className="text-xs font-mono text-slate-400">
                      SHA-256: {bk.sha256.slice(0, 36)}... · Lúc: {bk.createdAt}
                    </div>
                  </div>

                  <div className="flex items-center gap-2">
                    <button
                      onClick={async () => {
                        await onRestoreBackup(bk.id);
                        showToast(
                          `Đã khôi phục bản sao lưu ${bk.filename} (${bk.sizeMb} MB) cho ${bk.targetApp}`
                        );
                      }}
                      className="px-3 py-1.5 text-xs font-medium text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                    >
                      <RotateCcw className="w-3.5 h-3.5" />
                      Khôi phục (Restore)
                    </button>
                    <button
                      onClick={() => onDeleteBackup(bk.id)}
                      className="p-2 text-slate-400 hover:text-rose-500 rounded-lg cursor-pointer"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* 4. SFTP CHROOT ACCOUNTS */}
      {subTab === 'sftp' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 mb-4 font-display">
              Tạo Tài khoản SFTP Chroot (FerrousSFTP russh)
            </h2>
            <form onSubmit={handleSubmitSftp} className="space-y-3.5">
              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Username SFTP
                </label>
                <input
                  type="text"
                  required
                  value={sftpUsername}
                  onChange={(e) => setSftpUsername(e.target.value)}
                  placeholder="deploy_partner"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Gắn vào Thư mục Website (Chroot Jail)
                </label>
                <select
                  value={sftpApp}
                  onChange={(e) => setSftpApp(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  {apps.map((a) => (
                    <option key={a.id} value={a.domain}>
                      {a.domain} ({a.workingDir})
                    </option>
                  ))}
                </select>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Phương thức Xác thực
                </label>
                <select
                  value={sftpAuth}
                  onChange={(e) =>
                    setSftpAuth(e.target.value as typeof sftpAuth)
                  }
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                >
                  <option value="Ed25519 SSH Key">Ed25519 SSH Key</option>
                  <option value="Argon2id Password">Argon2id Password</option>
                </select>
              </div>

              <div className="space-y-1">
                <label className="text-xs font-medium text-slate-600 dark:text-slate-400">
                  Hạn ngạch ổ đĩa (Quota MB)
                </label>
                <input
                  type="number"
                  value={sftpQuota}
                  onChange={(e) => setSftpQuota(Number(e.target.value))}
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                />
              </div>

              <button
                type="submit"
                className="w-full px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                Tạo SFTP Account
              </button>
            </form>
          </div>

          <div className="lg:col-span-8 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5">
            <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 mb-4 font-display">
              Danh sách Tài khoản SFTP cô lập theo Website
            </h2>
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-mono uppercase text-slate-500">
                    <th className="py-2.5 px-3">Username</th>
                    <th className="py-2.5 px-3">Thư mục Chroot</th>
                    <th className="py-2.5 px-3">Xác thực</th>
                    <th className="py-2.5 px-3">Dung lượng</th>
                    <th className="py-2.5 px-3 text-right">Xóa</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs font-mono">
                  {cpanel.sftpAccounts.length === 0 ? (
                    <tr>
                      <td
                        colSpan={5}
                        className="py-8 text-center text-slate-500 dark:text-slate-400 font-sans text-xs"
                      >
                        Chưa có tài khoản SFTP nào được tạo.
                      </td>
                    </tr>
                  ) :
                    cpanel.sftpAccounts.map((acc) => (
                    <tr key={acc.id}>
                      <td className="py-2.5 px-3 font-bold text-slate-900 dark:text-slate-100">
                        {acc.username}
                      </td>
                      <td className="py-2.5 px-3 text-orange-600 dark:text-orange-400">
                        {acc.chrootDir}
                      </td>
                      <td className="py-2.5 px-3 text-emerald-600 dark:text-emerald-400">
                        {acc.authMethod}
                      </td>
                      <td className="py-2.5 px-3 text-slate-500">
                        {acc.usedMb} / {acc.quotaMb} MB
                      </td>
                      <td className="py-2.5 px-3 text-right">
                        <button
                          onClick={() => onDeleteSftp(acc.id)}
                          className="p-1 text-slate-400 hover:text-rose-500 rounded cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
