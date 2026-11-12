import React from 'react';
import { LogOut, Menu, Plus, Search } from 'lucide-react';
import { ActiveTab } from '../../types';
import { PanelLanguage } from '../../i18n';
import { FerrousLogo } from '../FerrousLogo';

interface TopHeaderBarProps {
  activeTab: ActiveTab;
  onSelectTab: (tab: ActiveTab) => void;
  onOpenMobileMenu: () => void;
  onOpenCommandPalette: () => void;
  onOpenDeployModal: () => void;
  onLogout: () => void;
  lang: PanelLanguage;
  onSelectLang: (lang: PanelLanguage) => void;
  tr: (viText: string, enText: string) => string;
}

export const TopHeaderBar: React.FC<TopHeaderBarProps> = ({
  activeTab,
  onSelectTab,
  onOpenMobileMenu,
  onOpenCommandPalette,
  onOpenDeployModal,
  onLogout,
  lang,
  onSelectLang,
  tr
}) => {
  return (
    <header className="w-full max-w-[100vw] flex items-center justify-between gap-2 sm:gap-6 lg:gap-8 px-3.5 sm:px-6 py-3 border-b border-slate-200 dark:border-slate-800 bg-white/95 dark:bg-slate-900/95 backdrop-blur-md sticky top-0 z-30">
      {/* Zone 1: Mobile Drawer Trigger + Brand lockup */}
      <div className="flex items-center gap-2.5 min-w-0 shrink-0">
        <button
          type="button"
          onClick={onOpenMobileMenu}
          className="lg:hidden p-2 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors cursor-pointer shrink-0"
          aria-label={tr('Mở menu điều hướng', 'Open navigation menu')}
        >
          <Menu className="w-4 h-4" />
        </button>

        <a
          href="#overview"
          onClick={(e) => {
            e.preventDefault();
            onSelectTab('overview');
          }}
          className="whitespace-nowrap shrink-0"
        >
          <FerrousLogo size="sm" showWordmark={true} />
        </a>
      </div>

      {/* Zone 2: 5 clean single-line text navigation links */}
      <nav className="hidden xl:flex items-center gap-6 text-sm font-medium text-slate-600 dark:text-slate-400">
        <a
          href="#websites"
          onClick={(e) => {
            e.preventDefault();
            onSelectTab('websites');
          }}
          className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
            activeTab === 'websites'
              ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
              : ''
          }`}
        >
          Websites
        </a>
        <a
          href="#edge-cloud"
          onClick={(e) => {
            e.preventDefault();
            onSelectTab('edge-cloud');
          }}
          className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
            activeTab === 'edge-cloud'
              ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
              : ''
          }`}
        >
          Rust Edge &amp; WASI
        </a>
        <a
          href="#php"
          onClick={(e) => {
            e.preventDefault();
            onSelectTab('php');
          }}
          className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
            activeTab === 'php'
              ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
              : ''
          }`}
        >
          Multi-PHP &amp; INI
        </a>
        <a
          href="#databases"
          onClick={(e) => {
            e.preventDefault();
            onSelectTab('databases');
          }}
          className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
            activeTab === 'databases'
              ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
              : ''
          }`}
        >
          MySQL &amp; phpMyAdmin
        </a>
        <a
          href="#rust-source"
          onClick={(e) => {
            e.preventDefault();
            onSelectTab('rust-source');
          }}
          className={`hover:text-slate-900 dark:hover:text-slate-100 hover:underline underline-offset-4 transition-colors whitespace-nowrap shrink-0 ${
            activeTab === 'rust-source'
              ? 'text-slate-900 dark:text-slate-100 font-semibold underline'
              : ''
          }`}
        >
          {tr('Nhân Rust & FFI', 'Rust Core & FFI')}
        </a>
      </nav>

      {/* Zone 3: Quick Search + Segmented Language Switcher + Primary Action + Logout */}
      <div className="flex items-center gap-1.5 sm:gap-2 shrink-0">
        <button
          type="button"
          onClick={onOpenCommandPalette}
          className="p-2 sm:px-2.5 sm:py-1.5 text-xs font-medium rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950/60 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-600 dark:text-slate-400 transition-colors inline-flex items-center gap-1.5 cursor-pointer shrink-0"
          title={tr(
            'Tìm kiếm nhanh phân hệ (Ctrl+K)',
            'Quick search modules (Ctrl+K)'
          )}
        >
          <Search className="w-3.5 h-3.5" />
          <span className="hidden md:inline font-mono text-[11px]">Ctrl+K</span>
        </button>

        {/* Segmented Language Switcher: Instant 1-Click Tiếng Việt / English */}
        <div
          data-fp-lang-ignore="true"
          className="inline-flex items-center p-0.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950/80 shrink-0"
          role="group"
          aria-label="Language switcher"
        >
          <button
            type="button"
            onClick={() => onSelectLang('vi')}
            className={`px-2 sm:px-2.5 py-1 text-xs font-medium rounded-md transition-colors cursor-pointer whitespace-nowrap ${
              lang === 'vi'
                ? 'bg-orange-600 text-white font-semibold shadow-xs'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
            }`}
            title="Chuyển sang Tiếng Việt"
          >
            <span className="hidden sm:inline">Tiếng Việt</span>
            <span className="sm:hidden font-mono">VI</span>
          </button>
          <button
            type="button"
            onClick={() => onSelectLang('en')}
            className={`px-2 sm:px-2.5 py-1 text-xs font-medium rounded-md transition-colors cursor-pointer whitespace-nowrap ${
              lang === 'en'
                ? 'bg-orange-600 text-white font-semibold shadow-xs'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
            }`}
            title="Switch to English"
          >
            <span className="hidden sm:inline">English</span>
            <span className="sm:hidden font-mono">EN</span>
          </button>
        </div>

        <button
          onClick={onOpenDeployModal}
          className="px-2.5 sm:px-3.5 py-1.5 text-xs font-medium text-white bg-orange-600 rounded-lg hover:bg-orange-500 transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer shadow-xs"
        >
          <Plus className="w-3.5 h-3.5 shrink-0" />
          <span className="hidden sm:inline">
            {tr('Thêm Website Mới', 'Add New Website')}
          </span>
          <span className="sm:hidden">{tr('Thêm Web', 'Add Site')}</span>
        </button>

        <button
          type="button"
          onClick={onLogout}
          className="p-2 sm:px-2.5 sm:py-1.5 text-xs font-medium rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950/60 hover:bg-rose-50 hover:text-rose-600 dark:hover:bg-rose-950/30 dark:hover:text-rose-400 text-slate-600 dark:text-slate-400 transition-colors inline-flex items-center gap-1.5 cursor-pointer shrink-0"
          title={tr('Đăng xuất khỏi FerrousPanel', 'Sign out of FerrousPanel')}
        >
          <LogOut className="w-3.5 h-3.5" />
          <span className="hidden md:inline">
            {tr('Đăng xuất', 'Sign Out')}
          </span>
        </button>
      </div>
    </header>
  );
};
