// FerrousPanel Pure-Rust UI — Infrastructure Views (Web Engine, PHP/Software, Cron/Backup/DNS, Mail)
import {
  api, esc, fmtTime, statusClass, toast, openModal, closeModal, confirmAction, openJobDrawer,
} from './app.js';

// ============================================================================
// 5. WEB SERVER ENGINE (NGINX / FERROUSX)
// ============================================================================
export async function renderWebserver(container) {
  const [ws, fxSync, fxIpc] = await Promise.all([
    api('GET', '/api/webserver'),
    api('GET', '/api/ferrousx/sync-status').catch(() => null),
    api('GET', '/api/ferrousx/ipc/status').catch(() => null),
  ]);

  const activeEngine = ws?.frontendEngine || ws?.frontend_engine || 'nginx';
  const sites = ws?.sites || [];

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Web Server Engine (Nginx & FerrousX)</h1>
        <p class="page-desc">Chuyển đổi nguyên tử giữa Nginx và FerrousX thuần Rust trên cổng 80/443 với tự động rollback khi kiểm tra lỗi.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="ws-validate-nginx">Kiểm tra cấu hình Nginx (-t)</button>
        <button type="button" class="btn ${activeEngine === 'nginx' ? '' : 'btn-primary'}" id="ws-switch-nginx" ${activeEngine === 'nginx' ? 'disabled' : ''}>Chuyển sang Nginx</button>
        <button type="button" class="btn ${activeEngine === 'ferrousx' ? '' : 'btn-primary'}" id="ws-switch-ferrousx" ${activeEngine === 'ferrousx' ? 'disabled' : ''}>Chuyển sang FerrousX</button>
      </div>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Nginx</h2>
          <span class="${statusClass(ws?.nginx?.active ? 'active' : 'stopped')}">${ws?.nginx?.active ? 'Đang chạy' : 'Đã dừng'}</span>
        </div>
        <div class="meta-line">
          <span>Đã cài: <strong>${ws?.nginx?.installed ? 'Có' : 'Chưa'}</strong></span>
          <span>·</span>
          <span>Phiên bản: <span class="mono">${esc(ws?.nginx?.version || '—')}</span></span>
          <span>·</span>
          <span>PID: <span class="mono">${esc(ws?.nginx?.pid || '—')}</span></span>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">FerrousX (Pure-Rust HTTP/3 Engine)</h2>
          <span class="${statusClass(ws?.ferrousx?.active ? 'active' : 'stopped')}">${ws?.ferrousx?.active ? 'Đang chạy' : 'Đã dừng'}</span>
        </div>
        <div class="meta-line" style="margin-bottom:10px">
          <span>Đã cài: <strong>${ws?.ferrousx?.installed ? 'Có' : 'Chưa'}</strong></span>
          <span>·</span>
          <span>Phiên bản: <span class="mono">${esc(ws?.ferrousx?.version || '—')}</span></span>
          <span>·</span>
          <span>IPC: <span class="mono">${esc(fxIpc?.connected ? 'Connected' : 'Standby')}</span></span>
        </div>
        ${fxSync ? `<button type="button" class="btn btn-sm" id="ws-sync-fx">Đồng bộ FerrousX Crates</button>` : ''}
      </section>
    </div>

    <section class="surface">
      <div class="surface-head">
        <h2 class="surface-title">Virtual Hosts đang cấu hình (${sites.length})</h2>
        <span class="meta-line"><span>Worker threads: <strong class="mono">${esc(ws?.workerThreads || ws?.worker_threads || 4)}</strong></span></span>
      </div>
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr><th>Tên miền</th><th>Chế độ định tuyến</th><th>HTTPS / SSL</th><th>Chứng chỉ hết hạn</th></tr>
          </thead>
          <tbody>
            ${
              sites.length
                ? sites.map((s) => `
                  <tr>
                    <td class="mono"><strong>${esc(s.domain)}</strong></td>
                    <td class="mono">${esc(s.mode?.kind || 'proxy')}</td>
                    <td><span class="${s.ssl?.enabled ? 'st-ok' : 'st-muted'}">${s.ssl?.enabled ? 'HTTPS Bật' : 'HTTP'}</span></td>
                    <td class="mono">${esc(s.ssl?.expiresAt || '—')}</td>
                  </tr>`).join('')
                : '<tr><td colspan="4" class="empty-state">Chưa có virtual host nào được đăng ký.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  function doSwitch(target) {
    confirmAction('Chuyển đổi Web Engine', `Chuyển Web Server cổng 80/443 sang ${target}?`, async () => {
      try {
        await api('POST', '/api/webserver/switch', { targetEngine: target });
        toast(`Đã chuyển Web Engine sang ${target}.`);
        await renderWebserver(container);
      } catch (err) {
        toast(err.message, 'err');
      }
    }, false);
  }

  document.getElementById('ws-switch-nginx')?.addEventListener('click', () => doSwitch('nginx'));
  document.getElementById('ws-switch-ferrousx')?.addEventListener('click', () => doSwitch('ferrousx'));
  document.getElementById('ws-validate-nginx')?.addEventListener('click', async () => {
    try {
      const res = await api('POST', '/api/nginx/validate', {});
      toast(res.message || 'Cấu hình Nginx hợp lệ.');
    } catch (err) {
      toast(err.message, 'err');
    }
  });
  document.getElementById('ws-sync-fx')?.addEventListener('click', async () => {
    try {
      await api('POST', '/api/ferrousx/sync-status', {});
      toast('Đã đồng bộ cấu hình FerrousX.');
      await renderWebserver(container);
    } catch (err) {
      toast(err.message, 'err');
    }
  });
}

