import React, { useState, useEffect } from 'react';
import {
  CheckCircle2,
  Cpu,
  Download,
  FileCode2,
  Globe,
  Layers,
  Play,
  Plus,
  RotateCw,
  Save,
  Settings,
  ShieldAlert,
  Square,
  Trash2
} from 'lucide-react';
import { ManagedApp, PhpVersionInstance } from '../types';

interface PhpManagerViewProps {
  phpVersions: PhpVersionInstance[];
  apps: ManagedApp[];
  onPhpAction: (
    version: string,
    action: 'start' | 'stop' | 'restart' | 'reload' | 'install' | 'uninstall'
  ) => Promise<void>;
  onUpdatePhpSettings: (
    version: string,
    payload: Partial<PhpVersionInstance>
  ) => Promise<void>;
  onTogglePhpExtension: (version: string, extName: string) => Promise<void>;
  onSwitchSitePhpVersion: (appId: string, phpVersion: string) => Promise<void>;
}

export const PhpManagerView: React.FC<PhpManagerViewProps> = ({
  phpVersions,
  apps,
  onPhpAction,
  onUpdatePhpSettings,
  onTogglePhpExtension,
  onSwitchSitePhpVersion
}) => {
  const [selectedVer, setSelectedVer] = useState<string>('8.3');
  const [subTab, setSubTab] = useState<
    'tuning' | 'extensions' | 'disabled_funcs' | 'raw_ini' | 'multiphp_sites'
  >('tuning');
  const [busyState, setBusyState] = useState<string | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);

  const activePhp =
    phpVersions.find((p) => p.version === selectedVer) || phpVersions[0];

  // Editable tuning fields
  const [memoryLimit, setMemoryLimit] = useState('512M');
  const [uploadMaxFilesize, setUploadMaxFilesize] = useState('128M');
  const [postMaxSize, setPostMaxSize] = useState('128M');
  const [maxExecutionTime, setMaxExecutionTime] = useState(120);
  const [maxInputVars, setMaxInputVars] = useState(5000);
  const [maxChildren, setMaxChildren] = useState(80);
  const [displayErrors, setDisplayErrors] = useState(false);
  const [opcacheEnabled, setOpcacheEnabled] = useState(true);
  const [opcacheJitBuffer, setOpcacheJitBuffer] = useState('128M');
  const [rawPhpIni, setRawPhpIni] = useState('');
  const [rawFpmPoolConf, setRawFpmPoolConf] = useState('');
  const [newDisabledFunc, setNewDisabledFunc] = useState('');

  useEffect(() => {
    if (activePhp) {
      setMemoryLimit(activePhp.memoryLimit);
      setUploadMaxFilesize(activePhp.uploadMaxFilesize);
      setPostMaxSize(activePhp.postMaxSize);
      setMaxExecutionTime(activePhp.maxExecutionTime);
      setMaxInputVars(activePhp.maxInputVars);
      setMaxChildren(activePhp.maxChildren);
      setDisplayErrors(activePhp.displayErrors);
      setOpcacheEnabled(activePhp.opcacheEnabled);
      setOpcacheJitBuffer(activePhp.opcacheJitBuffer);
      setRawPhpIni(activePhp.rawPhpIni);
      setRawFpmPoolConf(activePhp.rawFpmPoolConf);
    }
  }, [activePhp]);

  if (!activePhp) return null;

  const notify = (msg: string) => {
    setToastMsg(msg);
    setTimeout(() => setToastMsg(null), 4000);
  };

  const handleActionClick = async (
    version: string,
    action: 'start' | 'stop' | 'restart' | 'reload' | 'install' | 'uninstall'
  ) => {
    setBusyState(`${version}-${action}`);
    try {
      await onPhpAction(version, action);
      notify(
        `PHP ${version}-FPM: Đã thực thi lệnh [${action.toUpperCase()}] thành công trên Socket /run/php/php${version}-fpm.sock`
      );
    } finally {
      setBusyState(null);
    }
  };

  const handleSaveTuning = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusyState('save-tuning');
    try {
      await onUpdatePhpSettings(activePhp.version, {
        memoryLimit,
        uploadMaxFilesize,
        postMaxSize,
        maxExecutionTime,
        maxInputVars,
        maxChildren,
        displayErrors,
        opcacheEnabled,
        opcacheJitBuffer
      });
      notify(
        `Đã lưu thông số ${activePhp.iniPath} & nạp lại tiến trình php${activePhp.version}-fpm.service`
      );
    } finally {
      setBusyState(null);
    }
  };

  const handleSaveRawFiles = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusyState('save-raw');
    try {
      await onUpdatePhpSettings(activePhp.version, {
        rawPhpIni,
        rawFpmPoolConf
      });
      notify(
        `Đã ghi trực tiếp ${activePhp.iniPath} & ${activePhp.poolPath} (php-fpm${activePhp.version} -t OK)`
      );
    } finally {
      setBusyState(null);
    }
  };

  const handleAddDisabledFunc = async (e: React.FormEvent) => {
    e.preventDefault();
    const fnName = newDisabledFunc.trim().toLowerCase();
    if (!fnName) return;
    if (!activePhp.disableFunctions.includes(fnName)) {
      const updated = [...activePhp.disableFunctions, fnName];
      await onUpdatePhpSettings(activePhp.version, {
        disableFunctions: updated
      });
      setNewDisabledFunc('');
      notify(`Đã thêm hàm '${fnName}' vào danh sách disable_functions`);
    }
  };

  const handleRemoveDisabledFunc = async (fnName: string) => {
    const updated = activePhp.disableFunctions.filter((f) => f !== fnName);
    await onUpdatePhpSettings(activePhp.version, {
      disableFunctions: updated
    });
    notify(`Đã gỡ hàm '${fnName}' khỏi danh sách disable_functions`);
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 flex flex-wrap items-center justify-between gap-4">
        <div className="space-y-1">
          <h1 className="text-lg font-bold tracking-tight text-slate-900 dark:text-slate-100 font-display">
            Quản trị Đa Phiên bản PHP (Multi-PHP FPM 8.4 / 8.3 / 8.2 / 8.1 /
            7.4) &amp; Tinh chỉnh php.ini
          </h1>
          <p className="text-xs text-slate-500 dark:text-slate-400">
            Điều phối đồng thời nhiều phiên bản PHP-FPM qua Unix Socket riêng
            biệt, bật/tắt Extensions (Redis, OPcache JIT, Swoole, Imagick), khóa
            hàm nguy hiểm và chuyển đổi phiên bản PHP cho từng Website.
          </p>
        </div>

        <div className="flex items-center gap-2 text-xs font-mono text-slate-500 dark:text-slate-400">
          <Cpu className="w-4 h-4 text-orange-500" />
          <span>Native Rust Module: src/php_manager.rs</span>
        </div>
      </div>

      {toastMsg && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl px-4 py-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-xs font-medium text-emerald-700 dark:text-emerald-300">
            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            <span>{toastMsg}</span>
          </div>
          <button
            onClick={() => setToastMsg(null)}
            className="text-xs text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 cursor-pointer"
          >
            Đóng
          </button>
        </div>
      )}

      {/* 5 PHP Version Cards Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3.5">
        {phpVersions.map((inst) => {
          const isSelected = inst.version === selectedVer;
          return (
            <div
              key={inst.version}
              onClick={() => setSelectedVer(inst.version)}
              className={`rounded-xl border p-4 transition-colors cursor-pointer flex flex-col justify-between gap-3 ${
                isSelected
                  ? 'border-orange-500 bg-orange-500/5 dark:bg-orange-500/10'
                  : 'border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 hover:border-slate-300 dark:hover:border-slate-700'
              }`}
            >
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-bold font-mono text-slate-900 dark:text-slate-100">
                    PHP {inst.version}-FPM
                  </span>
                  <span
                    className={`inline-flex items-center gap-1 text-xs font-mono ${
                      inst.status === 'running'
                        ? 'text-emerald-600 dark:text-emerald-400'
                        : inst.status === 'stopped'
                          ? 'text-amber-600 dark:text-amber-400'
                          : 'text-slate-400'
                    }`}
                  >
                    <span
                      className={`w-2 h-2 rounded-full ${
                        inst.status === 'running'
                          ? 'bg-emerald-500'
                          : inst.status === 'stopped'
                            ? 'bg-amber-500'
                            : 'bg-slate-400'
                      }`}
                    />
                    {inst.status === 'running'
                      ? 'Running'
                      : inst.status === 'stopped'
                        ? 'Stopped'
                        : 'Chưa cài'}
                  </span>
                </div>
                <div className="text-xs font-mono text-slate-500 truncate">
                  {inst.fpmSocket}
                </div>
                <div className="text-xs font-mono tabular-nums text-slate-600 dark:text-slate-400">
                  RAM: {inst.memoryUsageMb} MB · Workers: {inst.activeWorkers}/
                  {inst.maxChildren}
                </div>
              </div>

              <div
                className="flex items-center gap-1.5 pt-2 border-t border-slate-100 dark:border-slate-800"
                onClick={(e) => e.stopPropagation()}
              >
                {inst.status === 'not_installed' ? (
                  <button
                    onClick={() => handleActionClick(inst.version, 'install')}
                    className="w-full py-1 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded inline-flex items-center justify-center gap-1 cursor-pointer"
                  >
                    <Download className="w-3 h-3" /> Cài đặt
                  </button>
                ) : (
                  <>
                    {inst.status === 'running' ? (
                      <button
                        onClick={() => handleActionClick(inst.version, 'stop')}
                        className="flex-1 py-1 text-xs font-medium text-amber-600 dark:text-amber-400 bg-amber-500/10 hover:bg-amber-500/20 rounded inline-flex items-center justify-center gap-1 cursor-pointer"
                      >
                        <Square className="w-3 h-3" /> Dừng
                      </button>
                    ) : (
                      <button
                        onClick={() => handleActionClick(inst.version, 'start')}
                        className="flex-1 py-1 text-xs font-medium text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 hover:bg-emerald-500/20 rounded inline-flex items-center justify-center gap-1 cursor-pointer"
                      >
                        <Play className="w-3 h-3" /> Bật
                      </button>
                    )}
                    <button
                      onClick={() => handleActionClick(inst.version, 'reload')}
                      className="flex-1 py-1 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded inline-flex items-center justify-center gap-1 cursor-pointer"
                      title="Reload php-fpm Zero-Downtime"
                    >
                      <RotateCw
                        className={`w-3 h-3 ${
                          busyState === `${inst.version}-reload`
                            ? 'animate-spin'
                            : ''
                        }`}
                      />
                      Reload
                    </button>
                  </>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* Main Editor Card for Selected PHP Version */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl overflow-hidden">
        {/* Sub-navigation bar */}
        <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/50 flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <span className="text-sm font-bold text-slate-900 dark:text-slate-100 font-display">
              Đang cấu hình: {activePhp.fullVersion}
            </span>
          </div>

          <div className="flex flex-wrap items-center gap-1.5">
            {[
              {
                id: 'tuning',
                label: 'Tinh chỉnh Thông số (php.ini & Pool)',
                icon: Settings
              },
              {
                id: 'extensions',
                label: `PHP Extensions (${activePhp.extensions.filter((e) => e.enabled).length}/${activePhp.extensions.length})`,
                icon: Layers
              },
              {
                id: 'disabled_funcs',
                label: `Hàm Bị Cấm (${activePhp.disableFunctions.length})`,
                icon: ShieldAlert
              },
              {
                id: 'raw_ini',
                label: 'Sửa File php.ini & www.conf',
                icon: FileCode2
              },
              {
                id: 'multiphp_sites',
                label: 'Đổi Phiên bản PHP cho Website',
                icon: Globe
              }
            ].map((tab) => {
              const Icon = tab.icon;
              const active = subTab === tab.id;
              return (
                <button
                  key={tab.id}
                  onClick={() => setSubTab(tab.id as typeof subTab)}
                  className={`px-3 py-1.5 text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
                    active
                      ? 'bg-orange-600 text-white'
                      : 'bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-400 border border-slate-200 dark:border-slate-800 hover:text-slate-900 dark:hover:text-slate-200'
                  }`}
                >
                  <Icon className="w-3.5 h-3.5" />
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>

        <div className="p-6">
          {/* TAB 1: TUNING PHP.INI & FPM POOL */}
          {subTab === 'tuning' && (
            <form onSubmit={handleSaveTuning} className="space-y-5">
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                    Giới hạn Bộ nhớ mỗi Script (memory_limit)
                  </label>
                  <select
                    value={memoryLimit}
                    onChange={(e) => setMemoryLimit(e.target.value)}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  >
                    <option value="128M">128M (Light CMS)</option>
                    <option value="256M">256M (Standard WordPress)</option>
                    <option value="512M">
                      512M (Khuyên dùng cho WooCommerce / Laravel)
                    </option>
                    <option value="1024M">1024M (Heavy Import / ERP)</option>
                    <option value="2048M">2048M (Enterprise Batch)</option>
                  </select>
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                    Dung lượng Upload Tối đa (upload_max_filesize)
                  </label>
                  <select
                    value={uploadMaxFilesize}
                    onChange={(e) => {
                      setUploadMaxFilesize(e.target.value);
                      setPostMaxSize(e.target.value);
                    }}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  >
                    <option value="32M">32M</option>
                    <option value="64M">64M</option>
                    <option value="128M">128M</option>
                    <option value="256M">256M</option>
                    <option value="512M">512M</option>
                    <option value="1024M">1024M (1 GB)</option>
                  </select>
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                    Dung lượng POST Tối đa (post_max_size)
                  </label>
                  <select
                    value={postMaxSize}
                    onChange={(e) => setPostMaxSize(e.target.value)}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  >
                    <option value="32M">32M</option>
                    <option value="64M">64M</option>
                    <option value="128M">128M</option>
                    <option value="256M">256M</option>
                    <option value="512M">512M</option>
                    <option value="1024M">1024M (1 GB)</option>
                  </select>
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                    Thời gian chạy tối đa (max_execution_time - Giây)
                  </label>
                  <input
                    type="number"
                    min={15}
                    max={3600}
                    value={maxExecutionTime}
                    onChange={(e) =>
                      setMaxExecutionTime(Number(e.target.value))
                    }
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                    Số biến Input tối đa (max_input_vars)
                  </label>
                  <input
                    type="number"
                    min={1000}
                    max={50000}
                    step={500}
                    value={maxInputVars}
                    onChange={(e) => setMaxInputVars(Number(e.target.value))}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  />
                </div>

                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                    FPM Max Children Workers (pm.max_children)
                  </label>
                  <input
                    type="number"
                    min={5}
                    max={500}
                    value={maxChildren}
                    onChange={(e) => setMaxChildren(Number(e.target.value))}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                  />
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-4 pt-2">
                <label className="flex items-center justify-between p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 cursor-pointer">
                  <div>
                    <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                      Bật Zend OPcache &amp; JIT Compiler
                    </div>
                    <div className="text-xs text-slate-500">
                      Lưu bytecode vào RAM, tăng tốc độ phản hồi PHP gấp 3 lần
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={opcacheEnabled}
                    onChange={(e) => setOpcacheEnabled(e.target.checked)}
                    className="rounded text-orange-600"
                  />
                </label>

                <div className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 flex items-center justify-between gap-2">
                  <div>
                    <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                      OPcache JIT Buffer Size
                    </div>
                    <div className="text-xs text-slate-500">
                      Bộ nhớ đệm biên dịch JIT (opcache.jit=tracing)
                    </div>
                  </div>
                  <select
                    value={opcacheJitBuffer}
                    onChange={(e) => setOpcacheJitBuffer(e.target.value)}
                    className="px-2.5 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                  >
                    <option value="0M">0M (Tắt JIT)</option>
                    <option value="64M">64M</option>
                    <option value="128M">128M</option>
                    <option value="256M">256M</option>
                  </select>
                </div>

                <label className="flex items-center justify-between p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 cursor-pointer">
                  <div>
                    <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                      Hiển thị lỗi ra trình duyệt (display_errors)
                    </div>
                    <div className="text-xs text-slate-500">
                      Chỉ bật khi Debug, nên tắt trên môi trường Production
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={displayErrors}
                    onChange={(e) => setDisplayErrors(e.target.checked)}
                    className="rounded text-orange-600"
                  />
                </label>
              </div>

              <div className="flex items-center justify-between pt-3 border-t border-slate-200 dark:border-slate-800">
                <div className="text-xs font-mono text-slate-500">
                  Tệp đích: {activePhp.iniPath} &amp; {activePhp.poolPath}
                </div>
                <button
                  type="submit"
                  disabled={busyState === 'save-tuning'}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                >
                  <Save className="w-3.5 h-3.5" />
                  {busyState === 'save-tuning'
                    ? 'Đang ghi php.ini...'
                    : `Lưu Thông số & Reload PHP ${activePhp.version}-FPM`}
                </button>
              </div>
            </form>
          )}

          {/* TAB 2: PHP EXTENSIONS MANAGER */}
          {subTab === 'extensions' && (
            <div className="space-y-4">
              <div className="text-xs text-slate-500">
                Bật hoặc tắt trực tiếp các tiện ích mở rộng (Extensions) cho{' '}
                <strong className="text-slate-900 dark:text-slate-100">
                  PHP {activePhp.version}-FPM
                </strong>
                . Hệ thống tự động thực thi{' '}
                <code className="font-mono text-orange-500">
                  phpenmod -v {activePhp.version}
                </code>{' '}
                hoặc{' '}
                <code className="font-mono text-orange-500">
                  phpdismod -v {activePhp.version}
                </code>{' '}
                và nạp lại FPM worker pool.
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
                {activePhp.extensions.map((ext) => (
                  <div
                    key={ext.name}
                    className="p-3.5 rounded-xl border border-slate-200 dark:border-slate-800 flex items-start justify-between gap-3"
                  >
                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="text-xs font-mono font-bold text-slate-900 dark:text-slate-100">
                          {ext.name}.so
                        </span>
                        <span className="text-xs font-mono text-orange-600 dark:text-orange-400">
                          {ext.category}
                        </span>
                      </div>
                      <p className="text-xs text-slate-500 leading-relaxed">
                        {ext.description}
                      </p>
                    </div>

                    <button
                      onClick={async () => {
                        await onTogglePhpExtension(activePhp.version, ext.name);
                        notify(
                          `PHP ${activePhp.version}: Đã ${ext.enabled ? 'tắt' : 'kích hoạt'} extension [${ext.name}.so]`
                        );
                      }}
                      className={`px-3 py-1.5 text-xs font-mono font-medium rounded-lg shrink-0 cursor-pointer transition-colors ${
                        ext.enabled
                          ? 'bg-emerald-600 text-white hover:bg-emerald-500'
                          : 'bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-200'
                      }`}
                    >
                      {ext.enabled ? 'Đang Bật' : 'Đang Tắt'}
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* TAB 3: DISABLED FUNCTIONS GUARD */}
          {subTab === 'disabled_funcs' && (
            <div className="space-y-4">
              <div className="text-xs text-slate-500">
                Danh sách các hàm PHP nguy hiểm bị vô hiệu hóa trong{' '}
                <code className="font-mono text-orange-500">
                  disable_functions
                </code>{' '}
                nhằm ngăn chặn mã độc Webshell thực thi lệnh hệ thống trái phép.
              </div>

              <form
                onSubmit={handleAddDisabledFunc}
                className="flex items-center gap-2 max-w-md"
              >
                <input
                  type="text"
                  value={newDisabledFunc}
                  onChange={(e) => setNewDisabledFunc(e.target.value)}
                  placeholder="Nhập tên hàm cần khóa (vd: symlink, putenv)..."
                  className="flex-1 px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                />
                <button
                  type="submit"
                  className="px-3.5 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1 cursor-pointer shrink-0"
                >
                  <Plus className="w-3.5 h-3.5" /> Khóa hàm
                </button>
              </form>

              <div className="flex flex-wrap gap-2 pt-2">
                {activePhp.disableFunctions.map((fn) => (
                  <div
                    key={fn}
                    className="px-3 py-1.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 flex items-center gap-2 text-xs font-mono"
                  >
                    <span className="text-rose-600 dark:text-rose-400 font-semibold">
                      {fn}()
                    </span>
                    <button
                      onClick={() => handleRemoveDisabledFunc(fn)}
                      className="text-slate-400 hover:text-slate-900 dark:hover:text-slate-100 cursor-pointer"
                      title="Gỡ khóa hàm này"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* TAB 4: RAW PHP.INI & FPM POOL EDITOR */}
          {subTab === 'raw_ini' && (
            <form onSubmit={handleSaveRawFiles} className="space-y-4">
              <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs font-mono">
                    <span className="font-semibold text-slate-700 dark:text-slate-300">
                      {activePhp.iniPath}
                    </span>
                    <span className="text-slate-500">php.ini</span>
                  </div>
                  <textarea
                    rows={16}
                    value={rawPhpIni}
                    onChange={(e) => setRawPhpIni(e.target.value)}
                    spellCheck={false}
                    className="w-full p-3.5 rounded-xl bg-slate-950 text-emerald-300 font-mono text-xs leading-relaxed border border-slate-800"
                  />
                </div>

                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs font-mono">
                    <span className="font-semibold text-slate-700 dark:text-slate-300">
                      {activePhp.poolPath}
                    </span>
                    <span className="text-slate-500">FPM Worker Pool</span>
                  </div>
                  <textarea
                    rows={16}
                    value={rawFpmPoolConf}
                    onChange={(e) => setRawFpmPoolConf(e.target.value)}
                    spellCheck={false}
                    className="w-full p-3.5 rounded-xl bg-slate-950 text-sky-300 font-mono text-xs leading-relaxed border border-slate-800"
                  />
                </div>
              </div>

              <div className="flex items-center justify-end gap-2 pt-2 border-t border-slate-200 dark:border-slate-800">
                <button
                  type="submit"
                  disabled={busyState === 'save-raw'}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                >
                  <Save className="w-3.5 h-3.5" />
                  Lưu Cả 2 Tệp &amp; Reload PHP {activePhp.version}-FPM
                </button>
              </div>
            </form>
          )}

          {/* TAB 5: MULTIPHP DOMAIN VERSION SWITCHER */}
          {subTab === 'multiphp_sites' && (
            <div className="space-y-4">
              <div className="text-xs text-slate-500">
                Chọn phiên bản PHP-FPM riêng biệt cho từng Website (tương tự{' '}
                <strong>cPanel MultiPHP Manager</strong> &amp;{' '}
                <strong>aaPanel PHP Switcher</strong>). Khi thay đổi, hệ thống
                tự động cập nhật chỉ thị{' '}
                <code className="font-mono text-orange-500">
                  fastcgi_pass unix:/run/php/php8.x-fpm.sock;
                </code>{' '}
                trong Vhost Nginx của Website đó.
              </div>

              <div className="overflow-x-auto">
                <table className="w-full text-left border-collapse">
                  <thead>
                    <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-mono uppercase text-slate-500">
                      <th className="py-3 px-4">Tên miền Website</th>
                      <th className="py-3 px-4">Framework / Runtime</th>
                      <th className="py-3 px-4">Thư mục gốc</th>
                      <th className="py-3 px-4">Socket PHP-FPM Hiện tại</th>
                      <th className="py-3 px-4 text-right">
                        Chuyển đổi Phiên bản PHP
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs">
                    {apps.map((site) => {
                      const currentPhp =
                        site.phpVersion ||
                        (site.framework.includes('PHP') ? '8.3' : 'None');
                      return (
                        <tr key={site.id}>
                          <td className="py-3 px-4 font-mono font-bold text-slate-900 dark:text-slate-100">
                            {site.domain}
                          </td>
                          <td className="py-3 px-4 text-slate-600 dark:text-slate-300">
                            {site.framework}
                          </td>
                          <td className="py-3 px-4 font-mono text-slate-500">
                            {site.workingDir}
                          </td>
                          <td className="py-3 px-4 font-mono">
                            {currentPhp !== 'None' ? (
                              <span className="text-emerald-600 dark:text-emerald-400">
                                /run/php/php{currentPhp}-fpm.sock
                              </span>
                            ) : (
                              <span className="text-slate-400">
                                Pure Reverse Proxy (Không dùng PHP)
                              </span>
                            )}
                          </td>
                          <td className="py-3 px-4 text-right">
                            <select
                              value={currentPhp}
                              onChange={async (e) => {
                                const nextVer = e.target.value;
                                await onSwitchSitePhpVersion(site.id, nextVer);
                                notify(
                                  `Đã chuyển Website ${site.domain} sang ${nextVer === 'None' ? 'chế độ Pure Proxy' : `PHP ${nextVer}-FPM`}`
                                );
                              }}
                              className="px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100 cursor-pointer"
                            >
                              <option value="8.4">PHP 8.4-FPM (JIT 2.0)</option>
                              <option value="8.3">
                                PHP 8.3-FPM (LTS Khuyên dùng)
                              </option>
                              <option value="8.2">PHP 8.2-FPM (Stable)</option>
                              <option value="8.1">PHP 8.1-FPM</option>
                              <option value="7.4">
                                PHP 7.4-FPM (Legacy CMS)
                              </option>
                              <option value="None">
                                Không dùng PHP (Pure Rust / Node Proxy)
                              </option>
                            </select>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
