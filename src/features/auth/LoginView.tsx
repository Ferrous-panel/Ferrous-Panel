import React, { useState } from 'react';
import {
  AlertCircle,
  Eye,
  EyeOff,
  Globe,
  KeyRound,
  Moon,
  RefreshCw,
  Server,
  ShieldCheck,
  Sun,
  UserCheck
} from 'lucide-react';
import { PanelAdminAccount } from '../types';
import { PanelLanguage } from '../i18n';
import { FerrousLogo } from '../../components/FerrousLogo';

interface LoginViewProps {
  onLoginSuccess: (token: string, user: { username: string }) => void;
  panelAdminConfig?: PanelAdminAccount | null;
  serverHost?: string;
  isDark: boolean;
  onToggleTheme: () => void;
  lang?: PanelLanguage;
  onToggleLang?: () => void;
  onSelectLang?: (lang: PanelLanguage) => void;
}

export const LoginView: React.FC<LoginViewProps> = ({
  onLoginSuccess,
  panelAdminConfig,
  serverHost = typeof window !== 'undefined' && window.location.hostname
    ? window.location.hostname
    : '127.0.0.1',
  isDark,
  onToggleTheme,
  lang = 'vi',
  onToggleLang,
  onSelectLang
}) => {
  const panelPort = panelAdminConfig?.panelPort || 3000;
  const isEn = lang === 'en';

  const handleChooseLang = (target: PanelLanguage) => {
    if (onSelectLang) {
      onSelectLang(target);
    } else if (onToggleLang && target !== lang) {
      onToggleLang();
    }
  };

  const [username, setUsername] = useState<string>('');
  const [password, setPassword] = useState<string>('');
  const [showPassword, setShowPassword] = useState<boolean>(false);
  const [rememberMe, setRememberMe] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage(null);
    setLoading(true);

    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          username: username.trim(),
          password: password.trim(),
          rememberMe
        })
      });

      const data = await res.json();

      if (!res.ok || !data.success) {
        setErrorMessage(
          isEn
            ? 'Invalid username or password. Please verify your credentials.'
            : data.error ||
                'Tên tài khoản hoặc mật khẩu không chính xác. Vui lòng kiểm tra lại.'
        );
        return;
      }

      onLoginSuccess(data.token, data.user);
    } catch {
      setErrorMessage(
        isEn
          ? 'Unable to reach the authentication server. Please check your network connection.'
          : 'Không thể kết nối đến dịch vụ xác thực máy chủ. Vui lòng kiểm tra kết nối mạng.'
      );
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex flex-col justify-between bg-slate-100 text-slate-900 dark:bg-slate-950 dark:text-slate-100 transition-colors">
      {/* Top Bar Utilities */}
      <header className="px-4 sm:px-6 py-3.5 flex items-center justify-between border-b border-slate-200 dark:border-slate-800 bg-white/70 dark:bg-slate-900/70 backdrop-blur-xs">
        <div className="flex items-center gap-3">
          <FerrousLogo size="md" showWordmark={true} />
          <span className="hidden md:inline text-xs font-mono text-slate-500 dark:text-slate-400">
            · Rust Polyglot Server Engine
          </span>
        </div>

        <div className="flex items-center gap-2">
          <div className="hidden lg:inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-mono tabular-nums bg-slate-200/80 dark:bg-slate-800 text-slate-700 dark:text-slate-300">
            <Server className="w-3.5 h-3.5 text-orange-500" />
            <span>
              {panelAdminConfig?.customDomain
                ? `Domain: ${panelAdminConfig.customDomain}`
                : `Node: ${serverHost}:${panelPort}`}
            </span>
          </div>

          <div
            data-fp-lang-ignore="true"
            className="inline-flex items-center p-0.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-200/70 dark:bg-slate-950/80"
          >
            <button
              type="button"
              onClick={() => handleChooseLang('vi')}
              className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors cursor-pointer ${
                !isEn
                  ? 'bg-orange-600 text-white shadow-xs font-semibold'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
              }`}
            >
              Tiếng Việt
            </button>
            <button
              type="button"
              onClick={() => handleChooseLang('en')}
              className={`px-2.5 py-1 text-xs font-medium rounded-md transition-colors cursor-pointer ${
                isEn
                  ? 'bg-orange-600 text-white shadow-xs font-semibold'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
              }`}
            >
              English
            </button>
          </div>

          <button
            type="button"
            onClick={onToggleTheme}
            className="p-2 rounded-lg bg-slate-200/80 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-300 transition-colors cursor-pointer"
            title={isEn ? 'Toggle Light / Dark Theme' : 'Chuyển đổi giao diện Sáng / Tối'}
          >
            {isDark ? (
              <Sun className="w-4 h-4 text-amber-400" />
            ) : (
              <Moon className="w-4 h-4 text-slate-600" />
            )}
          </button>
        </div>
      </header>

      {/* Main Login Card Container */}
      <main className="flex-1 flex items-center justify-center p-4 sm:p-6 my-auto">
        <div className="w-full max-w-md space-y-4">
          <div className="rounded-2xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-xl p-6 sm:p-8 space-y-6">
            <div className="space-y-2 text-center">
              <div className="inline-flex items-center justify-center mb-1">
                <FerrousLogo size="xl" showWordmark={false} />
              </div>
              <h1 className="text-xl font-bold tracking-tight text-slate-900 dark:text-slate-100">
                {isEn ? 'Administrator Sign In' : 'Đăng Nhập Quản Trị Viên'}
              </h1>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                {isEn
                  ? 'Protected by Argon2id & Rust Actix-Web System Core'
                  : 'Bảo vệ bởi cơ chế Argon2id & Nhân Hệ Thống Rust Actix-Web'}
              </p>
              <div
                data-fp-lang-ignore="true"
                className="pt-1 flex items-center justify-center gap-1.5 text-[11px]"
              >
                <Globe className="w-3.5 h-3.5 text-orange-500" />
                <span className="text-slate-500 dark:text-slate-400">
                  {isEn ? 'Language:' : 'Ngôn ngữ:'}
                </span>
                <button
                  type="button"
                  onClick={() => handleChooseLang('vi')}
                  className={`px-2 py-0.5 rounded font-medium cursor-pointer transition-colors ${
                    !isEn
                      ? 'bg-orange-600/15 text-orange-600 dark:text-orange-400 font-semibold'
                      : 'text-slate-500 hover:text-slate-800 dark:hover:text-slate-200'
                  }`}
                >
                  Tiếng Việt (VI)
                </button>
                <span className="text-slate-300 dark:text-slate-700">|</span>
                <button
                  type="button"
                  onClick={() => handleChooseLang('en')}
                  className={`px-2 py-0.5 rounded font-medium cursor-pointer transition-colors ${
                    isEn
                      ? 'bg-orange-600/15 text-orange-600 dark:text-orange-400 font-semibold'
                      : 'text-slate-500 hover:text-slate-800 dark:hover:text-slate-200'
                  }`}
                >
                  English (EN)
                </button>
              </div>
            </div>

            {errorMessage && (
              <div className="p-3.5 rounded-lg border border-rose-500/40 bg-rose-50 dark:bg-rose-950/30 text-xs text-rose-700 dark:text-rose-300 flex items-start gap-2.5">
                <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-rose-500" />
                <div className="space-y-1">
                  <div className="font-semibold">
                    {isEn ? 'Authentication Failed' : 'Đăng nhập thất bại'}
                  </div>
                  <div className="leading-relaxed">{errorMessage}</div>
                </div>
              </div>
            )}

            <form onSubmit={handleSubmit} className="space-y-4">
              <div className="space-y-1.5">
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                  {isEn ? 'Admin Username' : 'Tài khoản Admin (Username)'}
                </label>
                <div className="relative">
                  <input
                    type="text"
                    required
                    autoFocus
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    placeholder={
                      isEn
                        ? 'Enter administrator username'
                        : 'Nhập tài khoản quản trị viên'
                    }
                    className="w-full px-3.5 py-2.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-orange-500/30 focus:border-orange-500"
                  />
                  <UserCheck className="w-4 h-4 text-slate-400 absolute right-3 top-3 pointer-events-none" />
                </div>
              </div>

              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300">
                    {isEn ? 'Password' : 'Mật khẩu (Password)'}
                  </label>
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="text-[11px] text-slate-500 hover:text-slate-700 dark:hover:text-slate-300 inline-flex items-center gap-1 cursor-pointer"
                  >
                    {showPassword ? (
                      <>
                        <EyeOff className="w-3 h-3" /> {isEn ? 'Hide' : 'Ẩn'}
                      </>
                    ) : (
                      <>
                        <Eye className="w-3 h-3" /> {isEn ? 'Show' : 'Hiện'}
                      </>
                    )}
                  </button>
                </div>
                <div className="relative">
                  <input
                    type={showPassword ? 'text' : 'password'}
                    required
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="••••••••••••"
                    className="w-full px-3.5 py-2.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-orange-500/30 focus:border-orange-500"
                  />
                  <KeyRound className="w-4 h-4 text-slate-400 absolute right-3 top-3 pointer-events-none" />
                </div>
              </div>

              <div className="flex items-center justify-between pt-1">
                <label className="inline-flex items-center gap-2 text-xs text-slate-600 dark:text-slate-400 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={rememberMe}
                    onChange={(e) => setRememberMe(e.target.checked)}
                    className="w-4 h-4 rounded text-orange-600 focus:ring-orange-500 cursor-pointer"
                  />
                  <span>
                    {isEn
                      ? 'Keep me signed in (30 days)'
                      : 'Ghi nhớ phiên đăng nhập (30 ngày)'}
                  </span>
                </label>
              </div>

              <button
                type="submit"
                disabled={loading}
                className="w-full py-2.5 px-4 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 disabled:opacity-60 rounded-lg transition-colors shadow-sm inline-flex items-center justify-center gap-2 cursor-pointer"
              >
                {loading ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                    <span>
                      {isEn
                        ? 'Verifying credentials...'
                        : 'Đang kiểm tra tài khoản...'}
                    </span>
                  </>
                ) : (
                  <>
                    <ShieldCheck className="w-4 h-4" />
                    <span>
                      {isEn ? 'Sign In to Panel' : 'Đăng Nhập Vào Panel'}
                    </span>
                  </>
                )}
              </button>
            </form>
          </div>

          <div className="text-center text-xs text-slate-500 dark:text-slate-400 tabular-nums">
            {isEn
              ? `FerrousPanel v2.5.0 · Port ${panelPort} · Manage Websites, Multi-PHP, FerrousX/Nginx & MySQL`
              : `FerrousPanel v2.5.0 · Cổng ${panelPort} · Quản lý Web, Multi-PHP, FerrousX/Nginx & MySQL`}
          </div>
        </div>
      </main>

      <footer className="px-6 py-3 border-t border-slate-200 dark:border-slate-800 text-center text-xs text-slate-500 dark:text-slate-400 bg-white/50 dark:bg-slate-900/50">
        © 2026 FerrousPanel. All rights reserved. Powered by Rust Actix-Web Backend Engine.
      </footer>
    </div>
  );
};