// ============================================================================
// 6. PHP-FPM & SOFTWARE STORE
// ============================================================================
export async function renderPhpSoftware(container) {
  const [phpList, pkgs] = await Promise.all([
    api('GET', '/api/php').catch(() => []),
    api('GET', '/api/software').catch(() => []),
  ]);

  let tab = 'software';

  function draw() {
    container.innerHTML = `
      <div class="page-head">
        <div>
          <h1 class="page-title">PHP-FPM & Kho Phần mềm Hệ thống</h1>
          <p class="page-desc">Quản lý các gói dịch vụ cốt lõi (MariaDB, PostgreSQL, Redis, Bun, Tailwind CSS, Certbot) và Multi-PHP FPM.</p>
        </div>
        <div class="tabs">
          <button type="button" class="tab-btn ${tab === 'software' ? 'active' : ''}" id="sw-tab-pkgs">Phần mềm (${pkgs.length})</button>
          <button type="button" class="tab-btn ${tab === 'php' ? 'active' : ''}" id="sw-tab-php">PHP-FPM (${phpList.length})</button>
        </div>
      </div>

      ${tab === 'software' ? `
        <section class="surface">
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr><th>Gói dịch vụ</th><th>Phiên bản</th><th>Trạng thái</th><th class="right">Điều khiển</th></tr>
              </thead>
              <tbody>
                ${
                  pkgs.length
                    ? pkgs.map((p) => `
                      <tr>
                        <td>
                          <div><strong>${esc(p.name)}</strong></div>
                          <div class="meta-line"><span>${esc(p.category || '')}</span><span>·</span><span>${esc(p.description || '')}</span></div>
                        </td>
                        <td class="mono">${esc(p.version || '—')}</td>
                        <td><span class="${statusClass(p.status)}">${esc(p.status)}</span></td>
                        <td class="right">
                          <div class="row" style="justify-content:flex-end">
                            ${p.status === 'not_installed' || p.status === 'missing'
                              ? `<button type="button" class="btn btn-sm btn-primary" data-pkg-act="install" data-id="${esc(p.id)}">Cài đặt</button>`
                              : `<button type="button" class="btn btn-sm" data-pkg-act="restart" data-id="${esc(p.id)}">Restart</button>
                                 <button type="button" class="btn btn-sm" data-pkg-act="stop" data-id="${esc(p.id)}">Stop</button>`}
                          </div>
                        </td>
                      </tr>`).join('')
                    : '<tr><td colspan="4" class="empty-state">Chưa có danh sách gói phần mềm.</td></tr>'
                }
              </tbody>
            </table>
          </div>
        </section>
      ` : `
        <div class="grid-2">
          ${
            phpList.length
              ? phpList.map((p) => `
                <section class="surface">
                  <div class="surface-head">
                    <div>
                      <h3 class="surface-title">PHP ${esc(p.version)}</h3>
                      <div class="meta-line"><span>${esc(p.fpmSocket || '')}</span><span>·</span><span class="${statusClass(p.status)}">${esc(p.status)}</span></div>
                    </div>
                    <div class="row">
                      <button type="button" class="btn btn-sm" data-php-act="restart" data-ver="${esc(p.version)}">Restart</button>
                      <button type="button" class="btn btn-sm" data-php-act="install" data-ver="${esc(p.version)}">Cài / Sửa</button>
                    </div>
                  </div>
                  <div class="meta-line" style="margin-bottom:8px">
                    <span>Memory: <strong class="mono">${esc(p.memoryLimit || '256M')}</strong></span>
                    <span>·</span>
                    <span>Upload: <strong class="mono">${esc(p.uploadMaxFilesize || '64M')}</strong></span>
                    <span>·</span>
                    <span>Max Exec: <strong class="mono">${esc(p.maxExecutionTime || 60)}s</strong></span>
                  </div>
                  <div class="row">
                    ${(p.extensions || []).slice(0, 10).map((ext) => `
                      <button type="button" class="btn btn-sm" data-php-ext="${esc(ext.name)}" data-ver="${esc(p.version)}">
                        ${esc(ext.name)}: ${ext.enabled ? 'Bật' : 'Tắt'}
                      </button>`).join('')}
                  </div>
                </section>`).join('')
              : '<div class="surface"><div class="empty-state">Không tìm thấy phiên bản PHP-FPM nào.</div></div>'
          }
        </div>
      `}
    `;

    document.getElementById('sw-tab-pkgs')?.addEventListener('click', () => { tab = 'software'; draw(); });
    document.getElementById('sw-tab-php')?.addEventListener('click', () => { tab = 'php'; draw(); });

    container.querySelectorAll('[data-pkg-act]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          await api('POST', `/api/software/${encodeURIComponent(btn.dataset.id)}/action`, { action: btn.dataset.pkgAct });
          toast(`Đã thực thi '${btn.dataset.pkgAct}' cho gói dịch vụ.`);
          await renderPhpSoftware(container);
        } catch (err) {
          toast(err.message, 'err');
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('[data-php-act]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          await api('POST', `/api/php/${encodeURIComponent(btn.dataset.ver)}/action`, { action: btn.dataset.phpAct });
          toast(`Đã gửi lệnh ${btn.dataset.phpAct} cho PHP ${btn.dataset.ver}.`);
          await renderPhpSoftware(container);
        } catch (err) {
          toast(err.message, 'err');
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('[data-php-ext]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          await api('POST', `/api/php/${encodeURIComponent(btn.dataset.ver)}/extensions/toggle`, { extension: btn.dataset.phpExt });
          toast(`Đã cập nhật extension ${btn.dataset.phpExt}.`);
          await renderPhpSoftware(container);
        } catch (err) {
          toast(err.message, 'err');
          btn.disabled = false;
        }
      });
    });
  }

  draw();
}

