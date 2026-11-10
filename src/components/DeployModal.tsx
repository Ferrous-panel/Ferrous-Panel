import React, { useState } from 'react';
import { X } from 'lucide-react';
import { WebsiteRuntimeType } from '../types';

interface DeployModalProps {
  isOpen: boolean;
  onClose: () => void;
  onDeploy: (payload: {
    name: string;
    framework: WebsiteRuntimeType;
    domain: string;
    aliases: string[];
    internalPort: number;
    workingDir: string;
    runSubDir: string;
    execCommand: string;
    workers: number;
    memoryLimitMb: number;
    sslEnabled: boolean;
    http3Quic: boolean;
    websocketEnabled: boolean;
    rateLimitRps: number;
    createDatabase: boolean;
    dbEngine: 'PostgreSQL 17' | 'MariaDB 11.4';
    dbName: string;
    dbUser: string;
    dbPassword: string;
    createFtp: boolean;
    ftpUser: string;
    gitRepoUrl: string;
    gitBranch: string;
  }) => Promise<void>;
}

const generateRandomSqlPassword = () => {
  const chars =
    'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*';
  let out = 'Fp#';
  for (let i = 0; i < 13; i++) {
    out += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return out;
};

export const DeployModal: React.FC<DeployModalProps> = ({
  isOpen,
  onClose,
  onDeploy
}) => {
  const [domain, setDomain] = useState('');
  const [aliasesText, setAliasesText] = useState('');
  const [framework, setFramework] =
    useState<WebsiteRuntimeType>('Actix-Web (Rust)');
  const [workingDir, setWorkingDir] = useState('/www/wwwroot/');
  const [runSubDir, setRunSubDir] = useState('/');
  const [internalPort, setInternalPort] = useState(8080);
  const [execCommand, setExecCommand] = useState(
    '/www/wwwroot/app/target/release/app'
  );
  const [workers, setWorkers] = useState(4);
  const [memoryLimitMb, setMemoryLimitMb] = useState(512);

  // aaPanel-style integrated Database & FTP provisioning
  const [createDatabase, setCreateDatabase] = useState(true);
  const [dbEngine, setDbEngine] = useState<'PostgreSQL 17' | 'MariaDB 11.4'>(
    'PostgreSQL 17'
  );
  const [dbName, setDbName] = useState('');
  const [dbUser, setDbUser] = useState('');
  const [dbPassword, setDbPassword] = useState(() =>
    generateRandomSqlPassword()
  );

  const [createFtp, setCreateFtp] = useState(true);
  const [ftpUser, setFtpUser] = useState('');

  const [gitRepoUrl, setGitRepoUrl] = useState('');
  const [gitBranch, setGitBranch] = useState('main');

  const [sslEnabled, setSslEnabled] = useState(true);
  const [http3Quic, setHttp3Quic] = useState(true);
  const [websocketEnabled, setWebsocketEnabled] = useState(true);
  const [submitting, setSubmitting] = useState(false);

  if (!isOpen) return null;

  const handleDomainChange = (val: string) => {
    setDomain(val);
    const clean = val.toLowerCase().trim();
    const prefix = clean.split('.')[0]?.replace(/[^a-z0-9_]/g, '_') || 'site';
    setWorkingDir(`/www/wwwroot/${clean}`);
    setAliasesText(`www.${clean}`);
    setDbName(`${prefix}_db`);
    setDbUser(`${prefix}_user`);
    setFtpUser(`ftp_${prefix}`);
    if (framework.includes('Rust')) {
      setExecCommand(`/www/wwwroot/${clean}/target/release/${prefix}`);
    }
  };

  const handleFrameworkChange = (nextFw: WebsiteRuntimeType) => {
    setFramework(nextFw);
    const clean = domain.toLowerCase().trim() || 'app.ferroushub.vn';
    const prefix = clean.split('.')[0]?.replace(/[^a-z0-9-]/g, '-') || 'app';
    if (nextFw.includes('Rust')) {
      setRunSubDir('/');
      setDbEngine('PostgreSQL 17');
      setExecCommand(`/www/wwwroot/${clean}/target/release/${prefix}`);
    } else if (nextFw === 'Next.js 15 (React SSR)') {
      setRunSubDir('/.next/standalone');
      setExecCommand(
        `/usr/bin/node /www/wwwroot/${clean}/.next/standalone/server.js`
      );
    } else if (nextFw === 'Nuxt 3 (Vue 3 SSR)') {
      setRunSubDir('/.output/public');
      setExecCommand(
        `/usr/bin/node /www/wwwroot/${clean}/.output/server/index.mjs`
      );
    } else if (nextFw === 'SvelteKit 2 (Svelte 5)') {
      setRunSubDir('/build');
      setExecCommand(`/usr/bin/node /www/wwwroot/${clean}/build/index.js`);
    } else if (nextFw === 'Astro 5 (Hybrid / Static)') {
      setRunSubDir('/dist/client');
      setExecCommand(
        `/usr/bin/node /www/wwwroot/${clean}/dist/server/entry.mjs`
      );
    } else if (nextFw === 'Remix / React Router v7') {
      setRunSubDir('/build/client');
      setExecCommand(
        `/usr/bin/npx react-router-serve /www/wwwroot/${clean}/build/server/index.js`
      );
    } else if (nextFw === 'NestJS 10 (TypeScript API)') {
      setRunSubDir('/dist');
      setExecCommand(`/usr/bin/node /www/wwwroot/${clean}/dist/main.js`);
    } else if (nextFw === 'Vite + React 19 / Vue 3 SPA' || nextFw === 'Static HTML / SPA') {
      setRunSubDir('/dist');
      setCreateDatabase(false);
      setExecCommand('nginx static try_files /index.html');
    } else if (nextFw === 'PHP 8.3-FPM') {
      setRunSubDir('/public');
      setDbEngine('MariaDB 11.4');
      setExecCommand('php-fpm8.3 (unix:/run/php/php8.3-fpm.sock + libferrous_bridge.so)');
    } else if (nextFw === 'Python / FastAPI') {
      setRunSubDir('/');
      setExecCommand(
        `/www/wwwroot/${clean}/.venv/bin/gunicorn -k uvicorn.workers.UvicornWorker main:app`
      );
    } else if (nextFw === 'Go / Fiber') {
      setRunSubDir('/');
      setExecCommand(`/www/wwwroot/${clean}/bin/${prefix}_go_cgo`);
    } else if (nextFw === 'Java 21 / Spring Boot 3') {
      setRunSubDir('/target');
      setExecCommand(
        `/usr/bin/java -XX:+UseZGC -Djava.library.path=/opt/Ferrous-panel/lib -jar /www/wwwroot/${clean}/target/${prefix}.jar`
      );
    } else if (nextFw === 'C# / .NET 8 ASP.NET Core') {
      setRunSubDir('/publish');
      setExecCommand(`/usr/bin/dotnet /www/wwwroot/${clean}/publish/${prefix}.dll`);
    } else if (nextFw === 'Ruby 3.3 / Rails & Elixir') {
      setRunSubDir('/public');
      setExecCommand(`/usr/local/bin/bundle exec puma -C /www/wwwroot/${clean}/config/puma.rb`);
    } else if (nextFw === 'C++20 / Zig Native Daemon') {
      setRunSubDir('/');
      setExecCommand(`/www/wwwroot/${clean}/build/${prefix}_native`);
    } else if (nextFw === 'WebAssembly (WASI Wasmtime)') {
      setRunSubDir('/');
      setExecCommand(`/usr/local/bin/wasmtime serve --addr 127.0.0.1:${internalPort} /www/wwwroot/${clean}/${prefix}.wasm`);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      const clean = domain.toLowerCase().trim();
      const nameSlug = clean.replace(/[^a-z0-9-]/g, '-');
      const aliases = aliasesText
        .split(/[\s,]+/)
        .map((s) => s.trim().toLowerCase())
        .filter(Boolean);

      await onDeploy({
        name: nameSlug,
        framework,
        domain: clean,
        aliases,
        internalPort,
        workingDir,
        runSubDir,
        execCommand,
        workers,
        memoryLimitMb,
        sslEnabled,
        http3Quic,
        websocketEnabled,
        rateLimitRps: 120,
        createDatabase,
        dbEngine,
        dbName,
        dbUser,
        dbPassword,
        createFtp,
        ftpUser,
        gitRepoUrl,
        gitBranch
      });
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/75 backdrop-blur-xs">
      <div className="w-full max-w-2xl rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-2xl overflow-hidden">
        <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between bg-slate-50/70 dark:bg-slate-950/50">
          <div>
            <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
              Thêm Website Mới (Tất cả Framework &amp; Ngôn ngữ chạy trên Nhân Rust)
            </h2>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Tự động nạp cầu nối Rust Polyglot FFI/IPC (<code className="font-mono">libferrous_bridge.so</code>), Nginx Reverse Proxy, Systemd Sandbox, Database SQL và SSL
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 rounded-lg cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form
          onSubmit={handleSubmit}
          className="p-6 space-y-5 max-h-[82vh] overflow-y-auto"
        >
          {/* Domain & Runtime */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Tên miền chính (Primary Domain)
              </label>
              <input
                type="text"
                required
                value={domain}
                onChange={(e) => handleDomainChange(e.target.value)}
                className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
              />
            </div>

            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Tên miền phụ / Alias (Cách nhau bởi dấu phẩy)
              </label>
              <input
                type="text"
                value={aliasesText}
                onChange={(e) => setAliasesText(e.target.value)}
                className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
              />
            </div>

            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Framework / Ngôn ngữ (Được giám sát bởi Nhân Rust FFI/IPC)
              </label>
              <select
                value={framework}
                onChange={(e) =>
                  handleFrameworkChange(e.target.value as WebsiteRuntimeType)
                }
                className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
              >
                <optgroup label="1. Rust Native Backends (Direct ABI)">
                  <option value="Actix-Web (Rust)">
                    Actix-Web 4.9 (Rust Native LTO)
                  </option>
                  <option value="Axum (Rust)">Axum 0.8 (Rust Tokio Hyper)</option>
                  <option value="Rocket (Rust)">Rocket 0.5 (Rust Async)</option>
                </optgroup>
                <optgroup label="2. Modern JS/TS Frameworks (NAPI-RS + Rust IPC Bridge)">
                  <option value="Next.js 15 (React SSR)">
                    Next.js 15 (React 19 App Router SSR + NAPI-RS)
                  </option>
                  <option value="Nuxt 3 (Vue 3 SSR)">
                    Nuxt 3.14 (Vue 3 Nitro SSR + NAPI-RS)
                  </option>
                  <option value="SvelteKit 2 (Svelte 5)">
                    SvelteKit 2.8 (Svelte 5 Runes SSR + Bun/Node FFI)
                  </option>
                  <option value="Astro 5 (Hybrid / Static)">
                    Astro 5.0 (Content Islands Hybrid + NAPI-RS)
                  </option>
                  <option value="Remix / React Router v7">
                    Remix / React Router v7 SSR
                  </option>
                  <option value="NestJS 10 (TypeScript API)">
                    NestJS 10 (Enterprise TypeScript API + Rust Addon)
                  </option>
                  <option value="Vite + React 19 / Vue 3 SPA">
                    Vite 6 + React 19 / Vue 3 SPA
                  </option>
                </optgroup>
                <optgroup label="3. Polyglot Server Languages (PyO3 / Zend FFI / CGO / JNI / P/Invoke)">
                  <option value="PHP 8.3-FPM">
                    PHP 8.4 / 8.3 / 7.4-FPM (Laravel / WordPress + Zend Rust FFI)
                  </option>
                  <option value="Python / FastAPI">
                    Python 3.12 / FastAPI / Django (PyO3 Rust C-Extension)
                  </option>
                  <option value="Go / Fiber">
                    Go 1.23 / Fiber / Gin (CGO Static Link libferrous_bridge.a)
                  </option>
                  <option value="Java 21 / Spring Boot 3">
                    Java 21 / Kotlin Spring Boot 3 (Rust JNI &amp; Panama FFM)
                  </option>
                  <option value="C# / .NET 8 ASP.NET Core">
                    C# / .NET 8.0 ASP.NET Core Kestrel (Rust P/Invoke csbindgen)
                  </option>
                  <option value="Ruby 3.3 / Rails & Elixir">
                    Ruby 3.3 Rails / Elixir Phoenix (Magnus &amp; Rustler NIF)
                  </option>
                  <option value="C++20 / Zig Native Daemon">
                    C / C++20 / Zig Daemon (cbindgen Zero-Overhead C-ABI)
                  </option>
                  <option value="WebAssembly (WASI Wasmtime)">
                    WebAssembly WASI Preview 2 (Rust Wasmtime Sandbox)
                  </option>
                </optgroup>
              </select>
            </div>

            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Thư mục gốc Website (Document Root)
              </label>
              <input
                type="text"
                required
                value={workingDir}
                onChange={(e) => setWorkingDir(e.target.value)}
                className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
              />
            </div>
          </div>

          {/* Upstream Port & Exec Command */}
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Cổng Upstream (127.0.0.1)
              </label>
              <input
                type="number"
                value={internalPort}
                onChange={(e) => setInternalPort(Number(e.target.value))}
                className="w-full px-3 py-2 text-xs font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
              />
            </div>

            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Số luồng Worker
              </label>
              <input
                type="number"
                min={1}
                max={64}
                value={workers}
                onChange={(e) => setWorkers(Number(e.target.value))}
                className="w-full px-3 py-2 text-xs font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
              />
            </div>

            <div className="space-y-1.5">
              <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                Giới hạn RAM (MB)
              </label>
              <input
                type="number"
                step={64}
                value={memoryLimitMb}
                onChange={(e) => setMemoryLimitMb(Number(e.target.value))}
                className="w-full px-3 py-2 text-xs font-mono tabular-nums rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
              />
            </div>
          </div>

          {/* Integrated SQL Database Creation */}
          <div className="p-4 rounded-xl bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-3">
            <div className="flex items-center justify-between">
              <label className="inline-flex items-center gap-2 text-xs font-bold text-slate-900 dark:text-slate-100 cursor-pointer">
                <input
                  type="checkbox"
                  checked={createDatabase}
                  onChange={(e) => setCreateDatabase(e.target.checked)}
                  className="w-4 h-4 accent-orange-600"
                />
                <span>Khởi tạo Database SQL đi kèm cho Website này</span>
              </label>
              {createDatabase && (
                <select
                  value={dbEngine}
                  onChange={(e) =>
                    setDbEngine(
                      e.target.value as 'PostgreSQL 17' | 'MariaDB 11.4'
                    )
                  }
                  className="px-2.5 py-1 text-xs font-mono rounded border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900"
                >
                  <option value="PostgreSQL 17">PostgreSQL 17</option>
                  <option value="MariaDB 11.4">MariaDB 11.4 / MySQL (phpMyAdmin)</option>
                </select>
              )}
            </div>

            {createDatabase && (
              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-1">
                <input
                  type="text"
                  placeholder="Tên Database"
                  value={dbName}
                  onChange={(e) => setDbName(e.target.value)}
                  className="px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900"
                />
                <input
                  type="text"
                  placeholder="Tên User Database"
                  value={dbUser}
                  onChange={(e) => setDbUser(e.target.value)}
                  className="px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900"
                />
                <div className="flex items-center gap-1.5">
                  <input
                    type="text"
                    placeholder="Mật khẩu SQL tùy chỉnh"
                    value={dbPassword}
                    onChange={(e) => setDbPassword(e.target.value)}
                    className="flex-1 min-w-0 px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900"
                  />
                  <button
                    type="button"
                    onClick={() => setDbPassword(generateRandomSqlPassword())}
                    className="px-2 py-1.5 text-[11px] font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 hover:bg-slate-100 dark:hover:bg-slate-800 shrink-0 cursor-pointer"
                    title="Sinh mật khẩu SQL ngẫu nhiên"
                  >
                    Ngẫu nhiên
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Integrated FTP / SFTP Account & Git Repo */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="p-3.5 rounded-xl bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-2">
              <label className="inline-flex items-center gap-2 text-xs font-bold text-slate-900 dark:text-slate-100 cursor-pointer">
                <input
                  type="checkbox"
                  checked={createFtp}
                  onChange={(e) => setCreateFtp(e.target.checked)}
                  className="w-4 h-4 accent-orange-600"
                />
                <span>Tạo tài khoản FTP/SFTP Chroot</span>
              </label>
              {createFtp && (
                <input
                  type="text"
                  value={ftpUser}
                  onChange={(e) => setFtpUser(e.target.value)}
                  className="w-full px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900"
                />
              )}
            </div>

            <div className="p-3.5 rounded-xl bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-2">
              <div className="text-xs font-bold text-slate-900 dark:text-slate-100">
                Git Repository (Tùy chọn Auto-Deploy)
              </div>
              <input
                type="text"
                value={gitRepoUrl}
                onChange={(e) => setGitRepoUrl(e.target.value)}
                placeholder="https://github.com/user/repo.git"
                className="w-full px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900"
              />
            </div>
          </div>

          {/* SSL & Protocol Checkboxes */}
          <div className="pt-1 flex flex-wrap items-center gap-6 text-xs text-slate-700 dark:text-slate-300">
            <label className="inline-flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={sslEnabled}
                onChange={(e) => setSslEnabled(e.target.checked)}
                className="w-4 h-4 accent-orange-600"
              />
              <span>Tự động cấp SSL Let’s Encrypt &amp; Force HTTPS</span>
            </label>
            <label className="inline-flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={http3Quic}
                onChange={(e) => setHttp3Quic(e.target.checked)}
                className="w-4 h-4 accent-orange-600"
              />
              <span>Bật HTTP/3 QUIC</span>
            </label>
            <label className="inline-flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={websocketEnabled}
                onChange={(e) => setWebsocketEnabled(e.target.checked)}
                className="w-4 h-4 accent-orange-600"
              />
              <span>Hỗ trợ WebSocket</span>
            </label>
          </div>

          <div className="pt-4 border-t border-slate-200 dark:border-slate-800 flex items-center justify-end gap-3">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors cursor-pointer"
            >
              Hủy bỏ
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="px-5 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors cursor-pointer"
            >
              {submitting
                ? 'Đang tạo Website, DB & Nginx...'
                : 'Khởi tạo Website Ngay'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
