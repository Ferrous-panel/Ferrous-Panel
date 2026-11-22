// FerrousPanel Pure-Rust UI — Security, Optimizer/Diagnostics, SSH/Terminal, Jobs & Panel Settings
import {
  api, esc, fmtTime, statusClass, toast, openModal, closeModal, openJobDrawer, handleLogout,
} from './app.js';

// ============================================================================
// 9. SECURITY, FIREWALL & AUDIT LOGS
// ============================================================================
export async function renderSecurity(container) {
  const [sec, auditLogs] = await Promise.all([
    api('GET', '/api/security'),
    api('GET', '/api/security/audit-logs?limit=50').catch(() => []),
  ]);

  const ufw = sec.ufwRules || [];
  const jails = sec.fail2banJails || [];
  const hardening = sec.hardeningChecks || [];
  const waf = sec.wafRules || [];
  const ddosMode = sec.ddosShield?.mode || 'normal';

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Bảo mật, Tường lửa UFW & Nhật ký kiểm toán</h1>
        <p class="page-desc">Quản lý quy tắc UFW, Fail2ban, bảo vệ DDoS, WAF L7, quét toàn vẹn SHA-256 FIM và lịch sử thao tác quản trị.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="sec-fim-btn">Quét SHA-256 FIM</button>
        <button type="button" class="btn btn-primary" id="sec-add-ufw">Thêm luật UFW</button>
      </div>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Tường lửa UFW (${ufw.length})</h2>
          <div class="tabs">
            ${['normal', 'elevated', 'under_attack'].map((m) => `
              <button type="button" class="tab-btn ${ddosMode === m ? 'active' : ''}" data-ddos-mode="${esc(m)}">${esc(m)}</button>
            `).join('')}
          </div>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Cổng / Giao thức</th><th>Hành động</th><th>Ghi chú</th><th class="right">Thao tác</th></tr></thead>
            <tbody>
              ${
                ufw.length
                  ? ufw.map((r) => `
                    <tr>
                      <td class="mono"><strong>${esc(r.port)}</strong>/${esc(r.protocol)}</td>
                      <td><span class="${r.action === 'ALLOW' ? 'st-ok' : 'st-err'}">${esc(r.action)}</span></td>
                      <td>${esc(r.comment || r.source || '—')}</td>
                      <td class="right">
                        <div class="row" style="justify-content:flex-end">
                          <button type="button" class="btn btn-sm" data-ufw-toggle="${esc(r.id)}">${r.enabled ? 'Tắt' : 'Bật'}</button>
                          <button type="button" class="btn btn-sm btn-danger" data-ufw-del="${esc(r.id)}">Xóa</button>
                        </div>
                      </td>
                    </tr>`).join('')
                  : '<tr><td colspan="4" class="empty-state">Chưa có luật UFW nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Fail2ban & Hardening (${hardening.length})</h2>
          <button type="button" class="btn btn-sm" id="sec-ban-ip">Chặn IP</button>
        </div>
        <div class="meta-line" style="margin-bottom:10px">
          ${jails.map((j) => `<span>${esc(j.name)}: <strong class="mono">${esc((j.bannedIps || []).length)}</strong> IP bị chặn</span>`).join('<span>·</span>') || '<span>Fail2ban sẵn sàng</span>'}
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Cấu hình bảo mật hệ thống</th><th>Trạng thái</th><th class="right">Chuyển đổi</th></tr></thead>
            <tbody>
              ${
                hardening.slice(0, 6).map((h) => `
                  <tr>
                    <td>${esc(h.title || h.key)}</td>
                    <td><span class="${h.enabled ? 'st-ok' : 'st-warn'}">${h.enabled ? 'Đã bật' : 'Chưa bật'}</span></td>
                    <td class="right"><button type="button" class="btn btn-sm" data-harden-id="${esc(h.id)}">${h.enabled ? 'Tắt' : 'Bật'}</button></td>
                  </tr>`).join('')
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <section class="surface">
      <div class="surface-head">
        <h2 class="surface-title">L7 WAF Rules (${waf.length})</h2>
      </div>
      <div class="row">
        ${waf.map((w) => `
          <button type="button" class="btn btn-sm" data-waf-toggle="${esc(w.id)}">
            ${esc(w.name)}: <strong class="${w.enabled ? 'st-ok' : 'st-muted'}">${w.enabled ? 'Bật' : 'Tắt'}</strong>
          </button>`).join('')}
      </div>
    </section>

    <section class="surface">
      <div class="surface-head">
        <h2 class="surface-title">Nhật ký kiểm toán bảo mật (${auditLogs.length} sự kiện gần nhất)</h2>
      </div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>Thời gian</th><th>Tài khoản</th><th>IP</th><th>Hành động</th><th>Chi tiết</th></tr></thead>
          <tbody>
            ${
              Array.isArray(auditLogs) && auditLogs.length
                ? auditLogs.map((l) => `
                  <tr>
                    <td class="mono">${esc(fmtTime(l.timestamp))}</td>
                    <td class="mono">${esc(l.username || 'system')}</td>
                    <td class="mono">${esc(l.ip)}</td>
                    <td class="mono"><strong>${esc(l.action)}</strong></td>
                    <td>${esc(l.detail)}</td>
                  </tr>`).join('')
                : '<tr><td colspan="5" class="empty-state">Chưa có nhật ký kiểm toán.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('sec-fim-btn')?.addEventListener('click', async () => {
    try {
      await api('POST', '/api/security/fim-scan', {});
      toast('Đã hoàn tất quét toàn vẹn SHA-256 FIM.');
      await renderSecurity(container);
    } catch (err) { toast(err.message, 'err'); }
  });

  document.getElementById('sec-add-ufw')?.addEventListener('click', () => {
    openModal('Thêm quy tắc tường lửa UFW', `
      <form id="ufw-form">
        <div class="form-grid">
          <div class="field"><label>Cổng (ví dụ 8443)</label><input class="input mono" name="port" required /></div>
          <div class="field"><label>Giao thức</label><select class="select" name="protocol"><option value="tcp">tcp</option><option value="udp">udp</option></select></div>
          <div class="field"><label>Hành động</label><select class="select" name="action"><option value="ALLOW">ALLOW</option><option value="DENY">DENY</option></select></div>
          <div class="field"><label>Ghi chú</label><input class="input" name="comment" placeholder="Custom service" /></div>
        </div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Áp dụng luật</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#ufw-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/security/ufw', {
              port: fd.get('port'),
              protocol: fd.get('protocol'),
              action: fd.get('action'),
              source: 'Anywhere',
              comment: fd.get('comment') || '',
            });
            closeModal();
            toast('Đã thêm quy tắc UFW.');
            await renderSecurity(container);
          } catch (err) { toast(err.message, 'err'); }
        });
      },
    });
  });

  document.getElementById('sec-ban-ip')?.addEventListener('click', () => {
    openModal('Chặn IP bằng Fail2ban', `
      <form id="ban-form">
        <div class="field"><label>Địa chỉ IP</label><input class="input mono" name="ip" placeholder="203.0.113.10" required /></div>
        <div class="field"><label>Jail</label><input class="input mono" name="jailName" value="sshd" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-danger">Chặn IP ngay</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#ban-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/security/fail2ban/ban', { jailName: fd.get('jailName'), ip: fd.get('ip') });
            closeModal();
            toast('Đã chặn IP.');
            await renderSecurity(container);
          } catch (err) { toast(err.message, 'err'); }
        });
      },
    });
  });

  container.querySelectorAll('[data-ddos-mode]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('POST', '/api/security/ddos-mode', { mode: b.dataset.ddosMode }); toast(`Đã chuyển Anti-DDoS sang ${b.dataset.ddosMode}`); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-ufw-toggle]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('PUT', `/api/security/ufw/${encodeURIComponent(b.dataset.ufwToggle)}/toggle`, {}); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-ufw-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/security/ufw/${encodeURIComponent(b.dataset.ufwDel)}`); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-harden-id]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('PUT', `/api/security/hardening/${encodeURIComponent(b.dataset.hardenId)}`, {}); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-waf-toggle]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('PUT', `/api/security/waf/${encodeURIComponent(b.dataset.wafToggle)}/toggle`, {}); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
}

