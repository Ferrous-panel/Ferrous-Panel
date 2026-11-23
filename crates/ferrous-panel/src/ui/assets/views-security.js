// FerrousPanel Pure-Rust UI — Security, Optimizer/Diagnostics, SSH/Terminal, Jobs & Panel Settings
import {
  api, esc, fmtBytes, fmtTime, fmtUptime, statusClass, toast, openModal, closeModal, confirmAction, openJobDrawer, handleLogout,
  capBannerHtml, capDisabledAttr, withBusy,
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
  const hardening = sec.hardeningDirectives || [];
  const shield = sec.antiDdosShield || {};
  const waf = shield.wafRules || [];
  const ddosMode = shield.protectionMode || 'normal';
  const ufwDis = capDisabledAttr('ufw');
  const f2bDis = capDisabledAttr('fail2ban');

  const bannedRows = jails.flatMap((j) => (j.bannedIps || []).map((ip) => ({ jail: j.name, ip })));

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Bảo mật, Tường lửa UFW & Nhật ký kiểm toán</h1>
        <p class="page-desc">Quản lý UFW thật (chống tự khóa SSH/Panel), Fail2ban, Kernel SYN Anti-DDoS, WAF L7 và quét toàn vẹn tệp BLAKE3 + SHA-256.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="sec-fim-btn">Quét BLAKE3 + SHA-256 FIM</button>
        <button type="button" class="btn btn-primary" id="sec-add-ufw" ${ufwDis}>Thêm luật UFW</button>
      </div>
    </div>

    ${capBannerHtml(['ufw', 'fail2ban', 'ebpf_xdp'])}

    <div class="kv-grid">
      <div class="kv-item">
        <span class="kv-label">Kernel Anti-DDoS</span>
        <span class="kv-value mono"><span class="${statusClass(shield.synCookiesEnabled ? 'active' : 'warning')}">${shield.synCookiesEnabled ? 'SYN Cookies Bật' : 'SYN Cookies Tắt'}</span> · backlog ${esc(shield.synRateLimitPps ?? 4096)}</span>
      </div>
      <div class="kv-item">
        <span class="kv-label">Gói Drop (Kernel /proc/net)</span>
        <span class="kv-value mono">${esc(shield.droppedPacketsTotal ?? 0)} packets</span>
      </div>
      <div class="kv-item">
        <span class="kv-label">Fail2ban Tổng chặn</span>
        <span class="kv-value mono">${esc(shield.blockedL7AttacksToday ?? 0)} lượt (${bannedRows.length} đang chặn)</span>
      </div>
      <div class="kv-item">
        <span class="kv-label">Toàn vẹn tệp (BLAKE3 FIM)</span>
        <span class="kv-value mono" title="${esc(shield.fimIntegrityStatus || '')}">${esc(shield.fimIntegrityStatus || 'Chưa quét')}</span>
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
                          <button type="button" class="btn btn-sm" data-ufw-toggle="${esc(r.id)}" data-port="${esc(r.port)}" ${ufwDis}>${r.enabled ? 'Tắt' : 'Bật'}</button>
                          <button type="button" class="btn btn-sm btn-danger" data-ufw-del="${esc(r.id)}" data-port="${esc(r.port)}" ${ufwDis}>Xóa</button>
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
          <h2 class="surface-title">Fail2ban (${jails.length} jails · ${bannedRows.length} IP) & Hardening</h2>
          <button type="button" class="btn btn-sm" id="sec-ban-ip" ${f2bDis}>Chặn IP</button>
        </div>
        <div class="meta-line" style="margin-bottom:10px">
          ${jails.map((j) => `<span><strong>${esc(j.name)}</strong>: <span class="mono">${esc(j.currentlyBanned ?? (j.bannedIps || []).length)}</span> chặn · maxretry=<span class="mono">${esc(j.maxRetry ?? 5)}</span> · bantime=<span class="mono">${esc(j.banTimeSec ?? 600)}s</span></span>`).join('<span>·</span>') || '<span>Fail2ban chưa kích hoạt jail nào</span>'}
        </div>
        ${bannedRows.length ? `
          <div class="row" style="margin-bottom:12px">
            ${bannedRows.map((b) => `
              <button type="button" class="btn btn-sm btn-danger" data-unban-ip="${esc(b.ip)}" data-unban-jail="${esc(b.jail)}" ${f2bDis}>
                Gỡ chặn ${esc(b.ip)} (${esc(b.jail)})
              </button>
            `).join('')}
          </div>
        ` : ''}
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Chỉ thị Hardening (Kernel / SSH)</th><th>Hiện tại / Chuẩn</th><th class="right">Áp dụng</th></tr></thead>
            <tbody>
              ${
                hardening.length
                  ? hardening.map((h) => `
                    <tr>
                      <td>
                        <div class="mono"><strong>${esc(h.key)}</strong></div>
                        <div class="meta-line">${esc(h.category || '')}${h.description ? ` · ${esc(h.description)}` : ''}</div>
                      </td>
                      <td class="mono"><span class="${h.enabled ? 'st-ok' : 'st-warn'}">${esc(h.currentValue || '—')}</span> / ${esc(h.recommendedValue || '—')}</td>
                      <td class="right"><button type="button" class="btn btn-sm" data-harden-id="${esc(h.id)}">${h.enabled ? 'Đạt chuẩn' : 'Bật chuẩn'}</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Không có chỉ thị hardening nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <section class="surface">
      <div class="surface-head">
        <h2 class="surface-title">L7 WAF Rules (${waf.length})</h2>
        <span class="meta-line">${esc(shield.xdpStatus || '')}</span>
      </div>
      <div class="row">
        ${waf.length ? waf.map((w) => `
          <button type="button" class="btn btn-sm" data-waf-toggle="${esc(w.id)}" title="${esc(w.description || '')}">
            <span class="mono">${esc(w.code || '')}</span> ${esc(w.category || w.name)}: <strong class="${w.enabled ? 'st-ok' : 'st-muted'}">${w.enabled ? 'Bật' : 'Tắt'}</strong>
          </button>`).join('') : '<span class="st-muted">Chưa cấu hình luật WAF.</span>'}
      </div>
    </section>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Nhật ký kiểm toán bảo mật (${auditLogs.length} sự kiện gần nhất)</h2></div>
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

  const fimBtn = document.getElementById('sec-fim-btn');
  fimBtn?.addEventListener('click', () => withBusy(fimBtn, async () => {
    try {
      const res = await api('POST', '/api/security/fim-scan', {});
      toast(`Đã quét toàn vẹn ${res.scannedFiles ?? 0} tệp bằng BLAKE3 + SHA-256.`);
      await renderSecurity(container);
    } catch (err) { toast(err.message, 'err'); }
  }));

  document.getElementById('sec-add-ufw')?.addEventListener('click', () => {
    openModal('Thêm quy tắc tường lửa UFW', `
      <form id="ufw-form">
        <div class="form-grid">
          <div class="field"><label>Cổng (ví dụ 8443)</label><input class="input mono" name="port" required /></div>
          <div class="field"><label>Giao thức</label><select class="select" name="protocol"><option value="tcp">tcp</option><option value="udp">udp</option></select></div>
          <div class="field"><label>Hành động</label><select class="select" name="action"><option value="ALLOW">ALLOW</option><option value="DENY">DENY</option></select></div>
          <div class="field"><label>Ghi chú</label><input class="input" name="comment" placeholder="Custom service" /></div>
        </div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="ufw-sub">Áp dụng luật</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#ufw-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#ufw-sub'), async () => {
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
              toast('Đã thêm quy tắc UFW và xác nhận qua ufw status.');
              await renderSecurity(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('sec-ban-ip')?.addEventListener('click', () => {
    openModal('Chặn IP bằng Fail2ban', `
      <form id="ban-form">
        <div class="field"><label>Địa chỉ IP</label><input class="input mono" name="ip" placeholder="203.0.113.10" required /></div>
        <div class="field"><label>Jail</label><input class="input mono" name="jailName" value="sshd" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-danger" id="ban-sub">Chặn IP ngay</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#ban-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#ban-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/security/fail2ban/ban', { jailName: fd.get('jailName'), ip: fd.get('ip') });
              closeModal();
              toast('Đã chặn IP qua fail2ban-client.');
              await renderSecurity(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  container.querySelectorAll('[data-unban-ip]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Gỡ chặn IP Fail2ban', `Xác nhận gỡ chặn IP ${b.dataset.unbanIp} khỏi jail ${b.dataset.unbanJail}?`, () => withBusy(b, async () => {
      try {
        await api('POST', '/api/security/fail2ban/unban', { jailName: b.dataset.unbanJail, ip: b.dataset.unbanIp });
        toast(`Đã gỡ chặn ${b.dataset.unbanIp}.`);
        await renderSecurity(container);
      } catch (err) { toast(err.message, 'err'); }
    }), false);
  }));

  container.querySelectorAll('[data-ddos-mode]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('POST', '/api/security/ddos-mode', { mode: b.dataset.ddosMode }); toast(`Đã chuyển Anti-DDoS sang ${b.dataset.ddosMode}`); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  })));

  container.querySelectorAll('[data-ufw-toggle]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Thay đổi luật UFW', `Xác nhận bật/tắt quy tắc cổng ${b.dataset.port}?`, () => withBusy(b, async () => {
      try { await api('PUT', `/api/security/ufw/${encodeURIComponent(b.dataset.ufwToggle)}/toggle`, {}); await renderSecurity(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));

  container.querySelectorAll('[data-ufw-del]').forEach((b) => b.addEventListener('click', () => {
    const id = b.dataset.ufwDel;
    const port = b.dataset.port;
    confirmAction('Xóa luật tường lửa UFW', `Xác nhận xóa quy tắc UFW cho cổng ${port}?`, () => withBusy(b, async () => {
      try {
        await api('DELETE', `/api/security/ufw/${encodeURIComponent(id)}`);
        toast(`Đã xóa luật UFW cổng ${port}.`);
        await renderSecurity(container);
      } catch (err) {
        if (err.status === 409) {
          confirmAction(
            'Cảnh báo chống tự khóa máy chủ!',
            `${err.message}\n\nBạn có CHẮC CHẮN muốn ép xóa (force=true) quy tắc bảo vệ cổng ${port} này không?`,
            async () => {
              try {
                await api('DELETE', `/api/security/ufw/${encodeURIComponent(id)}?force=true`);
                toast(`Đã ép xóa luật UFW cổng ${port}.`);
                await renderSecurity(container);
              } catch (err2) { toast(err2.message, 'err'); }
            },
          );
        } else {
          toast(err.message, 'err');
        }
      }
    }));
  }));

  container.querySelectorAll('[data-harden-id]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('PUT', `/api/security/hardening/${encodeURIComponent(b.dataset.hardenId)}`, {}); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  })));

  container.querySelectorAll('[data-waf-toggle]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('PUT', `/api/security/waf/${encodeURIComponent(b.dataset.wafToggle)}/toggle`, {}); await renderSecurity(container); }
    catch (err) { toast(err.message, 'err'); }
  })));
}

// ============================================================================
// 10. SYSTEM OPTIMIZER, KERNEL SYSCTL, ANTI-CRASH GUARD & DIAGNOSTICS
// ============================================================================
export async function renderOptimizer(container) {
  const [opt, diag] = await Promise.all([
    api('GET', '/api/optimizer'),
    api('GET', '/api/system/diagnostics').catch(() => []),
  ]);

  const profiles = opt.profiles || [];
  const activeId = opt.activeProfileId || '';
  const sysctls = opt.sysctlParams || [];
  const guard = opt.antiCrashGuard || {};
  const wd = opt.watchdog || {};
  const oomMap = guard.oomImmunityMap || [];
  const optDis = capDisabledAttr('optimizer_sysctl');

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Tối ưu hệ thống, Kernel Sysctl & Chẩn đoán VPS</h1>
        <p class="page-desc">Áp dụng /etc/sysctl.d/99-ferrous-optimizer.conf + MariaDB tuning, cấu hình ZRAM swap thật qua zramctl và đọc trực tiếp từ /proc.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="opt-reclaim">Giải phóng RAM</button>
        <button type="button" class="btn btn-primary" id="opt-heal">Chạy Watchdog Heal</button>
      </div>
    </div>

    ${capBannerHtml(['optimizer_sysctl'])}

    <div class="kv-grid">
      <div class="kv-item">
        <span class="kv-label">Panel Daemon (/proc/self/status)</span>
        <span class="kv-value mono">PID ${esc(wd.daemonPid ?? 0)} · RSS ${Number(wd.residentRamMb || 0).toFixed(1)} MB · Heap ${Number(wd.allocatedRamMb || 0).toFixed(1)} MB</span>
      </div>
      <div class="kv-item">
        <span class="kv-label">Threads & OOM Score</span>
        <span class="kv-value mono">${esc(wd.workerThreadsActive ?? 0)} threads (${esc(wd.workerThreadsTotal ?? 1)} cores) · OOM ${esc(wd.oomScoreAdj ?? 0)}</span>
      </div>
      <div class="kv-item">
        <span class="kv-label">ZRAM Kernel Swap (/sys/block/zram0)</span>
        <span class="kv-value mono"><span class="${statusClass(guard.zramEnabled ? 'active' : 'muted')}">${guard.zramEnabled ? 'Đang bật' : 'Chưa bật'}</span> · ${esc(guard.zramSizeMb ?? 0)} MB (${esc(guard.zramCompressionAlgo || 'zstd')})</span>
      </div>
      <div class="kv-item">
        <span class="kv-label">Watchdog & Mitigation</span>
        <span class="kv-value mono">${esc(wd.circuitBreakerStatus || 'closed')} · ${esc(guard.mitigationCount ?? 0)} lần xử lý</span>
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
          <button type="button" class="btn btn-sm ${p.id === activeId ? '' : 'btn-primary'}" data-opt-profile="${esc(p.id)}" ${p.id === activeId ? 'disabled' : optDis}>
            ${p.id === activeId ? 'Đã kích hoạt' : 'Áp dụng hồ sơ'}
          </button>
        </section>`).join('')}
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Tham số Kernel Sysctl (đọc thật từ /proc/sys)</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Key</th><th>Giá trị thật</th><th>Khuyến nghị</th><th class="right">Bật/Tắt</th></tr></thead>
            <tbody>
              ${
                sysctls.map((s) => `
                  <tr>
                    <td class="mono"><strong>${esc(s.key)}</strong></td>
                    <td class="mono">${esc(s.currentValue)}</td>
                    <td class="mono">${esc(s.recommendedValue)}</td>
                    <td class="right"><button type="button" class="btn btn-sm" data-sysctl-toggle="${esc(s.id)}" ${optDis}>${s.enabled ? 'Đang bật' : 'Đã tắt'}</button></td>
                  </tr>`).join('')
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Anti-Crash Guard, ZRAM Swap & Cứu hộ</h2></div>
        <form id="opt-ac-form" style="margin-bottom:14px">
          <div class="form-grid">
            <div class="field"><label>Ngưỡng xả tải CPU (%)</label><input class="input mono" name="cpuShedThresholdPercent" type="number" min="50" max="99" value="${esc(guard.cpuShedThresholdPercent || 92)}" /></div>
            <div class="field"><label>Ngưỡng xả tải RAM (%)</label><input class="input mono" name="ramShedThresholdPercent" type="number" min="50" max="99" value="${esc(guard.ramShedThresholdPercent || 90)}" /></div>
            <div class="field"><label>Dung lượng ZRAM Swap (MB)</label><input class="input mono" name="zramSizeMb" type="number" min="128" max="16384" value="${esc(guard.zramSizeMb || 1024)}" /></div>
            <div class="field"><label>Slow Query Auto-Kill (s)</label><input class="input mono" name="slowQueryAutoKillSec" type="number" min="5" max="600" value="${esc(guard.slowQueryAutoKillSec || 30)}" /></div>
          </div>
          <div class="row" style="justify-content:space-between;margin-top:4px">
            <div class="row">
              <label class="checkbox-row"><input type="checkbox" name="loadSheddingEnabled" ${guard.loadSheddingEnabled !== false ? 'checked' : ''} /><span>Load Shedding</span></label>
              <label class="checkbox-row"><input type="checkbox" name="zramEnabled" ${guard.zramEnabled ? 'checked' : ''} /><span>Bật ZRAM (zramctl)</span></label>
            </div>
            <button type="submit" class="btn btn-sm btn-primary" id="opt-ac-sub">Lưu & Áp dụng</button>
          </div>
        </form>
        <hr class="divider" />
        <div class="row">
          <button type="button" class="btn btn-sm btn-danger" data-mitigate="purge_syn_backlog" ${optDis}>Chống SYN Backlog</button>
          <button type="button" class="btn btn-sm btn-danger" data-mitigate="isolate_leaking_workers">Giải phóng & Reload PHP</button>
          <button type="button" class="btn btn-sm btn-danger" data-mitigate="reset_hung_db_queries">Reload DB Engine</button>
        </div>
      </section>
    </div>

    ${oomMap.length ? `
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Bản đồ bảo vệ OOM Dịch vụ hệ thống (systemd thật · ${oomMap.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Service</th><th class="right">PID</th><th class="right">OOM Score</th><th>Chính sách bảo vệ</th></tr></thead>
            <tbody>
              ${oomMap.map((o) => `
                <tr>
                  <td class="mono"><strong>${esc(o.service)}</strong></td>
                  <td class="right mono">${esc(o.pid || '—')}</td>
                  <td class="right mono">${esc(o.oomScoreAdj)}</td>
                  <td>${esc(o.policy || '—')}</td>
                </tr>`).join('')}
            </tbody>
          </table>
        </div>
      </section>
    ` : ''}

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Chẩn đoán hệ thống (${diag.length} hạng mục)</h2></div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>Phân hệ</th><th>Đối tượng</th><th>Trạng thái</th><th class="right">Độ trễ</th><th>Chi tiết</th></tr></thead>
          <tbody>
            ${
              Array.isArray(diag) && diag.length
                ? diag.map((d) => `
                  <tr>
                    <td><strong>${esc(d.subsystem || d.id)}</strong></td>
                    <td class="mono">${esc(d.target || '—')}</td>
                    <td><span class="${statusClass(d.status)}">${esc(d.status)}</span></td>
                    <td class="right mono">${esc(d.latencyMs ?? 0)} ms</td>
                    <td>${esc(d.detail || '—')}</td>
                  </tr>`).join('')
                : '<tr><td colspan="5" class="empty-state">Không có dữ liệu chẩn đoán.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  const recBtn = document.getElementById('opt-reclaim');
  recBtn?.addEventListener('click', () => withBusy(recBtn, async () => {
    try { const r = await api('POST', '/api/optimizer/reclaim-memory', {}); toast(r.message || 'Đã giải phóng bộ nhớ đệm.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  }));

  const healBtn = document.getElementById('opt-heal');
  healBtn?.addEventListener('click', () => withBusy(healBtn, async () => {
    try { await api('POST', '/api/optimizer/watchdog-heal', {}); toast('Đã hoàn tất kiểm tra & phục hồi dịch vụ.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  }));

  container.querySelectorAll('[data-opt-profile]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('POST', '/api/optimizer/apply-profile', { profileId: b.dataset.optProfile }); toast('Đã áp dụng hồ sơ sysctl + MariaDB tuning và xác nhận qua /proc/sys.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  })));

  container.querySelectorAll('[data-sysctl-toggle]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('PUT', `/api/optimizer/sysctl/${encodeURIComponent(b.dataset.sysctlToggle)}`, {}); toast('Đã cập nhật sysctl.'); await renderOptimizer(container); }
    catch (err) { toast(err.message, 'err'); }
  })));

  document.getElementById('opt-ac-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    await withBusy(document.getElementById('opt-ac-sub'), async () => {
      const fd = new FormData(e.target);
      try {
        await api('PUT', '/api/optimizer/anti-crash', {
          loadSheddingEnabled: fd.get('loadSheddingEnabled') === 'on',
          cpuShedThresholdPercent: Number(fd.get('cpuShedThresholdPercent') || 92),
          ramShedThresholdPercent: Number(fd.get('ramShedThresholdPercent') || 90),
          zramEnabled: fd.get('zramEnabled') === 'on',
          zramSizeMb: Number(fd.get('zramSizeMb') || 1024),
          slowQueryAutoKillSec: Number(fd.get('slowQueryAutoKillSec') || 30),
        });
        toast('Đã lưu ngưỡng Anti-Crash Guard & cấu hình ZRAM.');
        await renderOptimizer(container);
      } catch (err) { toast(err.message, 'err'); }
    });
  });

  container.querySelectorAll('[data-mitigate]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Thực thi cứu hộ khẩn cấp', `Xác nhận chạy biện pháp "${b.textContent.trim()}" trên VPS?`, () => withBusy(b, async () => {
      try {
        const res = await api('POST', '/api/optimizer/emergency-mitigate', { action: b.dataset.mitigate });
        toast(res.message || 'Đã thực thi biện pháp cứu hộ.');
        await renderOptimizer(container);
      } catch (err) { toast(err.message, 'err'); }
    }));
  }));
}

// ============================================================================
// 11. SSH & WEB TERMINAL (PORT CONFIRM + REAL OUTPUT + ACTIVE SESSIONS)
// ============================================================================
export async function renderSsh(container) {
  const ssh = await api('GET', '/api/ssh');
  const cfg = ssh.config || {};
  const keys = ssh.authorizedKeys || [];
  const sessions = ssh.activeSessions || [];
  const sshDis = capDisabledAttr('ssh_manager');
  const origPort = Number(cfg.port || 22);

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Quản trị SSH & Web Terminal</h1>
        <p class="page-desc">Cấu hình OpenSSH daemon (kiểm tra sshd -t + rollback + xác nhận đổi cổng), khóa Ed25519 thật và lệnh chẩn đoán allowlist.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="ssh-add-key" ${sshDis}>Thêm Public Key</button>
        <button type="button" class="btn btn-primary" id="ssh-gen-key" ${sshDis}>Sinh cặp khóa Ed25519</button>
      </div>
    </div>

    ${capBannerHtml(['ssh_manager'])}

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Cấu hình SSHD (${esc(ssh.serverHost || 'localhost')})</h2></div>
        <form id="ssh-cfg-form">
          <div class="form-grid">
            <div class="field"><label>Cổng SSH</label><input class="input mono" name="port" type="number" min="1" max="65535" value="${esc(origPort)}" /></div>
            <div class="field"><label>PermitRootLogin</label><input class="input mono" name="permitRootLogin" value="${esc(cfg.permitRootLogin || 'prohibit-password')}" /></div>
          </div>
          <div class="row" style="margin-bottom:14px">
            <label class="checkbox-row"><input type="checkbox" name="passwordAuthentication" ${cfg.passwordAuthentication ? 'checked' : ''} /><span>Cho phép đăng nhập mật khẩu</span></label>
            <label class="checkbox-row"><input type="checkbox" name="pubkeyAuthentication" ${cfg.pubkeyAuthentication !== false ? 'checked' : ''} /><span>Xác thực Public Key</span></label>
          </div>
          <button type="submit" class="btn btn-primary" id="ssh-cfg-sub" ${sshDis}>Lưu cấu hình SSH</button>
        </form>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Web Terminal (Allowlist Command Runner)</h2></div>
        <form id="ssh-exec-form" class="row" style="margin-bottom:10px">
          <input class="input mono" id="ssh-cmd-input" style="flex:1" placeholder="uname -a | df -h | ip -brief addr | ss -tulpn | sysctl net.core.somaxconn" required />
          <button type="submit" class="btn btn-primary" id="ssh-exec-sub">Thực thi</button>
        </form>
        <pre class="terminal-box" id="ssh-exec-out">Sẵn sàng nhận lệnh kiểm tra hệ thống (chỉ cho phép lệnh trong allowlist, không shell).</pre>
      </section>
    </div>

    <div class="grid-2">
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
                      <td><strong>${esc(k.label)}</strong><div class="meta-line mono">${esc(k.algorithm || '')}</div></td>
                      <td class="mono">${esc(k.fingerprint || k.publicKey?.slice(0, 48) || '')}</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-ssh-del="${esc(k.id)}" data-label="${esc(k.label)}">Thu hồi</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có khóa SSH nào trong authorized_keys.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Phiên SSH đang hoạt động (${sessions.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>User & PTS</th><th>IP nguồn / Thời gian</th><th class="right">Ngắt phiên</th></tr></thead>
            <tbody>
              ${
                sessions.length
                  ? sessions.map((s) => `
                    <tr>
                      <td class="mono"><strong>${esc(s.user || 'root')}</strong> · ${esc(s.pts || 'pts/0')} ${s.pid ? `(PID ${esc(s.pid)})` : ''}</td>
                      <td>
                        <div class="mono">${esc(s.clientIp || '—')}</div>
                        <div class="meta-line mono">${esc(s.connectedAt || '')}</div>
                      </td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-ssh-kick="${esc(s.id)}">Ngắt</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Không có phiên SSH nào đang kết nối.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>
  `;

  document.getElementById('ssh-cfg-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const fd = new FormData(e.target);
    const newPort = Number(fd.get('port') || 22);
    const payload = {
      port: newPort,
      permitRootLogin: fd.get('permitRootLogin'),
      passwordAuthentication: fd.get('passwordAuthentication') === 'on',
      pubkeyAuthentication: fd.get('pubkeyAuthentication') === 'on',
      confirmPortChange: false,
    };
    const submitBtn = document.getElementById('ssh-cfg-sub');
    const doSave = (confirmed) => withBusy(submitBtn, async () => {
      try {
        await api('PUT', '/api/ssh/config', { ...payload, confirmPortChange: confirmed });
        toast('Đã kiểm tra sshd -t và cập nhật cấu hình SSH.');
        await renderSsh(container);
      } catch (err) { toast(err.message, 'err'); }
    });
    if (newPort !== origPort) {
      confirmAction(
        'Xác nhận đổi cổng SSH',
        `Bạn sắp đổi cổng SSH từ ${origPort} sang ${newPort}. Đảm bảo tường lửa đã mở cổng ${newPort} để tránh mất quyền truy cập máy chủ!`,
        () => doSave(true),
      );
    } else {
      await doSave(false);
    }
  });

  document.getElementById('ssh-exec-form')?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const cmd = document.getElementById('ssh-cmd-input')?.value.trim();
    const outEl = document.getElementById('ssh-exec-out');
    if (!cmd || !outEl) return;
    await withBusy(document.getElementById('ssh-exec-sub'), async () => {
      outEl.textContent = `$ ${cmd}\nĐang chạy…`;
      try {
        const res = await api('POST', '/api/ssh/exec', { command: cmd, cwd: '/www/wwwroot' });
        const body = res.output ?? res.stdout ?? '';
        outEl.textContent = `$ ${cmd}\n${body}\n(exit ${res.exitCode ?? 0})`;
      } catch (err) {
        outEl.textContent = `[LỖI] ${err.message}`;
        toast(err.message, 'err');
      }
    });
  });

  document.getElementById('ssh-add-key')?.addEventListener('click', () => {
    openModal('Thêm SSH Public Key vào authorized_keys', `
      <form id="ssh-import-form">
        <div class="field"><label>Nhãn thiết bị</label><input class="input" name="label" placeholder="macbook-pro" required /></div>
        <div class="field"><label>SSH Public Key (ssh-ed25519 / ssh-rsa)</label><textarea class="textarea" name="publicKey" placeholder="ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA... user@host" required></textarea></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="ssh-import-sub">Lưu Public Key</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#ssh-import-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#ssh-import-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/ssh/keys', {
                label: fd.get('label'),
                publicKey: fd.get('publicKey'),
                clientHint: 'web-ui',
              });
              closeModal();
              toast('Đã thêm SSH Public Key vào authorized_keys.');
              await renderSsh(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  const genBtn = document.getElementById('ssh-gen-key');
  genBtn?.addEventListener('click', () => withBusy(genBtn, async () => {
    try {
      const kp = await api('POST', '/api/ssh/keys/generate', { label: 'ferrous-admin-key', clientHint: 'web-ui' });
      openModal('Cặp khóa Ed25519 mới (ssh-keygen)', `
        <p class="page-desc" style="margin-bottom:10px">Lưu lại khóa riêng tư ngay bây giờ (chỉ hiển thị một lần).</p>
        <pre class="terminal-box">${esc(kp.privateKey || '')}</pre>
      `, { large: true });
      await renderSsh(container);
    } catch (err) { toast(err.message, 'err'); }
  }));

  container.querySelectorAll('[data-ssh-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Thu hồi khóa SSH', `Xác nhận xóa khóa "${b.dataset.label}" khỏi authorized_keys?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/ssh/keys/${encodeURIComponent(b.dataset.sshDel)}`); await renderSsh(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));

  container.querySelectorAll('[data-ssh-kick]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Ngắt phiên SSH', 'Xác nhận chấm dứt phiên SSH đang hoạt động này?', () => withBusy(b, async () => {
      try { await api('DELETE', `/api/ssh/sessions/${encodeURIComponent(b.dataset.sshKick)}`); toast('Đã ngắt phiên SSH.'); await renderSsh(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
}

// ============================================================================
// 12. BACKGROUND JOBS & PANEL ADMIN SETTINGS
// ============================================================================
export async function renderSettingsJobs(container) {
  const [admin, jobs, upd] = await Promise.all([
    api('GET', '/api/panel-admin'),
    api('GET', '/api/jobs?limit=30').catch(() => []),
    api('GET', '/api/system/update-status').catch(() => null),
  ]);

  const origPort = Number(admin.panelPort || 8443);
  const shortSha = (sha) => (sha ? String(sha).slice(0, 10) : '—');
  const updRunning = Boolean(upd?.updaterRunning);
  const canUiUpd = Boolean(upd?.canTriggerFromUi);
  const hasRollback = Boolean(upd?.rollbackAvailable);
  const updDisabled = canUiUpd && !updRunning ? '' : 'disabled';
  const rbDisabled = canUiUpd && hasRollback && !updRunning ? '' : 'disabled';

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Tác vụ nền & Cài đặt FerrousPanel</h1>
        <p class="page-desc">Cập nhật nhanh (profile vps / prebuilt SHA-256), rollback tức thì, tác vụ nền thuần Rust và cấu hình cổng panel.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="set-restart-panel-btn">Khởi động lại FerrousPanel</button>
        <button type="button" class="btn btn-danger" id="set-logout-btn">Đăng xuất ngay</button>
      </div>
    </div>

    <section class="surface">
      <div class="surface-head">
        <div>
          <h2 class="surface-title">Cập nhật & Rollback FerrousPanel</h2>
          <div class="meta-line">
            <span>Phiên bản: <strong class="mono">v${esc(upd?.currentVersion || '2.5.0')}</strong></span>
            <span>·</span>
            <span>Commit hiện tại: <strong class="mono" id="upd-local-sha">${esc(shortSha(upd?.installedCommit))}</strong></span>
            <span>·</span>
            <span>Bản mới nhất (${esc(upd?.branch || 'main')}): <strong class="mono" id="upd-remote-sha">${esc(shortSha(upd?.remoteCommit))}</strong></span>
            <span>·</span>
            <span id="upd-badge" class="${updRunning ? 'st-warn' : upd?.updateAvailable ? 'st-warn' : 'st-ok'}">
              ${updRunning ? 'Đang cập nhật…' : upd?.updateAvailable ? 'Có bản cập nhật mới' : 'Đã đồng bộ'}
            </span>
          </div>
        </div>
        <div class="row">
          <button type="button" class="btn btn-sm" id="upd-check-btn">Kiểm tra bản mới</button>
          <button type="button" class="btn btn-sm btn-primary" id="upd-source-btn" ${updDisabled}>Cập nhật (Tối ưu VPS)</button>
          <button type="button" class="btn btn-sm" id="upd-prebuilt-btn" ${updDisabled}>Cập nhật nhanh (Prebuilt)</button>
          <button type="button" class="btn btn-sm btn-danger" id="upd-rollback-btn" ${rbDisabled} title="${hasRollback ? `Khôi phục bản trước (${shortSha(upd?.rollbackCommit)})` : 'Chưa có bản sao lưu trước đó'}">Rollback bản trước</button>
        </div>
      </div>
      <div class="meta-line" style="margin-bottom:8px">
        <span>Build profile: <strong class="mono">${esc(upd?.buildProfile || 'vps (thin-LTO incremental)')}</strong></span>
        <span>·</span>
        <span>Lệnh CLI nhanh: <code class="mono">${esc(upd?.cliCommand || 'sudo fp update')}</code> · <code class="mono">sudo fp update --prebuilt</code> · <code class="mono">sudo fp update --rollback</code></span>
      </div>
      ${upd?.recentLogs?.length ? `
        <pre class="code-box mono" id="upd-log-box" style="max-height:180px;overflow:auto;margin-top:8px">${esc(upd.recentLogs.join('\n'))}</pre>
      ` : '<pre class="code-box mono" id="upd-log-box" style="max-height:180px;overflow:auto;margin-top:8px;display:none"></pre>'}
    </section>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Cấu hình bảng điều khiển & Tài khoản quản trị</h2></div>
      <form id="panel-admin-form">
        <div class="form-grid">
          <div class="field">
            <label>Cổng Panel (1–65535, cấm 80 & 443, mặc định 8443)</label>
            <input class="input mono" name="panelPort" type="number" min="1" max="65535" value="${esc(origPort)}" required />
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
          <button type="submit" class="btn btn-primary" id="panel-admin-sub">Lưu & Áp dụng cấu hình</button>
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

  const applyUpdateStatusDom = (st) => {
    if (!st) return;
    const localEl = document.getElementById('upd-local-sha');
    const remoteEl = document.getElementById('upd-remote-sha');
    const badgeEl = document.getElementById('upd-badge');
    const logBox = document.getElementById('upd-log-box');
    if (localEl) localEl.textContent = shortSha(st.installedCommit);
    if (remoteEl) remoteEl.textContent = shortSha(st.remoteCommit);
    if (badgeEl) {
      badgeEl.className = st.updaterRunning ? 'st-warn' : st.updateAvailable ? 'st-warn' : 'st-ok';
      badgeEl.textContent = st.updaterRunning
        ? 'Đang cập nhật…'
        : st.updateAvailable
          ? 'Có bản cập nhật mới'
          : 'Đã mới nhất';
    }
    if (logBox && Array.isArray(st.recentLogs) && st.recentLogs.length) {
      logBox.style.display = 'block';
      logBox.textContent = st.recentLogs.join('\n');
      logBox.scrollTop = logBox.scrollHeight;
    }
  };

  const pollUpdaterUntilReady = async () => {
    const deadline = Date.now() + 180000;
    let sawRestart = false;
    while (Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 1500));
      try {
        const st = await api('GET', '/api/system/update-status');
        applyUpdateStatusDom(st);
        if (!st.updaterRunning) {
          if (sawRestart) {
            toast('Cập nhật FerrousPanel hoàn tất — đang tải lại giao diện mới.');
            window.location.reload();
            return;
          }
          toast('Tiến trình cập nhật đã hoàn tất.');
          await renderSettingsJobs(container);
          return;
        }
      } catch (_) {
        sawRestart = true;
      }
    }
    toast('Tiến trình cập nhật đang chạy nền; kiểm tra log bằng `journalctl -u ferrous-panel-updater -f`.', 'warn');
  };

  const checkUpdBtn = document.getElementById('upd-check-btn');
  checkUpdBtn?.addEventListener('click', () => withBusy(checkUpdBtn, async () => {
    try {
      const st = await api('GET', '/api/system/update-status?refresh=1');
      applyUpdateStatusDom(st);
      if (st.updateAvailable) {
        toast(`Có bản cập nhật mới: ${shortSha(st.remoteCommit)} (hiện tại ${shortSha(st.installedCommit)}).`);
      } else if (st.remoteCommit) {
        toast(`FerrousPanel đang ở bản mới nhất (${shortSha(st.remoteCommit)}).`);
      } else {
        toast('Không truy vấn được commit từ GitHub (kiểm tra kết nối mạng VPS).', 'err');
      }
    } catch (err) { toast(err.message, 'err'); }
  }));

  const launchUpdate = (btn, mode, title, desc, danger = false) => {
    btn?.addEventListener('click', () => {
      confirmAction(
        title,
        desc,
        () => withBusy(btn, async () => {
          try {
            const res = await api('POST', '/api/system/update', { mode });
            toast(res.message || 'Đã khởi chạy tiến trình cập nhật.');
            await pollUpdaterUntilReady();
          } catch (err) { toast(err.message, 'err'); }
        }),
        danger,
      );
    });
  };

  launchUpdate(
    document.getElementById('upd-source-btn'),
    'source',
    'Cập nhật FerrousPanel từ mã nguồn (Profile VPS)',
    'Chạy cập nhật tối ưu (bỏ qua nếu đã mới nhất, biên dịch thin-LTO tăng dần, tự động rollback nếu /api/health lỗi).',
  );
  launchUpdate(
    document.getElementById('upd-prebuilt-btn'),
    'prebuilt',
    'Cập nhật nhanh bằng bản Prebuilt (SHA-256)',
    'Tải binary dựng sẵn từ GitHub Release (xác thực bắt buộc SHA-256) và thay thế nguyên tử trong vài giây.',
  );
  launchUpdate(
    document.getElementById('upd-rollback-btn'),
    'rollback',
    'Rollback về phiên bản ổn định trước đó',
    'Khôi phục ngay binary đã sao lưu trước lần cập nhật gần nhất và khởi động lại ferrous-panel.service.',
    true,
  );

  document.getElementById('set-logout-btn')?.addEventListener('click', handleLogout);
  const restartPanelBtn = document.getElementById('set-restart-panel-btn');
  restartPanelBtn?.addEventListener('click', () => {
    confirmAction(
      'Khởi động lại FerrousPanel',
      'Dịch vụ ferrous-panel.service sẽ được khởi động lại trong 2–5 giây và tự động kết nối lại.',
      () => withBusy(restartPanelBtn, async () => {
        try {
          await api('POST', '/api/panel-admin/cli-action', { option: 1 });
          toast('Đã gửi lệnh khởi động lại ferrous-panel.service — đang chờ kết nối lại…');
          await new Promise((r) => setTimeout(r, 1200));
          const deadline = Date.now() + 15000;
          let ready = false;
          while (Date.now() < deadline) {
            try {
              const r = await fetch('/api/health', { cache: 'no-store', credentials: 'same-origin' });
              if (r.ok) {
                ready = true;
                break;
              }
            } catch (_) {
              // still restarting
            }
            await new Promise((r) => setTimeout(r, 600));
          }
          if (ready) {
            toast('FerrousPanel đã khởi động lại thành công.');
            await renderSettingsJobs(container);
          } else {
            toast('Không nhận được phản hồi /api/health sau 15 giây — hãy kiểm tra `sudo fp logs`.', 'err');
          }
        } catch (err) {
          toast(err.message, 'err');
        }
      }),
      false,
    );
  });
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

    const sub = document.getElementById('panel-admin-sub');
    const doSave = () => withBusy(sub, async () => {
      try {
        await api('PUT', '/api/panel-admin', payload);
        toast('Đã lưu cài đặt FerrousPanel. Nếu đổi cổng, dịch vụ sẽ tự khởi động lại trên cổng mới.');
      } catch (err) { toast(err.message, 'err'); }
    });

    if (port !== origPort) {
      confirmAction(
        'Xác nhận đổi cổng FerrousPanel',
        `Bạn sắp đổi cổng Panel từ ${origPort} sang ${port}. Dịch vụ sẽ tự mở cổng UFW và khởi động lại trên cổng mới.`,
        doSave,
      );
    } else {
      await doSave();
    }
  });
}

// ============================================================================
// 13. LINUX SYSOPS (PROCESSES /proc, PORTS ss, SYSTEMD SERVICES, SWAP & SYSCTL)
// ============================================================================
export async function renderSysops(container) {
  let data = await api('GET', '/api/sysops/overview');
  let activeTab = 'processes';
  let search = '';

  function openServiceJournalModal(unit) {
    openModal(`Nhật ký Systemd: ${unit}`, '<div class="skel skel-lg" style="margin:12px 0"></div>', {
      large: true,
      onMount(body) {
        async function fetchLogs(lines = 150) {
          try {
            const res = await api('GET', `/api/sysops/services/${encodeURIComponent(unit)}/logs?lines=${lines}`);
            body.innerHTML = `
              <div class="row-between" style="margin-bottom:10px">
                <span class="meta-line mono">${esc(res.unit)} · ${esc(res.lines)} dòng cuối · ${esc(fmtTime(res.fetchedAt))}</span>
                <button type="button" class="btn btn-sm" id="sysops-j-refresh">Làm mới</button>
              </div>
              <pre class="terminal-box" style="min-height:340px;max-height:480px;white-space:pre-wrap">${esc(res.content || '')}</pre>
            `;
            body.querySelector('#sysops-j-refresh')?.addEventListener('click', (e) => withBusy(e.currentTarget, () => fetchLogs(lines)));
          } catch (err) {
            body.innerHTML = `<div class="alert-err">${esc(err.message || 'Không đọc được nhật ký systemd.')}</div>`;
          }
        }
        fetchLogs(150);
      },
    });
  }

  function draw() {
    const h = data.host || {};
    const sc = data.sysctl || {};
    const procs = Array.isArray(data.processes) ? data.processes : [];
    const ports = Array.isArray(data.ports) ? data.ports : [];
    const disks = Array.isArray(data.disks) ? data.disks : [];
    const services = Array.isArray(data.services) ? data.services : [];
    const q = search.trim().toLowerCase();

    const filteredProcs = q
      ? procs.filter((p) => String(p.pid).includes(q) || String(p.name || '').toLowerCase().includes(q) || String(p.user || '').toLowerCase().includes(q) || String(p.command || '').toLowerCase().includes(q))
      : procs;
    const filteredPorts = q
      ? ports.filter((p) => String(p.port).includes(q) || String(p.process || '').toLowerCase().includes(q) || String(p.localAddr || '').toLowerCase().includes(q))
      : ports;
    const filteredServices = q
      ? services.filter((s) => String(s.unit || '').toLowerCase().includes(q) || String(s.description || '').toLowerCase().includes(q) || String(s.activeState || '').toLowerCase().includes(q))
      : services;

    const loadArr = Array.isArray(h.loadAvg) ? h.loadAvg : [0, 0, 0];
    const memUsedMb = Math.max(0, Number(h.memTotalMb || 0) - Number(h.memAvailableMb || 0));

    container.innerHTML = `
      <div class="page-head">
        <div>
          <h1 class="page-title">Tiến trình, Dịch vụ &amp; Hệ thống Linux</h1>
          <p class="page-desc">Quản trị tiến trình trực tiếp từ <span class="mono">/proc</span>, cổng lắng nghe <span class="mono">ss</span>, dịch vụ <span class="mono">systemd</span>, bộ nhớ Swap <span class="mono">/swapfile</span>, ổ đĩa và tham số nhân Kernel.</p>
        </div>
        <div class="row">
          <button type="button" class="btn btn-sm" id="sysops-refresh-btn">Cập nhật ngay</button>
        </div>
      </div>

      <section class="surface" style="margin-bottom:16px">
        <div class="kv-grid">
          <div class="kv-item">
            <span class="kv-label">Máy chủ &amp; Hệ điều hành</span>
            <span class="kv-value"><strong>${esc(h.hostname || 'localhost')}</strong> · ${esc(h.osPretty || 'Linux')}</span>
          </div>
          <div class="kv-item">
            <span class="kv-label">Kernel &amp; CPU / Múi giờ</span>
            <span class="kv-value mono">${esc(h.kernel || '—')} (${esc(h.arch || '')}) · ${esc(h.cpuCores || 1)} vCPU · ${esc(h.timezone || 'UTC')}</span>
          </div>
          <div class="kv-item">
            <span class="kv-label">Uptime &amp; Load Average</span>
            <span class="kv-value mono">${esc(fmtUptime(h.uptimeSecs || 0))} · ${esc(loadArr.join(' / '))}</span>
          </div>
          <div class="kv-item">
            <span class="kv-label">RAM &amp; Swap thực tế</span>
            <span class="kv-value mono">RAM ${esc(memUsedMb)}/${esc(h.memTotalMb || 0)} MB · Swap ${esc(h.swapUsedMb || 0)}/${esc(h.swapTotalMb || 0)} MB</span>
          </div>
        </div>
      </section>

      <div class="row-between" style="margin-bottom:14px;gap:12px;flex-wrap:wrap">
        <div class="tabs">
          <button type="button" class="tab-btn ${activeTab === 'processes' ? 'active' : ''}" data-sysops-tab="processes">Tiến trình (${procs.length})</button>
          <button type="button" class="tab-btn ${activeTab === 'ports' ? 'active' : ''}" data-sysops-tab="ports">Cổng lắng nghe (${ports.length})</button>
          <button type="button" class="tab-btn ${activeTab === 'services' ? 'active' : ''}" data-sysops-tab="services">Dịch vụ Systemd (${services.length})</button>
          <button type="button" class="tab-btn ${activeTab === 'storage' ? 'active' : ''}" data-sysops-tab="storage">Swap, Ổ đĩa &amp; Kernel</button>
        </div>
        ${activeTab !== 'storage' ? `<input class="input" id="sysops-search" style="max-width:300px" placeholder="Lọc theo PID, tên, cổng hoặc unit…" value="${esc(search)}" />` : ''}
      </div>

      ${activeTab === 'processes' ? `
        <section class="surface">
          <div class="surface-head">
            <h2 class="surface-title">Top tiến trình theo RAM (đọc trực tiếp từ /proc)</h2>
            <span class="meta-line">Hiển thị <strong class="mono">${filteredProcs.length}</strong> tiến trình · Panel PID <span class="mono">${esc(h.panelPid || '—')}</span> được bảo vệ</span>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>PID</th>
                  <th>User</th>
                  <th>Tiến trình &amp; Lệnh</th>
                  <th>Trạng thái</th>
                  <th class="right">RAM (MB)</th>
                  <th class="right">Threads</th>
                  <th class="right">Thao tác</th>
                </tr>
              </thead>
              <tbody>
                ${
                  filteredProcs.length
                    ? filteredProcs.map((p) => `
                      <tr>
                        <td class="mono"><strong>${esc(p.pid)}</strong></td>
                        <td class="mono">${esc(p.user)}</td>
                        <td>
                          <div><strong>${esc(p.name)}</strong> ${p.protected ? '<span class="muted">(Được bảo vệ)</span>' : ''}</div>
                          <div class="meta-line mono" style="max-width:440px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title="${esc(p.command)}">${esc(p.command)}</div>
                        </td>
                        <td class="mono">${esc(p.state)}</td>
                        <td class="right mono"><strong>${esc(p.rssMb)}</strong></td>
                        <td class="right mono">${esc(p.threads)}</td>
                        <td class="right">
                          ${
                            p.protected
                              ? '<span class="muted">Anti-Lockout</span>'
                              : `<div class="row" style="justify-content:flex-end">
                                   <button type="button" class="btn btn-sm" data-proc-sig="TERM" data-pid="${esc(p.pid)}" data-pname="${esc(p.name)}">TERM</button>
                                   <button type="button" class="btn btn-sm btn-danger" data-proc-sig="KILL" data-pid="${esc(p.pid)}" data-pname="${esc(p.name)}">KILL</button>
                                 </div>`
                          }
                        </td>
                      </tr>`).join('')
                    : '<tr><td colspan="7" class="empty-state">Không có tiến trình phù hợp bộ lọc.</td></tr>'
                }
              </tbody>
            </table>
          </div>
        </section>
      ` : ''}

      ${activeTab === 'ports' ? `
        <section class="surface">
          <div class="surface-head">
            <h2 class="surface-title">Cổng mạng đang lắng nghe (ss -tulpnH)</h2>
            <span class="meta-line">Tổng <strong class="mono">${filteredPorts.length}</strong> socket đang mở</span>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>Giao thức</th>
                  <th>Địa chỉ lắng nghe</th>
                  <th>Cổng</th>
                  <th>Tiến trình</th>
                  <th>PID</th>
                  <th>Trạng thái</th>
                </tr>
              </thead>
              <tbody>
                ${
                  filteredPorts.length
                    ? filteredPorts.map((p) => `
                      <tr>
                        <td class="mono"><strong>${esc(String(p.protocol || '').toUpperCase())}</strong></td>
                        <td class="mono">${esc(p.localAddr)}</td>
                        <td class="mono"><strong>:${esc(p.port)}</strong></td>
                        <td class="mono">${esc(p.process)}</td>
                        <td class="mono">${esc(p.pid ?? '—')}</td>
                        <td><span class="${statusClass(p.state === 'LISTEN' ? 'active' : 'running')}">${esc(p.state)}</span></td>
                      </tr>`).join('')
                    : '<tr><td colspan="6" class="empty-state">Không phát hiện cổng lắng nghe nào (hoặc thiếu lệnh ss).</td></tr>'
                }
              </tbody>
            </table>
          </div>
        </section>
      ` : ''}

      ${activeTab === 'services' ? `
        <section class="surface">
          <div class="surface-head">
            <h2 class="surface-title">Dịch vụ Systemd trên máy chủ</h2>
            <span class="meta-line">Hiển thị <strong class="mono">${filteredServices.length}</strong> dịch vụ · Anti-Lockout bảo vệ SSH &amp; FerrousPanel</span>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>Unit Systemd</th>
                  <th>Trạng thái</th>
                  <th>Mô tả</th>
                  <th class="right">Điều khiển</th>
                </tr>
              </thead>
              <tbody>
                ${
                  filteredServices.length
                    ? filteredServices.map((s) => `
                      <tr>
                        <td class="mono"><strong>${esc(s.unit)}</strong> ${s.protected ? '<span class="muted">(Core)</span>' : ''}</td>
                        <td><span class="${statusClass(s.activeState)}">${esc(s.activeState)}</span> <span class="muted mono">(${esc(s.subState)})</span></td>
                        <td style="max-width:360px">${esc(s.description)}</td>
                        <td class="right">
                          <div class="row" style="justify-content:flex-end">
                            ${s.activeState !== 'active' ? `<button type="button" class="btn btn-sm btn-primary" data-svc-act="start" data-unit="${esc(s.unit)}">Start</button>` : ''}
                            <button type="button" class="btn btn-sm" data-svc-act="restart" data-unit="${esc(s.unit)}">Restart</button>
                            ${!s.protected && s.activeState === 'active' ? `<button type="button" class="btn btn-sm btn-danger" data-svc-act="stop" data-unit="${esc(s.unit)}">Stop</button>` : ''}
                            <button type="button" class="btn btn-sm" data-svc-logs="${esc(s.unit)}">Nhật ký</button>
                          </div>
                        </td>
                      </tr>`).join('')
                    : '<tr><td colspan="4" class="empty-state">Không tải được danh sách dịch vụ systemd.</td></tr>'
                }
              </tbody>
            </table>
          </div>
        </section>
      ` : ''}

      ${activeTab === 'storage' ? `
        <section class="surface" style="margin-bottom:16px">
          <div class="surface-head">
            <h2 class="surface-title">Phân vùng &amp; Ổ đĩa thực tế (df -PT)</h2>
            <span class="meta-line">${disks.length} phân vùng</span>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>Thiết bị</th>
                  <th>Loại FS</th>
                  <th>Điểm gắn</th>
                  <th class="right">Đã dùng / Tổng</th>
                  <th class="right">Còn trống</th>
                  <th class="right">Sử dụng</th>
                </tr>
              </thead>
              <tbody>
                ${
                  disks.length
                    ? disks.map((d) => `
                      <tr>
                        <td class="mono">${esc(d.device)}</td>
                        <td class="mono">${esc(d.fsType)}</td>
                        <td class="mono"><strong>${esc(d.mountPoint)}</strong></td>
                        <td class="right mono">${esc(fmtBytes(d.usedBytes))} / ${esc(fmtBytes(d.totalBytes))}</td>
                        <td class="right mono">${esc(fmtBytes(d.availableBytes))}</td>
                        <td class="right mono"><strong class="${d.usePercent > 85 ? 'st-err' : d.usePercent > 70 ? 'st-warn' : 'st-ok'}">${esc(d.usePercent)}%</strong></td>
                      </tr>`).join('')
                    : '<tr><td colspan="6" class="empty-state">Không đọc được thông tin phân vùng ổ đĩa.</td></tr>'
                }
              </tbody>
            </table>
          </div>
        </section>

        <div class="grid-2">
          <section class="surface">
            <div class="surface-head">
              <h2 class="surface-title">Bộ nhớ ảo Swap (/swapfile)</h2>
              <span class="${h.swapFileActive ? 'st-ok' : 'st-muted'}">${h.swapFileActive ? `Đang bật (${h.swapFileSizeMb} MB)` : 'Chưa kích hoạt'}</span>
            </div>
            <p class="page-desc" style="margin-bottom:12px">
              Tạo tệp <span class="mono">/swapfile</span> quyền <span class="mono">0600</span> thuần Rust, kích hoạt qua <span class="mono">mkswap</span> + <span class="mono">swapon</span> và tự ghi vào <span class="mono">/etc/fstab</span> để giữ sau khi reboot.
            </p>
            <form id="sysops-swap-form">
              <div class="form-grid">
                <div class="field">
                  <label>Dung lượng Swap (MB, 128 – 16384)</label>
                  <input class="input mono" name="sizeMb" type="number" min="128" max="16384" value="${esc(h.swapFileSizeMb || 2048)}" required />
                </div>
                <div class="field">
                  <label>vm.swappiness (0 – 100)</label>
                  <input class="input mono" name="swappiness" type="number" min="0" max="100" value="${esc(sc.swappiness ?? 10)}" />
                </div>
              </div>
              <div class="row" style="margin-top:12px;gap:8px;flex-wrap:wrap">
                <button type="submit" class="btn btn-primary" id="sysops-swap-sub">Tạo / Cập nhật /swapfile</button>
                ${(h.swapFileActive || h.swapFileSizeMb > 0) ? '<button type="button" class="btn btn-danger" id="sysops-swap-del">Tắt &amp; Xóa /swapfile</button>' : ''}
              </div>
            </form>
          </section>

          <section class="surface">
            <div class="surface-head">
              <h2 class="surface-title">Tinh chỉnh nhân Linux (Sysctl Allowlist)</h2>
              <span class="meta-line mono">/etc/sysctl.d/99-ferrous-sysops.conf</span>
            </div>
            <form id="sysops-sysctl-form">
              <div class="form-grid">
                <div class="field">
                  <label>vm.swappiness (0 – 100)</label>
                  <input class="input mono" name="swappiness" type="number" min="0" max="100" value="${esc(sc.swappiness ?? 10)}" required />
                </div>
                <div class="field">
                  <label>net.core.somaxconn (128 – 65535)</label>
                  <input class="input mono" name="somaxconn" type="number" min="128" max="65535" value="${esc(sc.somaxconn ?? 4096)}" required />
                </div>
                <div class="field">
                  <label>TCP Congestion Control</label>
                  <select class="select mono" name="tcpCongestionControl">
                    ${['bbr', 'cubic', 'reno'].map((cc) => `<option value="${cc}" ${sc.tcpCongestionControl === cc ? 'selected' : ''}>${cc}</option>`).join('')}
                  </select>
                </div>
                <div class="field">
                  <label>fs.file-max (65536 – 10000000)</label>
                  <input class="input mono" name="fileMax" type="number" min="65536" max="10000000" value="${esc(sc.fileMax ?? 1048576)}" required />
                </div>
              </div>
              <div class="row" style="margin-top:12px">
                <button type="submit" class="btn btn-primary" id="sysops-sysctl-sub">Áp dụng &amp; Lưu bền vững</button>
              </div>
            </form>
          </section>
        </div>
      ` : ''}
    `;

    document.getElementById('sysops-refresh-btn')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
      try {
        data = await api('GET', '/api/sysops/overview');
        draw();
      } catch (err) { toast(err.message, 'err'); }
    }));

    container.querySelectorAll('[data-sysops-tab]').forEach((b) => {
      b.addEventListener('click', () => {
        activeTab = b.dataset.sysopsTab;
        draw();
      });
    });

    document.getElementById('sysops-search')?.addEventListener('input', (e) => {
      search = e.target.value;
      draw();
      const el = document.getElementById('sysops-search');
      if (el) { el.focus(); el.setSelectionRange(search.length, search.length); }
    });

    container.querySelectorAll('[data-proc-sig]').forEach((b) => {
      b.addEventListener('click', () => {
        const pid = Number(b.dataset.pid);
        const sig = b.dataset.procSig;
        const pname = b.dataset.pname || `PID ${pid}`;
        confirmAction(
          `Gửi SIG${sig} tới tiến trình`,
          `Xác nhận gửi tín hiệu SIG${sig} tới tiến trình ${pname} (PID ${pid})?`,
          () => withBusy(b, async () => {
            try {
              const res = await api('POST', `/api/sysops/processes/${pid}/signal`, { signal: sig });
              toast(res.message || `Đã gửi SIG${sig} tới PID ${pid}.`);
              data = await api('GET', '/api/sysops/overview');
              draw();
            } catch (err) { toast(err.message, 'err'); }
          }),
          sig === 'KILL',
        );
      });
    });

    container.querySelectorAll('[data-svc-act]').forEach((b) => {
      b.addEventListener('click', () => {
        const unit = b.dataset.unit;
        const action = b.dataset.svcAct;
        const execAction = () => withBusy(b, async () => {
          try {
            const res = await api('POST', `/api/sysops/services/${encodeURIComponent(unit)}/action`, { action });
            toast(res.message || `Đã ${action} ${unit}.`);
            data = await api('GET', '/api/sysops/overview');
            draw();
          } catch (err) { toast(err.message, 'err'); }
        });
        if (action === 'stop') {
          confirmAction(`Dừng dịch vụ ${unit}`, `Xác nhận chạy systemctl stop ${unit}?`, execAction, true);
        } else {
          execAction();
        }
      });
    });

    container.querySelectorAll('[data-svc-logs]').forEach((b) => {
      b.addEventListener('click', () => openServiceJournalModal(b.dataset.svcLogs));
    });

    document.getElementById('sysops-swap-form')?.addEventListener('submit', async (e) => {
      e.preventDefault();
      await withBusy(document.getElementById('sysops-swap-sub'), async () => {
        const fd = new FormData(e.target);
        const sizeMb = Number(fd.get('sizeMb') || 2048);
        const swRaw = String(fd.get('swappiness') || '').trim();
        try {
          data = await api('POST', '/api/sysops/swap', {
            sizeMb,
            swappiness: swRaw !== '' ? Number(swRaw) : undefined,
          });
          toast(`Đã cấu hình & kích hoạt /swapfile (${sizeMb} MB) thành công.`);
          draw();
        } catch (err) { toast(err.message, 'err'); }
      });
    });

    document.getElementById('sysops-swap-del')?.addEventListener('click', (e) => {
      const btn = e.currentTarget;
      confirmAction(
        'Tắt & Xóa /swapfile',
        'Xác nhận chạy swapoff, xóa tệp /swapfile và gỡ dòng tương ứng khỏi /etc/fstab?',
        () => withBusy(btn, async () => {
          try {
            data = await api('DELETE', '/api/sysops/swap');
            toast('Đã tắt và xóa /swapfile.');
            draw();
          } catch (err) { toast(err.message, 'err'); }
        }),
      );
    });

    document.getElementById('sysops-sysctl-form')?.addEventListener('submit', async (e) => {
      e.preventDefault();
      await withBusy(document.getElementById('sysops-sysctl-sub'), async () => {
        const fd = new FormData(e.target);
        try {
          const updated = await api('PUT', '/api/sysops/sysctl', {
            swappiness: Number(fd.get('swappiness')),
            somaxconn: Number(fd.get('somaxconn')),
            tcpCongestionControl: String(fd.get('tcpCongestionControl') || 'cubic'),
            fileMax: Number(fd.get('fileMax')),
          });
          data.sysctl = updated;
          toast('Đã áp dụng tham số sysctl trực tiếp và lưu vào /etc/sysctl.d/99-ferrous-sysops.conf.');
          draw();
        } catch (err) { toast(err.message, 'err'); }
      });
    });
  }

  draw();
}