// ============================================================================
// 7. CRON, BACKUPS, DNS & SFTP
// ============================================================================
export async function renderCpanel(container) {
  const [cp, apps] = await Promise.all([
    api('GET', '/api/cpanel'),
    api('GET', '/api/apps').catch(() => []),
  ]);

  const crons = cp.cronJobs || [];
  const backups = cp.backups || [];
  const dns = cp.dnsRecords || [];
  const sftps = cp.sftpAccounts || [];

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Cron Jobs, Sao lưu Zstd, DNS & SFTP</h1>
        <p class="page-desc">Lập lịch crontab hệ thống, sao lưu nén tar.zst bất đồng bộ và quản trị bản ghi DNS / SFTP.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="cp-add-cron">Thêm Cron Job</button>
        <button type="button" class="btn btn-primary" id="cp-create-backup">Tạo Backup nền (Async)</button>
      </div>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Crontab (${crons.length})</h2>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Tên & Lịch</th><th>Lệnh</th><th class="right">Thao tác</th></tr></thead>
            <tbody>
              ${
                crons.length
                  ? crons.map((c) => `
                    <tr>
                      <td>
                        <div><strong>${esc(c.title)}</strong></div>
                        <div class="meta-line mono"><span>${esc(c.schedule)}</span><span>·</span><span class="${c.enabled ? 'st-ok' : 'st-muted'}">${c.enabled ? 'Bật' : 'Tắt'}</span></div>
                      </td>
                      <td class="mono">${esc(c.command)}</td>
                      <td class="right">
                        <div class="row" style="justify-content:flex-end">
                          <button type="button" class="btn btn-sm" data-cron-run="${esc(c.id)}">Chạy</button>
                          <button type="button" class="btn btn-sm" data-cron-toggle="${esc(c.id)}">Bật/Tắt</button>
                          <button type="button" class="btn btn-sm btn-danger" data-cron-del="${esc(c.id)}">Xóa</button>
                        </div>
                      </td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có lịch Cron nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Bản sao lưu Zstd (${backups.length})</h2>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Ứng dụng & Tệp</th><th class="right">Kích thước</th><th class="right">Thao tác</th></tr></thead>
            <tbody>
              ${
                backups.length
                  ? backups.map((b) => `
                    <tr>
                      <td>
                        <div><strong>${esc(b.targetApp)}</strong></div>
                        <div class="meta-line mono"><span>${esc(b.filename || b.id)}</span><span>·</span><span>${esc(fmtTime(b.createdAt))}</span></div>
                      </td>
                      <td class="right mono">${Number(b.sizeMb || 0).toFixed(2)} MB</td>
                      <td class="right">
                        <div class="row" style="justify-content:flex-end">
                          <button type="button" class="btn btn-sm" data-bk-restore="${esc(b.id)}">Khôi phục</button>
                          <button type="button" class="btn btn-sm btn-danger" data-bk-del="${esc(b.id)}">Xóa</button>
                        </div>
                      </td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có bản sao lưu nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Bản ghi DNS (${dns.length})</h2>
          <button type="button" class="btn btn-sm" id="cp-add-dns">Thêm bản ghi</button>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Host</th><th>Loại</th><th>Giá trị</th><th class="right">Xóa</th></tr></thead>
            <tbody>
              ${
                dns.length
                  ? dns.map((r) => `
                    <tr>
                      <td class="mono">${esc(r.name)}</td>
                      <td class="mono">${esc(r.recordType)}</td>
                      <td class="mono">${esc(r.value)}</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-dns-del="${esc(r.id)}">Xóa</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="4" class="empty-state">Chưa có bản ghi DNS nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Tài khoản SFTP Chroot (${sftps.length})</h2>
          <button type="button" class="btn btn-sm" id="cp-add-sftp">Thêm SFTP</button>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Tài khoản</th><th>Thư mục gốc</th><th class="right">Xóa</th></tr></thead>
            <tbody>
              ${
                sftps.length
                  ? sftps.map((u) => `
                    <tr>
                      <td class="mono"><strong>${esc(u.username)}</strong></td>
                      <td class="mono">${esc(u.rootPath)}</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-sftp-del="${esc(u.id)}">Xóa</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có tài khoản SFTP chroot.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>
  `;

  document.getElementById('cp-add-cron')?.addEventListener('click', () => {
    openModal('Thêm Cron Job mới', `
      <form id="cron-form">
        <div class="field"><label>Tiêu đề</label><input class="input" name="title" placeholder="Dọn dẹp cache" required /></div>
        <div class="field"><label>Lịch Cron (5 trường)</label><input class="input mono" name="schedule" value="0 3 * * *" required /></div>
        <div class="field"><label>Lệnh thực thi</label><input class="input mono" name="command" placeholder="/usr/local/bin/fp status" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Lưu Cron</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#cron-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/cpanel/cron', {
              title: fd.get('title'),
              schedule: fd.get('schedule'),
              command: fd.get('command'),
              user: 'root',
              targetApp: '',
            });
            closeModal();
            toast('Đã thêm Cron Job.');
            await renderCpanel(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      },
    });
  });

  document.getElementById('cp-create-backup')?.addEventListener('click', () => {
    openModal('Tạo bản sao lưu nền (Zstd)', `
      <form id="bk-form">
        <div class="field">
          <label>Website mục tiêu</label>
          ${apps.length
            ? `<select class="select" name="targetApp">${apps.map((a) => `<option value="${esc(a.domain)}">${esc(a.domain)}</option>`).join('')}</select>`
            : '<input class="input" name="targetApp" placeholder="example.com" required />'}
        </div>
        <div class="field"><label>Kèm Database (tùy chọn)</label><input class="input mono" name="databaseIncluded" placeholder="Để trống nếu chỉ sao lưu mã nguồn" /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Chạy sao lưu nền</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#bk-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            const job = await api('POST', '/api/jobs/cpanel/backups', {
              targetApp: fd.get('targetApp'),
              databaseIncluded: fd.get('databaseIncluded') || '',
            });
            closeModal();
            toast('Đã xếp hàng tác vụ sao lưu.');
            openJobDrawer(job.id);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      },
    });
  });

  document.getElementById('cp-add-dns')?.addEventListener('click', () => {
    openModal('Thêm bản ghi DNS', `
      <form id="dns-form">
        <div class="form-grid">
          <div class="field"><label>Domain / Zone</label><input class="input mono" name="domain" placeholder="example.com" required /></div>
          <div class="field"><label>Loại</label><select class="select" name="recordType"><option>A</option><option>AAAA</option><option>CNAME</option><option>MX</option><option>TXT</option></select></div>
          <div class="field"><label>Host</label><input class="input mono" name="name" placeholder="@" required /></div>
          <div class="field"><label>Giá trị</label><input class="input mono" name="value" placeholder="1.2.3.4" required /></div>
        </div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Lưu bản ghi</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#dns-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/cpanel/dns', {
              domain: fd.get('domain'),
              recordType: fd.get('recordType'),
              name: fd.get('name'),
              value: fd.get('value'),
              ttl: 300,
            });
            closeModal();
            toast('Đã thêm bản ghi DNS.');
            await renderCpanel(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      },
    });
  });

  document.getElementById('cp-add-sftp')?.addEventListener('click', () => {
    openModal('Thêm tài khoản SFTP', `
      <form id="sftp-form">
        <div class="field"><label>Tên tài khoản</label><input class="input mono" name="username" placeholder="deploy_user" required /></div>
        <div class="field"><label>Thư mục gốc</label><input class="input mono" name="rootPath" placeholder="/www/wwwroot/example.com" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Tạo SFTP</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#sftp-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/cpanel/sftp', { username: fd.get('username'), rootPath: fd.get('rootPath') });
            closeModal();
            toast('Đã thêm tài khoản SFTP.');
            await renderCpanel(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      },
    });
  });

  container.querySelectorAll('[data-cron-run]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('POST', `/api/cpanel/cron/${encodeURIComponent(b.dataset.cronRun)}/run`, {}); toast('Đã chạy Cron Job.'); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-cron-toggle]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('PUT', `/api/cpanel/cron/${encodeURIComponent(b.dataset.cronToggle)}/toggle`, {}); await renderCpanel(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-cron-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/cpanel/cron/${encodeURIComponent(b.dataset.cronDel)}`); await renderCpanel(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-bk-restore]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Khôi phục bản sao lưu', 'Xác nhận khôi phục dữ liệu từ bản sao lưu này?', async () => {
      try { await api('POST', `/api/cpanel/backups/${encodeURIComponent(b.dataset.bkRestore)}/restore`, {}); toast('Đã khôi phục bản sao lưu.'); }
      catch (err) { toast(err.message, 'err'); }
    });
  }));
  container.querySelectorAll('[data-bk-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/cpanel/backups/${encodeURIComponent(b.dataset.bkDel)}`); await renderCpanel(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-dns-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/cpanel/dns/${encodeURIComponent(b.dataset.dnsDel)}`); await renderCpanel(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
  container.querySelectorAll('[data-sftp-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/cpanel/sftp/${encodeURIComponent(b.dataset.sftpDel)}`); await renderCpanel(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
}

// ============================================================================
// 8. MAIL SERVER
// ============================================================================
export async function renderMail(container) {
  const data = await api('GET', '/api/mail');
  const domains = data.domains || [];
  const mailboxes = data.mailboxes || [];
  const forwarders = data.forwarders || [];

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Máy chủ Mail (Postfix / Dovecot / Rspamd)</h1>
        <p class="page-desc">Quản lý tên miền thư điện tử, hộp thư IMAP/SMTP và chuyển tiếp thư.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="mail-add-dom">Thêm tên miền Mail</button>
        <button type="button" class="btn btn-primary" id="mail-add-box">Tạo hộp thư</button>
      </div>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Tên miền Mail (${domains.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Tên miền</th><th>SPF / DKIM / DMARC</th></tr></thead>
            <tbody>
              ${
                domains.length
                  ? domains.map((d) => `
                    <tr>
                      <td class="mono"><strong>${esc(d.domain)}</strong></td>
                      <td class="meta-line"><span>SPF: ${d.spfValid ? 'OK' : 'Cần trỏ'}</span><span>·</span><span>DKIM: ${d.dkimValid ? 'OK' : 'Cần trỏ'}</span><span>·</span><span>DMARC: ${d.dmarcValid ? 'OK' : 'Cần trỏ'}</span></td>
                    </tr>`).join('')
                  : '<tr><td colspan="2" class="empty-state">Chưa cấu hình tên miền Mail.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Hộp thư (${mailboxes.length}) · Forwarders (${forwarders.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Địa chỉ Email</th><th class="right">Dung lượng</th><th class="right">Xóa</th></tr></thead>
            <tbody>
              ${
                mailboxes.length
                  ? mailboxes.map((m) => `
                    <tr>
                      <td class="mono"><strong>${esc(m.email)}</strong></td>
                      <td class="right mono">${esc(m.quotaMb || 1024)} MB</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-mb-del="${esc(m.id)}">Xóa</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có hộp thư nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>
  `;

  document.getElementById('mail-add-dom')?.addEventListener('click', () => {
    openModal('Thêm tên miền Mail', `
      <form id="md-form">
        <div class="field"><label>Tên miền</label><input class="input mono" name="domain" placeholder="mail.example.com" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Thêm tên miền</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#md-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const domain = new FormData(e.target).get('domain');
          try {
            await api('POST', '/api/mail/domains', { domain });
            closeModal();
            toast('Đã thêm tên miền mail.');
            await renderMail(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      },
    });
  });

  document.getElementById('mail-add-box')?.addEventListener('click', () => {
    openModal('Tạo hộp thư mới', `
      <form id="mb-form">
        <div class="form-grid">
          <div class="field"><label>Email</label><input class="input mono" name="email" placeholder="info@example.com" required /></div>
          <div class="field"><label>Mật khẩu</label><input class="input" name="password" type="password" required /></div>
          <div class="field"><label>Hạn mức (MB)</label><input class="input mono" name="quotaMb" type="number" value="1024" /></div>
        </div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary">Tạo hộp thư</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#mb-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/mail/mailboxes', {
              email: fd.get('email'),
              password: fd.get('password'),
              quotaMb: Number(fd.get('quotaMb') || 1024),
            });
            closeModal();
            toast('Đã tạo hộp thư mới.');
            await renderMail(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      },
    });
  });

  container.querySelectorAll('[data-mb-del]').forEach((b) => b.addEventListener('click', async () => {
    try { await api('DELETE', `/api/mail/mailboxes/${encodeURIComponent(b.dataset.mbDel)}`); await renderMail(container); }
    catch (err) { toast(err.message, 'err'); }
  }));
}
