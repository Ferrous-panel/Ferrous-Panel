import React, { useState, useRef, useEffect } from 'react';
import {
  Check,
  Code2,
  Copy,
  Cpu,
  Download,
  KeyRound,
  Laptop,
  Lock,
  Play,
  Plus,
  RefreshCw,
  Server,
  ShieldCheck,
  Terminal,
  Trash2,
  Unplug
} from 'lucide-react';
import {
  ActiveTab,
  PanelAdminAccount,
  SshDaemonConfig,
  SshGeneratedKeypair,
  SshPanelData,
  WebSshCommandOutput
} from '../types';

interface SshTerminalViewProps {
  sshData: SshPanelData;
  onUpdateConfig: (payload: Partial<SshDaemonConfig>) => Promise<void>;
  onAddAuthorizedKey: (payload: {
    label: string;
    publicKey: string;
    clientHint: string;
  }) => Promise<void>;
  onGenerateKeypair: (payload: {
    label: string;
    clientHint: string;
  }) => Promise<SshGeneratedKeypair | null>;
  onDeleteKey: (id: string) => Promise<void>;
  onTerminateSession: (id: string) => Promise<void>;
  onExecCommand: (
    command: string,
    cwd: string
  ) => Promise<WebSshCommandOutput>;
  onUpdatePanelAdmin: (payload: Partial<PanelAdminAccount>) => Promise<void>;
  onRunFpCliAction: (
    option: number,
    value?: string
  ) => Promise<{ output: string; panelAdmin: PanelAdminAccount } | null>;
  onNavigate: (tab: ActiveTab) => void;
}

interface PtySessionTab {
  id: string;
  title: string;
  cwd: string;
  history: WebSshCommandOutput[];
}

const QUICK_SSH_COMMANDS = [
  { label: 'fp (Panel Terminal 1-18)', cmd: 'fp' },
  { label: 'fp default (Info Admin)', cmd: 'fp default' },
  { label: 'fp repair (Sửa chữa VPS)', cmd: 'fp repair' },
  { label: 'fp check (Kiểm tra 8 tầng)', cmd: 'fp check' },
  { label: 'ls -la /wwwroot', cmd: 'ls -la' },
  { label: 'ss -tulpn (Ports)', cmd: 'ss -tulpn' },
  { label: 'systemctl status', cmd: 'systemctl status' },
  { label: 'htop / ps aux', cmd: 'htop' },
  { label: 'cat sshd_config', cmd: 'cat /etc/ssh/sshd_config' },
  { label: 'ufw status', cmd: 'ufw status numbered' },
  { label: 'ferrousx -t', cmd: 'ferrousx -t' },
  { label: 'nginx -t', cmd: 'nginx -t' },
  { label: 'free -h (RAM)', cmd: 'free -h' },
  { label: 'Switch Web Server (FerrousX/Nginx)', cmd: 'fp 19' }
];

const FP_CLI_OPTIONS: {
  option: number;
  title: string;
  command: string;
  category: 'service' | 'admin' | 'repair';
  promptPlaceholder?: string;
}[] = [
  {
    option: 1,
    title: 'Khởi động lại Ferrous-panel Daemon (Restart)',
    command: 'fp 1',
    category: 'service'
  },
  {
    option: 2,
    title: 'Dừng Ferrous-panel Daemon (Stop)',
    command: 'fp 2',
    category: 'service'
  },
  {
    option: 3,
    title: 'Bật Ferrous-panel Daemon (Start)',
    command: 'fp 3',
    category: 'service'
  },
  {
    option: 4,
    title: 'Nạp lại cấu hình Panel & Nginx (Reload)',
    command: 'fp 4',
    category: 'service'
  },
  {
    option: 5,
    title: 'Đổi mật khẩu Admin Panel (Change Admin Password)',
    command: 'fp 5',
    category: 'admin',
    promptPlaceholder: 'Nhập mật khẩu Admin mới (để trống = tự sinh)'
  },
  {
    option: 6,
    title: 'Đổi tên tài khoản Admin Panel (Change Admin Username)',
    command: 'fp 6',
    category: 'admin',
    promptPlaceholder: 'Nhập tên tài khoản Admin mới (VD: superadmin)'
  },
  {
    option: 8,
    title: 'Đổi cổng Web Panel (Change Panel Port + UFW)',
    command: 'fp 8',
    category: 'admin',
    promptPlaceholder: 'Nhập cổng mới (VD: 8443 hoặc 8888)'
  },
  {
    option: 9,
    title: 'Đổi đường dẫn bảo mật (Security Entrance Path)',
    command: 'fp 9',
    category: 'admin',
    promptPlaceholder: 'Nhập đường dẫn bảo mật mới (VD: /ferrous-admin)'
  },
  {
    option: 10,
    title: 'Xóa bỏ đường dẫn bảo mật (Clear Security Entrance)',
    command: 'fp 10',
    category: 'admin'
  },
  {
    option: 11,
    title: 'Đặt lại mật khẩu Root SQL (MariaDB & PostgreSQL)',
    command: 'fp 11',
    category: 'admin',
    promptPlaceholder: 'Nhập mật khẩu Root SQL mới'
  },
  {
    option: 12,
    title: 'Sửa chữa & Đồng bộ toàn diện kiến trúc VPS (Repair)',
    command: 'fp 12',
    category: 'repair'
  },
  {
    option: 13,
    title: 'Dọn dẹp Cache hệ thống & Giải phóng RAM (Drop Caches)',
    command: 'fp 13',
    category: 'repair'
  },
  {
    option: 14,
    title: 'Mở khóa IP Admin (Unban Fail2ban) & Tắt khóa 2FA',
    command: 'fp 14',
    category: 'repair'
  },
  {
    option: 15,
    title: 'Xem thông tin đăng nhập mặc định (fp default)',
    command: 'fp 15',
    category: 'admin'
  },
  {
    option: 16,
    title: 'Kiểm tra sức khỏe 8 tầng dịch vụ VPS (Health Check)',
    command: 'fp 16',
    category: 'repair'
  },
  {
    option: 17,
    title: 'Gán Tên Miền Riêng cho Web Panel (Custom Domain)',
    command: 'fp 17 <domain>',
    category: 'admin',
    promptPlaceholder: 'Nhập tên miền (VD: panel.domain.com hoặc cp.ferroushub.vn)'
  },
  {
    option: 18,
    title: 'Xóa bỏ Tên Miền Riêng (Trở về đăng nhập bằng IP VPS)',
    command: 'fp 18',
    category: 'admin'
  },
  {
    option: 19,
    title:
      'Chuyển đổi Web Server Engine: FerrousX (github.com/Ferrous-panel/FerrousX) ↔ Nginx 1.26',
    command: 'fp 19 [ferrousx|nginx]',
    category: 'service',
    promptPlaceholder: 'Nhập ferrousx (mặc định) hoặc nginx'
  }
];