// ============================================================================
// 10. SYSTEM OPTIMIZER & DIAGNOSTICS
// ============================================================================
export async function renderOptimizer(container) {
  const [opt, diag] = await Promise.all([
    api('GET', '/api/optimizer'),
    api('GET', '/api/system/diagnostics').catch(() => []),
  ]);

  const profiles = opt.profiles || [];
  const activeId = opt.activeProfileId || '';

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Tối ưu hệ thống & Chẩn đoán VPS</h1>
        <p class="page-desc">Áp dụng cấu hình sysctl tối ưu, giải phóng bộ nhớ đệm, tự phục hồi watchdog và kiểm tra sức khỏe hạ tầng.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="opt-reclaim">Giải phóng RAM</button>
        <button type="button" class="btn btn-primary" id="opt-heal">Chạy Watchdog Heal</button>
      </div>
    </div>

    <div class="grid-3">
      ${profiles.map((p) => `
        <section class="surface">
          <div class="surface-head">
            <h3 class="surface-title">${esc(p.name)}</h3>
            <span class="${p.id === activeId ? 'st-ok' : 'st-muted'}">${p.id === activeId ? 'Đang dùng' : esc(p.badge || '')}</span>
          </div>
          <p class="page-desc" style="margin-bottom:12px">${esc(p.description || '')}</p>
          <button type="button" class="btn btn-sm ${p.id === activeId ? '' : 'btn-primary'}" data-opt-profile="${esc(p.id)}" ${p.id === activeId ? 'disabled' : ''}>
            ${p.id === activeId ? 'Đã kích hoạt' : 'Áp dụng hồ sơ'}
          </button>
        </section>`).join('')}
    </div>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Chẩn đoán hệ thống (${diag.length} hạng mục)</h2></div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>Hạng mục</th><th>Trạng thái</th><th>Chi tiết</th></tr></thead>
          <tbody>
            ${
              Array.isArray(diag) && diag.length
                ? diag.map((d) => `
                  <tr>
                    <td><strong>${esc(d.label || d.name || d.id)}</strong></td>
                    <td><span class="${statusClass(d.status)}">${esc(d.status)}</span></td>
                    <td>${esc(d.detail || d.message || '—')}</td>
                  </tr>`).join('')
                : '<tr><td colspan="3" class="empty-state">Không có dữ liệu chẩn đoán.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('opt-reclaim')?.addEventListener('click', async () => {
    try { await api('POST', '/api/optimizer/reclaim-memory', {}); toast('Đã giải phóng bộ nhớ đệm.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  });
  document.getElementById('opt-heal')?.addEventListener('click', async () => {
    try { await api('POST', '/api/optimizer/watchdog-heal', {}); toast('Đã chạy chu kỳ kiểm tra & phục hồi dịch vụ.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  });
  container.querySelectorAll('[data-opt-profile]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('POST', '/api/optimizer/apply-profile', { profileId: b.dataset.optProfile }); toast('Đã áp dụng hồ sơ tối ưu.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
}

// ============================================================================
// 11. SSH & WEB TERMINAL
// ============================================================================
export async function renderSsh(container) {
  const ssh = await api('GET', '/api/ssh');
  const cfg = ssh.config || {};
  const keys = ssh.authorizedKeys || [];

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Quản trị SSH & Web Terminal</h1>
        <p class="page-desc">Cấu hình OpenSSH daemon, tạo khóa Ed25519 và thực thi lệnh kiểm tra hệ thống an toàn qua allowlist.</p>
      </div>
      <div class="row">
        <button type="button" class="btn btn-primary" id="ssh-gen-key">Sinh cặp khóa Ed25519</button>
      </div>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Cấu hình SSHD (${esc(ssh.serverHost || 'localhost')})</h2></div>
        <form id="ssh-cfg-form">
          <div class="form-grid">
            <div class="field"><label>Cổng SSH</label><input class="input mono" name="port" type="number" value="${esc(cfg.port || 22)}" /></div>
            <div class="field"><label>PermitRootLogin</label><input class="input mono" name="permitRootLogin" value="${esc(cfg.permitRootLogin || 'prohibit-password')}" /></div>
          </div>
          <div class="row" style="margin-bottom:14px">
            <label class="checkbox-row"><input type="checkbox" name="passwordAuthentication" ${cfg.passwordAuthentication ? 'checked' : ''} /><span>Cho phép đăng nhập mật khẩu</span></label>
            <label class="checkbox-row"><input type="checkbox" name="pubkeyAuthentication" ${cfg.pubkeyAuthentication !== false ? 'checked' : ''} /><span>Xác thực Public Key</span></label>
          </div>
          <button type="submit" class="btn btn-primary">Lưu cấu hình SSH</button>
        </form>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Web Terminal (Allowlist Command Runner)</h2></div>
        <form id="ssh-exec-form" class="row" style="margin-bottom:10px">
          <input class="input mono" id="ssh-cmd-input" style="flex:1" placeholder="uname -a | df -h | systemctl status ferrous-panel" required />
          <button type="submit" class="btn btn-primary">Thực thi</button>
        </form>
        <pre class="terminal-box" id="ssh-exec-out">Sẵn sàng nhận lệnh kiểm tra hệ thống.</pre>
      </section>
    </div>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Khóa SSH được ủy quyền (${keys.length})</h2></div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>Nhãn</th><th>Fingerprint</th><th class="right">Xóa</th></tr></thead>
          <tbody>
            ${
              keys.length
                ? keys.map((k) => `
                  <tr>
                    <td><strong>${esc(k.label)}</strong></td>
                    <td class="mono">${esc(k.fingerprint || k.publicKey?.slice(0, 48) || '')}</td>
                    <td class="right"><button type="button" class="btn btn-sm btn-danger" data-ssh-del="${esc(k.id)}">Thu hồi</button></td>
                  </tr>`).join('')
                : '<tr><td colspan="3" class="empty-state">Chưa có khóa SSH nào trong authorized_keys.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('ssh-cfg-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    try {
      await api('PUT', '/api/ssh/config', {
        port: Number(fd.get('port') || 22),
        permitRootLogin: fd.get('permitRootLogin'),
        passwordAuthentication: fd.get('passwordAuthentication') === 'on',
        pubkeyAuthentication: fd.get('pubkeyAuthentication') === 'on',
      });
      toast('Đã cập nhật cấu hình SSH.');
    } catch (err) { toast(err.message, 'err'); }
  });

  document.getElementById('ssh-exec-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const cmd = document.getElementById('ssh-cmd-input')?.value.trim();
    const outEl = document.getElementById('ssh-exec-out');
    if (!cmd || !outEl) return;
    outEl.textContent = `$ ${cmd}\nĐang chạy…`;
    try {
      const res = await api('POST', '/api/ssh/exec', { command: cmd, cwd: '/root' });
      outEl.textContent = `$ ${cmd}\n${res.stdout || ''}${res.stderr ? `\n[stderr]\n${res.stderr}` : ''}\n(exit ${res.exitCode ?? 0})`;
    } catch (err) {
      outEl.textContent = `[LỖI] ${err.message}`;
    }
  });

  document.getElementById('ssh-gen-key')?.addEventListener('click', async () => {
    try {
      const kp = await api('POST', '/api/ssh/keys/generate', { label: 'ferrous-admin-key', clientHint: 'web-ui' });
      openModal('Cặp khóa Ed25519 mới', `
        <p class="page-desc" style="margin-bottom:10px">Lưu lại khóa riêng tư ngay bây giờ (chỉ hiển thị một lần).</p>
        <pre class="terminal-box">${esc(kp.privateKey || '')}</pre>
      `, { large: true });
      await renderSsh(container);
    } catch (err) { toast(err.message, 'err'); }
  });

  container.querySelectorAll('[data-ssh-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/ssh/keys/${encodeURIComponent(b.dataset.sshDel)}`); await renderSsh(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
}

// ============================================================================
// 12. BACKGROUND JOBS & PANEL ADMIN SETTINGS
// ============================================================================
export async function renderSettingsJobs(container) {
  const [admin, jobs] = await Promise.all([
    api('GET', '/api/panel-admin'),
    api('GET', '/api/jobs?limit=30').catch(() => []),
  ]);

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Tác vụ nền & Cài đặt FerrousPanel</h1>
        <p class="page-desc">Theo dõi hàng đợi tác vụ nền thuần Rust và thay đổi cổng panel (mặc định 8443; cổng 80/443 dành riêng cho website).</p>
      </div>
      <div class="row">
        <button type="button" class="btn btn-danger" id="set-logout-btn">Đăng xuất ngay</button>
      </div>
    </div>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Cấu hình bảng điều khiển & Tài khoản quản trị</h2></div>
      <form id="panel-admin-form">
        <div class="form-grid">
          <div class="field">
            <label>Cổng Panel (1–65535, cấm 80 & 443, mặc định 8443)</label>
            <input class="input mono" name="panelPort" type="number" min="1" max="65535" value="${esc(admin.panelPort || 8443)}" required />
          </div>
          <div class="field">
            <label>Đường dẫn bảo mật (Security Entrance)</label>
            <input class="input mono" name="securityEntrance" value="${esc(admin.securityEntrance || '/panel')}" />
          </div>
          <div class="field">
            <label>Tên miền Panel tùy chỉnh (tùy chọn)</label>
            <input class="input mono" name="customDomain" value="${esc(admin.customDomain || '')}" placeholder="panel.example.com" />
          </div>
          <div class="field">
            <label>Tài khoản quản trị</label>
            <input class="input mono" name="adminUsername" value="${esc(admin.adminUsername || 'admin_ferrous')}" />
          </div>
          <div class="field">
            <label>Đổi mật khẩu quản trị (để trống nếu giữ nguyên)</label>
            <input class="input" name="adminPassword" type="password" placeholder="Tối thiểu 8 ký tự" />
          </div>
        </div>
        <div class="row" style="justify-content:flex-end;margin-top:8px">
          <button type="submit" class="btn btn-primary">Lưu & Áp dụng cấu hình</button>
        </div>
      </form>
    </section>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Hàng đợi Tác vụ nền (${jobs.length})</h2></div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>ID & Tác vụ</th><th>Loại</th><th>Trạng thái</th><th>Thời gian</th><th class="right">Log</th></tr></thead>
          <tbody>
            ${
              Array.isArray(jobs) && jobs.length
                ? jobs.map((j) => `
                  <tr>
                    <td>
                      <div><strong>${esc(j.title)}</strong></div>
                      <div class="meta-line mono">${esc(j.id)}</div>
                    </td>
                    <td class="mono">${esc(j.kind)}</td>
                    <td><span class="${statusClass(j.status)}">${esc(j.status)}</span> · <span class="mono">${esc(j.progress)}%</span></td>
                    <td class="mono">${esc(fmtTime(j.createdAt))}</td>
                    <td class="right"><button type="button" class="btn btn-sm" data-job-open="${esc(j.id)}">Xem log</button></td>
                  </tr>`).join('')
                : '<tr><td colspan="5" class="empty-state">Chưa có tác vụ nền nào.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('set-logout-btn')?.addEventListener('click', handleLogout);
  container.querySelectorAll('[data-job-open]').forEach((b) => {
    b.addEventListener('click', () => openJobDrawer(b.dataset.jobOpen));
  });

  document.getElementById('panel-admin-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    const port = Number(fd.get('panelPort') || 8443);
    if (port === 80 || port === 443) {
      toast('Cổng 80 và 443 dành riêng cho Website (Nginx/FerrousX). Vui lòng chọn cổng khác (mặc định 8443).', 'err');
      return;
    }
    const payload = {
      panelPort: port,
      securityEntrance: String(fd.get('securityEntrance') || '/panel').trim(),
      customDomain: String(fd.get('customDomain') || '').trim(),
      adminUsername: String(fd.get('adminUsername') || '').trim(),
    };
    const pw = String(fd.get('adminPassword') || '');
    if (pw) payload.adminPassword = pw;

    try {
      await api('PUT', '/api/panel-admin', payload);
      toast('Đã lưu cài đặt FerrousPanel. Nếu đổi cổng, dịch vụ sẽ tự khởi động lại trên cổng mới.');
    } catch (err) {
      toast(err.message, 'err');
    }
  });
}
