import React from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { Moon, RefreshCw, Sun, X } from 'lucide-react';
import { ActiveTab } from '../../types';
import { PanelLanguage } from '../../i18n';
import { SidebarNavItem } from '../../config/navigation';
import { FerrousLogo } from '../FerrousLogo';

interface MobileDrawerProps {
  isOpen: boolean;
  onClose: () => void;
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

export const MobileDrawer: React.FC<MobileDrawerProps> = ({
  isOpen,
  onClose,
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
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-50 lg:hidden flex">
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            onClick={onClose}
            className="fixed inset-0 bg-slate-950/70 backdrop-blur-xs"
          />
          <motion.aside
            initial={{ x: '-100%' }}
            animate={{ x: 0 }}
            exit={{ x: '-100%' }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
            className="relative z-10 w-72 max-w-[85vw] h-full bg-white dark:bg-slate-900 border-r border-slate-200 dark:border-slate-800 p-4 flex flex-col justify-between overflow-y-auto shadow-2xl"
          >
            <div className="space-y-4">
              <div className="flex items-center justify-between pb-3 border-b border-slate-200 dark:border-slate-800">
                <FerrousLogo size="sm" showWordmark={true} />
                <button
                  type="button"
                  onClick={onClose}
                  className="p-1.5 rounded-lg text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>

              <div className="px-2 py-1 space-y-0.5">
                <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                  Ubuntu Rust Control Panel
                </div>
                <div className="text-[11px] font-mono tabular-nums text-slate-500 dark:text-slate-400">
                  100% Rust Core · Wasm ABI · Polyglot FFI
                </div>
              </div>

              <div className="space-y-1">
                {items.map((item) => {
                  const Icon = item.icon;
                  const active = activeTab === item.id;
                  return (
                    <button
                      key={item.id}
                      onClick={() => {
                        onSelectTab(item.id);
                        onClose();
                      }}
                      className={`w-full flex items-center gap-3 px-3 py-2.5 text-xs font-medium rounded-lg transition-colors cursor-pointer ${
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

            <div className="pt-4 border-t border-slate-200 dark:border-slate-800 space-y-2.5">
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

              <div className="flex items-center gap-2">
                <button
                  onClick={() => {
                    onRefresh();
                    onClose();
                  }}
                  className="flex-1 px-2.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 rounded-lg inline-flex items-center justify-center gap-1.5 cursor-pointer"
                >
                  <RefreshCw
                    className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin' : ''}`}
                  />
                  {tr('Làm mới', 'Refresh')}
                </button>
                <button
                  onClick={onToggleTheme}
                  className="px-2.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 rounded-lg inline-flex items-center justify-center gap-1.5 cursor-pointer"
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
              <div className="px-2 text-[11px] font-mono tabular-nums text-slate-500 dark:text-slate-400">
                Admin: {adminUsername} · Port {sshPort}
              </div>
            </div>
          </motion.aside>
        </div>
      )}
    </AnimatePresence>
  );
};