export const SshTerminalView: React.FC<SshTerminalViewProps> = ({
  sshData,
  onUpdateConfig,
  onAddAuthorizedKey,
  onGenerateKeypair,
  onDeleteKey,
  onTerminateSession,
  onExecCommand,
  onUpdatePanelAdmin,
  onRunFpCliAction,
  onNavigate
}) => {
  const [subTab, setSubTab] = useState<
    'web_terminal' | 'fp_panel_cli' | 'external_clients' | 'rust_modules'
  >('web_terminal');

  // Panel Admin Account & fp CLI state
  const panelAdmin: PanelAdminAccount = sshData.panelAdmin || {
    adminUsername: 'admin_ferrous',
    adminPassword: 'Fp#Admin9824!xK',
    passwordHashAlgo: 'Argon2id (Rust native)',
    panelPort: 8443,
    securityEntrance: '/panel',
    customDomain: '',
    sslEnabled: false,
    mysqlRootPassword: 'Fp#SqlRoot9941!zM',
    postgresRootPassword: 'Fp#PgRoot9941!zM',
    twoFactorEnabled: false,
    allowedAdminIps: '0.0.0.0/0',
    configFile: '/etc/ferrous-panel/admin.json',
    installedAt: '2025-05-15 08:00:00',
    lastModifiedAt: '2025-05-15 11:30:00'
  };
  const [adminUserInput, setAdminUserInput] = useState<string>(
    panelAdmin.adminUsername
  );
  const [adminPassInput, setAdminPassInput] = useState<string>(
    panelAdmin.adminPassword
  );
  const [panelPortInput, setPanelPortInput] = useState<number>(
    panelAdmin.panelPort
  );
  const [entranceInput, setEntranceInput] = useState<string>(
    panelAdmin.securityEntrance
  );
  const [customDomainInput, setCustomDomainInput] = useState<string>(
    panelAdmin.customDomain || ''
  );
  const [sqlRootPassInput, setSqlRootPassInput] = useState<string>(
    panelAdmin.mysqlRootPassword
  );
  const [fpCliOutput, setFpCliOutput] = useState<string>('');
  const [runningFpOption, setRunningFpOption] = useState<number | null>(null);
  const [fpOptionParam, setFpOptionParam] = useState<string>('');
  const [savingAdminAccount, setSavingAdminAccount] = useState<boolean>(false);

  // Web SSH PTY Multi-Session State
  const [sessions, setSessions] = useState<PtySessionTab[]>([
    {
      id: 'pty-1',
      title: 'pts/0 · /www/wwwroot',
      cwd: '/www/wwwroot',
      history: []
    }
  ]);
  const [activePtyId, setActivePtyId] = useState<string>('pty-1');
  const [commandInput, setCommandInput] = useState<string>('');
  const [cmdHistoryList, setCmdHistoryList] = useState<string[]>([]);
  const [historyCursor, setHistoryCursor] = useState<number>(-1);
  const [runningCmd, setRunningCmd] = useState<boolean>(false);
  const terminalBottomRef = useRef<HTMLDivElement | null>(null);

  // External SSH Configuration State
  const [portInput, setPortInput] = useState<number>(sshData.config.port);
  const [rootLoginPolicy, setRootLoginPolicy] = useState<
    SshDaemonConfig['permitRootLogin']
  >(sshData.config.permitRootLogin);
  const [passwordAuth, setPasswordAuth] = useState<boolean>(
    sshData.config.passwordAuthentication
  );
  const [pubkeyAuth, setPubkeyAuth] = useState<boolean>(
    sshData.config.pubkeyAuthentication
  );
  const [tcpForwarding, setTcpForwarding] = useState<boolean>(
    sshData.config.allowTcpForwarding
  );
  const [maxAuthTries, setMaxAuthTries] = useState<number>(
    sshData.config.maxAuthTries
  );
  const [savingConfig, setSavingConfig] = useState<boolean>(false);
  const [statusBanner, setStatusBanner] = useState<string | null>(null);

  // Key Management State
  const [newKeyLabel, setNewKeyLabel] = useState<string>('');
  const [newKeyClient, setNewKeyClient] = useState<string>(
    'Termius / VS Code Remote-SSH'
  );
  const [newPublicKeyText, setNewPublicKeyText] = useState<string>('');
  const [generatedKeypair, setGeneratedKeypair] =
    useState<SshGeneratedKeypair | null>(null);
  const [generatingKey, setGeneratingKey] = useState<boolean>(false);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const activePty =
    sessions.find((s) => s.id === activePtyId) || sessions[0];

  useEffect(() => {
    terminalBottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [activePty.history.length, activePtyId]);

  const handleCopyText = (id: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  const executeTerminalCommand = async (rawCommand: string) => {
    const trimmed = rawCommand.trim();
    if (!trimmed) return;

    if (trimmed.toLowerCase() === 'clear') {
      setSessions((prev) =>
        prev.map((s) => (s.id === activePty.id ? { ...s, history: [] } : s))
      );
      setCommandInput('');
      return;
    }

    setRunningCmd(true);
    setCmdHistoryList((prev) => [trimmed, ...prev.slice(0, 29)]);
    setHistoryCursor(-1);
    setCommandInput('');

    try {
      const result = await onExecCommand(trimmed, activePty.cwd);
      setSessions((prev) =>
        prev.map((s) =>
          s.id === activePty.id
            ? {
                ...s,
                cwd: result.cwd,
                title: `${s.title.split('·')[0].trim()} · ${result.cwd}`,
                history: [...s.history, result]
              }
            : s
        )
      );
    } finally {
      setRunningCmd(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      executeTerminalCommand(commandInput);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const nextIdx = Math.min(historyCursor + 1, cmdHistoryList.length - 1);
      if (nextIdx >= 0 && cmdHistoryList[nextIdx]) {
        setHistoryCursor(nextIdx);
        setCommandInput(cmdHistoryList[nextIdx]);
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const nextIdx = historyCursor - 1;
      if (nextIdx >= 0 && cmdHistoryList[nextIdx]) {
        setHistoryCursor(nextIdx);
        setCommandInput(cmdHistoryList[nextIdx]);
      } else {
        setHistoryCursor(-1);
        setCommandInput('');
      }
    }
  };

  const handleCreateNewPtyTab = () => {
    const idx = sessions.length;
    const newId = `pty-${Date.now()}`;
    const newTab: PtySessionTab = {
      id: newId,
      title: `pts/${idx} · /www/wwwroot`,
      cwd: '/www/wwwroot',
      history: [
        {
          id: `init-${Date.now()}`,
          cwd: '/www/wwwroot',
          command: 'whoami && pwd',
          output: 'root\n/www/wwwroot',
          exitCode: 0,
          timestamp: new Date().toLocaleTimeString('vi-VN', { hour12: false })
        }
      ]
    };
    setSessions((prev) => [...prev, newTab]);
    setActivePtyId(newId);
  };

  const handleSaveSshdConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingConfig(true);
    setStatusBanner(null);
    try {
      await onUpdateConfig({
        port: portInput,
        permitRootLogin: rootLoginPolicy,
        passwordAuthentication: passwordAuth,
        pubkeyAuthentication: pubkeyAuth,
        allowTcpForwarding: tcpForwarding,
        maxAuthTries
      });
      setStatusBanner(
        `Đã ghi /etc/ssh/sshd_config (Port ${portInput}), đồng bộ tường lửa UFW và nạp lại dịch vụ OpenSSH (systemctl reload ssh) không ngắt kết nối hiện tại.`
      );
    } finally {
      setSavingConfig(false);
    }
  };

  const handleGenerateEd25519 = async () => {
    const label = newKeyLabel.trim() || `workstation-${Date.now().toString().slice(-4)}`;
    setGeneratingKey(true);
    try {
      const generated = await onGenerateKeypair({
        label,
        clientHint: newKeyClient
      });
      if (generated) {
        setGeneratedKeypair(generated);
        setNewKeyLabel('');
        setStatusBanner(
          `Đã sinh cặp khóa Ed25519 "${generated.label}" và tự động thêm Public Key vào /root/.ssh/authorized_keys.`
        );
      }
    } finally {
      setGeneratingKey(false);
    }
  };

  const handleDownloadPrivateKey = (kp: SshGeneratedKeypair) => {
    const blob = new Blob([kp.privateKeyPem], {
      type: 'application/x-pem-file;charset=utf-8'
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${kp.label.toLowerCase().replace(/[^a-z0-9_-]/g, '_')}_ed25519.pem`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleAddManualPublicKey = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newPublicKeyText.trim()) return;
    await onAddAuthorizedKey({
      label: newKeyLabel.trim() || 'External Client Key',
      publicKey: newPublicKeyText.trim(),
      clientHint: newKeyClient
    });
    setNewKeyLabel('');
    setNewPublicKeyText('');
    setStatusBanner(
      'Đã thêm khóa công khai vào ~/.ssh/authorized_keys. Bạn có thể đăng nhập ngay từ phần mềm SSH bên ngoài.'
    );
  };

  const handleSavePanelAdmin = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingAdminAccount(true);
    try {
      await onUpdatePanelAdmin({
        adminUsername: adminUserInput,
        adminPassword: adminPassInput,
        panelPort: panelPortInput,
        securityEntrance: entranceInput,
        customDomain: customDomainInput.trim().toLowerCase(),
        mysqlRootPassword: sqlRootPassInput
      });
      setStatusBanner(
        `Đã cập nhật cấu hình Admin VPS (${adminUserInput}) và Tên miền Panel (${customDomainInput || 'Mặc định IP'}) vào /etc/ferrous-panel/admin.json.`
      );
      setFpCliOutput(
        `[OK] Đã ghi cấu hình Admin VPS vào ${panelAdmin.configFile}\n  Admin Username  : ${adminUserInput}\n  Admin Password  : ${adminPassInput}\n  Panel Domain    : ${customDomainInput || '(Chưa gán tên miền riêng)'}\n  Panel Port      : ${panelPortInput}/TCP (UFW Allowed)\n  Entrance Path   : ${entranceInput}`
      );
    } finally {
      setSavingAdminAccount(false);
    }
  };

  const handleTriggerFpOption = async (opt: number) => {
    setRunningFpOption(opt);
    try {
      const res = await onRunFpCliAction(opt, fpOptionParam.trim() || undefined);
      if (res) {
        setFpCliOutput(res.output);
        if (res.panelAdmin) {
          setAdminUserInput(res.panelAdmin.adminUsername);
          setAdminPassInput(res.panelAdmin.adminPassword);
          setPanelPortInput(res.panelAdmin.panelPort);
          setEntranceInput(res.panelAdmin.securityEntrance);
          setCustomDomainInput(res.panelAdmin.customDomain || '');
          setSqlRootPassInput(res.panelAdmin.mysqlRootPassword);
        }
        setStatusBanner(
          `Đã thực thi lệnh nội bộ 'fp ${opt}' trên VPS thành công.`
        );
      }
    } finally {
      setRunningFpOption(null);
    }
  };

  const vpsInstallCommandSnippet = `git clone https://github.com/Ferrous-panel/FerrousX.git /opt/Ferrous-panel && cd /opt/Ferrous-panel && bash Ferrous-panel/install-ubuntu.sh --admin-user="${adminUserInput}" --admin-pass="${adminPassInput}"${customDomainInput.trim() ? ` --domain="${customDomainInput.trim()}"` : ''} --port=${panelPortInput} --entrance="${entranceInput}"`;

  const totalRustMemoryKb = sshData.rustModules.reduce(
    (sum, m) => sum + m.memoryFootprintKb,
    0
  );

  const sshCliSnippet = `ssh -p ${sshData.config.port} root@${sshData.serverHost}`;
  const sftpUriSnippet = `sftp://root@${sshData.serverHost}:${sshData.config.port}/www/wwwroot`;
  const vscodeConfigSnippet = [
    'Host ferrous-ubuntu-prod',
    `    HostName ${sshData.serverHost}`,
    `    Port ${sshData.config.port}`,
    '    User root',
    '    IdentityFile ~/.ssh/id_ed25519',
    '    ServerAliveInterval 60',
    '    ForwardAgent yes'
  ].join('\n');

  return (
    <div className="space-y-6">
      {/* Header Banner */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="text-xs font-mono text-orange-600 dark:text-orange-400">
              RUST NATIVE SSH ENGINE · PANEL CLI TOOLBOX (/usr/local/bin/fp) · ADMIN VPS ({panelAdmin.adminUsername})
            </div>
            <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100">
              Web SSH Terminal, Công cụ Sửa chữa Nội bộ VPS (`fp` CLI) &amp; Tài khoản Admin
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Thực thi lệnh Ubuntu trực tiếp, gọi bảng điều khiển nội bộ 16 chức năng (`fp` / `ferrous` tương tự `bt` của aaPanel) để sửa chữa VPS và quản lý tài khoản Admin khởi tạo khi cài đặt.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2 shrink-0">
            <button
              onClick={() => setSubTab('web_terminal')}
              className={`px-3.5 py-2 text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
                subTab === 'web_terminal'
                  ? 'bg-orange-600 text-white'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              }`}
            >
              <Terminal className="w-3.5 h-3.5" />
              Web SSH trên Panel
            </button>
            <button
              onClick={() => setSubTab('fp_panel_cli')}
              className={`px-3.5 py-2 text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
                subTab === 'fp_panel_cli'
                  ? 'bg-orange-600 text-white'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              }`}
            >
              <Server className="w-3.5 h-3.5" />
              Panel Terminal Nội Bộ (`fp` CLI &amp; Admin VPS)
            </button>
            <button
              onClick={() => setSubTab('external_clients')}
              className={`px-3.5 py-2 text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
                subTab === 'external_clients'
                  ? 'bg-orange-600 text-white'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              }`}
            >
              <Laptop className="w-3.5 h-3.5" />
              SSH Phần mềm Ngoài ({sshData.authorizedKeys.length})
            </button>
            <button
              onClick={() => setSubTab('rust_modules')}
              className={`px-3.5 py-2 text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
                subTab === 'rust_modules'
                  ? 'bg-orange-600 text-white'
                  : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'
              }`}
            >
              <Cpu className="w-3.5 h-3.5" />
              14 Module Rust
            </button>
          </div>
        </div>

        {/* Summary Metrics Bar */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-2 border-t border-slate-200 dark:border-slate-800">
          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/60 border border-slate-200 dark:border-slate-800 space-y-1">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Địa chỉ &amp; Cổng OpenSSH
            </div>
            <div className="text-sm font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              {sshData.serverHost}:{sshData.config.port}
            </div>
            <div className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400">
              sshd Active · UFW Allowed
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/60 border border-slate-200 dark:border-slate-800 space-y-1">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Phương thức Xác thực Ngoài
            </div>
            <div className="text-sm font-bold font-mono text-slate-900 dark:text-white">
              {sshData.config.pubkeyAuthentication ? 'Ed25519 Key' : ''}{' '}
              {sshData.config.passwordAuthentication ? '+ Password' : '(Key Only)'}
            </div>
            <div className="text-[11px] font-mono text-slate-500">
              Root: {sshData.config.permitRootLogin}
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/60 border border-slate-200 dark:border-slate-800 space-y-1">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Phiên SSH Đang Kết nối
            </div>
            <div className="text-sm font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              {sshData.activeSessions.length} phiên (pts/*)
            </div>
            <div className="text-[11px] font-mono text-sky-600 dark:text-sky-400">
              Web PTY + Termius + VS Code
            </div>
          </div>

          <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950/60 border border-slate-200 dark:border-slate-800 space-y-1">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              Tỷ lệ Chuyển đổi sang Rust
            </div>
            <div className="text-sm font-bold font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
              100% ({sshData.rustModules.length}/12 Modules)
            </div>
            <div className="text-[11px] font-mono tabular-nums text-slate-500">
              Tổng RAM: {(totalRustMemoryKb / 1024).toFixed(1)} MB
            </div>
          </div>
        </div>

        {statusBanner && (
          <div className="p-3.5 rounded-lg border border-emerald-500/40 bg-emerald-50/70 dark:bg-emerald-950/30 text-xs font-mono text-emerald-800 dark:text-emerald-300 flex items-center justify-between gap-3">
            <span>{statusBanner}</span>
            <button
              onClick={() => setStatusBanner(null)}
              className="text-xs underline shrink-0 cursor-pointer"
            >
              Đóng
            </button>
          </div>
        )}
      </section>

      {/* SUBTAB 1: INTEGRATED WEB SSH TERMINAL ON PANEL */}
      {subTab === 'web_terminal' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
          {/* Main PTY Terminal Console (8 cols) */}
          <section className="lg:col-span-8 rounded-xl border border-slate-800 bg-slate-950 text-slate-100 overflow-hidden shadow-lg">
            {/* Terminal Top Session Bar */}
            <div className="px-4 py-2.5 bg-slate-900 border-b border-slate-800 flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-1.5 overflow-x-auto">
                {sessions.map((sess) => {
                  const isCurrent = sess.id === activePty.id;
                  return (
                    <button
                      key={sess.id}
                      onClick={() => setActivePtyId(sess.id)}
                      className={`px-3 py-1.5 rounded-md text-xs font-mono inline-flex items-center gap-1.5 cursor-pointer transition-colors ${
                        isCurrent
                          ? 'bg-orange-600 text-white font-semibold'
                          : 'bg-slate-800 text-slate-400 hover:text-slate-200'
                      }`}
                    >
                      <Terminal className="w-3.5 h-3.5" />
                      <span>{sess.title}</span>
                    </button>
                  );
                })}
                <button
                  onClick={handleCreateNewPtyTab}
                  className="px-2.5 py-1.5 rounded-md text-xs font-mono bg-slate-800 hover:bg-slate-700 text-slate-300 inline-flex items-center gap-1 cursor-pointer"
                  title="Mở thêm phiên PTY mới"
                >
                  <Plus className="w-3.5 h-3.5" />
                  Phiên mới
                </button>
              </div>

              <div className="flex items-center gap-2 text-[11px] font-mono text-slate-400">
                <span>Rust PTY Bridge (actix-ws)</span>
                <button
                  onClick={() => executeTerminalCommand('clear')}
                  className="px-2 py-1 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 cursor-pointer"
                >
                  Xóa màn hình
                </button>
              </div>
            </div>

            {/* Quick Command Macro Toolbar */}
            <div className="px-4 py-2 bg-slate-900/60 border-b border-slate-800/80 flex items-center gap-1.5 overflow-x-auto">
              <span className="text-[11px] font-mono text-slate-400 shrink-0 mr-1">
                Lệnh nhanh:
              </span>
              {QUICK_SSH_COMMANDS.map((q) => (
                <button
                  key={q.cmd}
                  onClick={() => executeTerminalCommand(q.cmd)}
                  className="px-2.5 py-1 rounded bg-slate-800/90 hover:bg-orange-600 hover:text-white text-slate-300 font-mono text-[11px] whitespace-nowrap shrink-0 transition-colors cursor-pointer"
                >
                  {q.label}
                </button>
              ))}
            </div>

            {/* Terminal Output Viewport */}
            <div className="p-5 font-mono text-xs leading-relaxed space-y-4 h-[460px] overflow-y-auto">
              <div className="text-slate-400 border-b border-slate-800/80 pb-3">
                <div>
                  Welcome to Ubuntu 24.04.1 LTS (GNU/Linux 6.8.0-48-generic x86_64)
                </div>
                <div className="text-emerald-400 mt-0.5">
                  * FerrousPanel Web SSH PTY v2.0.0 — Powered by Rust (portable-pty + actix-ws)
                </div>
                <div className="text-slate-500 mt-0.5">
                  * Gõ &apos;help&apos; để xem danh sách lệnh hoặc nhấn phím Mũi tên Lên/Xuống để gọi lại lệnh cũ.
                </div>
              </div>

              {activePty.history.map((item) => (
                <div key={item.id} className="space-y-1.5">
                  <div className="flex items-center gap-2 text-xs">
                    <span className="text-emerald-400 font-semibold">
                      root@ubuntu-prod-node01
                    </span>
                    <span className="text-slate-500">:</span>
                    <span className="text-sky-400 font-semibold">{item.cwd}</span>
                    <span className="text-slate-400">#</span>
                    <span className="text-white font-semibold">
                      {item.command}
                    </span>
                    <span className="ml-auto text-[11px] text-slate-600 tabular-nums">
                      {item.timestamp}
                    </span>
                  </div>
                  {item.output && (
                    <pre className="text-slate-300 whitespace-pre-wrap break-words pl-2 border-l border-slate-800">
                      {item.output}
                    </pre>
                  )}
                </div>
              ))}
              <div ref={terminalBottomRef} />
            </div>

            {/* Interactive Prompt Input Line */}
            <div className="px-4 py-3 bg-slate-900 border-t border-slate-800 flex items-center gap-2 font-mono text-xs">
              <span className="text-emerald-400 font-semibold shrink-0">
                root@ubuntu-prod-node01:{activePty.cwd}#
              </span>
              <input
                type="text"
                value={commandInput}
                onChange={(e) => setCommandInput(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder="Nhập lệnh Linux (vd: ls -la, ss -tulpn, systemctl status, cd /etc/ssh, help)..."
                className="flex-1 bg-transparent text-white placeholder-slate-500 focus:outline-none"
                autoFocus
              />
              <button
                onClick={() => executeTerminalCommand(commandInput)}
                disabled={runningCmd}
                className="px-3 py-1.5 rounded bg-orange-600 hover:bg-orange-500 disabled:opacity-50 text-white font-sans font-semibold text-xs inline-flex items-center gap-1 shrink-0 cursor-pointer"
              >
                <Play className="w-3 h-3" />
                Thực thi
              </button>
            </div>
          </section>

          {/* Right Column: External Client Quick Connect Card & Active Sessions (4 cols) */}
          <div className="lg:col-span-4 space-y-6">
            <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5 space-y-4">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                  <Laptop className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                  Kết nối Nhanh từ Phần mềm Ngoài
                </h2>
                <button
                  onClick={() => setSubTab('external_clients')}
                  className="text-xs font-medium text-orange-600 dark:text-orange-400 hover:underline cursor-pointer"
                >
                  Cấu hình sshd
                </button>
              </div>

              <p className="text-xs text-slate-500 dark:text-slate-400">
                Sao chép lệnh dưới đây để đăng nhập vào máy chủ từ <strong>Termius, PuTTY, MobaXterm, Tabby</strong> hoặc <strong>Terminal macOS / Windows PowerShell</strong>:
              </p>

              <div className="space-y-2.5">
                <div className="p-3 rounded-lg bg-slate-950 border border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between text-[11px] text-slate-400">
                    <span>OpenSSH CLI / Termius / Tabby</span>
                    <button
                      onClick={() => handleCopyText('cli-quick', sshCliSnippet)}
                      className="text-orange-400 hover:underline inline-flex items-center gap-1 cursor-pointer"
                    >
                      {copiedId === 'cli-quick' ? (
                        <>
                          <Check className="w-3 h-3" /> Đã chép
                        </>
                      ) : (
                        <>
                          <Copy className="w-3 h-3" /> Sao chép
                        </>
                      )}
                    </button>
                  </div>
                  <div className="font-mono text-xs text-emerald-400 select-all">
                    {sshCliSnippet}
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-slate-950 border border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between text-[11px] text-slate-400">
                    <span>SFTP (FileZilla / WinSCP / Cyberduck)</span>
                    <button
                      onClick={() => handleCopyText('sftp-quick', sftpUriSnippet)}
                      className="text-orange-400 hover:underline inline-flex items-center gap-1 cursor-pointer"
                    >
                      {copiedId === 'sftp-quick' ? (
                        <>
                          <Check className="w-3 h-3" /> Đã chép
                        </>
                      ) : (
                        <>
                          <Copy className="w-3 h-3" /> Sao chép
                        </>
                      )}
                    </button>
                  </div>
                  <div className="font-mono text-xs text-sky-400 select-all break-all">
                    {sftpUriSnippet}
                  </div>
                </div>
              </div>
            </section>

            {/* Active SSH Sessions (pts/*) */}
            <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-5 space-y-3">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                  Phiên SSH Đang Đăng nhập ({sshData.activeSessions.length})
                </h2>
                <span className="text-xs font-mono text-emerald-600 dark:text-emerald-400">
                  who / pts
                </span>
              </div>

              <div className="space-y-2.5">
                {sshData.activeSessions.map((sess) => (
                  <div
                    key={sess.id}
                    className="p-3 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60 space-y-1.5"
                  >
                    <div className="flex items-center justify-between text-xs">
                      <span className="font-mono font-bold text-slate-900 dark:text-white">
                        {sess.user}@{sess.pts} (PID {sess.pid})
                      </span>
                      {sess.pts !== 'pts/0' && (
                        <button
                          onClick={() => onTerminateSession(sess.id)}
                          className="text-rose-600 dark:text-rose-400 hover:underline inline-flex items-center gap-1 text-[11px] cursor-pointer"
                        >
                          <Unplug className="w-3 h-3" />
                          Ngắt kết nối
                        </button>
                      )}
                    </div>
                    <div className="text-xs font-mono text-slate-600 dark:text-slate-300">
                      IP: {sess.clientIp}
                    </div>
                    <div className="text-[11px] font-mono text-slate-500 truncate">
                      Client: {sess.clientSoftware}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </div>
        </div>
      )}

      {/* SUBTAB 1.5: PANEL TERMINAL NỘI BỘ (FP CLI 1-18) & TÀI KHOẢN ADMIN KHI INSTALL TRÊN VPS */}
      {subTab === 'fp_panel_cli' && (
        <div className="space-y-6">
          {/* Top Section: VPS Install Admin Account & Credentials Provisioning */}
          <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
            <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4 border-b border-slate-200 dark:border-slate-800 pb-4">
              <div>
                <div className="text-xs font-mono text-orange-600 dark:text-orange-400">
                  WEB PANEL ADMIN CREDENTIALS · /etc/ferrous-panel/admin.json · /www/server/panel/default.pl
                </div>
                <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 mt-0.5">
                  Tài khoản Admin Đăng Nhập Web Panel Khởi Tạo Khi Install Trên VPS
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                  Thiết lập tài khoản Quản trị viên (Admin Username &amp; Admin Password) dùng để đăng nhập vào trang Login của Panel, cổng Panel và đường dẫn an toàn (Security Entrance).
                </p>
              </div>
              <button
                type="button"
                onClick={() => onNavigate('rust-source')}
                className="px-3.5 py-2 text-xs font-semibold rounded-lg bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-800 dark:text-slate-200 inline-flex items-center gap-1.5 shrink-0 cursor-pointer"
              >
                <Code2 className="w-3.5 h-3.5 text-orange-500" />
                Xem mã nguồn ferrous-cli.sh &amp; install-ubuntu.sh
              </button>
            </div>

            <form
              onSubmit={handleSavePanelAdmin}
              className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4 items-end"
            >
              <div>
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                  Tên đăng nhập Admin Web (--admin-user)
                </label>
                <input
                  type="text"
                  required
                  value={adminUserInput}
                  onChange={(e) => setAdminUserInput(e.target.value)}
                  placeholder="admin_ferrous"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                  Mật khẩu Admin Web (--admin-pass)
                </label>
                <input
                  type="text"
                  required
                  value={adminPassInput}
                  onChange={(e) => setAdminPassInput(e.target.value)}
                  placeholder="Mật khẩu đăng nhập Web"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                  Tên miền riêng Panel (Custom Domain / fp 17)
                </label>
                <input
                  type="text"
                  value={customDomainInput}
                  onChange={(e) => setCustomDomainInput(e.target.value)}
                  placeholder="panel.domain.com (tùy chọn)"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                />
              </div>

              <div>
                <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                  Cổng Panel (--port) &amp; Đường dẫn an toàn
                </label>
                <div className="flex items-center gap-1.5">
                  <input
                    type="number"
                    min={1024}
                    max={65535}
                    value={panelPortInput}
                    onChange={(e) => setPanelPortInput(Number(e.target.value))}
                    className="w-20 px-2 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                  />
                  <input
                    type="text"
                    value={entranceInput}
                    onChange={(e) => setEntranceInput(e.target.value)}
                    placeholder="/ferrous_admin"
                    className="flex-1 min-w-0 px-2 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                  />
                </div>
              </div>

              <div>
                <button
                  type="submit"
                  disabled={savingAdminAccount}
                  className="w-full py-2 px-3 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 disabled:opacity-50 rounded-lg inline-flex items-center justify-center gap-1.5 cursor-pointer whitespace-nowrap"
                >
                  <Lock className="w-3.5 h-3.5" />
                  {savingAdminAccount
                    ? 'Đang lưu...'
                    : 'Cập nhật Admin & Domain'}
                </button>
              </div>
            </form>

            {/* One-line VPS Install Command with Custom Admin Login Credentials */}
            <div className="p-4 rounded-xl bg-slate-950 border border-slate-800 space-y-2">
              <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
                <span className="font-mono text-slate-400">
                  Lệnh cài đặt trực tiếp trên VPS kèm tạo Tài khoản Admin đăng nhập Web Panel:
                </span>
                <button
                  type="button"
                  onClick={() =>
                    handleCopyText('vps-install-cmd', vpsInstallCommandSnippet)
                  }
                  className="text-orange-400 hover:underline font-mono inline-flex items-center gap-1 cursor-pointer"
                >
                  {copiedId === 'vps-install-cmd' ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-400" /> Đã sao chép lệnh cài VPS
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" /> Sao chép lệnh Install VPS
                    </>
                  )}
                </button>
              </div>
              <pre className="font-mono text-xs text-emerald-400 overflow-x-auto select-all">
                {vpsInstallCommandSnippet}
              </pre>
            </div>
          </section>

          {/* Bottom Section: Interactive Panel Terminal Toolbox (fp / ferrous 1-16) */}
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
            {/* Left 7 cols: 16 Interactive Repair & Internal Config Buttons */}
            <section className="lg:col-span-7 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
              <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-slate-200 dark:border-slate-800 pb-3">
                <div>
                  <h3 className="text-sm font-bold text-slate-900 dark:text-slate-100">
                    Bảng Điều Khiển Sửa Chữa Nội Bộ Trên Terminal VPS (Lệnh tắt `fp` / `ferrous` 1–18)
                  </h3>
                  <p className="text-xs text-slate-500 dark:text-slate-400">
                    Tương tự lệnh <code className="font-mono">bt</code> của aaPanel: cho phép khôi phục mật khẩu Admin, đổi cổng Panel, đặt lại mật khẩu SQL và tự sửa lỗi kiến trúc 8 tầng ngay từ Terminal VPS.
                  </p>
                </div>
              </div>

              {/* Optional Parameter Input for Options 5, 6, 8, 9, 11 */}
              <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center gap-3">
                <div className="text-xs font-medium text-slate-700 dark:text-slate-300 shrink-0">
                  Tham số tùy chọn (cho mục 5, 6, 8, 9, 11):
                </div>
                <input
                  type="text"
                  value={fpOptionParam}
                  onChange={(e) => setFpOptionParam(e.target.value)}
                  placeholder="Nhập mật khẩu mới / username mới / port mới / đường dẫn entrance trước khi bấm..."
                  className="flex-1 px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white"
                />
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                {FP_CLI_OPTIONS.map((item) => (
                  <button
                    key={item.option}
                    type="button"
                    disabled={runningFpOption !== null}
                    onClick={() => handleTriggerFpOption(item.option)}
                    className="p-3 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/60 hover:border-orange-500/60 hover:bg-orange-50/30 dark:hover:bg-orange-950/20 text-left transition-colors flex items-start justify-between gap-2 cursor-pointer"
                  >
                    <div className="space-y-0.5 min-w-0">
                      <div className="text-xs font-bold text-slate-900 dark:text-slate-100 flex items-center gap-1.5">
                        <span className="px-1.5 py-0.5 rounded bg-slate-900 dark:bg-slate-800 text-orange-400 font-mono text-[11px]">
                          ({item.option})
                        </span>
                        <span className="truncate">{item.title}</span>
                      </div>
                      <div className="text-[11px] font-mono text-slate-500">
                        Lệnh VPS: {item.command}
                      </div>
                    </div>
                    <Play className="w-3.5 h-3.5 text-orange-600 dark:text-orange-400 shrink-0 mt-1" />
                  </button>
                ))}
              </div>
            </section>

            {/* Right 5 cols: Live Terminal Output of /usr/local/bin/fp */}
            <section className="lg:col-span-5 rounded-xl border border-slate-800 bg-slate-950 text-slate-100 overflow-hidden shadow-lg">
              <div className="px-4 py-3 bg-slate-900 border-b border-slate-800 flex items-center justify-between">
                <div className="flex items-center gap-2 text-xs font-mono">
                  <Terminal className="w-3.5 h-3.5 text-orange-400" />
                  <span className="font-semibold text-white">
                    /usr/local/bin/fp — VPS Internal Terminal Output
                  </span>
                </div>
                <span className="text-[11px] font-mono text-emerald-400">
                  root@vps:~# fp
                </span>
              </div>

              <div className="p-5 font-mono text-xs leading-relaxed space-y-4 max-h-[540px] overflow-y-auto">
                <pre className="text-emerald-300 whitespace-pre-wrap break-words">
                  {fpCliOutput}
                </pre>

                <div className="pt-3 border-t border-slate-800 text-[11px] text-slate-400 space-y-1">
                  <div>
                    * Trên máy chủ VPS thực tế, bạn chỉ cần SSH vào quyền root và gõ lệnh <code className="text-orange-400">fp</code> hoặc <code className="text-orange-400">ferrous</code> để mở bảng điều khiển này bất cứ lúc nào.
                  </div>
                  <div>
                    * Bạn cũng có thể chuyển sang tab <strong>Web SSH trên Panel</strong> và gõ <code className="text-orange-400">fp</code>, <code className="text-orange-400">fp default</code>, <code className="text-orange-400">fp repair</code> hoặc <code className="text-orange-400">fp 1</code>.. <code className="text-orange-400">fp 16</code>.
                  </div>
                </div>
              </div>
            </section>
          </div>
        </div>
      )}

      {/* SUBTAB 2: EXTERNAL SSH CLIENT CONFIG & ED25519 KEY MANAGER */}
      {subTab === 'external_clients' && (
        <div className="space-y-6">
          {/* External Software Connection Profiles (Termius, PuTTY, VS Code Remote-SSH, FileZilla) */}
          <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
            <div>
              <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                Hồ sơ Kết nối cho Phần mềm SSH Bên ngoài (External SSH Client Profiles)
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                Sử dụng thông số dưới đây để đăng nhập trực tiếp vào máy chủ Ubuntu từ máy tính cá nhân qua Termius, PuTTY, MobaXterm, VS Code Remote-SSH hoặc WinSCP/FileZilla.
              </p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {/* Card 1: Termius / PuTTY / MobaXterm */}
              <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/60 space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-slate-900 dark:text-white">
                    1. Termius / PuTTY / MobaXterm
                  </span>
                  <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400">
                    SSH-2.0 Ready
                  </span>
                </div>
                <div className="space-y-1.5 text-xs font-mono">
                  <div className="flex justify-between">
                    <span className="text-slate-500">Host / IP:</span>
                    <span className="font-semibold text-slate-900 dark:text-white select-all">
                      {sshData.serverHost}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-500">Port:</span>
                    <span className="font-semibold text-orange-600 dark:text-orange-400">
                      {sshData.config.port}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-500">Username:</span>
                    <span className="font-semibold text-slate-900 dark:text-white">
                      root
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-slate-500">Xác thực:</span>
                    <span className="text-emerald-600 dark:text-emerald-400">
                      Ed25519 Key / Password
                    </span>
                  </div>
                </div>
                <button
                  onClick={() => handleCopyText('putty-cmd', sshCliSnippet)}
                  className="w-full py-2 px-3 rounded-lg bg-slate-900 dark:bg-slate-800 hover:bg-slate-800 text-white text-xs font-mono inline-flex items-center justify-center gap-1.5 cursor-pointer"
                >
                  {copiedId === 'putty-cmd' ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-400" />
                      Đã sao chép lệnh SSH
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      Chép: {sshCliSnippet}
                    </>
                  )}
                </button>
              </div>

              {/* Card 2: VS Code Remote-SSH & Cursor */}
              <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/60 space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-slate-900 dark:text-white">
                    2. VS Code Remote-SSH (~/.ssh/config)
                  </span>
                  <button
                    onClick={() =>
                      handleCopyText('vscode-cfg', vscodeConfigSnippet)
                    }
                    className="text-xs font-mono text-orange-600 dark:text-orange-400 hover:underline inline-flex items-center gap-1 cursor-pointer"
                  >
                    {copiedId === 'vscode-cfg' ? (
                      <>
                        <Check className="w-3 h-3" /> Đã chép
                      </>
                    ) : (
                      <>
                        <Copy className="w-3 h-3" /> Sao chép
                      </>
                    )}
                  </button>
                </div>
                <pre className="p-2.5 rounded-lg bg-slate-950 text-slate-200 font-mono text-[11px] leading-relaxed overflow-x-auto">
                  {vscodeConfigSnippet}
                </pre>
              </div>

              {/* Card 3: FileZilla / WinSCP SFTP */}
              <div className="p-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/60 space-y-3">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-bold text-slate-900 dark:text-white">
                    3. SFTP Quản lý File (FileZilla / WinSCP)
                  </span>
                  <span className="text-[11px] font-mono text-sky-600 dark:text-sky-400">
                    internal-sftp
                  </span>
                </div>
                <p className="text-xs text-slate-500 dark:text-slate-400 leading-relaxed">
                  Truy cập trực tiếp cây thư mục <code className="font-mono">/www/wwwroot</code> qua giao thức SFTP mã hóa Ed25519 mà không cần cài thêm FTP truyền thống.
                </p>
                <div className="p-2.5 rounded-lg bg-slate-950 font-mono text-[11px] text-sky-400 break-all select-all">
                  {sftpUriSnippet}
                </div>
                <button
                  onClick={() => handleCopyText('sftp-uri', sftpUriSnippet)}
                  className="w-full py-2 px-3 rounded-lg bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 text-slate-900 dark:text-white text-xs font-medium inline-flex items-center justify-center gap-1.5 cursor-pointer"
                >
                  {copiedId === 'sftp-uri' ? (
                    <>
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                      Đã chép đường dẫn SFTP
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      Sao chép SFTP URI
                    </>
                  )}
                </button>
              </div>
            </div>
          </section>

          {/* OpenSSH Daemon (/etc/ssh/sshd_config) Editor + Ed25519 Key Generator */}
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
            {/* Left: /etc/ssh/sshd_config Form & Preview (6 cols) */}
            <form
              onSubmit={handleSaveSshdConfig}
              className="lg:col-span-6 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5"
            >
              <div className="flex items-center justify-between border-b border-slate-200 dark:border-slate-800 pb-4">
                <div>
                  <h3 className="text-base font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <ShieldCheck className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                    Cấu hình OpenSSH Daemon (/etc/ssh/sshd_config)
                  </h3>
                  <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                    Tinh chỉnh cổng SSH, quyền đăng nhập mật khẩu/khóa công khai và TCP Forwarding cho phần mềm ngoài
                  </p>
                </div>
                <button
                  type="submit"
                  disabled={savingConfig}
                  className="px-4 py-2 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 disabled:opacity-50 rounded-lg inline-flex items-center gap-1.5 shrink-0 cursor-pointer"
                >
                  <RefreshCw
                    className={`w-3.5 h-3.5 ${savingConfig ? 'animate-spin' : ''}`}
                  />
                  Lưu &amp; Reload SSHD
                </button>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                    Cổng SSH (Port — Tự động đồng bộ UFW)
                  </label>
                  <input
                    type="number"
                    min={22}
                    max={65535}
                    value={portInput}
                    onChange={(e) => setPortInput(Number(e.target.value))}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                    Chính sách Đăng nhập Root (PermitRootLogin)
                  </label>
                  <select
                    value={rootLoginPolicy}
                    onChange={(e) =>
                      setRootLoginPolicy(
                        e.target.value as SshDaemonConfig['permitRootLogin']
                      )
                    }
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                  >
                    <option value="prohibit-password">
                      prohibit-password (Chỉ cho phép SSH Key)
                    </option>
                    <option value="yes">
                      yes (Cho phép cả Mật khẩu &amp; SSH Key)
                    </option>
                    <option value="no">no (Khóa đăng nhập trực tiếp Root)</option>
                  </select>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                    Giới hạn thử mật khẩu sai (MaxAuthTries)
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={10}
                    value={maxAuthTries}
                    onChange={(e) => setMaxAuthTries(Number(e.target.value))}
                    className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-white"
                  />
                </div>

                <div className="space-y-2 pt-1">
                  <label className="flex items-center gap-2 text-xs font-medium text-slate-700 dark:text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={pubkeyAuth}
                      onChange={(e) => setPubkeyAuth(e.target.checked)}
                      className="rounded border-slate-300 text-orange-600 focus:ring-orange-500"
                    />
                    <span>Bật PubkeyAuthentication (Ed25519 / RSA)</span>
                  </label>
                  <label className="flex items-center gap-2 text-xs font-medium text-slate-700 dark:text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={passwordAuth}
                      onChange={(e) => setPasswordAuth(e.target.checked)}
                      className="rounded border-slate-300 text-orange-600 focus:ring-orange-500"
                    />
                    <span>Cho phép PasswordAuthentication (PuTTY / Termius)</span>
                  </label>
                  <label className="flex items-center gap-2 text-xs font-medium text-slate-700 dark:text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={tcpForwarding}
                      onChange={(e) => setTcpForwarding(e.target.checked)}
                      className="rounded border-slate-300 text-orange-600 focus:ring-orange-500"
                    />
                    <span>Cho phép AllowTcpForwarding (VS Code Remote-SSH)</span>
                  </label>
                </div>
              </div>

              <div className="space-y-2">
                <div className="text-xs font-semibold text-slate-700 dark:text-slate-300">
                  Nội dung tệp /etc/ssh/sshd_config được sinh bởi Rust Daemon:
                </div>
                <pre className="p-4 rounded-lg bg-slate-950 text-slate-200 font-mono text-[11px] leading-relaxed overflow-x-auto max-h-64">
                  {sshData.config.sshdConfigPreview}
                </pre>
              </div>
            </form>

            {/* Right: Ed25519 Keypair Generator & ~/.ssh/authorized_keys Manager (6 cols) */}
            <div className="lg:col-span-6 space-y-6">
              <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-base font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                      <KeyRound className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                      Khóa SSH Ủy quyền (~/.ssh/authorized_keys)
                    </h3>
                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                      Tạo cặp khóa Ed25519 mới để tải tệp <code className="font-mono">.pem</code> về máy hoặc dán Public Key có sẵn của bạn
                    </p>
                  </div>
                </div>

                {/* 1-Click Generate Keypair or Paste Key */}
                <form
                  onSubmit={handleAddManualPublicKey}
                  className="p-4 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/50 space-y-3"
                >
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                        Tên thiết bị / Nhãn khóa
                      </label>
                      <input
                        type="text"
                        value={newKeyLabel}
                        onChange={(e) => setNewKeyLabel(e.target.value)}
                        placeholder="VD: laptop-devops-ed25519"
                        className="w-full px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white"
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                        Phần mềm SSH sử dụng
                      </label>
                      <input
                        type="text"
                        value={newKeyClient}
                        onChange={(e) => setNewKeyClient(e.target.value)}
                        placeholder="Termius / PuTTY / VS Code"
                        className="w-full px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                      Dán khóa công khai có sẵn (ssh-ed25519 / ssh-rsa) hoặc nhấn &quot;Sinh cặp khóa Ed25519&quot;
                    </label>
                    <textarea
                      rows={2}
                      value={newPublicKeyText}
                      onChange={(e) => setNewPublicKeyText(e.target.value)}
                      placeholder="ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... user@laptop"
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white"
                    />
                  </div>

                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <button
                      type="button"
                      onClick={handleGenerateEd25519}
                      disabled={generatingKey}
                      className="px-3.5 py-2 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                    >
                      <KeyRound className="w-3.5 h-3.5" />
                      {generatingKey
                        ? 'Đang chạy ssh-keygen...'
                        : 'Sinh cặp khóa Ed25519 & Tải .PEM'}
                    </button>

                    <button
                      type="submit"
                      className="px-3.5 py-2 text-xs font-semibold text-slate-800 dark:text-slate-100 bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      Thêm Public Key đã dán
                    </button>
                  </div>
                </form>

                {/* Newly Generated Private Key Download Box */}
                {generatedKeypair && (
                  <div className="p-4 rounded-lg border border-emerald-500/50 bg-emerald-50/60 dark:bg-emerald-950/20 space-y-3">
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-bold text-emerald-800 dark:text-emerald-300">
                        Khóa riêng tư Ed25519 vừa tạo ({generatedKeypair.label})
                      </span>
                      <button
                        onClick={() =>
                          handleDownloadPrivateKey(generatedKeypair)
                        }
                        className="px-3 py-1.5 rounded-md bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-semibold inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        <Download className="w-3.5 h-3.5" />
                        Tải tệp khóa .PEM cho Termius / PuTTY
                      </button>
                    </div>
                    <pre className="p-3 rounded bg-slate-950 text-emerald-400 font-mono text-[11px] overflow-x-auto">
                      {generatedKeypair.privateKeyPem}
                    </pre>
                    <div className="text-[11px] font-mono text-slate-600 dark:text-slate-400">
                      Lệnh kết nối: <code className="select-all">{generatedKeypair.cliCommand}</code>
                    </div>
                  </div>
                )}

                {/* Existing Authorized Keys List */}
                <div className="space-y-2.5">
                  {sshData.authorizedKeys.map((item) => (
                    <div
                      key={item.id}
                      className="p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/60 space-y-1.5"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-bold text-slate-900 dark:text-white">
                          {item.label}
                        </span>
                        <div className="flex items-center gap-2">
                          <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400">
                            {item.algorithm}
                          </span>
                          <button
                            onClick={() => onDeleteKey(item.id)}
                            className="text-rose-600 dark:text-rose-400 hover:underline text-xs inline-flex items-center gap-1 cursor-pointer"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                            Xóa
                          </button>
                        </div>
                      </div>
                      <div className="text-[11px] font-mono text-slate-500 truncate">
                        {item.fingerprint} · {item.clientHint} · Thêm lúc {item.addedAt}
                      </div>
                      <div className="text-[11px] font-mono text-slate-600 dark:text-slate-400 truncate select-all">
                        {item.publicKey}
                      </div>
                    </div>
                  ))}
                </div>
              </section>
            </div>
          </div>
        </div>
      )}

      {/* SUBTAB 3: 100% NATIVE RUST MODULE CONVERSION ARCHITECTURE */}
      {subTab === 'rust_modules' && (
        <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          <div className="p-6 border-b border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                Bảng Chuyển Đổi Toàn Bộ 12/12 Phân Hệ Backend Sang Ngôn Ngữ Rust (Actix-Web + Tokio)
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                Toàn bộ các tác vụ quản trị Server (OpenSSH, Web PTY, 1-Click Store, Git Deploy, Optimizer, Nginx, Systemd, SQL, Mail, File Manager, cPanel, UFW) đều đã được viết bằng Rust nguyên bản.
              </p>
            </div>
            <button
              onClick={() => onNavigate('rust-source')}
              className="px-4 py-2 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 shrink-0 cursor-pointer"
            >
              <Code2 className="w-3.5 h-3.5" />
              Xem Mã Nguồn 12 Tệp .RS
            </button>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-slate-200 dark:border-slate-800 text-xs font-medium text-slate-500 dark:text-slate-400 bg-slate-50/60 dark:bg-slate-950/40">
                  <th className="py-3 px-5">Phân hệ (Rust Native Module)</th>
                  <th className="py-3 px-4">Tệp Mã nguồn Rust</th>
                  <th className="py-3 px-4">Thay thế Công nghệ Cũ</th>
                  <th className="py-3 px-4">Crates Sử dụng</th>
                  <th className="py-3 px-4 text-right">Bộ nhớ / Độ trễ</th>
                  <th className="py-3 px-5 text-right">Trạng thái</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs">
                {sshData.rustModules.map((mod) => (
                  <tr
                    key={mod.id}
                    className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40 transition-colors"
                  >
                    <td className="py-3.5 px-5">
                      <div className="font-bold text-slate-900 dark:text-slate-100">
                        {mod.moduleName}
                      </div>
                      <div className="text-slate-500 dark:text-slate-400 mt-0.5">
                        {mod.description}
                      </div>
                    </td>
                    <td className="py-3.5 px-4 font-mono font-semibold text-orange-600 dark:text-orange-400 whitespace-nowrap">
                      {mod.rustFile}
                    </td>
                    <td className="py-3.5 px-4 font-mono text-slate-500 line-through">
                      {mod.replacedLegacyStack}
                    </td>
                    <td className="py-3.5 px-4 font-mono text-slate-700 dark:text-slate-300">
                      {mod.crateDependencies}
                    </td>
                    <td className="py-3.5 px-4 text-right font-mono tabular-nums text-slate-800 dark:text-slate-200 whitespace-nowrap">
                      <div>{mod.memoryFootprintKb} KB RAM</div>
                      <div className="text-emerald-600 dark:text-emerald-400">
                        ~{mod.latencyMicros} µs
                      </div>
                    </td>
                    <td className="py-3.5 px-5 text-right font-mono text-emerald-600 dark:text-emerald-400 font-semibold whitespace-nowrap">
                      RUST COMPILED
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
};
