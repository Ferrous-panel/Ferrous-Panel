import React, { useEffect, useState } from 'react';
import { AnimatePresence, motion } from 'motion/react';
import { Moon, RefreshCw, Sun } from 'lucide-react';
import { ActiveTab } from './types';
import { usePanelLanguage } from './i18n';
import { getSidebarItems } from './config/navigation';
import { useAuthSession } from './hooks/useAuthSession';
import { usePanelController } from './hooks/usePanelController';
import { TopHeaderBar } from './components/layout/TopHeaderBar';
import { SidebarNav } from './components/layout/SidebarNav';
import { MobileDrawer } from './components/layout/MobileDrawer';
import { CommandPaletteModal } from './components/layout/CommandPaletteModal';
import {
  CpanelSuiteView,
  DeployModal,
  FileManagerView,
  LoginView,
  MailServerView,
  OneClickAppsView,
  OverviewView,
  PhpManagerView,
  RustSourceView,
  SecurityCenterView,
  SqlDatabaseView,
  SshTerminalView,
  SystemOptimizerView,
  WebsitesManagerView
} from './features';

export { actixGatewayFetch, type ActixGatewaySubsystem } from './services/actixGateway';

export default function App() {
  const { lang, setLang, toggleLang, tr } = usePanelLanguage();
  const [activeTab, setActiveTab] = useState<ActiveTab>('overview');
  const [isDark, setIsDark] = useState<boolean>(true);
  const [isDeployModalOpen, setIsDeployModalOpen] = useState<boolean>(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState<boolean>(false);
  const [commandOpen, setCommandOpen] = useState<boolean>(false);
  const [commandQuery, setCommandQuery] = useState<string>('');

  const controller = usePanelController(activeTab, setActiveTab);
  const { isAuthenticated, currentUser, handleLoginSuccess, handleLogout } =
    useAuthSession(controller.fetchAllData);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark);
  }, [isDark]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setCommandOpen((prev) => !prev);
      } else if (e.key === 'Escape') {
        setCommandOpen(false);
        setMobileMenuOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const sidebarItems = getSidebarItems(tr);

  // Mandatory Login Gate: If not authenticated, render LoginView only
  if (!isAuthenticated) {
    return (
      <LoginView
        onLoginSuccess={handleLoginSuccess}
        panelAdminConfig={controller.sshData?.panelAdmin}
        serverHost={controller.sshData?.serverHost || '103.145.88.210'}
        isDark={isDark}
        onToggleTheme={() => setIsDark(!isDark)}
        lang={lang}
        onToggleLang={toggleLang}
        onSelectLang={setLang}
      />
    );
  }

  const activeTabItem =
    sidebarItems.find((item) => item.id === activeTab) || sidebarItems[0];
  const adminUsername =
    currentUser?.username ||
    controller.sshData?.panelAdmin?.adminUsername ||
    'admin_ferrous';
  const sshPort = controller.sshData?.config.port ?? 22;

  return (
    <div className="min-h-screen w-full max-w-[100vw] overflow-x-hidden flex flex-col bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
      {/* Top Bar Contract: 3 zones separated by responsive gap */}
      <TopHeaderBar
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        onOpenMobileMenu={() => setMobileMenuOpen(true)}
        onOpenCommandPalette={() => setCommandOpen(true)}
        onOpenDeployModal={() => setIsDeployModalOpen(true)}
        onLogout={handleLogout}
        lang={lang}
        onSelectLang={setLang}
        tr={tr}
      />

      {/* Mobile Slide-Over Navigation Drawer */}
      <MobileDrawer
        isOpen={mobileMenuOpen}
        onClose={() => setMobileMenuOpen(false)}
        items={sidebarItems}
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        lang={lang}
        onSelectLang={setLang}
        isDark={isDark}
        onToggleTheme={() => setIsDark(!isDark)}
        refreshing={controller.refreshing}
        onRefresh={controller.fetchAllData}
        adminUsername={adminUsername}
        sshPort={sshPort}
        tr={tr}
      />

      {/* Command Palette / Quick Module Switcher (Ctrl+K) */}
      <CommandPaletteModal
        isOpen={commandOpen}
        onClose={() => setCommandOpen(false)}
        query={commandQuery}
        onQueryChange={setCommandQuery}
        items={sidebarItems}
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        tr={tr}
      />

      {/* Workspace Body: Left Sidebar + Main Viewport */}
      <div className="flex-1 flex w-full max-w-[100vw] min-w-0 overflow-x-hidden">
        <SidebarNav
          items={sidebarItems}
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          lang={lang}
          onSelectLang={setLang}
          isDark={isDark}
          onToggleTheme={() => setIsDark(!isDark)}
          refreshing={controller.refreshing}
          onRefresh={controller.fetchAllData}
          adminUsername={adminUsername}
          sshPort={sshPort}
          tr={tr}
        />

        {/* Main Viewport */}
        <main className="flex-1 min-w-0 w-full max-w-7xl mx-auto p-3.5 sm:p-6 lg:p-8 space-y-5 sm:space-y-6 overflow-x-hidden">
          {/* Mobile Contextual Bar + Horizontal Scrollable Tab Strip */}
          <div className="lg:hidden space-y-2.5 min-w-0 w-full">
            <div className="flex items-center justify-between gap-2 px-1">
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-xs font-bold text-slate-900 dark:text-slate-100 truncate">
                  {activeTabItem.label}
                </span>
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  type="button"
                  onClick={controller.fetchAllData}
                  className="px-2.5 py-1 text-[11px] font-medium rounded-md bg-slate-200/70 dark:bg-slate-800 text-slate-700 dark:text-slate-300 inline-flex items-center gap-1 cursor-pointer"
                >
                  <RefreshCw
                    className={`w-3 h-3 ${controller.refreshing ? 'animate-spin' : ''}`}
                  />
                  <span>{tr('Làm mới', 'Refresh')}</span>
                </button>
                <button
                  type="button"
                  onClick={() => setIsDark(!isDark)}
                  className="p-1.5 rounded-md bg-slate-200/70 dark:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                >
                  {isDark ? (
                    <Sun className="w-3.5 h-3.5 text-amber-400" />
                  ) : (
                    <Moon className="w-3.5 h-3.5 text-slate-600" />
                  )}
                </button>
              </div>
            </div>

            <div className="flex items-center gap-1.5 overflow-x-auto no-scrollbar pb-2 border-b border-slate-200 dark:border-slate-800 w-full max-w-full">
              {sidebarItems.map((item) => {
                const Icon = item.icon;
                const active = activeTab === item.id;
                return (
                  <button
                    key={item.id}
                    onClick={() => setActiveTab(item.id)}
                    className={`px-3 py-1.5 text-xs font-medium rounded-lg whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
                      active
                        ? 'bg-orange-600 text-white shadow-xs'
                        : 'text-slate-600 dark:text-slate-400 bg-white dark:bg-slate-900 border border-slate-200/80 dark:border-slate-800'
                    }`}
                  >
                    <Icon className="w-3.5 h-3.5 shrink-0" />
                    <span>{item.label}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Animated View Content */}
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.14, ease: [0.16, 1, 0.3, 1] }}
              className="min-w-0 w-full max-w-full"
            >
              {activeTab === 'overview' && (
                <OverviewView
                  telemetry={controller.telemetry}
                  apps={controller.apps}
                  databases={controller.databases}
                  cpanel={controller.cpanel}
                  security={controller.security}
                  mailData={controller.mailData}
                  diagnostics={controller.diagnostics}
                  onRunDiagnostics={controller.fetchDiagnostics}
                  onNavigate={controller.handleNavigate}
                  onAppAction={controller.handleAppAction}
                  onOpenDeployModal={() => setIsDeployModalOpen(true)}
                />
              )}

              {activeTab === 'websites' && (
                <WebsitesManagerView
                  apps={controller.apps}
                  selectedAppId={controller.selectedAppId}
                  autoOpenSettingsCounter={controller.autoOpenSettingsCounter}
                  onSelectApp={controller.setSelectedAppId}
                  onNavigate={controller.handleNavigate}
                  onAppAction={controller.handleAppAction}
                  onUpdateConfig={controller.handleUpdateAppConfig}
                  onUpdateSiteSettings={controller.handleUpdateSiteSettings}
                  onTriggerGitDeploy={controller.handleTriggerGitDeploy}
                  onTestWebsiteHttp={controller.handleTestWebsiteHttp}
                  onRenewSsl={controller.handleRenewSsl}
                  onDeleteWebsite={controller.handleDeleteWebsite}
                  onOpenDeployModal={() => setIsDeployModalOpen(true)}
                />
              )}

              {activeTab === 'php' && (
                <PhpManagerView
                  phpVersions={controller.phpVersions}
                  apps={controller.apps}
                  onPhpAction={controller.handlePhpAction}
                  onUpdatePhpSettings={controller.handleUpdatePhpSettings}
                  onTogglePhpExtension={controller.handleTogglePhpExtension}
                  onSwitchSitePhpVersion={controller.handleSwitchSitePhpVersion}
                />
              )}

              {activeTab === 'ssh' && controller.sshData && (
                <SshTerminalView
                  sshData={controller.sshData}
                  onUpdateConfig={controller.handleUpdateSshConfig}
                  onAddAuthorizedKey={controller.handleAddAuthorizedKey}
                  onGenerateKeypair={controller.handleGenerateSshKeypair}
                  onDeleteKey={controller.handleDeleteSshKey}
                  onTerminateSession={controller.handleTerminateSshSession}
                  onExecCommand={controller.handleExecWebSsh}
                  onUpdatePanelAdmin={controller.handleUpdatePanelAdmin}
                  onRunFpCliAction={controller.handleRunFpCliAction}
                  onNavigate={controller.handleNavigate}
                />
              )}

              {activeTab === 'oneclick' && (
                <OneClickAppsView
                  templates={controller.templates}
                  packages={controller.packages}
                  onDeployTemplate={controller.handleDeployOneClick}
                  onPackageAction={controller.handlePackageAction}
                  onSavePackageConfig={controller.handleSavePackageConfig}
                />
              )}

              {activeTab === 'files' && (
                <FileManagerView
                  apps={controller.apps}
                  selectedAppId={controller.selectedAppId}
                  onSelectApp={controller.setSelectedAppId}
                />
              )}

              {activeTab === 'databases' && (
                <SqlDatabaseView
                  databases={controller.databases}
                  apps={controller.apps}
                  mysqlConfig={controller.mysqlConfig}
                  onCreateDatabase={controller.handleCreateDatabase}
                  onChangeDatabasePassword={
                    controller.handleChangeDatabasePassword
                  }
                  onDeleteDatabase={controller.handleDeleteDatabase}
                  onExecuteQuery={controller.handleExecuteSqlQuery}
                  onUpdateMysqlConfig={controller.handleUpdateMysqlConfig}
                />
              )}

              {activeTab === 'mail' && controller.mailData && (
                <MailServerView
                  mailData={controller.mailData}
                  onAddDomain={controller.handleAddMailDomain}
                  onCreateMailbox={controller.handleCreateMailbox}
                  onDeleteMailbox={controller.handleDeleteMailbox}
                  onCreateForwarder={controller.handleCreateForwarder}
                  onDeleteForwarder={controller.handleDeleteForwarder}
                  onSendTestMail={controller.handleSendTestMail}
                />
              )}

              {activeTab === 'optimizer' && controller.optimizer && (
                <SystemOptimizerView
                  optimizer={controller.optimizer}
                  onApplyProfile={controller.handleApplyOptimizationProfile}
                  onToggleSysctl={controller.handleToggleSysctlParam}
                  onReclaimMemory={controller.handleReclaimMemory}
                  onWatchdogHeal={controller.handleWatchdogHeal}
                  onUpdateAntiCrash={controller.handleUpdateAntiCrash}
                  onEmergencyMitigate={controller.handleEmergencyMitigate}
                />
              )}

              {activeTab === 'cpanel' && (
                <CpanelSuiteView
                  cpanel={controller.cpanel}
                  apps={controller.apps}
                  databases={controller.databases}
                  onAddCron={controller.handleAddCron}
                  onRunCronNow={controller.handleRunCronNow}
                  onToggleCron={controller.handleToggleCron}
                  onDeleteCron={controller.handleDeleteCron}
                  onAddDns={controller.handleAddDns}
                  onDeleteDns={controller.handleDeleteDns}
                  onCreateBackup={controller.handleCreateBackup}
                  onRestoreBackup={controller.handleRestoreBackup}
                  onDeleteBackup={controller.handleDeleteBackup}
                  onAddSftp={controller.handleAddSftp}
                  onDeleteSftp={controller.handleDeleteSftp}
                />
              )}

              {activeTab === 'security' && (
                <SecurityCenterView
                  security={controller.security}
                  onAddUfwRule={controller.handleAddUfwRule}
                  onToggleUfwRule={controller.handleToggleUfwRule}
                  onDeleteUfwRule={controller.handleDeleteUfwRule}
                  onBanIp={controller.handleBanIp}
                  onUnbanIp={controller.handleUnbanIp}
                  onToggleHardening={controller.handleToggleHardening}
                  onChangeDdosMode={controller.handleChangeDdosMode}
                  onToggleWafRule={controller.handleToggleWafRule}
                  onRunFimScan={controller.handleRunFimScan}
                />
              )}

              {activeTab === 'rust-source' && (
                <RustSourceView files={controller.rustFiles} />
              )}
            </motion.div>
          </AnimatePresence>
        </main>
      </div>

      {/* Add Website (aaPanel / DirectAdmin / cPanel Style) Modal */}
      <DeployModal
        isOpen={isDeployModalOpen}
        onClose={() => setIsDeployModalOpen(false)}
        onDeploy={controller.handleDeployNewWebsite}
      />
    </div>
  );
}
