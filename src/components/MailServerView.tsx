import React, { useState } from 'react';
import {
  Mail,
  Plus,
  Trash2,
  ShieldCheck,
  Send,
  Inbox,
  ArrowRight,
  Copy,
  Check,
  KeyRound,
  Server,
  RefreshCw,
} from 'lucide-react';
import { MailServerData, MailMessageItem } from '../types';

interface MailServerViewProps {
  mailData: MailServerData;
  onAddDomain: (domain: string) => Promise<void>;
  onCreateMailbox: (payload: { email: string; domain: string; displayName: string; quotaMb: number }) => Promise<void>;
  onDeleteMailbox: (id: string) => Promise<void>;
  onCreateForwarder: (payload: { sourceEmail: string; destinationEmail: string; domain: string }) => Promise<void>;
  onDeleteForwarder: (id: string) => Promise<void>;
  onSendTestMail: (payload: { from: string; to: string; subject: string; body: string }) => Promise<void>;
}

export const MailServerView: React.FC<MailServerViewProps> = ({
  mailData,
  onAddDomain,
  onCreateMailbox,
  onDeleteMailbox,
  onCreateForwarder,
  onDeleteForwarder,
  onSendTestMail,
}) => {
  const [subTab, setSubTab] = useState<'mailboxes' | 'domains' | 'forwarders' | 'webmail'>('mailboxes');

  // Mailbox creation state
  const [localPart, setLocalPart] = useState('');
  const [selectedDomain, setSelectedDomain] = useState(
    mailData.domains[0]?.domain || ''
  );
  const [displayName, setDisplayName] = useState('');
  const [quotaMb, setQuotaMb] = useState(2048);
  const [creatingBox, setCreatingBox] = useState(false);

  // Domain creation state
  const [newDomain, setNewDomain] = useState('');
  const [addingDomain, setAddingDomain] = useState(false);
  const [activeDnsDomainId, setActiveDnsDomainId] = useState<string>(
    mailData.domains[0]?.id || ''
  );
  const [copiedRecord, setCopiedRecord] = useState<string | null>(null);

  // Forwarder state
  const [fwdSource, setFwdSource] = useState('');
  const [fwdDest, setFwdDest] = useState('');
  const [fwdDomain, setFwdDomain] = useState(
    mailData.domains[0]?.domain || ''
  );
  const [creatingFwd, setCreatingFwd] = useState(false);

  // Webmail & SMTP Test state
  const [mailFolder, setMailFolder] = useState<'inbox' | 'sent'>('inbox');
  const [selectedMessage, setSelectedMessage] =
    useState<MailMessageItem | null>(mailData.messages[0] || null);
  const [composeFrom, setComposeFrom] = useState(
    mailData.mailboxes[0]?.email || ''
  );
  const [composeTo, setComposeTo] = useState('');
  const [composeSubject, setComposeSubject] = useState('');
  const [composeBody, setComposeBody] = useState('');
  const [sendingMail, setSendingMail] = useState(false);
  const [sendSuccessBanner, setSendSuccessBanner] = useState<string | null>(
    null
  );

  const activeDnsDomain =
    mailData.domains.find((d) => d.id === activeDnsDomainId) ||
    mailData.domains[0] ||
    null;

  const handleCopy = (key: string, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedRecord(key);
    setTimeout(() => setCopiedRecord(null), 1800);
  };

  const handleCreateBoxSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!localPart.trim() || !selectedDomain) return;
    setCreatingBox(true);
    try {
      const fullEmail = localPart.includes('@') ? localPart.trim() : `${localPart.trim()}@${selectedDomain}`;
      await onCreateMailbox({
        email: fullEmail,
        domain: selectedDomain,
        displayName: displayName.trim() || localPart.trim(),
        quotaMb,
      });
      setLocalPart('');
      setDisplayName('');
    } finally {
      setCreatingBox(false);
    }
  };

  const handleAddDomainSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newDomain.trim()) return;
    setAddingDomain(true);
    try {
      await onAddDomain(newDomain.trim().toLowerCase());
      setNewDomain('');
    } finally {
      setAddingDomain(false);
    }
  };

  const handleCreateForwarderSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!fwdSource.trim() || !fwdDest.trim()) return;
    setCreatingFwd(true);
    try {
      const src = fwdSource.includes('@') ? fwdSource.trim() : `${fwdSource.trim()}@${fwdDomain}`;
      await onCreateForwarder({
        sourceEmail: src,
        destinationEmail: fwdDest.trim(),
        domain: fwdDomain,
      });
      setFwdSource('');
      setFwdDest('');
    } finally {
      setCreatingFwd(false);
    }
  };

  const handleSendMailSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!composeTo.trim() || !composeSubject.trim()) return;
    setSendingMail(true);
    setSendSuccessBanner(null);
    try {
      await onSendTestMail({
        from: composeFrom,
        to: composeTo.trim(),
        subject: composeSubject.trim(),
        body: composeBody,
      });
      setSendSuccessBanner(`Đã ký OpenDKIM (RSA-2048) và chuyển tiếp qua Postfix tới ${composeTo.trim()}`);
      setMailFolder('sent');
    } finally {
      setSendingMail(false);
    }
  };

  const filteredMessages = mailData.messages.filter((m) => m.folder === mailFolder);

  return (
    <div className="space-y-6">
      {/* Header & Stack Summary */}
      <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-6 shadow-xs">
        <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4 pb-5 border-b border-slate-100 dark:border-slate-800/80">
          <div>
            <h1 className="text-xl font-bold tracking-tight text-slate-900 dark:text-white flex items-center gap-2.5">
              <Mail className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
              Quản trị Mail Server &amp; Webmail (Postfix · Dovecot · OpenDKIM · Rspamd)
            </h1>
            <p className="text-sm text-slate-600 dark:text-slate-400 mt-1">
               Quản lý hộp thư tên miền riêng, tự động sinh khóa DKIM 2048-bit, lọc Spam Rspamd và trình đọc Webmail tích hợp.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-3 text-xs font-mono text-slate-600 dark:text-slate-300 bg-slate-50 dark:bg-slate-950 px-3.5 py-2 rounded-lg border border-slate-200 dark:border-slate-800 tabular-nums">
            <span>{mailData.postfixVersion}</span>
            <span className="text-slate-300 dark:text-slate-700">·</span>
            <span>{mailData.dovecotVersion}</span>
            <span className="text-slate-300 dark:text-slate-700">·</span>
            <span>{mailData.rspamdVersion}</span>
            <span className="text-slate-300 dark:text-slate-700">·</span>
            <span className="text-emerald-600 dark:text-emerald-400 font-semibold">
              Queue: {mailData.queueCount} msg · Đã gửi: {mailData.sentToday}
            </span>
          </div>
        </div>

        {/* Sub-navigation */}
        <div className="flex flex-wrap items-center justify-between gap-4 pt-4">
          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={() => setSubTab('mailboxes')}
              className={`px-3.5 py-2 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                subTab === 'mailboxes'
                  ? 'bg-slate-900 dark:bg-emerald-500 text-white dark:text-slate-950'
                  : 'bg-slate-100 dark:bg-slate-800/70 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-800'
              }`}
            >
              Hộp thư Điện tử ({mailData.mailboxes.length})
            </button>
            <button
              onClick={() => setSubTab('domains')}
              className={`px-3.5 py-2 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                subTab === 'domains'
                  ? 'bg-slate-900 dark:bg-emerald-500 text-white dark:text-slate-950'
                  : 'bg-slate-100 dark:bg-slate-800/70 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-800'
              }`}
            >
              Tên miền &amp; Bản ghi DKIM / SPF / DMARC ({mailData.domains.length})
            </button>
            <button
              onClick={() => setSubTab('forwarders')}
              className={`px-3.5 py-2 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                subTab === 'forwarders'
                  ? 'bg-slate-900 dark:bg-emerald-500 text-white dark:text-slate-950'
                  : 'bg-slate-100 dark:bg-slate-800/70 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-800'
              }`}
            >
              Chuyển tiếp Thư ({mailData.forwarders.length})
            </button>
            <button
              onClick={() => setSubTab('webmail')}
              className={`px-3.5 py-2 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                subTab === 'webmail'
                  ? 'bg-slate-900 dark:bg-emerald-500 text-white dark:text-slate-950'
                  : 'bg-slate-100 dark:bg-slate-800/70 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-800'
              }`}
            >
              Webmail Roundcube &amp; Kiểm thử SMTP ({mailData.messages.length})
            </button>
          </div>

          <div className="text-xs font-mono text-slate-500 dark:text-slate-400 tabular-nums">
            SMTP: 25 / 465 (SSL) / 587 (STARTTLS) · IMAP: 993 (SSL) · POP3: 995
          </div>
        </div>
      </div>

      {/* TAB 1: MAILBOXES */}
      {subTab === 'mailboxes' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          {/* Create Mailbox Form */}
          <div className="lg:col-span-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 shadow-xs space-y-4">
            <h2 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
              <Plus className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
              Khởi tạo Hộp thư Mới (Dovecot Virtual User)
            </h2>
            <form onSubmit={handleCreateBoxSubmit} className="space-y-3.5">
              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Địa chỉ Email &amp; Tên miền
                </label>
                <div className="flex items-center gap-1.5">
                  <input
                    type="text"
                    required
                    value={localPart}
                    onChange={(e) => setLocalPart(e.target.value)}
                    placeholder="contact"
                    className="w-full px-3 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
                  />
                  <span className="text-slate-400 font-mono text-sm">@</span>
                  <select
                    value={selectedDomain}
                    onChange={(e) => setSelectedDomain(e.target.value)}
                    className="px-2.5 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
                  >
                    {mailData.domains.length === 0 ? (
                      <option value="">(Chưa có domain - vui lòng thêm domain trước)</option>
                    ) : (
                      mailData.domains.map((d) => (
                        <option key={d.id} value={d.domain}>
                          {d.domain}
                        </option>
                      ))
                    )}
                  </select>
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Tên hiển thị (Display Name)
                </label>
                <input
                  type="text"
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                  placeholder="Phòng Kỹ thuật / Support"
                  className="w-full px-3 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white focus:outline-none focus:border-emerald-500"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Dung lượng Hộp thư (Quota Maildir)
                </label>
                <select
                  value={quotaMb}
                  onChange={(e) => setQuotaMb(Number(e.target.value))}
                  className="w-full px-3 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
                >
                  <option value={1024}>1,024 MB (1 GB)</option>
                  <option value={2048}>2,048 MB (2 GB)</option>
                  <option value={5120}>5,120 MB (5 GB)</option>
                  <option value={10240}>10,240 MB (10 GB)</option>
                </select>
              </div>

              <button
                type="submit"
                disabled={creatingBox}
                className="w-full py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-semibold text-xs rounded-lg transition-colors cursor-pointer"
              >
                {creatingBox ? 'Đang tạo Maildir & mã hóa Argon2id...' : 'Tạo Hộp thư Điện tử'}
              </button>
            </form>

            {/* Client Configuration Box */}
            <div className="pt-4 border-t border-slate-100 dark:border-slate-800 space-y-2 text-xs">
              <div className="font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-1.5">
                <Server className="w-3.5 h-3.5 text-emerald-500" />
                Thông số kết nối Outlook / Thunderbird / Apple Mail
              </div>
              <div className="bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg p-3 font-mono text-[11px] space-y-1 text-slate-600 dark:text-slate-400 tabular-nums">
                <div>IMAP Host: mail.{selectedDomain} (Port 993 · SSL/TLS)</div>
                <div>SMTP Host: mail.{selectedDomain} (Port 465 SSL / 587 STARTTLS)</div>
                <div>Thư mục lưu trữ: /var/vmail/{selectedDomain}/</div>
              </div>
            </div>
          </div>

          {/* Mailbox Table */}
          <div className="lg:col-span-8 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl overflow-hidden shadow-xs">
            <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <h2 className="text-sm font-bold text-slate-900 dark:text-white">
                Danh sách Tài khoản Hộp thư Hoạt động ({mailData.mailboxes.length})
              </h2>
              <button
                onClick={() => setSubTab('webmail')}
                className="text-xs font-semibold text-emerald-600 dark:text-emerald-400 hover:underline flex items-center gap-1 cursor-pointer"
              >
                Mở Webmail Roundcube <ArrowRight className="w-3.5 h-3.5" />
              </button>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-slate-800 text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 bg-slate-50/60 dark:bg-slate-950/50">
                    <th className="py-3 px-4">Tài khoản Email</th>
                    <th className="py-3 px-4">Dung lượng Maildir</th>
                    <th className="py-3 px-4">Giao thức</th>
                    <th className="py-3 px-4">Đăng nhập cuối</th>
                    <th className="py-3 px-4 text-right">Thao tác</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs">
                  {mailData.mailboxes.length === 0 ? (
                    <tr>
                      <td
                        colSpan={5}
                        className="py-12 text-center text-slate-500 dark:text-slate-400"
                      >
                        <Mail className="w-8 h-8 mx-auto mb-2 opacity-40 text-slate-400" />
                        <div className="font-semibold text-xs text-slate-700 dark:text-slate-300">
                          Chưa có hộp thư nào được tạo
                        </div>
                        <div className="text-[11px] mt-1 text-slate-400">
                          Sử dụng form bên trái để tạo tài khoản email đầu tiên.
                        </div>
                      </td>
                    </tr>
                  ) :
                    mailData.mailboxes.map((box) => {
                    const pct = Math.min(100, Math.round((box.usedMb / box.quotaMb) * 100));
                    return (
                      <tr key={box.id} className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40 transition-colors">
                        <td className="py-3.5 px-4">
                          <div className="font-mono font-semibold text-slate-900 dark:text-white">{box.email}</div>
                          <div className="text-slate-500 dark:text-slate-400 text-[11px] mt-0.5">{box.displayName}</div>
                        </td>
                        <td className="py-3.5 px-4 tabular-nums">
                          <div className="flex items-center justify-between text-[11px] font-mono text-slate-600 dark:text-slate-300 mb-1">
                            <span>{box.usedMb} MB</span>
                            <span>/ {box.quotaMb} MB ({pct}%)</span>
                          </div>
                          <div className="w-36 h-1.5 bg-slate-200 dark:bg-slate-800 rounded-full overflow-hidden">
                            <div
                              className="h-full bg-emerald-500 rounded-full"
                              style={{ width: `${Math.max(4, pct)}%` }}
                            />
                          </div>
                        </td>
                        <td className="py-3.5 px-4 font-mono text-[11px] text-slate-600 dark:text-slate-300 tabular-nums">
                          {box.imapServer} · {box.smtpServer}
                        </td>
                        <td className="py-3.5 px-4 font-mono text-[11px] text-slate-500 dark:text-slate-400 tabular-nums">
                          {box.lastLogin}
                        </td>
                        <td className="py-3.5 px-4 text-right">
                          <div className="inline-flex items-center gap-2">
                            <button
                              onClick={() => {
                                setComposeFrom(box.email);
                                setSubTab('webmail');
                              }}
                              className="px-2.5 py-1 text-[11px] font-medium bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-200 rounded hover:bg-emerald-500/15 hover:text-emerald-600 dark:hover:text-emerald-400 transition-colors cursor-pointer"
                            >
                              Webmail
                            </button>
                            <button
                              onClick={() => onDeleteMailbox(box.id)}
                              className="p-1.5 text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors cursor-pointer"
                              title="Xóa hộp thư"
                            >
                              <Trash2 className="w-4 h-4" />
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* TAB 2: DOMAINS & DKIM / SPF / DMARC */}
      {subTab === 'domains' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 shadow-xs space-y-5">
            <div>
              <h2 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
                <KeyRound className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                Thêm Tên miền Mail &amp; Sinh khóa OpenDKIM 2048-bit
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                Hệ thống tự động chạy <code className="font-mono">opendkim-genkey -b 2048</code> và cấu hình bảng ký số cho Postfix.
              </p>
            </div>

            <form onSubmit={handleAddDomainSubmit} className="flex gap-2">
              <input
                type="text"
                required
                value={newDomain}
                onChange={(e) => setNewDomain(e.target.value)}
                placeholder="vd: maildomain.vn"
                className="flex-1 px-3 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
              />
              <button
                type="submit"
                disabled={addingDomain}
                className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white text-xs font-semibold rounded-lg transition-colors cursor-pointer shrink-0"
              >
                {addingDomain ? 'Đang tạo khóa...' : 'Thêm Tên miền'}
              </button>
            </form>

            <div className="divide-y divide-slate-200 dark:divide-slate-800 border border-slate-200 dark:border-slate-800 rounded-lg overflow-hidden">
              {mailData.domains.length === 0 ? (
                <div className="p-6 text-center text-slate-500 dark:text-slate-400">
                  <div className="text-xs font-semibold">Chưa có tên miền nào</div>
                  <div className="text-[11px] mt-1 text-slate-400">
                    Thêm tên miền ở form bên trên để bắt đầu.
                  </div>
                </div>
              ) : (
                mailData.domains.map((dom) => (
                  <button
                    key={dom.id}
                    onClick={() => setActiveDnsDomainId(dom.id)}
                    className={`w-full text-left p-3.5 transition-colors flex items-center justify-between cursor-pointer ${
                      activeDnsDomain?.id === dom.id
                        ? 'bg-emerald-50/70 dark:bg-emerald-950/30'
                        : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                    }`}
                  >
                    <div>
                      <div className="font-mono font-bold text-sm text-slate-900 dark:text-white">
                        {dom.domain}
                      </div>
                      <div className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5 tabular-nums">
                        {dom.mailboxesCount} hộp thư · Selector: {dom.dkimSelector} ·
                        Catch-All:{' '}
                        <span className="font-mono">
                          {dom.catchAllAddress || 'Tắt'}
                        </span>
                      </div>
                    </div>
                    <div className="text-right font-mono text-[11px] text-emerald-600 dark:text-emerald-400 font-semibold">
                      DKIM · SPF · DMARC
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>

          {/* DNS Records Inspector */}
          {activeDnsDomain ? (
            <div className="lg:col-span-7 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 shadow-xs space-y-4">
              <div className="flex items-center justify-between border-b border-slate-200 dark:border-slate-800 pb-3.5">
                <div>
                  <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                    Bản ghi Xác thực DNS bắt buộc cho <span className="font-mono text-emerald-600 dark:text-emerald-400">{activeDnsDomain.domain}</span>
                  </h3>
                  <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                    Sao chép các bản ghi dưới đây vào Cloudflare hoặc BIND9 DNS Zone để đạt điểm 10/10 Mail-Tester.
                  </p>
                </div>
                <span className="text-xs font-mono text-emerald-600 dark:text-emerald-400 font-semibold flex items-center gap-1">
                  <ShieldCheck className="w-4 h-4" /> Đã xác thực DNS
                </span>
              </div>

              <div className="space-y-3.5 text-xs">
                {/* MX Record */}
                <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="font-semibold text-slate-800 dark:text-slate-200">
                      1. Bản ghi MX (Mail Exchange)
                    </span>
                    <span className="font-mono text-[11px] text-emerald-600 dark:text-emerald-400">Priority: 10 · PASS</span>
                  </div>
                  <div className="font-mono text-[11px] text-slate-600 dark:text-slate-300 break-all">
                    Host: <strong className="text-slate-900 dark:text-white">@</strong> → Value:{' '}
                    <strong className="text-slate-900 dark:text-white">mail.{activeDnsDomain.domain}</strong>
                  </div>
                </div>

                {/* SPF Record */}
                <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="font-semibold text-slate-800 dark:text-slate-200">
                      2. Bản ghi TXT — SPF (Chống giả mạo IP gửi thư)
                    </span>
                    <button
                      onClick={() =>
                        handleCopy(
                          'spf',
                          `v=spf1 a mx ip4:103.145.88.210 include:_spf.${activeDnsDomain.domain} ~all`
                        )
                      }
                      className="inline-flex items-center gap-1 text-[11px] font-mono text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                    >
                      {copiedRecord === 'spf' ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                      {copiedRecord === 'spf' ? 'Đã chép' : 'Sao chép'}
                    </button>
                  </div>
                  <div className="font-mono text-[11px] text-slate-700 dark:text-slate-300 bg-white dark:bg-slate-900 p-2.5 rounded border border-slate-200 dark:border-slate-800 break-all">
                    v=spf1 a mx ip4:103.145.88.210 include:_spf.{activeDnsDomain.domain} ~all
                  </div>
                </div>

                {/* DKIM Record */}
                <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="font-semibold text-slate-800 dark:text-slate-200">
                      3. Bản ghi TXT — OpenDKIM 2048-bit ({activeDnsDomain.dkimSelector}._domainkey)
                    </span>
                    <button
                      onClick={() => handleCopy('dkim', activeDnsDomain.dkimPublicKey)}
                      className="inline-flex items-center gap-1 text-[11px] font-mono text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                    >
                      {copiedRecord === 'dkim' ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                      {copiedRecord === 'dkim' ? 'Đã chép' : 'Sao chép'}
                    </button>
                  </div>
                  <div className="font-mono text-[11px] text-slate-700 dark:text-slate-300 bg-white dark:bg-slate-900 p-2.5 rounded border border-slate-200 dark:border-slate-800 break-all">
                    {activeDnsDomain.dkimPublicKey}
                  </div>
                </div>

                {/* DMARC Record */}
                <div className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-1.5">
                  <div className="flex items-center justify-between">
                    <span className="font-semibold text-slate-800 dark:text-slate-200">
                      4. Bản ghi TXT — DMARC (_dmarc.{activeDnsDomain.domain})
                    </span>
                    <button
                      onClick={() =>
                        handleCopy(
                          'dmarc',
                          `v=DMARC1; p=quarantine; rua=mailto:postmaster@${activeDnsDomain.domain}; pct=100; adkim=s; aspf=s`
                        )
                      }
                      className="inline-flex items-center gap-1 text-[11px] font-mono text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                    >
                      {copiedRecord === 'dmarc' ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                      {copiedRecord === 'dmarc' ? 'Đã chép' : 'Sao chép'}
                    </button>
                  </div>
                  <div className="font-mono text-[11px] text-slate-700 dark:text-slate-300 bg-white dark:bg-slate-900 p-2.5 rounded border border-slate-200 dark:border-slate-800 break-all">
                    v=DMARC1; p=quarantine; rua=mailto:postmaster@{activeDnsDomain.domain}; pct=100; adkim=s; aspf=s
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="lg:col-span-7 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-12 text-center text-slate-500 dark:text-slate-400">
              <ShieldCheck className="w-10 h-10 mx-auto mb-2 opacity-30 text-slate-400" />
              <div className="text-xs font-semibold">Chưa có bản ghi DNS để hiển thị</div>
              <div className="text-[11px] mt-1 text-slate-400">
                Vui lòng thêm tên miền mail để hệ thống tự động sinh bản ghi SPF, DKIM 2048-bit và DMARC.
              </div>
            </div>
          )}
        </div>
      )}

      {/* TAB 3: FORWARDERS */}
      {subTab === 'forwarders' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <div className="lg:col-span-4 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 shadow-xs space-y-4">
            <h2 className="text-sm font-bold text-slate-900 dark:text-white">
              Tạo Chuyển tiếp Thư (Postfix Virtual Alias)
            </h2>
            <form onSubmit={handleCreateForwarderSubmit} className="space-y-3.5">
              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Địa chỉ nhận ban đầu (Alias)
                </label>
                <div className="flex items-center gap-1.5">
                  <input
                    type="text"
                    required
                    value={fwdSource}
                    onChange={(e) => setFwdSource(e.target.value)}
                    placeholder="sales"
                    className="w-full px-3 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
                  />
                  <span className="text-slate-400 font-mono text-sm">@</span>
                  <select
                    value={fwdDomain}
                    onChange={(e) => setFwdDomain(e.target.value)}
                    className="px-2.5 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
                  >
                    {mailData.domains.map((d) => (
                      <option key={d.id} value={d.domain}>
                        {d.domain}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Chuyển tiếp tới (Destination Email)
                </label>
                <input
                  type="email"
                  required
                  value={fwdDest}
                  onChange={(e) => setFwdDest(e.target.value)}
                  placeholder="admin@api.ferrous.vn"
                  className="w-full px-3 py-2 text-sm bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white font-mono focus:outline-none focus:border-emerald-500"
                />
              </div>

              <button
                type="submit"
                disabled={creatingFwd}
                className="w-full py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-semibold text-xs rounded-lg transition-colors cursor-pointer"
              >
                {creatingFwd ? 'Đang cập nhật /etc/postfix/virtual...' : 'Thêm Quy tắc Chuyển tiếp'}
              </button>
            </form>
          </div>

          <div className="lg:col-span-8 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl overflow-hidden shadow-xs">
            <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800">
              <h2 className="text-sm font-bold text-slate-900 dark:text-white">
                Danh sách Chuyển tiếp Thư đang hoạt động ({mailData.forwarders.length})
              </h2>
            </div>
            <table className="w-full text-left border-collapse">
              <thead>
                <tr className="border-b border-slate-200 dark:border-slate-800 text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 bg-slate-50/60 dark:bg-slate-950/50">
                  <th className="py-3 px-4">Địa chỉ Nguồn</th>
                  <th className="py-3 px-4">Chuyển tiếp Tới</th>
                  <th className="py-3 px-4">Tên miền</th>
                  <th className="py-3 px-4 text-right">Xóa</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs">
                {mailData.forwarders.map((f) => (
                  <tr key={f.id} className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
                    <td className="py-3.5 px-4 font-mono font-semibold text-slate-900 dark:text-white">
                      {f.sourceEmail}
                    </td>
                    <td className="py-3.5 px-4 font-mono text-emerald-600 dark:text-emerald-400">
                      → {f.destinationEmail}
                    </td>
                    <td className="py-3.5 px-4 font-mono text-slate-500 dark:text-slate-400">{f.domain}</td>
                    <td className="py-3.5 px-4 text-right">
                      <button
                        onClick={() => onDeleteForwarder(f.id)}
                        className="p-1.5 text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors cursor-pointer"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* TAB 4: WEBMAIL ROUNDCUBE & SMTP TEST */}
      {subTab === 'webmail' && (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
          {/* Inbox / Sent List + Reader */}
          <div className="lg:col-span-7 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl overflow-hidden shadow-xs flex flex-col">
            <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setMailFolder('inbox')}
                  className={`px-3 py-1.5 rounded text-xs font-semibold flex items-center gap-1.5 cursor-pointer ${
                    mailFolder === 'inbox'
                      ? 'bg-slate-900 dark:bg-emerald-500 text-white dark:text-slate-950'
                      : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                  }`}
                >
                  <Inbox className="w-3.5 h-3.5" /> Hộp thư đến (
                  {mailData.messages.filter((m) => m.folder === 'inbox').length})
                </button>
                <button
                  onClick={() => setMailFolder('sent')}
                  className={`px-3 py-1.5 rounded text-xs font-semibold flex items-center gap-1.5 cursor-pointer ${
                    mailFolder === 'sent'
                      ? 'bg-slate-900 dark:bg-emerald-500 text-white dark:text-slate-950'
                      : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800'
                  }`}
                >
                  <Send className="w-3.5 h-3.5" /> Đã gửi (
                  {mailData.messages.filter((m) => m.folder === 'sent').length})
                </button>
              </div>
              <span className="text-[11px] font-mono text-slate-500 dark:text-slate-400">
                Roundcube Webmail Embedded
              </span>
            </div>

            <div className="divide-y divide-slate-200 dark:divide-slate-800 max-h-64 overflow-y-auto">
              {filteredMessages.map((msg) => (
                <button
                  key={msg.id}
                  onClick={() => setSelectedMessage(msg)}
                  className={`w-full text-left px-5 py-3 transition-colors cursor-pointer ${
                    selectedMessage?.id === msg.id
                      ? 'bg-emerald-50/70 dark:bg-emerald-950/30'
                      : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                  }`}
                >
                  <div className="flex items-center justify-between text-xs">
                    <span className="font-mono font-semibold text-slate-900 dark:text-white truncate">
                      {mailFolder === 'inbox' ? msg.from : `Tới: ${msg.to}`}
                    </span>
                    <span className="font-mono text-[11px] text-slate-500 dark:text-slate-400 tabular-nums shrink-0">
                      {msg.timestamp}
                    </span>
                  </div>
                  <div className="text-xs font-medium text-slate-800 dark:text-slate-200 mt-0.5 truncate">
                    {msg.subject}
                  </div>
                </button>
              ))}
            </div>

            {selectedMessage && (
              <div className="p-5 border-t border-slate-200 dark:border-slate-800 bg-slate-50/60 dark:bg-slate-950/50 flex-1 space-y-3">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <h3 className="text-sm font-bold text-slate-900 dark:text-white">{selectedMessage.subject}</h3>
                    <div className="text-xs font-mono text-slate-500 dark:text-slate-400 mt-1">
                      Từ: {selectedMessage.from} → Tới: {selectedMessage.to}
                    </div>
                  </div>
                  <span className="text-[11px] font-mono text-emerald-600 dark:text-emerald-400 shrink-0">
                    DKIM Signed: {selectedMessage.dkimSigned ? 'PASS' : 'NONE'}
                  </span>
                </div>
                <pre className="text-xs font-mono text-slate-700 dark:text-slate-300 whitespace-pre-wrap bg-white dark:bg-slate-900 p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 leading-relaxed">
                  {selectedMessage.body || selectedMessage.snippet}
                </pre>
              </div>
            )}
          </div>

          {/* Compose / SMTP Test */}
          <div className="lg:col-span-5 bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl p-5 shadow-xs space-y-4">
            <div>
              <h2 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
                <Send className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                Soạn Thư &amp; Kiểm thử SMTP / OpenDKIM Trực tiếp
              </h2>
              <p className="text-xs text-slate-500 dark:text-slate-400 mt-0.5">
                Kiểm tra khả năng gửi thư ra ngoài với chữ ký RSA-2048 qua cổng 587 STARTTLS.
              </p>
            </div>

            {sendSuccessBanner && (
              <div className="p-3 rounded-lg bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200 dark:border-emerald-800/70 text-xs text-emerald-800 dark:text-emerald-300 font-medium">
                {sendSuccessBanner}
              </div>
            )}

            <form onSubmit={handleSendMailSubmit} className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Hộp thư Gửi đi (From)
                </label>
                <select
                  value={composeFrom}
                  onChange={(e) => setComposeFrom(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white"
                >
                  {mailData.mailboxes.map((b) => (
                    <option key={b.id} value={b.email}>
                      {b.email} ({b.displayName})
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Người nhận (To)
                </label>
                <input
                  type="email"
                  required
                  value={composeTo}
                  onChange={(e) => setComposeTo(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Tiêu đề (Subject)
                </label>
                <input
                  type="text"
                  required
                  value={composeSubject}
                  onChange={(e) => setComposeSubject(e.target.value)}
                  className="w-full px-3 py-2 text-xs bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700 dark:text-slate-300 mb-1">
                  Nội dung Thư
                </label>
                <textarea
                  rows={5}
                  value={composeBody}
                  onChange={(e) => setComposeBody(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-800 rounded-lg text-slate-900 dark:text-white"
                />
              </div>

              <button
                type="submit"
                disabled={sendingMail}
                className="w-full py-2.5 px-4 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white font-semibold text-xs rounded-lg transition-colors flex items-center justify-center gap-2 cursor-pointer"
              >
                {sendingMail ? (
                  <>
                    <RefreshCw className="w-3.5 h-3.5 animate-spin" /> Đang ký DKIM &amp; Gửi qua Postfix...
                  </>
                ) : (
                  <>
                    <Send className="w-3.5 h-3.5" /> Ký số OpenDKIM &amp; Gửi Email Ngay
                  </>
                )}
              </button>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
