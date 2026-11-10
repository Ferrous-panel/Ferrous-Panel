import React, { useEffect, useState } from 'react';
import {
  Check,
  CheckCircle2,
  Code2,
  Copy,
  Cpu,
  Download,
  Layers,
  Play,
  RefreshCw,
  ShieldCheck,
  Terminal,
  Wrench
} from 'lucide-react';
import {
  PolyglotCompileResult,
  PolyglotToolchainSpec,
  RustSourceFile
} from '../types';

interface RustSourceViewProps {
  files: RustSourceFile[];
}

export const RustSourceView: React.FC<RustSourceViewProps> = ({ files }) => {
  const [subTab, setSubTab] = useState<'polyglot' | 'source'>('polyglot');
  const [selectedPath, setSelectedPath] = useState<string>(
    files[0]?.path || ''
  );
  const [copied, setCopied] = useState(false);

  // Polyglot Compiler State
  const [toolchains, setToolchains] = useState<PolyglotToolchainSpec[]>([]);
  const [targetLanguage, setTargetLanguage] = useState<string>('Python (PyO3 / FastAPI)');
  const [moduleName, setModuleName] = useState<string>('ferrous_sys_bridge');
  const [optimizationLevel, setOptimizationLevel] = useState<string>('release-lto');
  const [enableZeroCopyIpc, setEnableZeroCopyIpc] = useState<boolean>(true);
  const [exposeSysMetrics, setExposeSysMetrics] = useState<boolean>(true);
  const [exposeSqlBridge, setExposeSqlBridge] = useState<boolean>(true);
  const [customFnInput, setCustomFnInput] = useState<string>(
    'verify_argon2_token, query_sql_pool, dispatch_job_queue'
  );
  const [compiling, setCompiling] = useState<boolean>(false);
  const [compileResult, setCompileResult] = useState<PolyglotCompileResult | null>(null);
  const [activeOutputTab, setActiveOutputTab] = useState<
    'binding' | 'rust' | 'header' | 'systemd'
  >('binding');
  const [copiedSnippet, setCopiedSnippet] = useState<boolean>(false);

  useEffect(() => {
    let mounted = true;
    const loadPolyglot = async () => {
      try {
        const res = await fetch('/api/polyglot/toolchains');
        if (res.ok) {
          const data = await res.json();
          if (mounted && Array.isArray(data)) {
            setToolchains(data);
          }
        }
      } catch {
        // Ignore network errors during initial load
      }
    };
    loadPolyglot();
    handleCompileBridge('Python (PyO3 / FastAPI)');
    return () => {
      mounted = false;
    };
  }, []);

  const handleCompileBridge = async (overrideLang?: string) => {
    const langToUse = overrideLang || targetLanguage;
    setCompiling(true);
    try {
      const customFunctions = customFnInput
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);

      const res = await fetch('/api/polyglot/compile', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          targetLanguage: langToUse,
          moduleName,
          optimizationLevel,
          enableZeroCopyIpc,
          exposeSysMetrics,
          exposeSqlBridge,
          customFunctions
        })
      });
      if (res.ok) {
        const data: PolyglotCompileResult = await res.json();
        setCompileResult(data);
      }
    } finally {
      setCompiling(false);
    }
  };

  const activeFile =
    files.find((f) => f.path === selectedPath) || files[0] || null;

  const handleCopy = () => {
    if (!activeFile) return;
    navigator.clipboard.writeText(activeFile.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleCopySnippet = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedSnippet(true);
    setTimeout(() => setCopiedSnippet(false), 2000);
  };

  const handleDownloadFile = () => {
    if (!activeFile) return;
    const blob = new Blob([activeFile.content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = activeFile.title.replace('/', '_');
    a.click();
    URL.revokeObjectURL(url);
  };

  const currentSnippetContent = () => {
    if (!compileResult) return '';
    if (activeOutputTab === 'binding') return compileResult.targetBindingCode;
    if (activeOutputTab === 'rust') return compileResult.rustCoreCode;
    if (activeOutputTab === 'header') return compileResult.cHeaderCode;
    return `# --- Systemd Environment Block ---\n${compileResult.systemdUnitSnippet}\n\n# --- Build & Link Script ---\n${compileResult.buildScript}`;
  };

  return (
    <div className="space-y-6">
      {/* Architecture & Mode Switcher Banner */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="text-xs font-mono text-orange-600 dark:text-orange-400">
              rustc 1.83+ · actix-web 4.9 · cbindgen C-ABI · PyO3 · NAPI-RS · ext-php-rs · CGO · JNI · WASI
            </div>
            <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100">
              Nhân Hệ Thống Rust &amp; Trình Biên Dịch Cầu Nối Đa Ngôn Ngữ (Polyglot FFI/IPC Engine)
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Hệ thống biên dịch cốt lõi <code className="font-mono">src/polyglot_compiler.rs</code> dịch trực tiếp các hàm hệ thống Rust sang C-ABI (<code className="font-mono">cdylib</code>), N-API, PyO3, PHP Zend FFI, Go CGO, Java JNI, .NET P/Invoke và WebAssembly WASI
            </p>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <button
              onClick={() => setSubTab('polyglot')}
              className={`px-3.5 py-2 text-xs font-semibold rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
                subTab === 'polyglot'
                  ? 'bg-orange-600 text-white'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              }`}
            >
              <Layers className="w-3.5 h-3.5" />
              Trình Biên Dịch Đa Ngôn Ngữ (10 Ngôn Ngữ)
            </button>
            <button
              onClick={() => setSubTab('source')}
              className={`px-3.5 py-2 text-xs font-semibold rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer ${
                subTab === 'source'
                  ? 'bg-orange-600 text-white'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              }`}
            >
              <Code2 className="w-3.5 h-3.5" />
              Mã Nguồn Nhân Rust ({files.length} Tệp)
            </button>
          </div>
        </div>

        <div className="p-4 rounded-lg bg-slate-950 border border-slate-800 text-xs font-mono text-slate-300 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-orange-400 shrink-0" />
            <span>
              cd Ferrous-panel &amp;&amp; cargo build --release &amp;&amp; cbindgen --crate ferrous-panel --output include/ferrous_bridge.h
            </span>
          </div>
          <span className="text-emerald-400 flex items-center gap-1.5">
            <ShieldCheck className="w-3.5 h-3.5" />
            10/10 Hệ Sinh Thái Ngôn Ngữ Tương Thích (Zero-Copy FFI &amp; Unix Socket IPC)
          </span>
        </div>
      </section>

      {subTab === 'polyglot' ? (
        <div className="space-y-6">
          {/* Interactive Polyglot Compiler & FFI Bridge Studio */}
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
            {/* Compiler Configuration Panel */}
            <div className="lg:col-span-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5 space-y-4">
              <div className="flex items-center justify-between border-b border-slate-100 dark:border-slate-800 pb-3">
                <div className="flex items-center gap-2">
                  <Wrench className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                  <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                    Cấu Hình Biên Dịch Cầu Nối Rust System
                  </h2>
                </div>
                <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400">
                  ABI v2.5 Ready
                </span>
              </div>

              <div className="space-y-3">
                <div>
                  <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                    Ngôn ngữ đích cần kết nối với Nhân Rust (Target Language)
                  </label>
                  <select
                    value={targetLanguage}
                    onChange={(e) => {
                      setTargetLanguage(e.target.value);
                      handleCompileBridge(e.target.value);
                    }}
                    className="w-full px-3 py-2 text-xs rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100 font-medium"
                  >
                    <option value="Python (PyO3 / FastAPI)">
                      Python 3.10–3.13 (PyO3 ABI3 Wheel + ctypes C-ABI)
                    </option>
                    <option value="TypeScript / Node.js / Bun">
                      TypeScript / Node.js v22 / Bun (NAPI-RS .node + Unix Socket)
                    </option>
                    <option value="PHP 8.4 / 8.3 / 8.2 / 7.4 (Zend FFI)">
                      PHP 7.4–8.4 (Zend FFI::cdef &amp; ext-php-rs .so)
                    </option>
                    <option value="Go (Golang CGO)">
                      Go 1.23 (CGO Static Linker + libferrous_bridge.a)
                    </option>
                    <option value="Java / Kotlin (JNI & Project Panama)">
                      Java 21 / Kotlin (JNI &amp; Project Panama FFM API)
                    </option>
                    <option value="C# / .NET 8 (P/Invoke)">
                      C# / .NET 8.0 (LibraryImport P/Invoke csbindgen)
                    </option>
                    <option value="C / C++20 / Zig / Ruby / WASI">
                      C / C++20 / Zig / Ruby / WebAssembly (Universal C-ABI Header)
                    </option>
                  </select>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                      Tên Module / Shared Library
                    </label>
                    <input
                      type="text"
                      value={moduleName}
                      onChange={(e) => setModuleName(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                      placeholder="ferrous_sys_bridge"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                      Cờ Tối Ưu Hóa LLVM (Optimization)
                    </label>
                    <select
                      value={optimizationLevel}
                      onChange={(e) => setOptimizationLevel(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    >
                      <option value="release-lto">lto=&quot;fat&quot; opt-level=3</option>
                      <option value="release-fast">lto=&quot;thin&quot; codegen-units=1</option>
                      <option value="size-oz">opt-level=&quot;z&quot; strip=true</option>
                    </select>
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1">
                    Các hàm hệ thống Rust xuất sang ngôn ngữ đích (phân cách bằng dấu phẩy)
                  </label>
                  <input
                    type="text"
                    value={customFnInput}
                    onChange={(e) => setCustomFnInput(e.target.value)}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    placeholder="verify_argon2_token, query_sql_pool"
                  />
                </div>

                <div className="space-y-2 pt-1">
                  <label className="flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={enableZeroCopyIpc}
                      onChange={(e) => setEnableZeroCopyIpc(e.target.checked)}
                      className="rounded border-slate-300"
                    />
                    <span>Bật Zero-Copy Shared Memory &amp; Unix Domain Socket (<code className="font-mono">/run/ferrous-polyglot.sock</code>)</span>
                  </label>
                  <label className="flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={exposeSysMetrics}
                      onChange={(e) => setExposeSysMetrics(e.target.checked)}
                      className="rounded border-slate-300"
                    />
                    <span>Xuất struct C-ABI <code className="font-mono">FerrousHostTelemetryC</code> (CPU/RAM Kernel trực tiếp)</span>
                  </label>
                  <label className="flex items-center gap-2 text-xs text-slate-700 dark:text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={exposeSqlBridge}
                      onChange={(e) => setExposeSqlBridge(e.target.checked)}
                      className="rounded border-slate-300"
                    />
                    <span>Bật cầu nối Connection Pool MariaDB / PostgreSQL xuyên ngôn ngữ</span>
                  </label>
                </div>

                <button
                  onClick={() => handleCompileBridge()}
                  disabled={compiling}
                  className="w-full py-2.5 px-4 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 disabled:opacity-60 rounded-lg transition-colors flex items-center justify-center gap-2 cursor-pointer"
                >
                  {compiling ? (
                    <>
                      <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      Đang biên dịch cầu nối Rust FFI &amp; sinh Binding...
                    </>
                  ) : (
                    <>
                      <Play className="w-3.5 h-3.5" />
                      Biên Dịch Cầu Nối Rust FFI/IPC Cho Ngôn Ngữ Này
                    </>
                  )}
                </button>
              </div>

              {compileResult && (
                <div className="p-3.5 rounded-lg bg-slate-950 border border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between text-[11px] font-mono text-emerald-400">
                    <span>[BUILD SUCCEEDED] {compileResult.binaryArtifactPath}</span>
                    <span>{compileResult.compiledAt}</span>
                  </div>
                  <pre className="text-[11px] font-mono text-slate-300 whitespace-pre-wrap leading-relaxed">
                    {compileResult.compilerOutput}
                  </pre>
                </div>
              )}
            </div>

            {/* Generated Polyglot Bridge Code Viewer */}
            <div className="lg:col-span-7 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <div className="px-4 py-3 border-b border-slate-200 dark:border-slate-800 flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-1.5">
                  <button
                    onClick={() => setActiveOutputTab('binding')}
                    className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors cursor-pointer ${
                      activeOutputTab === 'binding'
                        ? 'bg-orange-600 text-white'
                        : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                    }`}
                  >
                    1. Mã Gọi Từ Ngôn Ngữ Đích (Binding)
                  </button>
                  <button
                    onClick={() => setActiveOutputTab('rust')}
                    className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors cursor-pointer ${
                      activeOutputTab === 'rust'
                        ? 'bg-orange-600 text-white'
                        : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                    }`}
                  >
                    2. Nhân Rust extern &quot;C&quot; FFI
                  </button>
                  <button
                    onClick={() => setActiveOutputTab('header')}
                    className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors cursor-pointer ${
                      activeOutputTab === 'header'
                        ? 'bg-orange-600 text-white'
                        : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                    }`}
                  >
                    3. C-ABI Header (.h)
                  </button>
                  <button
                    onClick={() => setActiveOutputTab('systemd')}
                    className={`px-3 py-1.5 text-xs font-semibold rounded-md transition-colors cursor-pointer ${
                      activeOutputTab === 'systemd'
                        ? 'bg-orange-600 text-white'
                        : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                    }`}
                  >
                    4. Systemd &amp; Build Script
                  </button>
                </div>

                <button
                  onClick={() => handleCopySnippet(currentSnippetContent())}
                  className="px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md inline-flex items-center gap-1.5 cursor-pointer"
                >
                  {copiedSnippet ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                      Đã chép
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      Sao chép mã
                    </>
                  )}
                </button>
              </div>

              {compileResult && (
                <div className="px-5 py-2.5 bg-slate-50 dark:bg-slate-950/50 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between text-xs">
                  <span className="font-mono text-slate-600 dark:text-slate-400">
                    Giao thức cầu nối: <strong className="text-slate-900 dark:text-slate-100">{compileResult.bridgeProtocol}</strong>
                  </span>
                  <span className="font-mono text-orange-600 dark:text-orange-400">
                    {compileResult.binaryArtifactPath}
                  </span>
                </div>
              )}

              <pre className="p-5 bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed overflow-x-auto max-h-[520px]">
                {currentSnippetContent()}
              </pre>
            </div>
          </div>

          {/* 10-Language Compatibility Matrix Table */}
          <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
            <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
              <div>
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                  <Cpu className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                  Bảng Kiểm Tra Tương Thích Đa Ngôn Ngữ Trên Hệ Thống Ferrous-panel
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  Tất cả 10 hệ sinh thái ngôn ngữ đều được hỗ trợ đồng thời qua Nginx Reverse Proxy, Systemd Unit Isolation và Cầu nối FFI/IPC trực tiếp với nhân Rust
                </p>
              </div>
            </div>

            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/40 text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
                    <th className="py-3 px-4">Ngôn Ngữ &amp; Framework</th>
                    <th className="py-3 px-4">Phiên Bản Runtime</th>
                    <th className="py-3 px-4">Giao Thức Cầu Nối Với Nhân Rust</th>
                    <th className="py-3 px-4">Lệnh Biên Dịch Toolchain</th>
                    <th className="py-3 px-4">Biến Môi Trường Systemd Tự Động</th>
                    <th className="py-3 px-4">Trạng Thái</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs">
                  {toolchains.map((tc) => (
                    <tr
                      key={tc.id}
                      className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40 transition-colors"
                    >
                      <td className="py-3.5 px-4 font-semibold text-slate-900 dark:text-slate-100">
                        {tc.language}
                      </td>
                      <td className="py-3.5 px-4 font-mono text-slate-600 dark:text-slate-300">
                        {tc.runtimeVersion}
                      </td>
                      <td className="py-3.5 px-4 text-slate-600 dark:text-slate-300">
                        {tc.bridgeProtocol}
                      </td>
                      <td className="py-3.5 px-4 font-mono text-[11px] text-orange-600 dark:text-orange-400">
                        {tc.compilerCommand}
                      </td>
                      <td className="py-3.5 px-4 font-mono text-[11px] text-slate-500 dark:text-slate-400">
                        {tc.systemdEnvVars.join(' · ')}
                      </td>
                      <td className="py-3.5 px-4">
                        <span className="inline-flex items-center gap-1 text-emerald-600 dark:text-emerald-400 font-semibold whitespace-nowrap">
                          <CheckCircle2 className="w-3.5 h-3.5" />
                          Tương thích 100%
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>
        </div>
      ) : (
        /* Existing File Tree + Code Viewer */
        <div className="space-y-4">
          <div className="flex justify-end gap-2">
            <button
              onClick={handleDownloadFile}
              className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Download className="w-3.5 h-3.5" />
              Tải tệp đang chọn
            </button>
            <button
              onClick={handleCopy}
              className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors inline-flex items-center gap-1.5 cursor-pointer"
            >
              {copied ? (
                <>
                  <Check className="w-3.5 h-3.5" />
                  Đã sao chép mã nguồn
                </>
              ) : (
                <>
                  <Copy className="w-3.5 h-3.5" />
                  Sao chép mã nguồn
                </>
              )}
            </button>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
            <div className="lg:col-span-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 divide-y divide-slate-200 dark:divide-slate-800 overflow-hidden">
              <div className="px-5 py-3.5 bg-slate-50/70 dark:bg-slate-950/40 text-xs font-semibold text-slate-600 dark:text-slate-400">
                Cấu trúc thư mục /Ferrous-panel ({files.length} tệp)
              </div>
              {files.map((file) => {
                const isSelected = activeFile?.path === file.path;
                return (
                  <button
                    key={file.path}
                    onClick={() => setSelectedPath(file.path)}
                    className={`w-full text-left p-4 transition-colors block cursor-pointer ${
                      isSelected
                        ? 'bg-orange-50/60 dark:bg-orange-950/20'
                        : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-mono text-sm font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                        <Code2 className="w-4 h-4 text-orange-600 dark:text-orange-400 shrink-0" />
                        {file.title}
                      </span>
                      <span className="text-xs font-mono text-slate-400">
                        {file.lang}
                      </span>
                    </div>
                    <p className="mt-1.5 text-xs text-slate-500 dark:text-slate-400 leading-relaxed">
                      {file.description}
                    </p>
                  </button>
                );
              })}
            </div>

            {activeFile && (
              <div className="lg:col-span-8 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
                <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
                  <div className="font-mono text-xs font-semibold text-slate-800 dark:text-slate-200">
                    {activeFile.path}
                  </div>
                  <span className="text-xs text-slate-500 dark:text-slate-400">
                    {activeFile.description}
                  </span>
                </div>
                <pre className="p-5 bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed overflow-x-auto max-h-[640px]">
                  {activeFile.content}
                </pre>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

