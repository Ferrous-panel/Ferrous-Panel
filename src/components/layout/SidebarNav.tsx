import React from 'react';
import { Moon, RefreshCw, Sun } from 'lucide-react';
import { ActiveTab } from '../../types';
import { PanelLanguage } from '../../i18n';
import { SidebarNavItem } from '../../config/navigation';

interface SidebarNavProps {
  items: SidebarNavItem[];
  activeTab: ActiveTab;
  onSelectTab: (tab: ActiveTab) => void;
  lang: PanelLanguage;
  onSelectLang: (lang: PanelLanguage) => void;
  isDark: boolean;
  onToggleTheme: () => void;
  refreshing: boolean;
  onRefresh: () => void;
  adminUsername: string;
  sshPort: number;
  tr: (viText: string, enText: string) => string;
}

export const SidebarNav: React.FC<SidebarNavProps> = ({
  items,
  activeTab,
  onSelectTab,
  lang,
  onSelectLang,
  isDark,
  onToggleTheme,
  refreshing,
  onRefresh,
  adminUsername,
  sshPort,
  tr
}) => {
  return (
    <aside className="hidden lg:flex w-64 shrink-0 flex-col justify-between border-r border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4">
      <div className="space-y-6">
        <div className="px-3 py-2 space-y-1">
          <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
            Ubuntu Rust Control Panel
          </div>
          <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
            100% Rust Core · Wasm · Polyglot FFI
          </div>
        </div>

        <div className="space-y-1">
          {items.map((item) => {
            const Icon = item.icon;
            const active = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectTab(item.id)}
                className={`w-full flex items-center gap-3 px-3 py-2.5 text-xs font-medium rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                  active
                    ? 'bg-orange-600 text-white shadow-xs'
                    : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800 hover:text-slate-900 dark:hover:text-slate-100'
                }`}
              >
                <Icon className="w-4 h-4 shrink-0" />
                <span className="truncate">{item.label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Sidebar Footer: Workspace Utilities (Segmented Language Toggle, Refresh & Theme Toggle) */}
      <div className="pt-4 border-t border-slate-200 dark:border-slate-800 space-y-2">
        <div
          data-fp-lang-ignore="true"
          className="grid grid-cols-2 gap-1 p-1 rounded-lg bg-slate-100 dark:bg-slate-800"
        >
          <button
            type="button"
            onClick={() => onSelectLang('vi')}
            className={`py-1.5 text-xs font-medium rounded-md transition-colors cursor-pointer ${
              lang === 'vi'
                ? 'bg-orange-600 text-white font-semibold shadow-xs'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
            }`}
          >
            Tiếng Việt (VI)
          </button>
          <button
            type="button"
            onClick={() => onSelectLang('en')}
            className={`py-1.5 text-xs font-medium rounded-md transition-colors cursor-pointer ${
              lang === 'en'
                ? 'bg-orange-600 text-white font-semibold shadow-xs'
                : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-100'
            }`}
          >
            English (EN)
          </button>
        </div>

        <div className="flex items-center gap-1.5">
          <button
            onClick={onRefresh}
            className="flex-1 px-2.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors inline-flex items-center justify-center gap-1.5 whitespace-nowrap shrink-0 cursor-pointer"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin' : ''}`}
            />
            {tr('Làm mới', 'Refresh')}
          </button>
          <button
            onClick={onToggleTheme}
            className="px-2.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors inline-flex items-center justify-center gap-1 whitespace-nowrap shrink-0 cursor-pointer"
            title={tr(
              'Chuyển đổi giao diện Sáng / Tối',
              'Toggle Light / Dark theme'
            )}
          >
            {isDark ? (
              <>
                <Sun className="w-3.5 h-3.5 text-amber-400" />
                {tr('Sáng', 'Light')}
              </>
            ) : (
              <>
                <Moon className="w-3.5 h-3.5 text-slate-600" />
                {tr('Tối', 'Dark')}
              </>
            )}
          </button>
        </div>
        <div className="px-2 flex items-center justify-between text-[11px] font-mono tabular-nums text-slate-500 dark:text-slate-400">
          <span>{adminUsername}</span>
          <span>SSH :{sshPort}</span>
        </div>
      </div>
    </aside>
  );
};
